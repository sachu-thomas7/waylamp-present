// F4: multi-destination streaming through real IPC, the real controller and a simulated engine.
mod common;

use std::time::{Duration, Instant};

use common::TestApp;
use serde_json::{json, Value};
use streamdeck_lib::events::AppEvent;
use streamdeck_lib::stream::{DestinationState, StreamState};
use streamdeck_lib::stream_service::{BackendEvent, KeyStore};

const FB_KEY: &str = "FB-SECRET-KEY-1234";
const YT_KEY: &str = "yt-secret-key-5678";
const FB_URL: &str = "rtmps://live-api-s.facebook.com:443/rtmp/";
const YT_URL: &str = "rtmp://a.rtmp.youtube.com/live2";

fn dest(id: &str, platform: &str, url: &str) -> Value {
    json!({ "id": id, "name": format!("{id} live"), "platform": platform, "ingest_url": url, "enabled": true })
}

fn save(t: &TestApp, d: Value, key: Option<&str>) -> Value {
    t.invoke(
        "stream_save_destination",
        json!({ "destination": d, "key": key }),
    )
    .expect("save destination")
}

fn add_both(t: &TestApp) {
    save(t, dest("fb", "facebook", FB_URL), Some(FB_KEY));
    save(t, dest("yt", "youtube", YT_URL), Some(YT_KEY));
}

fn start(t: &TestApp) -> Value {
    t.invoke("stream_start", json!({})).expect("stream_start")
}

fn status(t: &TestApp) -> Value {
    t.invoke("stream_get_status", json!({})).expect("status")
}

fn dest_state(t: &TestApp, id: &str) -> Value {
    status(t)["destinations"]
        .as_array()
        .expect("destinations")
        .iter()
        .find(|d| d["id"] == id)
        .expect("destination")["state"]
        .clone()
}

fn calls_matching(t: &TestApp, what: &str) -> usize {
    t.stream_backend
        .calls()
        .iter()
        .filter(|c| *c == what)
        .count()
}

#[tokio::test(flavor = "multi_thread")]
async fn keys_go_to_the_keychain_never_to_toml_or_back_to_the_ui() {
    let t = TestApp::new();
    let view = save(&t, dest("fb", "facebook", FB_URL), Some(FB_KEY));

    assert_eq!(view["destinations"][0]["key_set"], true);
    assert!(
        !view.to_string().contains(FB_KEY),
        "key must not be echoed to the UI"
    );
    let toml = std::fs::read_to_string(t.config_path()).expect("config");
    assert!(
        toml.contains("[[stream.destinations]]")
            && toml.contains("rtmps://live-api-s.facebook.com")
    );
    assert!(!toml.contains(FB_KEY));
    assert_eq!(
        t.stream_keys.get("fb").expect("keys").as_deref(),
        Some(FB_KEY)
    );

    let again = save(&t, dest("fb", "facebook", FB_URL), None);
    assert_eq!(
        again["destinations"][0]["key_set"], true,
        "no key given keeps the stored one"
    );
}

#[tokio::test(flavor = "multi_thread")]
async fn start_needs_an_enabled_destination_with_a_key() {
    let t = TestApp::new();
    let none: Result<Value, Value> = t.invoke("stream_start", json!({}));
    assert_eq!(none.expect_err("no destinations")["type"], "InvalidInput");

    save(&t, dest("fb", "facebook", FB_URL), None);
    let nokey: Result<Value, Value> = t.invoke("stream_start", json!({}));
    let err = nokey.expect_err("no key");
    assert_eq!(err["type"], "InvalidInput");
    assert!(err["message"]
        .as_str()
        .unwrap_or("")
        .contains("no stream key"));
    assert!(
        t.stream_backend.calls().is_empty(),
        "engine untouched on a refused start"
    );
}

#[tokio::test(flavor = "multi_thread")]
async fn invalid_input_is_rejected_before_anything_is_saved() {
    let t = TestApp::new();
    for (d, key) in [
        (
            dest("fb", "facebook", "http://not-rtmp/x"),
            Some("abcd1234"),
        ),
        (dest("Bad Id", "custom", "rtmp://x/y"), Some("abcd1234")),
        (dest("ok", "custom", "rtmp://x/y"), Some("has space")),
    ] {
        let r: Result<Value, Value> = t.invoke(
            "stream_save_destination",
            json!({ "destination": d, "key": key }),
        );
        assert_eq!(r.expect_err("rejected")["type"], "InvalidInput");
    }
    let cfg: Value = t.invoke("stream_get_config", json!({})).expect("cfg");
    assert_eq!(cfg["destinations"], json!([]));
}

