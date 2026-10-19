// F2.3: 8 simulated NDI sources -> throttled 320x180 PNG frames, per-second stats, layout persistence, via real IPC.

mod common;

use std::collections::HashMap;
use std::time::{Duration, Instant};

use base64::Engine;
use common::TestApp;
use serde_json::{json, Value};
use streamdeck_lib::events::AppEvent;
use streamdeck_lib::ndi::NdiSource;
use streamdeck_lib::preview::PreviewFrame;

const SOURCE_FPS: u32 = 30;

async fn add_sources(t: &TestApp, n: usize) -> Vec<String> {
    let names: Vec<String> = (1..=n).map(|i| format!("Cam {i}")).collect();
    t.ndi_discovery
        .set(names.iter().map(|n| NdiSource::new(n)).collect());
    t.poll_ndi().await;
    for name in &names {
        t.invoke::<()>("ndi_add_source", json!({ "name": name }))
            .expect("add");
    }
    names
}

async fn collect_frames(
    rx: &mut tokio::sync::broadcast::Receiver<PreviewFrame>,
    window: Duration,
) -> Vec<PreviewFrame> {
    let mut out = Vec::new();
    let _ = tokio::time::timeout(window, async {
        while let Ok(f) = rx.recv().await {
            out.push(f);
        }
    })
    .await;
    out
}

fn png_size(b64: &str) -> (u32, u32) {
    let bytes = base64::engine::general_purpose::STANDARD
        .decode(b64)
        .expect("base64");
    let reader = png::Decoder::new(bytes.as_slice())
        .read_info()
        .expect("valid png");
    let info = reader.info();
    (info.width, info.height)
}

fn snapshot(t: &TestApp) -> Value {
    t.invoke::<Value>("preview_get_snapshot", json!({}))
        .expect("snapshot")
}

#[tokio::test(flavor = "multi_thread")]
async fn eight_sources_each_emit_at_most_8_fps_of_320x180_frames() {
    let t = TestApp::with_preview_feeds(SOURCE_FPS);
    let mut rx = t.preview.subscribe();
    let start = Instant::now();
    let names = add_sources(&t, 8).await;
    let frames = collect_frames(&mut rx, Duration::from_millis(2500)).await;
    let secs = start.elapsed().as_secs_f64();

    let mut per: HashMap<&str, usize> = HashMap::new();
    for f in &frames {
        assert_eq!((f.width, f.height), (320, 180));
        *per.entry(f.source.as_str()).or_default() += 1;
    }
    for name in &names {
        let n = per.get(name.as_str()).copied().unwrap_or(0);
        assert!(n >= 4, "{name} produced only {n} frames");
        assert!(
            n as f64 <= 8.0 * secs + 1.0,
            "{name}: {n} frames in {secs:.2}s exceeds 8 fps"
        );
    }
    assert_eq!(png_size(&frames[0].png_base64), (320, 180));
}

#[tokio::test(flavor = "multi_thread")]
async fn stats_report_native_resolution_fps_and_latency_per_source() {
    let t = TestApp::with_preview_feeds(SOURCE_FPS);
    let names = add_sources(&t, 3).await;
    tokio::time::sleep(Duration::from_millis(1200)).await;
    let mut events = t.collect_events();

    t.preview.flush_stats(&t.bus, Instant::now());

    let stats: Vec<_> = events
        .drain()
        .into_iter()
        .filter_map(|e| match e {
            AppEvent::SourceStats {
                name,
                width,
                height,
                fps,
                latency_ms,
            } => Some((name, width, height, fps, latency_ms)),
            _ => None,
        })
        .collect();
    assert_eq!(stats.len(), names.len());
    for (name, w, h, fps, latency) in stats {
        assert_eq!((w, h), (1920, 1080), "{name}");
        assert!((10.0..=31.0).contains(&fps), "{name}: fps {fps}");
        assert!(latency >= 12, "{name}: latency {latency}");
    }
}

