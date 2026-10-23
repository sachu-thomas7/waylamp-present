// Real GStreamer streaming against a local RTMP server (MediaMTX in Docker). Run with scripts/stream-e2e.sh, or:
//   STREAMDECK_RTMP_URL=rtmp://127.0.0.1:1935/live STREAMDECK_MEDIAMTX_API=127.0.0.1:9997 \
//   cargo test --features gstreamer --test stream_gst_e2e -- --ignored --nocapture
#![cfg(feature = "gstreamer")]

use std::net::SocketAddr;
use std::time::{Duration, Instant};

use serde_json::Value;
use streamdeck_lib::stream::{AudioSource, StreamSettings, VideoSource};
use streamdeck_lib::stream_gst::GstStreamBackend;
use streamdeck_lib::stream_service::{BackendEvent, StartPlan, StreamBackend};
use streamdeck_lib::whip_server::http_request;

fn env(name: &str) -> String {
    std::env::var(name).unwrap_or_else(|_| panic!("set {name} (use scripts/stream-e2e.sh)"))
}

async fn paths(api: SocketAddr) -> Vec<Value> {
    let r = http_request(api, "GET", "/v3/paths/list", &[], "")
        .await
        .expect("mediamtx api");
    let v: Value = serde_json::from_str(&r.body).expect("json");
    v["items"].as_array().cloned().unwrap_or_default()
}

fn path_named<'a>(items: &'a [Value], name: &str) -> Option<&'a Value> {
    items.iter().find(|p| p["name"] == name)
}

async fn wait_for<F: Fn(&[Value]) -> bool>(
    api: SocketAddr,
    what: &str,
    secs: u64,
    f: F,
) -> Vec<Value> {
    let until = Instant::now() + Duration::from_secs(secs);
    loop {
        let items = paths(api).await;
        if f(&items) {
            return items;
        }
        assert!(
            Instant::now() < until,
            "timed out waiting for {what}: {items:?}"
        );
        tokio::time::sleep(Duration::from_millis(300)).await;
    }
}

fn ready(items: &[Value], name: &str) -> bool {
    path_named(items, name).is_some_and(|p| p["ready"] == true)
}

fn drain_until(
    backend: &mut GstStreamBackend,
    secs: u64,
    want: impl Fn(&BackendEvent) -> bool,
) -> Vec<BackendEvent> {
    let until = Instant::now() + Duration::from_secs(secs);
    let mut seen = Vec::new();
    while Instant::now() < until {
        let batch = backend.poll();
        let hit = batch.iter().any(&want);
        seen.extend(batch);
        if hit {
            return seen;
        }
        std::thread::sleep(Duration::from_millis(200));
    }
    seen
}

