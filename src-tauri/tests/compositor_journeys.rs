// F3.1: compositor commands through real IPC, real bus and the real actor over a simulated backend.

mod common;

use common::TestApp;
use serde_json::{json, Value};
use streamdeck_lib::compositor::{demo_layout, SceneConfig, SourceGeometry};
use streamdeck_lib::config::StreamDeckConfig;
use streamdeck_lib::events::AppEvent;

fn start(t: &TestApp, scene: Value) -> Result<(), Value> {
    t.invoke::<()>("compositor_start", json!({ "sceneId": scene }))
}

fn state(t: &TestApp) -> Value {
    t.invoke::<Value>("compositor_get_state", json!({}))
        .expect("compositor_get_state")
}

fn geo(name: &str, x: i32, w: u32) -> SourceGeometry {
    SourceGeometry {
        name: name.into(),
        x,
        w,
        h: 360,
        ..SourceGeometry::default()
    }
}

fn config_with_scene() -> StreamDeckConfig {
    let mut c = StreamDeckConfig::default();
    c.scene.insert(
        "keynote".into(),
        SceneConfig {
            sources: vec![geo("speaker", 0, 1280), geo("wide", 1280, 640)],
        },
    );
    c.scene.insert(
        "broken".into(),
        SceneConfig {
            sources: vec![geo("a", 0, 0)],
        },
    );
    c
}

#[tokio::test(flavor = "multi_thread")]
async fn demo_start_set_layout_and_stop_round_trip_with_events() {
    let t = TestApp::new();
    let mut events = t.collect_events();
    assert_eq!(state(&t)["running"], json!(false));

    start(&t, Value::Null).expect("start demo");
    assert_eq!(
        t.pipeline_backend.compositor_attached(),
        ["cam-main", "cam-2", "cam-3"]
    );
    let s = state(&t);
    assert_eq!(s["running"], json!(true));
    assert_eq!(s["element"], json!("compositor"));
    assert_eq!(
        (s["width"].clone(), s["height"].clone(), s["fps"].clone()),
        (json!(1920), json!(1080), json!(30))
    );

    let mut layout = demo_layout();
    layout.truncate(2);
    t.invoke::<()>("compositor_set_layout", json!({ "sources": layout }))
        .expect("set layout");
    assert_eq!(
        t.pipeline_backend.compositor_attached(),
        ["cam-main", "cam-2"]
    );

    t.invoke::<()>("compositor_stop", json!({})).expect("stop");
    assert_eq!(state(&t)["running"], json!(false));

    let seen = events.drain();
    let kinds: Vec<String> = seen
        .iter()
        .filter_map(|e| match e {
            AppEvent::CompositorStateChanged { running, .. } => Some(format!("state:{running}")),
            AppEvent::CompositorLayoutChanged { sources } => {
                Some(format!("layout:{}", sources.len()))
            }
            _ => None,
        })
        .collect();
    assert_eq!(kinds, ["state:true", "layout:3", "layout:2", "state:false"]);
}

#[tokio::test(flavor = "multi_thread")]
async fn scenes_come_from_config_and_start_validates_them() {
    let t = TestApp::with_config(config_with_scene());
    let scenes: Vec<String> = t
        .invoke("compositor_list_scenes", json!({}))
        .expect("list scenes");
    assert_eq!(scenes, ["broken", "keynote"]);

    start(&t, json!("keynote")).expect("configured scene");
    assert_eq!(
        t.pipeline_backend.compositor_attached(),
        ["speaker", "wide"]
    );
    t.invoke::<()>("compositor_stop", json!({})).expect("stop");

    assert_eq!(
        start(&t, json!("nope")).expect_err("unknown")["type"],
        "NotFound"
    );
    let bad = start(&t, json!("broken")).expect_err("invalid scene");
    assert_eq!(bad["type"], "InvalidInput");
    assert!(bad["message"]
        .as_str()
        .unwrap_or_default()
        .contains("broken"));
    assert_eq!(state(&t)["running"], json!(false));
}

#[tokio::test(flavor = "multi_thread")]
async fn invalid_requests_return_typed_errors_and_leave_the_compositor_alone() {
    let t = TestApp::new();
    let err = t
        .invoke::<()>("compositor_set_layout", json!({ "sources": [] }))
        .expect_err("not running");
    assert_eq!(err["type"], "InvalidInput");

    start(&t, Value::Null).expect("start");
    assert_eq!(
        start(&t, Value::Null).expect_err("twice")["type"],
        "InvalidInput"
    );
    let dup = json!([geo("a", 0, 100), geo("a", 0, 100)]);
    let err = t
        .invoke::<()>("compositor_set_layout", json!({ "sources": dup }))
        .expect_err("duplicate names");
    assert_eq!(err["type"], "InvalidInput");
    assert_eq!(t.pipeline_backend.compositor_attached().len(), 3);
}

#[tokio::test(flavor = "multi_thread")]
async fn backend_failure_surfaces_and_the_compositor_stays_stopped() {
    let t = TestApp::new();
    t.pipeline_backend.fail_next_compositor_start("no display");
    let err = start(&t, Value::Null).expect_err("start fails");
    assert_eq!(err["type"], "Internal");
    assert_eq!(state(&t)["running"], json!(false));
    start(&t, Value::Null).expect("retry succeeds");
}
