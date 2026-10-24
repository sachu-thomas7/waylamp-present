// T1.3: get_config and TOML hot-reload (real notify watcher on a temp dir -> real bus).
// Hot-reload tests cover ConfigReloaded publication only: nothing consumes it and get_config is not refreshed.

mod common;

use std::time::Duration;

use common::TestApp;
use serde_json::{json, Value};
use waylamp_present_lib::config::WaylampConfig;
use waylamp_present_lib::config_loader::ConfigLoader;
use waylamp_present_lib::error::AppError;
use waylamp_present_lib::events::AppEvent;

// Deadline for the OS file watcher; the wait ends as soon as the event arrives.
const WATCH_DEADLINE: Duration = Duration::from_secs(15);

fn is_reload(e: &AppEvent) -> bool {
    matches!(e, AppEvent::ConfigReloaded)
}

// Re-runs `touch` until a reload arrives: some OS watchers (FSEvents) start asynchronously and miss an early write.
async fn touch_until_reload(
    events: &mut common::EventCollector,
    mut touch: impl FnMut(),
) -> Vec<AppEvent> {
    let mut seen = Vec::new();
    let deadline = std::time::Instant::now() + WATCH_DEADLINE;
    while std::time::Instant::now() < deadline && !seen.iter().any(is_reload) {
        touch();
        seen.extend(
            events
                .collect_until(Duration::from_millis(500), |e| e.iter().any(is_reload))
                .await,
        );
    }
    seen
}

#[tokio::test(flavor = "multi_thread")]
async fn get_config_returns_the_defaults_the_app_started_with() {
    let t = TestApp::new();

    let cfg: Value = t.invoke("get_config", json!({})).expect("get_config");

    assert_eq!(cfg["version"], 1);
    assert_eq!(cfg["app"]["log_level"], "info");
    assert_eq!(cfg["presentation"]["display_id"], Value::Null);
}

#[tokio::test(flavor = "multi_thread")]
async fn get_config_returns_loaded_non_default_values() {
    let mut custom = WaylampConfig::default();
    custom.app.log_level = "trace".into();
    custom.presentation.display_id = Some("Projector@1920,0".into());
    let t = TestApp::with_config(custom);

    let cfg: Value = t.invoke("get_config", json!({})).expect("get_config");

    assert_eq!(cfg["app"]["log_level"], "trace");
    assert_eq!(cfg["presentation"]["display_id"], "Projector@1920,0");
}

#[tokio::test(flavor = "multi_thread")]
async fn editing_the_config_file_publishes_config_reloaded_event_only() {
    let t = TestApp::new();
    let path = t.config_path();
    ConfigLoader::save(&path, &WaylampConfig::default()).expect("seed config");
    let _watcher = ConfigLoader::spawn_watcher(path.clone(), t.bus.clone()).expect("watcher");
    let mut events = t.collect_events();

    let mut edited = WaylampConfig::default();
    edited.app.log_level = "debug".into();

    let seen = touch_until_reload(&mut events, || {
        ConfigLoader::save(&path, &edited).expect("edit config")
    })
    .await;
    assert!(
        seen.iter().any(is_reload),
        "no ConfigReloaded: {seen:?} (path {path:?}, canonical {:?})",
        path.canonicalize()
    );
}

#[tokio::test(flavor = "multi_thread")]
async fn creating_the_config_file_after_the_watcher_starts_publishes_config_reloaded_event_only() {
    let t = TestApp::new();
    let path = t.config_path();
    std::fs::create_dir_all(path.parent().expect("parent")).expect("mkdir");
    let _watcher = ConfigLoader::spawn_watcher(path.clone(), t.bus.clone()).expect("watcher");
    let mut events = t.collect_events();

    let seen = touch_until_reload(&mut events, || {
        std::fs::write(&path, "version = 1\n").expect("create config")
    })
    .await;
    assert!(
        seen.iter().any(is_reload),
        "no ConfigReloaded: {seen:?} (path {path:?}, canonical {:?})",
        path.canonicalize()
    );
}

