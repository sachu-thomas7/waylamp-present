// T1.3: timer reset plus the read-only slide queries (slide_get_current, deck_get_all_slides)
// on the startup stub deck, through real IPC, real bus and real state machine.

mod common;

use common::TestApp;
use serde_json::{json, Value};
use waylamp_present_lib::config::WaylampConfig;
use waylamp_present_lib::events::AppEvent;

fn invoke_unit(t: &TestApp, cmd: &str, args: Value) {
    let _: () = t
        .invoke(cmd, args)
        .unwrap_or_else(|e| panic!("{cmd} failed: {e}"));
}

fn persisted_timer(t: &TestApp) -> Value {
    let raw = std::fs::read_to_string(t.data_dir.path().join("slide_state.json"))
        .expect("slide_state.json written");
    serde_json::from_str::<Value>(&raw).expect("valid json")["timer_started_at"].clone()
}

#[tokio::test(flavor = "multi_thread")]
async fn reset_starts_the_timer_publishes_reset_then_zero_tick_and_persists() {
    let t = TestApp::new();
    assert_eq!(t.machine.read().await.timer_started_at(), None);
    let mut events = t.collect_events();

    invoke_unit(&t, "slide_timer_reset", json!({}));

    let started = t.machine.read().await.timer_started_at();
    assert!(started.is_some(), "reset must start the timer");
    // The controller zeroes its display on TimerReset and then renders the 0 tick.
    assert!(matches!(
        events.drain().as_slice(),
        [
            AppEvent::TimerReset,
            AppEvent::TimerTick { elapsed_secs: 0 }
        ]
    ));
    assert!(started.is_some());
    assert_eq!(persisted_timer(&t), json!(started));
}

#[tokio::test(flavor = "multi_thread")]
async fn reset_keeps_the_current_slide_and_survives_a_restart() {
    let first = TestApp::new();
    invoke_unit(&first, "slide_jump_to", json!({ "index": 2 }));
    invoke_unit(&first, "slide_timer_reset", json!({}));
    let started = first.machine.read().await.timer_started_at();
    assert_eq!(first.machine.read().await.current_index(), 2);
    let TestApp { data_dir, .. } = first;
    let t = TestApp::with_data_dir(data_dir, WaylampConfig::default());

    waylamp_present_lib::state::restore_on_startup(&t.machine, &t.bus, t.data_dir.path()).await;

    assert_eq!(t.machine.read().await.current_index(), 2);
    assert_eq!(t.machine.read().await.timer_started_at(), started);
}

#[tokio::test(flavor = "multi_thread")]
async fn reset_when_state_cannot_be_persisted_errors_without_events() {
    let t = TestApp::new();
    // A file where the data dir should be makes create_dir_all fail.
    let dir = t.data_dir.path().to_path_buf();
    std::fs::remove_dir_all(&dir).expect("remove data dir");
    std::fs::write(&dir, b"not a directory").expect("occupy path with a file");
    let mut events = t.collect_events();

    let res: Result<(), Value> = t.invoke("slide_timer_reset", json!({}));

    assert_eq!(
        res.expect_err("persist failure surfaces")["type"],
        "Internal"
    );
    assert!(
        events.drain().is_empty(),
        "no TimerReset when persist fails"
    );
    assert_eq!(
        t.machine.read().await.timer_started_at(),
        None,
        "in-memory timer is left unchanged when persist fails"
    );
    // Restore the directory so TempDir cleanup succeeds.
    std::fs::remove_file(&dir).expect("remove blocker");
    std::fs::create_dir(&dir).expect("recreate data dir");
}

#[tokio::test(flavor = "multi_thread")]
async fn get_current_returns_the_stub_deck_first_slide_and_follows_navigation() {
    let t = TestApp::new();

    let first: Value = t.invoke("slide_get_current", json!({})).expect("current");
    assert_eq!(first["index"], 0);
    assert_eq!(first["total"], 5);
    assert_eq!(first["title"], "Welcome");
    assert_eq!(first["background_path"], Value::Null);
    assert_eq!(first["layout_unsupported"], false);

    invoke_unit(&t, "slide_jump_to", json!({ "index": 4 }));
    let last: Value = t.invoke("slide_get_current", json!({})).expect("current");
    assert_eq!(
        (last["index"].clone(), last["title"].clone()),
        (json!(4), json!("Next: F1.3"))
    );
}

#[tokio::test(flavor = "multi_thread")]
async fn get_all_slides_lists_the_stub_deck_in_order_with_consistent_totals() {
    let t = TestApp::new();

    let all: Vec<Value> = t.invoke("deck_get_all_slides", json!({})).expect("all");

    let titles: Vec<&str> = all
        .iter()
        .map(|s| s["title"].as_str().unwrap_or(""))
        .collect();
    assert_eq!(
        titles,
        [
            "Welcome",
            "Architecture",
            "Navigation",
            "Observability",
            "Next: F1.3"
        ]
    );
    for (i, s) in all.iter().enumerate() {
        assert_eq!(s["index"], i);
        assert_eq!(s["total"], 5);
    }
}
