// F2.1: pipeline commands through real IPC, real bus and the real actor over a simulated backend.

mod common;

use common::TestApp;
use serde_json::{json, Value};
use streamdeck_lib::events::AppEvent;
use streamdeck_lib::pipeline::{PipelineState, TEST_PIPELINE};

fn call(t: &TestApp, cmd: &str) -> Result<(), Value> {
    t.invoke::<()>(cmd, json!({}))
}

fn state(t: &TestApp) -> Value {
    t.invoke::<Value>("pipeline_get_state", json!({}))
        .expect("pipeline_get_state")
}

#[tokio::test(flavor = "multi_thread")]
async fn start_pause_resume_stop_round_trip_publishes_each_state() {
    let t = TestApp::new();
    let mut events = t.collect_events();
    assert_eq!(state(&t), json!("Null"));

    for cmd in [
        "pipeline_start_test",
        "pipeline_pause",
        "pipeline_resume",
        "pipeline_stop",
    ] {
        call(&t, cmd).unwrap_or_else(|e| panic!("{cmd}: {e}"));
    }

    let states: Vec<PipelineState> = events
        .drain()
        .into_iter()
        .filter_map(|e| match e {
            AppEvent::PipelineStateChanged { state } => Some(state),
            _ => None,
        })
        .collect();
    assert_eq!(
        states,
        [
            PipelineState::Playing,
            PipelineState::Paused,
            PipelineState::Playing,
            PipelineState::Null
        ]
    );
    assert_eq!(
        t.pipeline_backend.calls(),
        [
            format!("start:{TEST_PIPELINE}"),
            "pause".into(),
            "resume".into(),
            "stop".into()
        ]
    );
}

#[tokio::test(flavor = "multi_thread")]
async fn invalid_transition_returns_a_typed_error() {
    let t = TestApp::new();
    let err = call(&t, "pipeline_pause").expect_err("pause while idle must fail");
    assert_eq!(err["type"], "InvalidInput");
    assert!(t.pipeline_backend.calls().is_empty());
}

#[tokio::test(flavor = "multi_thread")]
async fn backend_start_failure_surfaces_as_an_error_and_state_stays_null() {
    let t = TestApp::new();
    t.pipeline_backend.fail_next_start("no display");
    let err = call(&t, "pipeline_start_test").expect_err("start fails");
    assert_eq!(err["type"], "Internal");
    assert_eq!(state(&t), json!("Null"));
}
