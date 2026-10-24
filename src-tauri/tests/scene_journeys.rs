// T1.3: scene media / source commands through real IPC, real bus, real scene state, on a temp
// app-data dir. Size limits are asserted at the exact boundary using sparse set_len files.

mod common;

use std::path::{Path, PathBuf};

use common::TestApp;
use serde_json::{json, Value};
use tauri::Manager;
use waylamp_present_lib::events::AppEvent;
use waylamp_present_lib::scene::{SceneMediaType, SceneSource, SceneStateHandle};

const MB: u64 = 1024 * 1024;

fn sized_file(dir: &Path, name: &str, bytes: u64) -> PathBuf {
    let path = dir.join(name);
    let f = std::fs::File::create(&path).expect("create media file");
    f.set_len(bytes).expect("size media file");
    path
}

fn s(p: &Path) -> String {
    p.to_str().expect("utf-8 path").to_string()
}

fn scene_media_dir(t: &TestApp) -> PathBuf {
    t.data_dir.path().join("scene_media")
}

fn invoke_unit(t: &TestApp, cmd: &str, args: Value) {
    let _: () = t
        .invoke(cmd, args)
        .unwrap_or_else(|e| panic!("{cmd} failed: {e}"));
}

fn active_source(t: &TestApp) -> SceneSource {
    t.invoke("scene_get_active_source", json!({}))
        .expect("scene_get_active_source")
}

#[tokio::test(flavor = "multi_thread")]
async fn push_image_copies_into_app_data_and_publishes_media_without_switching_source() {
    let t = TestApp::new();
    let src_dir = tempfile::tempdir().expect("source dir");
    let src = src_dir.path().join("logo.png");
    std::fs::write(&src, b"png-bytes").expect("write image");
    let mut events = t.collect_events();

    invoke_unit(&t, "scene_push_image", json!({ "path": s(&src) }));

    let dest = scene_media_dir(&t).join("logo.png");
    assert_eq!(std::fs::read(&dest).expect("copied"), b"png-bytes");
    let seen = events.drain();
    assert_eq!(seen.len(), 1, "only SceneMediaUpdate, got {seen:?}");
    let AppEvent::SceneMediaUpdate(p) = &seen[0] else {
        panic!("expected SceneMediaUpdate, got {:?}", seen[0]);
    };
    assert!(matches!(p.media_type, SceneMediaType::Image));
    assert_eq!(p.display_name, "logo.png");
    assert_eq!(p.path, s(&dest));

    // Pushing only stages the media; the operator's toggle is what puts it on the projector.
    assert_eq!(active_source(&t), SceneSource::SlideDeck);
    let scene = t.app.state::<SceneStateHandle>();
    let staged = scene.read().await.current_media.clone().expect("staged");
    assert_eq!(staged.path, s(&dest));
}

#[tokio::test(flavor = "multi_thread")]
async fn push_image_accepts_uppercase_extensions_and_replaces_the_staged_media() {
    let t = TestApp::new();
    let src_dir = tempfile::tempdir().expect("source dir");
    let first = src_dir.path().join("a.PNG");
    let second = src_dir.path().join("b.jpeg");
    std::fs::write(&first, b"a").expect("write");
    std::fs::write(&second, b"b").expect("write");

    invoke_unit(&t, "scene_push_image", json!({ "path": s(&first) }));
    invoke_unit(&t, "scene_push_image", json!({ "path": s(&second) }));

    let scene = t.app.state::<SceneStateHandle>();
    let staged = scene.read().await.current_media.clone().expect("staged");
    assert_eq!(staged.display_name, "b.jpeg");
    assert!(scene_media_dir(&t).join("a.PNG").is_file());
}

#[tokio::test(flavor = "multi_thread")]
async fn push_image_at_exactly_the_limit_is_accepted() {
    let t = TestApp::new();
    let src_dir = tempfile::tempdir().expect("source dir");
    let src = sized_file(src_dir.path(), "edge.png", 10 * MB);

    invoke_unit(&t, "scene_push_image", json!({ "path": s(&src) }));

    assert!(scene_media_dir(&t).join("edge.png").is_file());
}

#[tokio::test(flavor = "multi_thread")]
async fn push_image_over_the_limit_is_rejected_before_any_copy() {
    let t = TestApp::new();
    let src_dir = tempfile::tempdir().expect("source dir");
    let src = sized_file(src_dir.path(), "big.png", 10 * MB + 1);
    let mut events = t.collect_events();

    let res: Result<(), Value> = t.invoke("scene_push_image", json!({ "path": s(&src) }));

    let err = res.expect_err("oversize image is rejected");
    assert_eq!(err["type"], "InvalidInput");
    assert!(err["message"].as_str().unwrap_or("").contains("10 MB"));
    assert!(
        !scene_media_dir(&t).exists(),
        "rejection must happen before the scene_media dir or any copy exists"
    );
    assert!(events.drain().is_empty());
    let scene = t.app.state::<SceneStateHandle>();
    assert!(scene.read().await.current_media.is_none());
}

