// F2.4: phone WHIP ingest — enable/disable via real IPC, real HTTP against the embedded server, token, QR, preview.

mod common;

use std::net::SocketAddr;
use std::time::{Duration, Instant};

use common::TestApp;
use serde_json::{json, Value};
use streamdeck_lib::events::AppEvent;
use streamdeck_lib::whip::WhipLinkState;
use streamdeck_lib::whip_server::{http_request, HttpResponse};

const OFFER: &str = "v=0\r\ns=-\r\nt=0 0\r\nm=video 9 UDP/TLS/RTP/SAVPF 96 97\r\na=mid:0\r\na=rtpmap:96 VP8/90000\r\na=rtpmap:97 H264/90000\r\na=fmtp:97 packetization-mode=1;profile-level-id=42e01f\r\n";
const SDP: (&str, &str) = ("Content-Type", "application/sdp");

fn info(t: &TestApp) -> Value {
    t.invoke::<Value>("whip_get_info", json!({})).expect("info")
}

fn enable(t: &TestApp) -> SocketAddr {
    t.use_loopback_whip();
    t.invoke::<()>("whip_set_enabled", json!({ "enabled": true }))
        .expect("enable");
    let i = info(t);
    assert_eq!(i["running"], true);
    SocketAddr::from(([127, 0, 0, 1], i["port"].as_u64().expect("port") as u16))
}

async fn post(addr: SocketAddr, slot: &str, headers: &[(&str, &str)]) -> HttpResponse {
    http_request(addr, "POST", &format!("/whip/{slot}"), headers, OFFER)
        .await
        .expect("http")
}

#[tokio::test(flavor = "multi_thread")]
async fn endpoint_is_off_by_default_and_info_lists_four_phone_urls() {
    let t = TestApp::new();
    let i = info(&t);
    assert_eq!(i["enabled"], false);
    assert_eq!(i["running"], false);
    assert_eq!(i["host"], "192.168.1.20");
    assert_eq!(i["token_set"], false);
    let urls: Vec<&str> = i["slots"]
        .as_array()
        .expect("slots")
        .iter()
        .map(|s| s["url"].as_str().expect("url"))
        .collect();
    assert_eq!(
        urls,
        [
            "http://192.168.1.20:8088/whip/camera1",
            "http://192.168.1.20:8088/whip/camera2",
            "http://192.168.1.20:8088/whip/camera3",
            "http://192.168.1.20:8088/whip/camera4",
        ]
    );
}

#[tokio::test(flavor = "multi_thread")]
async fn enable_serves_http_publishes_source_events_and_disable_stops_it() {
    let t = TestApp::new();
    let mut events = t.collect_events();
    let addr = enable(&t);

    let r = post(addr, "camera1", &[SDP]).await;
    assert_eq!(r.status, 201);
    assert!(r.body.contains("a=recvonly"));
    let i = info(&t);
    assert_eq!(i["slots"][0]["session"]["state"], "Connected");
    assert_eq!(i["slots"][0]["session"]["codec"], "H264Baseline");
    assert_eq!(
        i["slots"][0]["url"],
        format!("http://192.168.1.20:{}/whip/camera1", addr.port())
    );

    let seen = events.drain();
    assert!(seen
        .iter()
        .any(|e| matches!(e, AppEvent::WhipServerChanged { running: true, .. })));
    assert!(seen
        .iter()
        .any(|e| matches!(e, AppEvent::SourceAdded { name, .. } if name == "camera1")));
    assert!(seen.iter().any(|e| matches!(e, AppEvent::WhipSessionChanged { slot, state: WhipLinkState::Connected } if slot == "camera1")));
    let toml = std::fs::read_to_string(t.config_path()).expect("config");
    assert!(toml.contains("[whip]") && toml.contains("enabled = true"));

    t.invoke::<()>("whip_set_enabled", json!({ "enabled": false }))
        .expect("disable");
    let seen = events.drain();
    assert!(seen
        .iter()
        .any(|e| matches!(e, AppEvent::SourceRemoved { name } if name == "camera1")));
    assert!(seen
        .iter()
        .any(|e| matches!(e, AppEvent::WhipServerChanged { running: false, .. })));
    assert!(http_request(addr, "OPTIONS", "/whip/camera1", &[], "")
        .await
        .is_err());
    assert_eq!(info(&t)["running"], false);
    assert!(std::fs::read_to_string(t.config_path())
        .expect("config")
        .contains("enabled = false"));
}

#[tokio::test(flavor = "multi_thread")]
async fn bearer_token_is_enforced_and_never_written_to_toml() {
    let t = TestApp::new();
    let addr = enable(&t);
    t.invoke::<()>("whip_set_token", json!({ "token": "tok-12345" }))
        .expect("set token");
    assert_eq!(info(&t)["token_set"], true);
    assert_eq!(post(addr, "camera1", &[SDP]).await.status, 401);
    let ok = [SDP, ("Authorization", "Bearer tok-12345")];
    assert_eq!(post(addr, "camera1", &ok).await.status, 201);

    let toml = std::fs::read_to_string(t.config_path()).expect("config");
    assert!(!toml.contains("tok-12345"));
    assert_eq!(t.whip_token().as_deref(), Some("tok-12345"));

    let bad: Result<(), Value> = t.invoke("whip_set_token", json!({ "token": "has space" }));
    assert_eq!(bad.expect_err("rejected")["type"], "InvalidInput");
    t.invoke::<()>("whip_set_token", json!({ "token": null }))
        .expect("clear");
    assert_eq!(info(&t)["token_set"], false);
    assert_eq!(post(addr, "camera2", &[SDP]).await.status, 201);
}

