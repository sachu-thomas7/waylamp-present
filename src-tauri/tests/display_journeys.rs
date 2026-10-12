// T1.3: display list / assignment / hotplug fallback through real IPC, real bus, real display
// state and a real streamdeck.toml in a temp dir. Only the monitor list is faked (OS boundary).

mod common;

use common::{display, TestApp};
use serde_json::{json, Value};
use streamdeck_lib::config::StreamDeckConfig;
use streamdeck_lib::config_loader::ConfigLoader;
use streamdeck_lib::display::{reconcile_displays, DisplayInfo, DisplayStateHandle};
use streamdeck_lib::events::AppEvent;
use tauri::{Manager, WebviewUrl, WebviewWindowBuilder};

const PRIMARY: &str = "Primary@0,0";
const PROJECTOR: &str = "Projector@1920,0";

fn state(t: &TestApp) -> DisplayStateHandle {
    t.app.state::<DisplayStateHandle>().inner().clone()
}

async fn reconcile(t: &TestApp) {
    let s = state(t);
    reconcile_displays(t.app.handle(), t.displays.as_ref(), &t.bus, &s).await;
}

fn saved_display_id(t: &TestApp) -> Option<String> {
    ConfigLoader::load_or_default(&t.config_path())
        .expect("config readable")
        .presentation
        .display_id
}

#[tokio::test(flavor = "multi_thread")]
async fn list_returns_the_provider_displays_and_caches_them_in_state() {
    let t = TestApp::new();

    let listed: Vec<DisplayInfo> = t.invoke("display_list", json!({})).expect("display_list");

    assert_eq!(
        listed,
        vec![
            display("Primary", 0, true),
            display("Projector", 1920, false)
        ]
    );
    assert_eq!(state(&t).read().await.displays, listed);
}

#[tokio::test(flavor = "multi_thread")]
async fn list_surfaces_provider_failure_and_keeps_cached_state() {
    let t = TestApp::new();
    let _: Vec<DisplayInfo> = t.invoke("display_list", json!({})).expect("display_list");
    t.displays.fail("monitor enumeration failed");

    let res: Result<Vec<DisplayInfo>, Value> = t.invoke("display_list", json!({}));

    let err = res.expect_err("provider failure is an error");
    assert_eq!(err["type"], "Internal");
    assert_eq!(state(&t).read().await.displays.len(), 2);
}

#[tokio::test(flavor = "multi_thread")]
async fn assign_persists_to_toml_updates_state_and_publishes() {
    let t = TestApp::new();
    let mut events = t.collect_events();

    let _: () = t
        .invoke("display_assign_presentation", json!({ "id": PROJECTOR }))
        .expect("assign");

    assert_eq!(saved_display_id(&t).as_deref(), Some(PROJECTOR));
    assert_eq!(
        state(&t).read().await.assigned_display_id.as_deref(),
        Some(PROJECTOR)
    );
    assert_eq!(state(&t).read().await.displays, t.displays.list_now());
    assert!(matches!(
        events.drain().as_slice(),
        [AppEvent::DisplayAssigned { display_id }] if display_id == PROJECTOR
    ));
}

#[tokio::test(flavor = "multi_thread")]
async fn assign_keeps_other_config_values_and_tolerates_a_presentation_window() {
    let t = TestApp::new();
    let mut custom = StreamDeckConfig::default();
    custom.app.log_level = "debug".into();
    ConfigLoader::save(&t.config_path(), &custom).expect("seed config");
    WebviewWindowBuilder::new(&t.app, "presentation", WebviewUrl::default())
        .build()
        .expect("mock presentation window");

    let _: () = t
        .invoke("display_assign_presentation", json!({ "id": PROJECTOR }))
        .expect("assign with a presentation window present");

    let saved = ConfigLoader::load_or_default(&t.config_path()).expect("config");
    assert_eq!(saved.app.log_level, "debug");
    assert_eq!(saved.presentation.display_id.as_deref(), Some(PROJECTOR));
}

#[tokio::test(flavor = "multi_thread")]
async fn assign_unknown_display_is_not_found_and_changes_nothing() {
    let t = TestApp::new();
    let mut events = t.collect_events();

    let res: Result<(), Value> =
        t.invoke("display_assign_presentation", json!({ "id": "Ghost@9,9" }));

    assert_eq!(res.expect_err("unknown id")["type"], "NotFound");
    assert!(!t.config_path().exists(), "no config written for a bad id");
    assert!(state(&t).read().await.assigned_display_id.is_none());
    assert!(events.drain().is_empty());
}