#[tokio::test(flavor = "multi_thread")]
async fn a_corrupt_edit_still_publishes_config_reloaded_event_only() {
    let t = TestApp::new();
    let path = t.config_path();
    ConfigLoader::save(&path, &WaylampConfig::default()).expect("seed config");
    let _watcher = ConfigLoader::spawn_watcher(path.clone(), t.bus.clone()).expect("watcher");
    let mut events = t.collect_events();

    let seen = touch_until_reload(&mut events, || {
        std::fs::write(&path, "not valid toml ][[[").expect("corrupt config")
    })
    .await;
    assert!(
        seen.iter().any(is_reload),
        "no ConfigReloaded: {seen:?} (path {path:?}, canonical {:?})",
        path.canonicalize()
    );
}

#[tokio::test(flavor = "multi_thread")]
async fn watcher_refuses_a_path_without_a_parent_directory() {
    let t = TestApp::new();

    let res = ConfigLoader::spawn_watcher(std::path::PathBuf::from("/"), t.bus.clone());

    assert!(matches!(res, Err(AppError::Config(_))));
}

#[tokio::test(flavor = "multi_thread")]
async fn changes_to_sibling_files_do_not_publish_config_reloaded() {
    let t = TestApp::new();
    let dir = t.config_path().parent().expect("parent").to_path_buf();
    std::fs::create_dir_all(&dir).expect("mkdir");
    let watched = dir.join("waylamp-present.toml");
    let sibling = dir.join("other.toml");
    // A second watcher on the same directory tells us when the OS has reported the sibling write.
    let (probe_bus, _rx) = waylamp_present_lib::bus::EventBus::new();
    let probe_bus = std::sync::Arc::new(probe_bus);
    let _probe = ConfigLoader::spawn_watcher(sibling.clone(), probe_bus.clone()).expect("probe");
    let _watcher = ConfigLoader::spawn_watcher(watched.clone(), t.bus.clone()).expect("watcher");
    // Independent reference: a raw notify watcher with its own path filter (not the code under test).
    let ref_hits = std::sync::Arc::new(std::sync::atomic::AtomicUsize::new(0));
    let ref_hits_cb = ref_hits.clone();
    let ref_targets = [
        watched.clone(),
        dir.canonicalize()
            .expect("canonical dir")
            .join("waylamp-present.toml"),
    ];
    let mut ref_watcher = notify::recommended_watcher(move |res: notify::Result<notify::Event>| {
        if let Ok(e) = res {
            if matches!(
                e.kind,
                notify::EventKind::Modify(_) | notify::EventKind::Create(_)
            ) && e.paths.iter().any(|p| ref_targets.contains(p))
            {
                ref_hits_cb.fetch_add(1, std::sync::atomic::Ordering::SeqCst);
            }
        }
    })
    .expect("reference watcher");
    notify::Watcher::watch(&mut ref_watcher, &dir, notify::RecursiveMode::NonRecursive)
        .expect("watch dir");
    let mut probe_events = common::EventCollector::subscribe(&probe_bus);
    let mut events = t.collect_events();

    let seen = touch_until_reload(&mut probe_events, || {
        std::fs::write(&sibling, "x = 1\n").expect("write sibling")
    })
    .await;
    assert!(seen.iter().any(is_reload), "probe never saw the write");

    // Both watchers share one directory, so ordering the real write behind the probe makes the
    // sibling write's events (if any) reach the real bus before this one.
    // The OS reports one write as several events, so compare against a reference watcher that
    // sees exactly the watched file's events: any surplus on the real bus came from the sibling.
    let mut real_count = 0;
    let mut ref_count = 0;
    let deadline = std::time::Instant::now() + WATCH_DEADLINE;
    let mut last_write: Option<std::time::Instant> = None;
    while std::time::Instant::now() < deadline {
        // Keep writing only until the reference watcher is live, then let the counts settle.
        if ref_hits.load(std::sync::atomic::Ordering::SeqCst) == 0
            && last_write.is_none_or(|t| t.elapsed() > Duration::from_millis(500))
        {
            std::fs::write(&watched, "version = 1\n").expect("write watched");
            last_write = Some(std::time::Instant::now());
        }
        real_count += events.drain().len();
        ref_count = ref_hits.load(std::sync::atomic::Ordering::SeqCst);
        if ref_count > 0 && real_count == ref_count {
            break;
        }
        tokio::time::sleep(Duration::from_millis(20)).await;
    }
    assert!(ref_count > 0, "reference watcher never saw the write");
    assert_eq!(
        real_count, ref_count,
        "sibling write leaked into the config watcher"
    );
}