#[tokio::test(flavor = "multi_thread")]
async fn go_live_encodes_once_and_adds_every_destination_with_its_own_key() {
    let t = TestApp::new();
    add_both(&t);
    let mut events = t.collect_events();

    let s = start(&t);
    assert_eq!(s["state"], "Live");
    assert_eq!(s["encoder"], "x264enc");

    let calls = t.stream_backend.calls();
    assert_eq!(
        calls.iter().filter(|c| c.starts_with("start:")).count(),
        1,
        "one encode: {calls:?}"
    );
    assert!(calls.contains(&"add:fb".to_string()) && calls.contains(&"add:yt".to_string()));
    assert_eq!(
        t.stream_backend.url_for("fb"),
        Some(format!("{FB_URL}{FB_KEY}"))
    );
    assert_eq!(
        t.stream_backend.url_for("yt"),
        Some(format!("{YT_URL}/{YT_KEY}"))
    );

    t.stream.tick(Instant::now()).expect("tick");
    assert_eq!(dest_state(&t, "fb")["state"], "Live");
    assert_eq!(dest_state(&t, "yt")["state"], "Live");

    let seen = events.drain();
    let all = format!("{seen:?}");
    assert!(
        !all.contains(FB_KEY) && !all.contains(YT_KEY),
        "keys never ride the event bus"
    );
    assert!(seen.iter().any(
        |e| matches!(e, AppEvent::StreamStateChanged { state } if *state == StreamState::Live)
    ));
    assert!(seen.iter().any(
        |e| matches!(e, AppEvent::StreamDestinationChanged { id, state: DestinationState::Live } if id == "yt")
    ));
    assert!(!status(&t).to_string().contains(FB_KEY));
}

#[tokio::test(flavor = "multi_thread")]
async fn one_destination_dropping_does_not_touch_the_other_and_reconnects_with_backoff() {
    let t = TestApp::new();
    add_both(&t);
    start(&t);
    let t0 = Instant::now();
    t.stream.tick(t0).expect("tick");

    t.stream_backend.push_event(BackendEvent::DestinationDown {
        id: "fb".into(),
        reason: format!("rtmp://x/{FB_KEY} reset"),
    });
    t.stream.tick(t0).expect("tick");
    assert_eq!(dest_state(&t, "fb")["state"], "Reconnecting");
    assert_eq!(dest_state(&t, "fb")["detail"]["retry_in_secs"], 1);
    assert_eq!(
        dest_state(&t, "yt")["state"],
        "Live",
        "the other destination is untouched"
    );
    assert_eq!(calls_matching(&t, "remove:fb"), 1);
    assert_eq!(calls_matching(&t, "remove:yt"), 0);

    // Not due yet: no second add.
    t.stream_backend.auto_up(false);
    let adds = calls_matching(&t, "add:fb");
    t.stream
        .tick(t0 + Duration::from_millis(500))
        .expect("tick");
    assert_eq!(calls_matching(&t, "add:fb"), adds);

    // Due: re-added; it fails again, so the next delay doubles.
    t.stream.tick(t0 + Duration::from_secs(1)).expect("tick");
    assert_eq!(calls_matching(&t, "add:fb"), adds + 1);
    t.stream_backend.push_event(BackendEvent::DestinationDown {
        id: "fb".into(),
        reason: "again".into(),
    });
    t.stream.tick(t0 + Duration::from_secs(1)).expect("tick");
    assert_eq!(dest_state(&t, "fb")["detail"]["retry_in_secs"], 2);
    assert_eq!(dest_state(&t, "fb")["detail"]["attempt"], 2);

    // It comes back: Live, and the back-off starts over.
    t.stream.tick(t0 + Duration::from_secs(3)).expect("tick");
    t.stream_backend
        .push_event(BackendEvent::DestinationUp("fb".into()));
    t.stream.tick(t0 + Duration::from_secs(3)).expect("tick");
    assert_eq!(dest_state(&t, "fb")["state"], "Live");
    t.stream_backend.push_event(BackendEvent::DestinationDown {
        id: "fb".into(),
        reason: "x".into(),
    });
    t.stream.tick(t0 + Duration::from_secs(4)).expect("tick");
    assert_eq!(
        dest_state(&t, "fb")["detail"]["retry_in_secs"],
        1,
        "back-off reset after a good connection"
    );
}

#[tokio::test(flavor = "multi_thread")]
async fn a_destination_the_engine_refuses_fails_alone_and_its_key_is_redacted() {
    let t = TestApp::new();
    add_both(&t);
    t.stream_backend
        .fail_add("yt", Some(&format!("cannot open {YT_URL}/{YT_KEY}")));

    let s = start(&t);

    assert_eq!(
        s["state"], "Live",
        "the stream runs for the destinations that work"
    );
    let yt = dest_state(&t, "yt");
    assert_eq!(yt["state"], "Failed");
    assert!(!yt.to_string().contains(YT_KEY), "{yt}");
    t.stream.tick(Instant::now()).expect("tick");
    assert_eq!(dest_state(&t, "fb")["state"], "Live");
}

