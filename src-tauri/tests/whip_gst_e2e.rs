// Real WebRTC loopback: whipclientsink -> axum endpoint -> whipserversrc -> preview hub.
// Needs GStreamer with gst-plugin-webrtc and openh264; run with:
//   cargo test --features whip --test whip_gst_e2e -- --ignored
#![cfg(feature = "whip")]

use std::sync::Arc;
use std::time::Duration;

use gstreamer as gst;
use gstreamer::prelude::*;
use streamdeck_lib::bus::EventBus;
use streamdeck_lib::config_loader::ConfigLoader;
use streamdeck_lib::preview::PreviewHub;
use streamdeck_lib::whip::gst_whip::GstWhipBackend;
use streamdeck_lib::whip::{
    FixedLocalIp, MemoryTokenStore, TokenStore, WhipController, WhipLinkState, WhipService,
};
use streamdeck_lib::whip_server::http_request;

#[tokio::test(flavor = "multi_thread")]
#[ignore = "needs GStreamer webrtc + openh264 plugins and loopback ICE"]
async fn gstreamer_whip_client_reaches_the_preview_hub_through_the_endpoint() {
    let hub = PreviewHub::new();
    let backend = GstWhipBackend::new(hub.clone()).expect("gst-plugin-webrtc is installed");
    let (bus, _keep) = EventBus::new();
    let bus = Arc::new(bus);
    let service = WhipService::new(Arc::new(backend), Arc::clone(&bus));

    let dir = tempfile::tempdir().expect("tmp");
    let config_path = dir.path().join("streamdeck.toml");
    let mut config = ConfigLoader::load_or_default(&config_path).expect("config");
    config.whip.bind_address = "127.0.0.1".into();
    config.whip.port = 0;
    ConfigLoader::save(&config_path, &config).expect("save");
    let tokens = Arc::new(MemoryTokenStore::default());
    let controller = WhipController::new(
        Arc::clone(&service),
        bus,
        config_path,
        Arc::new(FixedLocalIp(vec!["192.168.1.20".parse().expect("ip")])),
        tokens.clone(),
    );
    let addr = controller.start().await.expect("start");
    let token = tokens
        .get()
        .expect("store")
        .expect("first start generates a token");

    let mut frames = hub.subscribe();
    let started = std::time::Instant::now();
    let client = gst::parse::launch(&format!(
        "videotestsrc is-live=true ! videoconvert ! openh264enc ! h264parse ! whipclientsink signaller::whip-endpoint=http://127.0.0.1:{}/whip/camera1 signaller::auth-token={token}",
        addr.port()
    ))
    .expect("client pipeline");
    client.set_state(gst::State::Playing).expect("client plays");

    let negotiated = tokio::time::timeout(Duration::from_secs(20), async {
        while service.sessions().is_empty() {
            tokio::time::sleep(Duration::from_millis(20)).await;
        }
    })
    .await;
    eprintln!(
        "session registered after {:?} ({negotiated:?})",
        started.elapsed()
    );

    let frame = tokio::time::timeout(Duration::from_secs(20), async {
        loop {
            if let Ok(f) = frames.recv().await {
                if f.source == "camera1" {
                    return f;
                }
            }
        }
    })
    .await
    .expect("a camera1 frame arrives within 20s");
    eprintln!("first camera1 frame after {:?}", started.elapsed());
    assert_eq!((frame.width, frame.height), (320, 180));

    let sessions = service.sessions();
    assert_eq!(sessions.len(), 1);
    assert_eq!(sessions[0].state, WhipLinkState::Connected);

    let _ = client.set_state(gst::State::Null);
    let loc = format!("/whip/camera1/{}", "x");
    let auth = format!("Bearer {token}");
    let r = http_request(
        addr,
        "DELETE",
        &loc,
        &[("Authorization", auth.as_str())],
        "",
    )
    .await
    .expect("http");
    assert_eq!(r.status, 404, "a wrong session id never deletes");
    let stopped = std::time::Instant::now();
    controller.stop().await;
    eprintln!("stop took {:?}", stopped.elapsed());
    assert!(service.sessions().is_empty());
}
