// T1.1: proves the mock-runtime harness boots with real state and dispatches real commands.

mod common;

use std::time::Duration;

use common::TestApp;
use streamdeck_lib::config::StreamDeckConfig;
use streamdeck_lib::events::AppEvent;
use tauri::Manager;

#[test]
fn get_build_info_round_trips_through_ipc() {
    let t = TestApp::new();

    let info: serde_json::Value = t
        .invoke("get_build_info", serde_json::json!({}))
        .expect("get_build_info succeeds");

    assert_eq!(info["version"], env!("CARGO_PKG_VERSION"));
    assert!(!info["build_timestamp"].as_str().unwrap_or("").is_empty());
}

#[test]
fn get_config_returns_the_managed_config() {
    let mut config = StreamDeckConfig::default();
    config.app.log_level = "trace".into();
    let t = TestApp::with_config(config);

    let got: StreamDeckConfig = t
        .invoke("get_config", serde_json::json!({}))
        .expect("get_config succeeds");

    assert_eq!(got.app.log_level, "trace");
    assert_eq!(
        t.app.state::<StreamDeckConfig>().app.log_level,
        "trace",
        "config is managed state, not a copy built per call"
    );
}

#[test]
fn unknown_command_is_rejected_not_swallowed() {
    let t = TestApp::new();

    let res: Result<serde_json::Value, _> = t.invoke("no_such_command", serde_json::json!({}));

    let err = res.expect_err("unknown command must error");
    let text = err.to_string().to_lowercase();
    assert!(
        text.contains("no_such_command") && text.contains("not found"),
        "error should name the command and say not found, got: {text}"
    );
}

#[tokio::test(flavor = "multi_thread")]
async fn async_command_with_state_machine_reads_the_managed_deck() {
    let t = TestApp::new();

    let payload: serde_json::Value = t
        .invoke("slide_get_current", serde_json::json!({}))
        .expect("slide_get_current succeeds");

    assert_eq!(payload["index"], 0);
    assert_eq!(payload["title"], "Welcome");
}

#[tokio::test(flavor = "multi_thread")]
async fn slide_advance_via_ipc_mutates_state_and_publishes_events() {
    let t = TestApp::new();
    let mut events = t.collect_events();

    let _: () = t
        .invoke("slide_advance", serde_json::json!({}))
        .expect("slide_advance succeeds");

    let seen = events
        .collect_until(Duration::from_secs(2), |e| {
            e.iter().any(|x| matches!(x, AppEvent::SlideUpdate(_)))
        })
        .await;
    assert!(seen.iter().any(|e| matches!(e, AppEvent::SlideAdvance)));
    assert!(seen.iter().any(
        |e| matches!(e, AppEvent::SlideUpdate(p) if p.index == 1 && p.title == "Architecture")
    ));
    assert_eq!(t.machine.read().await.current_index(), 1);
}

#[tokio::test]
async fn bus_collector_helper_sees_events_published_after_subscribing() {
    let t = TestApp::new();
    let mut events = t.collect_events();

    t.bus.publish(AppEvent::Ping {
        message: "hello".into(),
    });

    let seen = events
        .collect_until(Duration::from_secs(2), |e| !e.is_empty())
        .await;
    assert!(matches!(
        seen.as_slice(),
        [AppEvent::Ping { message }] if message == "hello"
    ));
}