#[tokio::test(flavor = "multi_thread")]
async fn assign_with_unreadable_config_reports_config_error_and_does_not_overwrite_it() {
    let t = TestApp::new();
    std::fs::create_dir_all(t.config_path().parent().expect("parent")).expect("mkdir");
    std::fs::write(t.config_path(), "not valid toml ][[[").expect("seed corrupt config");
    let mut events = t.collect_events();

    let res: Result<(), Value> =
        t.invoke("display_assign_presentation", json!({ "id": PROJECTOR }));

    assert_eq!(res.expect_err("corrupt config")["type"], "Config");
    assert_eq!(
        std::fs::read_to_string(t.config_path()).expect("read"),
        "not valid toml ][[["
    );
    assert!(events.drain().is_empty(), "no DisplayAssigned on failure");
    assert!(state(&t).read().await.assigned_display_id.is_none());
}

#[tokio::test(flavor = "multi_thread")]
async fn startup_assignment_from_config_seeds_display_state() {
    let mut cfg = StreamDeckConfig::default();
    cfg.presentation.display_id = Some(PROJECTOR.into());
    let t = TestApp::with_config(cfg);

    assert_eq!(
        state(&t).read().await.assigned_display_id.as_deref(),
        Some(PROJECTOR)
    );
}

#[tokio::test(flavor = "multi_thread")]
async fn losing_the_assigned_display_publishes_lost_and_clears_the_assignment() {
    let t = TestApp::new();
    let _: () = t
        .invoke("display_assign_presentation", json!({ "id": PROJECTOR }))
        .expect("assign");
    reconcile(&t).await;
    let mut events = t.collect_events();

    t.displays.set(vec![display("Primary", 0, true)]);
    reconcile(&t).await;

    let seen = events.drain();
    assert!(
        matches!(
            seen.as_slice(),
            [AppEvent::DisplaysChanged(now), AppEvent::DisplayLost]
                if now == &vec![display("Primary", 0, true)]
        ),
        "got {seen:?}"
    );
    let s = state(&t);
    let guard = s.read().await;
    assert!(guard.assigned_display_id.is_none());
    assert_eq!(guard.displays, vec![display("Primary", 0, true)]);
}

#[tokio::test(flavor = "multi_thread")]
async fn display_lost_is_published_once_and_nothing_else_while_displays_are_stable() {
    let t = TestApp::new();
    let _: () = t
        .invoke("display_assign_presentation", json!({ "id": PROJECTOR }))
        .expect("assign");
    reconcile(&t).await;
    t.displays.set(vec![display("Primary", 0, true)]);
    reconcile(&t).await;
    let mut events = t.collect_events();

    reconcile(&t).await;
    reconcile(&t).await;

    assert!(events.drain().is_empty());
}

#[tokio::test(flavor = "multi_thread")]
async fn hotplug_without_losing_the_assigned_display_only_publishes_displays_changed() {
    let t = TestApp::new();
    let _: () = t
        .invoke("display_assign_presentation", json!({ "id": PRIMARY }))
        .expect("assign");
    reconcile(&t).await;
    let mut events = t.collect_events();

    let mut three = t.displays.list_now();
    three.push(display("TV", 3840, false));
    t.displays.set(three.clone());
    reconcile(&t).await;

    assert!(matches!(
        events.drain().as_slice(),
        [AppEvent::DisplaysChanged(now)] if now == &three
    ));
    assert_eq!(
        state(&t).read().await.assigned_display_id.as_deref(),
        Some(PRIMARY)
    );
}

#[tokio::test(flavor = "multi_thread")]
async fn provider_failure_during_polling_is_ignored() {
    let t = TestApp::new();
    let _: () = t
        .invoke("display_assign_presentation", json!({ "id": PROJECTOR }))
        .expect("assign");
    reconcile(&t).await;
    let mut events = t.collect_events();

    t.displays.fail("transient");
    reconcile(&t).await;

    assert!(events.drain().is_empty());
    assert_eq!(
        state(&t).read().await.assigned_display_id.as_deref(),
        Some(PROJECTOR)
    );
}

#[tokio::test(flavor = "multi_thread")]
async fn losing_the_only_display_still_publishes_lost_without_a_fallback() {
    let t = TestApp::new();
    let _: () = t
        .invoke("display_assign_presentation", json!({ "id": PROJECTOR }))
        .expect("assign");
    reconcile(&t).await;
    let mut events = t.collect_events();

    t.displays.set(Vec::new());
    reconcile(&t).await;

    let seen = events.drain();
    assert!(matches!(
        seen.as_slice(),
        [AppEvent::DisplaysChanged(now), AppEvent::DisplayLost] if now.is_empty()
    ));
}