#[tokio::test(flavor = "multi_thread")]
#[ignore = "needs a local RTMP server: scripts/stream-e2e.sh"]
async fn real_encode_fans_out_to_independent_rtmp_destinations() {
    let base = env("STREAMDECK_RTMP_URL");
    let api: SocketAddr = env("STREAMDECK_MEDIAMTX_API").parse().expect("api addr");

    let mut backend = GstStreamBackend::new().expect("GStreamer with flvmux and rtmp2sink");
    let settings = StreamSettings {
        width: 1280,
        height: 720,
        fps: 30,
        video_kbps: 2500,
        ..Default::default()
    };
    let plan = StartPlan {
        settings,
        video: VideoSource::TestPattern,
        audio: AudioSource::Silence,
        allow_gpl_encoder: true,
    };
    let encoder = backend.start(&plan).expect("start");
    eprintln!(
        "encoder: {} (hardware: {})",
        encoder.element, encoder.hardware
    );

    backend
        .add_destination("a", &format!("{base}/key-a"))
        .expect("add a");
    backend
        .add_destination("b", &format!("{base}/key-b"))
        .expect("add b");
    let up = drain_until(&mut backend, 15, |_| false);
    eprintln!("events after 15 s: {up:?}");
    let items = wait_for(api, "both paths ready", 10, |i| {
        ready(i, "live/key-a") && ready(i, "live/key-b")
    })
    .await;
    let a = path_named(&items, "live/key-a").expect("a");
    let kinds = a["tracks"].to_string();
    assert!(kinds.contains("H264"), "video track: {kinds}");
    assert!(
        kinds.contains("MPEG-4") || kinds.contains("AAC") || kinds.contains("Audio"),
        "audio track: {kinds}"
    );
    assert!(up
        .iter()
        .any(|e| matches!(e, BackendEvent::DestinationUp(id) if id == "a")));
    assert!(up.iter().any(|e| matches!(e, BackendEvent::Stats { id, bitrate_kbps, .. } if id == "a" && *bitrate_kbps > 100)), "bitrate reported: {up:?}");

    // Removing one destination at runtime leaves the other streaming.
    backend.remove_destination("b").expect("remove b");
    wait_for(api, "b gone, a still ready", 10, |i| {
        !ready(i, "live/key-b") && ready(i, "live/key-a")
    })
    .await;
    let before = path_named(&paths(api).await, "live/key-a").expect("a")["inboundBytes"]
        .as_u64()
        .unwrap_or(0);
    tokio::time::sleep(Duration::from_secs(2)).await;
    let after = path_named(&paths(api).await, "live/key-a").expect("a")["inboundBytes"]
        .as_u64()
        .unwrap_or(0);
    assert!(
        after > before,
        "a keeps receiving after b is removed ({before} -> {after})"
    );

    // A destination that refuses the connection fails alone.
    backend
        .add_destination("dead", "rtmp://127.0.0.1:1/live/nope")
        .expect("add dead");
    let events = drain_until(
        &mut backend,
        15,
        |e| matches!(e, BackendEvent::DestinationDown { id, .. } if id == "dead"),
    );
    assert!(
        events
            .iter()
            .any(|e| matches!(e, BackendEvent::DestinationDown { id, .. } if id == "dead")),
        "refused destination reports Down: {events:?}"
    );
    backend.remove_destination("dead").expect("remove dead");
    let b1 = path_named(&paths(api).await, "live/key-a").expect("a still ready")["inboundBytes"]
        .as_u64()
        .unwrap_or(0);
    tokio::time::sleep(Duration::from_secs(2)).await;
    let b2 = path_named(&paths(api).await, "live/key-a").expect("a")["inboundBytes"]
        .as_u64()
        .unwrap_or(0);
    assert!(
        b2 > b1,
        "a is unaffected by the dead destination ({b1} -> {b2})"
    );
    // b can be added again (runtime add after remove), then a graceful stop ends everything.
    backend
        .add_destination("b", &format!("{base}/key-b"))
        .expect("re-add b");
    wait_for(api, "b back", 15, |i| ready(i, "live/key-b")).await;
    backend.stop().expect("stop");
    wait_for(api, "all paths closed", 10, |i| {
        !ready(i, "live/key-a") && !ready(i, "live/key-b")
    })
    .await;
    backend.stop().expect("stop is idempotent");
}

#[tokio::test(flavor = "multi_thread")]
#[ignore = "needs a local RTMP server: scripts/stream-e2e.sh"]
async fn the_compositor_program_channel_is_what_gets_streamed() {
    use gstreamer as gst;
    use gstreamer::prelude::*;

    let base = env("STREAMDECK_RTMP_URL");
    let api: SocketAddr = env("STREAMDECK_MEDIAMTX_API").parse().expect("api addr");
    gst::init().expect("gst");
    // Stands in for the compositor's `intervideosink channel=program` branch.
    let producer = gst::parse::launch(
        "videotestsrc is-live=true pattern=smpte ! video/x-raw,width=1920,height=1080,framerate=30/1 ! intervideosink channel=program sync=false",
    )
    .expect("producer");
    producer
        .set_state(gst::State::Playing)
        .expect("producer plays");

    let mut backend = GstStreamBackend::new().expect("engine");
    let plan = StartPlan {
        settings: StreamSettings {
            width: 1280,
            height: 720,
            video_kbps: 2500,
            ..Default::default()
        },
        video: VideoSource::Program("program".into()),
        audio: AudioSource::Silence,
        allow_gpl_encoder: true,
    };
    backend.start(&plan).expect("start");
    backend
        .add_destination("prog", &format!("{base}/key-prog"))
        .expect("add");
    let items = wait_for(api, "program stream online", 15, |i| {
        ready(i, "live/key-prog")
    })
    .await;
    let before = path_named(&items, "live/key-prog").expect("path")["inboundBytes"]
        .as_u64()
        .unwrap_or(0);
    tokio::time::sleep(Duration::from_secs(3)).await;
    let after = path_named(&paths(api).await, "live/key-prog").expect("path")["inboundBytes"]
        .as_u64()
        .unwrap_or(0);
    // smpte bars encode to far more than a still-black stream would.
    assert!(
        after - before > 100_000,
        "program video is flowing and not black: {before} -> {after}"
    );

    backend.stop().expect("stop");
    let _ = producer.set_state(gst::State::Null);
}
