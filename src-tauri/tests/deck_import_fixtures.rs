// Integration tests against real .pptx/.pdf fixture files (src-tauri/tests/fixtures/).
// These are the same files handed to the user for manual app-launch validation —
// regenerate both from dev-docs/test-assets/make_test_assets.js if the parser's
// expected shape (ph types, rels layout) ever changes.

use std::path::Path;
use std::sync::Arc;
use streamdeck_lib::bus::EventBus;
use streamdeck_lib::deck_importer::import_deck;

fn fixture(name: &str) -> std::path::PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("tests/fixtures")
        .join(name)
}

#[tokio::test]
async fn pptx_fixture_extracts_titles_body_code_and_layout_flags() {
    let (bus, _rx) = EventBus::new();
    let bus = Arc::new(bus);
    let dest = tempfile::tempdir().unwrap();

    let deck = import_deck(&fixture("streamdeck-test-deck.pptx"), dest.path(), bus)
        .await
        .expect("pptx fixture should import cleanly");

    assert_eq!(deck.slides.len(), 5);

    assert_eq!(deck.slides[0].title, "StreamDeck Test Deck");
    assert!(deck.slides[0]
        .body
        .contains("A sample presentation for testing deck import."));

    assert_eq!(deck.slides[1].title, "Feature Overview");
    assert!(deck.slides[1].body.contains("Live presentation control"));

    assert_eq!(deck.slides[2].title, "Sample Code");
    assert!(
        deck.slides[2].is_code_slide,
        "slide 3 uses a Consolas run and should be flagged as code"
    );

    assert_eq!(deck.slides[3].title, "Visual Slide");
    assert!(
        deck.slides[3].has_background_image,
        "slide 4 embeds ppt/media/image1.png via slide rels"
    );
    assert!(deck.slides[3].background_path.is_some());

    assert_eq!(deck.slides[4].title, "Unsupported Layout Demo");
    assert!(
        deck.slides[4].layout_unsupported,
        "slide 5 uses a subTitle placeholder StreamDeck doesn't classify as body"
    );
    assert!(
        deck.slides[4].body.is_empty(),
        "unsupported placeholder text must not leak into body"
    );
}

// Ignored by default: pdfium-render's Pdfium::default() dynamically loads a native
// pdfium shared library that this repo does not bundle or download anywhere — on a
// machine without one already installed, this doesn't return an Err, it panics inside
// pdfium-render itself (LoadLibraryError). PDF import is effectively unverified/crash-prone
// until the native library is bundled (see CLAUDE.md). Run manually with
// `cargo test -- --ignored` once pdfium is available locally.
#[tokio::test]
#[ignore = "requires a native pdfium library on PATH/next to the binary — not bundled yet, see CLAUDE.md"]
async fn pdf_fixture_produces_one_slide_per_page_with_background() {
    let (bus, _rx) = EventBus::new();
    let bus = Arc::new(bus);
    let dest = tempfile::tempdir().unwrap();

    let deck = import_deck(&fixture("streamdeck-test-deck.pdf"), dest.path(), bus)
        .await
        .expect("pdf fixture should import cleanly");

    assert_eq!(deck.slides.len(), 3);
    for slide in &deck.slides {
        assert!(slide.has_background_image);
        assert!(slide.background_path.is_some());
    }
    assert!(deck.slides[0].title.contains("StreamDeck Test PDF"));
    assert!(deck.slides[1].title.contains("Second Page"));
}