#[tokio::test(flavor = "multi_thread")]
async fn removing_or_losing_a_source_stops_its_frames_and_resumes_on_return() {
    let t = TestApp::with_preview_feeds(SOURCE_FPS);
    let mut rx = t.preview.subscribe();
    add_sources(&t, 2).await;
    collect_frames(&mut rx, Duration::from_millis(400)).await;

    t.invoke::<()>("ndi_remove_source", json!({ "name": "Cam 1" }))
        .expect("remove");
    collect_frames(&mut rx, Duration::from_millis(300)).await;
    let later = collect_frames(&mut rx, Duration::from_millis(600)).await;
    assert!(later.iter().all(|f| f.source != "Cam 1"));
    assert!(later.iter().any(|f| f.source == "Cam 2"));

    t.ndi_discovery.disappear("Cam 2");
    t.poll_ndi().await;
    collect_frames(&mut rx, Duration::from_millis(300)).await;
    assert!(collect_frames(&mut rx, Duration::from_millis(500))
        .await
        .is_empty());
    assert_eq!(snapshot(&t)["frames"], json!([]));

    t.ndi_discovery.appear(NdiSource::new("Cam 2"));
    t.poll_ndi().await;
    let back = collect_frames(&mut rx, Duration::from_millis(600)).await;
    assert!(back.iter().any(|f| f.source == "Cam 2"));
}

#[tokio::test(flavor = "multi_thread")]
async fn snapshot_returns_the_latest_frame_per_source_for_a_resubscribing_ui() {
    let t = TestApp::with_preview_feeds(SOURCE_FPS);
    let mut rx = t.preview.subscribe();
    add_sources(&t, 2).await;
    collect_frames(&mut rx, Duration::from_millis(500)).await;
    t.preview.flush_stats(&t.bus, Instant::now());

    let snap = snapshot(&t);
    let sources: Vec<&str> = snap["frames"]
        .as_array()
        .expect("frames")
        .iter()
        .map(|f| f["source"].as_str().expect("source"))
        .collect();
    assert_eq!(sources, ["Cam 1", "Cam 2"]);
    assert_eq!(snap["frames"][0]["width"], 320);
    assert_eq!(snap["stats"].as_array().expect("stats").len(), 2);
}

#[tokio::test(flavor = "multi_thread")]
async fn without_feeds_no_frames_flow_and_the_snapshot_is_empty() {
    let t = TestApp::new();
    add_sources(&t, 1).await;
    tokio::time::sleep(Duration::from_millis(200)).await;
    assert_eq!(snapshot(&t), json!({ "frames": [], "stats": [] }));
}

#[tokio::test(flavor = "multi_thread")]
async fn layout_defaults_validates_and_persists_across_restart() {
    let t = TestApp::new();
    let layout = |t: &TestApp| t.invoke::<String>("preview_get_layout", json!({}));
    assert_eq!(layout(&t).expect("default"), "2x2");

    t.invoke::<()>("preview_set_layout", json!({ "layout": "2x4" }))
        .expect("set");
    assert_eq!(layout(&t).expect("get"), "2x4");

    let err = t
        .invoke::<()>("preview_set_layout", json!({ "layout": "5x5" }))
        .expect_err("invalid layout");
    assert_eq!(err["type"], "InvalidInput");
    assert_eq!(layout(&t).expect("unchanged"), "2x4");

    let TestApp { data_dir, .. } = t;
    let restarted = TestApp::with_data_dir(data_dir, Default::default());
    assert_eq!(layout(&restarted).expect("after restart"), "2x4");
}

// Measurement for the F2.3 notes; run with `cargo test --release --test preview_journeys -- --ignored --nocapture`.
#[tokio::test(flavor = "multi_thread")]
#[ignore = "measurement, not a gate"]
async fn cpu_budget_with_eight_simulated_streams() {
    use sysinfo::{get_current_pid, ProcessRefreshKind, ProcessesToUpdate, System};
    let t = TestApp::with_preview_feeds(SOURCE_FPS);
    add_sources(&t, 8).await;
    let pid = get_current_pid().expect("pid");
    let mut sys = System::new();
    let kind = ProcessRefreshKind::nothing().with_cpu();
    sys.refresh_processes_specifics(ProcessesToUpdate::Some(&[pid]), true, kind);
    let before = sys
        .process(pid)
        .map(|p| p.accumulated_cpu_time())
        .unwrap_or(0);
    let wall = Duration::from_secs(5);
    tokio::time::sleep(wall).await;
    sys.refresh_processes_specifics(ProcessesToUpdate::Some(&[pid]), true, kind);
    let after = sys
        .process(pid)
        .map(|p| p.accumulated_cpu_time())
        .unwrap_or(0);
    let cpu = (after - before) as f32 / wall.as_millis() as f32 * 100.0;
    let cores = std::thread::available_parallelism()
        .map(|n| n.get())
        .unwrap_or(1);
    println!(
        "CPU: {cpu:.1}% of one core = {:.1}% of {cores} cores",
        cpu / cores as f32
    );
}