#[tokio::test(flavor = "multi_thread")]
async fn engine_start_failure_leaves_the_stream_stopped_with_a_redacted_error() {
    let t = TestApp::new();
    add_both(&t);
    t.stream_backend
        .fail_start(Some(&format!("encoder failed for {FB_KEY}")));

    let r: Result<Value, Value> = t.invoke("stream_start", json!({}));

    let err = r.expect_err("start fails");
    assert!(!err.to_string().contains(FB_KEY), "{err}");
    assert_eq!(status(&t)["state"], "Stopped");
    assert!(!t
        .stream_backend
        .calls()
        .iter()
        .any(|c| c.starts_with("add:")));
}

#[tokio::test(flavor = "multi_thread")]
async fn muting_and_unmuting_one_destination_while_live_leaves_the_others_running() {
    let t = TestApp::new();
    add_both(&t);
    start(&t);
    t.stream.tick(Instant::now()).expect("tick");

    let _: Value = t
        .invoke(
            "stream_set_destination_enabled",
            json!({ "id": "fb", "enabled": false }),
        )
        .expect("mute");
    assert_eq!(calls_matching(&t, "remove:fb"), 1);
    assert_eq!(dest_state(&t, "fb")["state"], "Idle");
    assert_eq!(dest_state(&t, "yt")["state"], "Live");

    let _: Value = t
        .invoke(
            "stream_set_destination_enabled",
            json!({ "id": "fb", "enabled": true }),
        )
        .expect("unmute");
    t.stream.tick(Instant::now()).expect("tick");
    assert_eq!(dest_state(&t, "fb")["state"], "Live");
}

#[tokio::test(flavor = "multi_thread")]
async fn destinations_can_be_added_and_removed_while_live() {
    let t = TestApp::new();
    save(&t, dest("fb", "facebook", FB_URL), Some(FB_KEY));
    start(&t);

    save(&t, dest("yt", "youtube", YT_URL), Some(YT_KEY));
    t.stream.tick(Instant::now()).expect("tick");
    assert_eq!(dest_state(&t, "yt")["state"], "Live");

    let view: Value = t
        .invoke("stream_remove_destination", json!({ "id": "fb" }))
        .expect("remove");
    assert_eq!(view["destinations"].as_array().expect("list").len(), 1);
    assert_eq!(
        t.stream_keys.get("fb").expect("keys"),
        None,
        "key deleted with the destination"
    );
    assert_eq!(dest_state(&t, "yt")["state"], "Live");
}

#[tokio::test(flavor = "multi_thread")]
async fn stop_is_graceful_idempotent_and_settings_lock_while_live() {
    let t = TestApp::new();
    add_both(&t);
    start(&t);

    let again: Result<Value, Value> = t.invoke("stream_start", json!({}));
    assert_eq!(again.expect_err("already running")["type"], "InvalidInput");
    let locked: Result<Value, Value> = t.invoke(
        "stream_set_settings",
        json!({ "settings": { "video_kbps": 3000 } }),
    );
    assert_eq!(
        locked.expect_err("locked while live")["type"],
        "InvalidInput"
    );

    let stopped: Value = t.invoke("stream_stop", json!({})).expect("stop");
    assert_eq!(stopped["state"], "Stopped");
    assert_eq!(dest_state(&t, "fb")["state"], "Idle");
    let twice: Value = t.invoke("stream_stop", json!({})).expect("stop again");
    assert_eq!(twice["state"], "Stopped");
    assert_eq!(calls_matching(&t, "stop"), 1);

    let cfg: Value = t
        .invoke(
            "stream_set_settings",
            json!({ "settings": { "video_kbps": 3000, "width": 1280, "height": 720 } }),
        )
        .expect("settings while stopped");
    assert_eq!(cfg["settings"]["video_kbps"], 3000);
    let bad: Result<Value, Value> =
        t.invoke("stream_set_settings", json!({ "settings": { "fps": 0 } }));
    assert_eq!(bad.expect_err("invalid")["type"], "InvalidInput");

    start(&t);
    assert_eq!(calls_matching(&t, "start:1280x720@30:3000kbps"), 1);
}

#[tokio::test(flavor = "multi_thread")]
async fn unreadable_keychain_refuses_to_start() {
    let t = TestApp::new();
    add_both(&t);
    t.stream_keys.failing(Some("locked"));
    let r: Result<Value, Value> = t.invoke("stream_start", json!({}));
    assert_eq!(r.expect_err("fails closed")["type"], "Keychain");
    t.stream_keys.failing(None);
    assert_eq!(status(&t)["state"], "Stopped");
}
