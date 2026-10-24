// Integration tests against real .pptx/.pdf fixture files (src-tauri/tests/fixtures/).
// These are the same files handed to the user for manual app-launch validation —
// regenerate both from dev-docs/test-assets/make_test_assets.js if the parser's
// expected shape (ph types, rels layout) ever changes.

use std::path::Path;
use std::sync::Arc;
use waylamp_present_lib::bus::EventBus;
use waylamp_present_lib::deck_importer::import_deck;

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

    let deck = import_deck(&fixture("waylamp-present-test-deck.pptx"), dest.path(), bus)
        .await
        .expect("pptx fixture should import cleanly");

    assert_eq!(deck.slides.len(), 5);

    assert_eq!(deck.slides[0].title, "Waylamp Present Test Deck");
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
        "slide 5 uses a subTitle placeholder Waylamp Present doesn't classify as body"
    );
    assert!(
        deck.slides[4].body.is_empty(),
        "unsupported placeholder text must not leak into body"
    );
}

// Passes with or without a native pdfium library: a missing one must be an error, never a panic.
#[tokio::test]
async fn pdf_import_never_panics_when_pdfium_is_missing() {
    let (bus, _rx) = EventBus::new();
    let bus = Arc::new(bus);
    let dest = tempfile::tempdir().unwrap();

    let res = import_deck(&fixture("waylamp-present-test-deck.pdf"), dest.path(), bus).await;

    if let Err(e) = res {
        assert!(
            format!("{e:?}").contains("pdfium"),
            "unexpected error: {e:?}"
        );
    }
}

// Runs when the real loader finds pdfium (`bash scripts/fetch-deps.sh`); skips with a message otherwise.
#[tokio::test]
async fn pdf_fixture_produces_one_slide_per_page_with_background() {
    if !waylamp_present_lib::deck_importer::pdfium_available() {
        eprintln!(
            "SKIPPED: pdfium not found; run `bash scripts/fetch-deps.sh` to enable this test"
        );
        return;
    }
    let (bus, _rx) = EventBus::new();
    let bus = Arc::new(bus);
    let dest = tempfile::tempdir().unwrap();

    let deck = import_deck(&fixture("waylamp-present-test-deck.pdf"), dest.path(), bus)
        .await
        .expect("pdf fixture should import cleanly");

    assert_eq!(deck.slides.len(), 3);
    for slide in &deck.slides {
        assert!(slide.has_background_image);
        assert!(slide.background_path.is_some());
    }
    assert!(deck.slides[0].title.contains("Waylamp Present Test PDF"));
    assert!(deck.slides[1].title.contains("Second Page"));
}
