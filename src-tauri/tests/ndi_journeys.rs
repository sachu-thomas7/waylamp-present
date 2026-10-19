// F2.2: NDI discovery + add/remove through real IPC, bus and the pipeline actor over simulated discovery and backend.

mod common;

use common::TestApp;
use serde_json::{json, Value};
use streamdeck_lib::events::AppEvent;
use streamdeck_lib::ndi::NdiSource;

fn snapshot(t: &TestApp) -> Value {
    t.invoke::<Value>("ndi_list_sources", json!({}))
        .expect("ndi_list_sources")
}

fn add(t: &TestApp, name: &str) -> Result<(), Value> {
    t.invoke::<()>("ndi_add_source", json!({ "name": name }))
}

fn names(v: &Value) -> Vec<String> {
    v.as_array()
        .expect("array")
        .iter()
        .map(|s| s["name"].as_str().expect("name").to_string())
        .collect()
}

#[tokio::test(flavor = "multi_thread")]
async fn a_camera_appearing_is_published_and_listed() {
    let t = TestApp::new();
    let mut events = t.collect_events();
    assert_eq!(names(&snapshot(&t)["discovered"]), Vec::<String>::new());

    t.ndi_discovery.appear(NdiSource::new("MEVO (Core 1)"));
    t.poll_ndi().await;

    let seen = events.drain();
    assert!(seen.iter().any(
        |e| matches!(e, AppEvent::SourceDiscovered { source } if source.name == "MEVO (Core 1)")
    ));
    assert!(seen
        .iter()
        .any(|e| matches!(e, AppEvent::SourceListChanged { sources } if sources.len() == 1)));
    assert_eq!(names(&snapshot(&t)["discovered"]), ["MEVO (Core 1)"]);
}

#[tokio::test(flavor = "multi_thread")]
async fn add_and_remove_do_not_restart_the_pipeline() {
    let t = TestApp::new();
    t.invoke::<()>("pipeline_start_test", json!({}))
        .expect("start");
    t.ndi_discovery.appear(NdiSource::new("Cam A"));
    t.poll_ndi().await;
    let mut events = t.collect_events();

    add(&t, "Cam A").expect("add");
    t.ndi_discovery.appear(NdiSource::new("Cam B"));
    t.poll_ndi().await;
    t.invoke::<()>("ndi_remove_source", json!({ "name": "Cam A" }))
        .expect("remove");

    let seen = events.drain();
    assert!(seen
        .iter()
        .all(|e| !matches!(e, AppEvent::PipelineStateChanged { .. })));
    assert!(seen
        .iter()
        .any(|e| matches!(e, AppEvent::SourceAdded { name, .. } if name == "Cam A")));
    assert!(seen
        .iter()
        .any(|e| matches!(e, AppEvent::SourceRemoved { name } if name == "Cam A")));
    assert_eq!(
        t.pipeline_backend.calls(),
        [
            format!("start:{}", streamdeck_lib::pipeline::TEST_PIPELINE),
            "add_source:Cam A:Standard".into(),
            "remove_source:Cam A".into()
        ]
    );
    let state = t.invoke::<Value>("pipeline_get_state", json!({}));
    assert_eq!(state.expect("state"), json!("Playing"));
    assert_eq!(names(&snapshot(&t)["discovered"]), ["Cam A", "Cam B"]);
}

#[tokio::test(flavor = "multi_thread")]
async fn source_lost_marks_the_added_source_lost_without_crashing() {
    let t = TestApp::new();
    t.ndi_discovery.appear(NdiSource::new("Cam A"));
    t.poll_ndi().await;
    add(&t, "Cam A").expect("add");
    let mut events = t.collect_events();

    t.ndi_discovery.disappear("Cam A");
    t.poll_ndi().await;

    assert!(events
        .drain()
        .iter()
        .any(|e| matches!(e, AppEvent::SourceLost { name } if name == "Cam A")));
    let snap = snapshot(&t);
    assert_eq!(snap["added"][0]["lost"], json!(true));
    assert_eq!(names(&snap["discovered"]), Vec::<String>::new());
    assert!(t.pipeline_backend.attached_sources().is_empty());

    t.ndi_discovery.appear(NdiSource::new("Cam A"));
    t.poll_ndi().await;
    assert_eq!(snapshot(&t)["added"][0]["lost"], json!(false));
    assert_eq!(t.pipeline_backend.attached_sources(), ["Cam A"]);
}

#[tokio::test(flavor = "multi_thread")]
async fn adding_an_unknown_or_duplicate_source_is_a_typed_error() {
    let t = TestApp::new();
    assert_eq!(add(&t, "Ghost").expect_err("unknown")["type"], "NotFound");
    t.ndi_discovery.appear(NdiSource::new("Cam A"));
    t.poll_ndi().await;
    add(&t, "Cam A").expect("add");
    assert_eq!(
        add(&t, "Cam A").expect_err("duplicate")["type"],
        "InvalidInput"
    );
    let err = t
        .invoke::<()>("ndi_remove_source", json!({ "name": "Ghost" }))
        .expect_err("remove unknown");
    assert_eq!(err["type"], "NotFound");
}

#[tokio::test(flavor = "multi_thread")]
async fn new_format_is_auto_selected_with_a_standard_fallback() {
    let t = TestApp::new();
    t.ndi_discovery.set_new_format_runtime(true);
    t.ndi_discovery.set(vec![
        NdiSource::new("Modern").with_new_format(),
        NdiSource::new("Legacy"),
    ]);
    t.poll_ndi().await;
    add(&t, "Modern").expect("add");
    add(&t, "Legacy").expect("add");
    let snap = snapshot(&t);
    assert_eq!(snap["added"][0]["format"], "NewFormat");
    assert_eq!(snap["added"][1]["format"], "Standard");
}

#[tokio::test(flavor = "multi_thread")]
async fn a_failing_scan_is_survivable() {
    let t = TestApp::new();
    t.ndi_discovery.appear(NdiSource::new("Cam A"));
    t.poll_ndi().await;
    t.ndi_discovery.fail_with(Some("network down"));
    t.poll_ndi().await;
    assert_eq!(names(&snapshot(&t)["discovered"]), ["Cam A"]);
}