#[tokio::test(flavor = "multi_thread")]
async fn unreadable_keychain_keeps_the_endpoint_closed() {
    let t = TestApp::new();
    t.use_loopback_whip();
    t.whip_tokens.failing(Some("locked"));
    let e: Result<(), Value> = t.invoke("whip_set_enabled", json!({ "enabled": true }));
    assert_eq!(e.expect_err("fails closed")["type"], "Keychain");
    t.whip_tokens.failing(None);
    let i = info(&t);
    assert_eq!(i["running"], false);
    assert_eq!(i["enabled"], false);
}

#[tokio::test(flavor = "multi_thread")]
async fn occupied_port_is_reported_not_swallowed() {
    let blocker = std::net::TcpListener::bind("127.0.0.1:0").expect("bind");
    let port = blocker.local_addr().expect("addr").port();
    let t = TestApp::new();
    let path = t.config_path();
    let mut config =
        streamdeck_lib::config_loader::ConfigLoader::load_or_default(&path).expect("config");
    config.whip.bind_address = "127.0.0.1".into();
    config.whip.port = port;
    streamdeck_lib::config_loader::ConfigLoader::save(&path, &config).expect("save");
    let e: Result<(), Value> = t.invoke("whip_set_enabled", json!({ "enabled": true }));
    assert_eq!(e.expect_err("bind fails")["type"], "Internal");
    let i = info(&t);
    assert_eq!(i["running"], false);
    assert!(i["last_error"]
        .as_str()
        .is_some_and(|m| m.contains("cannot listen")));
}

#[tokio::test(flavor = "multi_thread")]
async fn qr_matrix_encodes_the_slot_url_and_rejects_bad_slots() {
    let t = TestApp::new();
    let q = t
        .invoke::<Value>("whip_get_qr", json!({ "slot": "camera1" }))
        .expect("qr");
    let size = q["size"].as_u64().expect("size");
    assert!(size >= 21);
    assert_eq!(
        q["modules"].as_array().expect("modules").len() as u64,
        size * size
    );
    let e: Result<Value, Value> = t.invoke("whip_get_qr", json!({ "slot": "../etc" }));
    assert_eq!(e.expect_err("bad slot")["type"], "InvalidInput");
}

#[tokio::test(flavor = "multi_thread")]
async fn phone_source_feeds_the_same_preview_path_as_ndi() {
    let t = TestApp::with_preview_feeds(30);
    let mut frames = t.preview.subscribe();
    let addr = enable(&t);
    assert_eq!(post(addr, "camera1", &[SDP]).await.status, 201);
    let frame = tokio::time::timeout(Duration::from_secs(3), frames.recv())
        .await
        .expect("a frame arrives")
        .expect("open");
    assert_eq!(
        (frame.source.as_str(), frame.width, frame.height),
        ("camera1", 320, 180)
    );
    let snap = t
        .invoke::<Value>("preview_get_snapshot", json!({}))
        .expect("snap");
    assert!(snap["frames"]
        .as_array()
        .expect("frames")
        .iter()
        .any(|f| f["source"] == "camera1"));

    t.invoke::<()>("whip_set_enabled", json!({ "enabled": false }))
        .expect("disable");
    let snap = t
        .invoke::<Value>("preview_get_snapshot", json!({}))
        .expect("snap");
    assert!(snap["frames"].as_array().expect("frames").is_empty());
}

#[tokio::test(flavor = "multi_thread")]
async fn session_survives_a_short_interruption_and_is_lost_after_the_grace_period() {
    let t = TestApp::new();
    let addr = enable(&t);
    let mut events = t.collect_events();
    let service = t.whip.service();
    let t0 = Instant::now();
    for slot in ["camera1", "camera2"] {
        let r = post(addr, slot, &[SDP]).await;
        let loc = r.header("location").expect("location").to_string();
        let id = loc.rsplit('/').next().expect("id").to_string();
        service.ice_signal(slot, &id, false, t0).expect("down");
        if slot == "camera1" {
            let frag = ("Content-Type", "application/trickle-ice-sdpfrag");
            let p = http_request(addr, "PATCH", &loc, &[frag], "a=ice-ufrag:x")
                .await
                .expect("patch");
            assert_eq!(p.status, 204);
        }
    }
    service.tick(t0 + Duration::from_secs(11));
    let i = info(&t);
    assert_eq!(i["slots"][0]["session"]["state"], "Connected");
    assert_eq!(i["slots"][1]["session"]["state"], "Lost");
    let seen = events.drain();
    assert!(seen
        .iter()
        .any(|e| matches!(e, AppEvent::SourceLost { name } if name == "camera2")));
    assert!(!seen
        .iter()
        .any(|e| matches!(e, AppEvent::SourceLost { name } if name == "camera1")));
    // The phone can take its slot back with a fresh offer.
    assert_eq!(post(addr, "camera2", &[SDP]).await.status, 201);
}