#[tokio::test(flavor = "multi_thread")]
async fn push_image_rejects_bad_extension_and_missing_file_without_side_effects() {
    let t = TestApp::new();
    let src_dir = tempfile::tempdir().expect("source dir");
    let gif = src_dir.path().join("anim.gif");
    std::fs::write(&gif, b"gif").expect("write");
    let mut events = t.collect_events();

    let bad_ext: Result<(), Value> = t.invoke("scene_push_image", json!({ "path": s(&gif) }));
    assert_eq!(bad_ext.expect_err("gif rejected")["type"], "InvalidInput");

    let missing: Result<(), Value> = t.invoke(
        "scene_push_image",
        json!({ "path": s(&src_dir.path().join("nope.png")) }),
    );
    assert_eq!(missing.expect_err("missing rejected")["type"], "Internal");

    assert!(!scene_media_dir(&t).exists());
    assert!(events.drain().is_empty());
    let scene = t.app.state::<SceneStateHandle>();
    assert!(scene.read().await.current_media.is_none());
}

#[tokio::test(flavor = "multi_thread")]
async fn push_video_references_the_original_path_and_publishes_media() {
    let t = TestApp::new();
    let src_dir = tempfile::tempdir().expect("source dir");
    let src = src_dir.path().join("clip.WEBM");
    std::fs::write(&src, b"video").expect("write");
    let mut events = t.collect_events();

    invoke_unit(&t, "scene_push_video", json!({ "path": s(&src) }));

    let seen = events.drain();
    assert_eq!(seen.len(), 1, "only SceneMediaUpdate, got {seen:?}");
    let AppEvent::SceneMediaUpdate(p) = &seen[0] else {
        panic!("expected SceneMediaUpdate, got {:?}", seen[0]);
    };
    assert!(matches!(p.media_type, SceneMediaType::Video));
    assert_eq!(p.display_name, "clip.WEBM");
    assert_eq!(
        p.path,
        s(&src),
        "videos are referenced in place, not copied"
    );
    assert!(!scene_media_dir(&t).exists());
    assert_eq!(active_source(&t), SceneSource::SlideDeck);
}

#[tokio::test(flavor = "multi_thread")]
async fn push_video_enforces_the_500_mb_limit_at_the_boundary() {
    let t = TestApp::new();
    let src_dir = tempfile::tempdir().expect("source dir");
    let ok = sized_file(src_dir.path(), "edge.mp4", 500 * MB);
    let big = sized_file(src_dir.path(), "big.mp4", 500 * MB + 1);
    let mut events = t.collect_events();

    invoke_unit(&t, "scene_push_video", json!({ "path": s(&ok) }));
    assert_eq!(events.drain().len(), 1);

    let res: Result<(), Value> = t.invoke("scene_push_video", json!({ "path": s(&big) }));
    let err = res.expect_err("oversize video is rejected");
    assert_eq!(err["type"], "InvalidInput");
    assert!(err["message"].as_str().unwrap_or("").contains("500 MB"));
    assert!(events.drain().is_empty());
    let scene = t.app.state::<SceneStateHandle>();
    let staged = scene.read().await.current_media.clone().expect("edge kept");
    assert_eq!(staged.display_name, "edge.mp4");
}

#[tokio::test(flavor = "multi_thread")]
async fn push_video_rejects_bad_extension_and_missing_file() {
    let t = TestApp::new();
    let src_dir = tempfile::tempdir().expect("source dir");
    let mov = src_dir.path().join("clip.mov");
    std::fs::write(&mov, b"v").expect("write");
    let mut events = t.collect_events();

    let bad_ext: Result<(), Value> = t.invoke("scene_push_video", json!({ "path": s(&mov) }));
    assert_eq!(bad_ext.expect_err("mov rejected")["type"], "InvalidInput");

    let missing: Result<(), Value> = t.invoke(
        "scene_push_video",
        json!({ "path": s(&src_dir.path().join("nope.mp4")) }),
    );
    assert_eq!(missing.expect_err("missing rejected")["type"], "Internal");

    assert!(events.drain().is_empty());
    let scene = t.app.state::<SceneStateHandle>();
    assert!(scene.read().await.current_media.is_none());
}

#[tokio::test(flavor = "multi_thread")]
async fn set_source_switches_publishes_and_is_readable_back() {
    let t = TestApp::new();
    assert_eq!(active_source(&t), SceneSource::SlideDeck);
    let mut events = t.collect_events();

    invoke_unit(&t, "scene_set_source", json!({ "source": "SceneMedia" }));
    assert_eq!(active_source(&t), SceneSource::SceneMedia);
    invoke_unit(&t, "scene_set_source", json!({ "source": "SlideDeck" }));
    assert_eq!(active_source(&t), SceneSource::SlideDeck);

    let seen = events.drain();
    assert!(matches!(
        seen.as_slice(),
        [
            AppEvent::SceneSourceChanged {
                source: SceneSource::SceneMedia
            },
            AppEvent::SceneSourceChanged {
                source: SceneSource::SlideDeck
            },
        ]
    ));
}

#[tokio::test(flavor = "multi_thread")]
async fn set_source_with_unknown_source_is_rejected_and_keeps_state() {
    let t = TestApp::new();
    invoke_unit(&t, "scene_set_source", json!({ "source": "SceneMedia" }));
    let mut events = t.collect_events();

    let res: Result<(), Value> = t.invoke("scene_set_source", json!({ "source": "Camera" }));

    assert!(res.is_err(), "unknown source variant must not deserialize");
    assert_eq!(active_source(&t), SceneSource::SceneMedia);
    assert!(events.drain().is_empty());
}
