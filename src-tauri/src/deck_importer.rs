use crate::bus::EventBus;
use crate::error::AppError;
use crate::events::AppEvent;
pub use crate::pptx::is_code_slide;
use crate::pptx::{ImportLimits, SlideOutcome};
use crate::slide::{SlideDeck, SlideEntry};
use std::path::{Path, PathBuf};
use std::sync::Arc;

pub enum DeckFormat {
    Pptx,
    Pdf,
}

impl DeckFormat {
    pub fn detect(path: &Path) -> Result<Self, AppError> {
        match path.extension().and_then(|e| e.to_str()) {
            Some("pptx") => Ok(Self::Pptx),
            Some("pdf") => Ok(Self::Pdf),
            Some(ext) => Err(AppError::InvalidInput(format!(
                "unsupported format: .{ext} — only .pptx and .pdf are supported"
            ))),
            None => Err(AppError::InvalidInput("file has no extension".into())),
        }
    }
}

pub struct ImportReport {
    pub deck: SlideDeck,
    /// Slides/pages that could not be read and were left out.
    pub skipped: u32,
}

pub async fn import_deck(
    path: &Path,
    dest_dir: &Path,
    bus: Arc<EventBus>,
) -> Result<SlideDeck, AppError> {
    import_deck_reporting(path, dest_dir, bus)
        .await
        .map(|r| r.deck)
}

pub async fn import_deck_reporting(
    path: &Path,
    dest_dir: &Path,
    bus: Arc<EventBus>,
) -> Result<ImportReport, AppError> {
    let format = DeckFormat::detect(path)?;
    let limits = ImportLimits::default();
    let size = std::fs::metadata(path)
        .map_err(|e| AppError::Internal(e.to_string()))?
        .len();
    if size > limits.max_file_bytes {
        return Err(AppError::InvalidInput(format!(
            "this file is larger than {} MB",
            limits.max_file_bytes / (1024 * 1024)
        )));
    }
    let path = path.to_path_buf();
    let dest_dir = dest_dir.to_path_buf();

    tokio::task::spawn_blocking(move || match format {
        DeckFormat::Pptx => import_pptx_file(&path, &dest_dir, &bus, &limits),
        DeckFormat::Pdf => import_pdf(&path, &dest_dir, &bus),
    })
    .await
    .map_err(|e| AppError::Internal(e.to_string()))?
}

fn import_pptx_file(
    path: &Path,
    dest_dir: &Path,
    bus: &Arc<EventBus>,
    limits: &ImportLimits,
) -> Result<ImportReport, AppError> {
    let file = std::fs::File::open(path).map_err(|e| AppError::Internal(e.to_string()))?;
    let archive = zip::ZipArchive::new(file)
        .map_err(|e| AppError::InvalidInput(format!("not a valid .pptx file: {e}")))?;
    let mut skipped = 0;
    let deck =
        crate::pptx::import_pptx(archive, &dest_dir.join("media"), limits, &mut |o| match o {
            SlideOutcome::Done { n, total } => {
                bus.publish(AppEvent::DeckImportProgress { slide: n, total });
            }
            SlideOutcome::Failed { n, reason } => {
                skipped += 1;
                bus.publish(AppEvent::DeckImportFailed { slide: n, reason });
            }
        })?;
    Ok(ImportReport { deck, skipped })
}

const MAX_PDF_PAGES: u32 = 500;

const PDFIUM_MISSING: &str =
    "PDF import is unavailable: the pdfium library was not found next to the app or on the system";

const PRODUCT_NAME: &str = "StreamDeck";

// Ordered search dirs: Windows/dev next to the exe, Tauri resource layouts, Linux deb/AppImage `../lib/<product>`.
fn pdfium_candidate_dirs(exe_dir: &Path, dev_stage: Option<&Path>) -> Vec<PathBuf> {
    let mut dirs = vec![
        exe_dir.to_path_buf(),
        exe_dir.join("resources").join("pdfium"),
        exe_dir.join("pdfium"),
        exe_dir.join("..").join("lib").join(PRODUCT_NAME),
    ];
    // cargo test binaries live in target/<profile>/deps; tauri-build copies resources one level up.
    if exe_dir.file_name().is_some_and(|n| n == "deps") {
        dirs.push(exe_dir.join(".."));
    }
    dirs.extend(dev_stage.map(Path::to_path_buf));
    dirs
}

fn default_pdfium_dirs() -> Vec<PathBuf> {
    let exe_dir = std::env::current_exe()
        .ok()
        .and_then(|p| p.parent().map(Path::to_path_buf))
        .unwrap_or_else(|| PathBuf::from("."));
    // Debug builds also look at the fetch-deps staging dir so `cargo tauri dev` and tests find it.
    let dev_stage = cfg!(debug_assertions)
        .then(|| Path::new(env!("CARGO_MANIFEST_DIR")).join("resources/pdfium"));
    pdfium_candidate_dirs(&exe_dir, dev_stage.as_deref())
}

// Pdfium::default() panics when the library is missing, so bind explicitly and return an error.
fn load_pdfium(
    dirs: &[PathBuf],
    allow_system: bool,
) -> Result<pdfium_render::prelude::Pdfium, AppError> {
    use pdfium_render::prelude::*;

    let bound = dirs.iter().find_map(|dir| {
        Pdfium::bind_to_library(Pdfium::pdfium_platform_library_name_at_path(dir)).ok()
    });
    let bindings = match bound {
        Some(b) => b,
        None if allow_system => Pdfium::bind_to_system_library()
            .map_err(|_| AppError::Internal(PDFIUM_MISSING.into()))?,
        None => return Err(AppError::Internal(PDFIUM_MISSING.into())),
    };
    Ok(Pdfium::new(bindings))
}

/// True when the same loader used by PDF import can find a pdfium library.
pub fn pdfium_available() -> bool {
    load_pdfium(&default_pdfium_dirs(), true).is_ok()
}

fn import_pdf(path: &Path, dest_dir: &Path, bus: &Arc<EventBus>) -> Result<ImportReport, AppError> {
    use pdfium_render::prelude::*;

    let media_dir = dest_dir.join("media");
    std::fs::create_dir_all(&media_dir).map_err(|e| AppError::Internal(e.to_string()))?;

    let pdfium = load_pdfium(&default_pdfium_dirs(), true)?;
    let document = pdfium
        .load_pdf_from_file(path, None)
        .map_err(|e| AppError::Internal(e.to_string()))?;

    let page_count = document.pages().len() as u32;
    if page_count == 0 {
        return Err(AppError::InvalidInput("this PDF has no pages".into()));
    }
    if page_count > MAX_PDF_PAGES {
        return Err(AppError::InvalidInput(format!(
            "this PDF has {page_count} pages; the limit is {MAX_PDF_PAGES}"
        )));
    }
    let mut slides = Vec::new();
    let mut skipped = 0;

    for (i, page) in document.pages().iter().enumerate() {
        let n = i as u32 + 1;
        let result = (|| -> Result<SlideEntry, AppError> {
            let render_config = PdfRenderConfig::new()
                .set_target_width(1920)
                .set_target_height(1080);

            let image = page
                .render_with_config(&render_config)
                .map_err(|e| AppError::Internal(e.to_string()))?
                .as_image();

            let bg_path = media_dir.join(format!("slide_{n}_bg.png"));
            image
                .save(&bg_path)
                .map_err(|e| AppError::Internal(e.to_string()))?;

            let raw_text = page
                .text()
                .map_err(|e| AppError::Internal(e.to_string()))?
                .all();

            let title = raw_text
                .lines()
                .next()
                .unwrap_or("")
                .chars()
                .take(60)
                .collect::<String>();

            let bg_str = bg_path.to_str().map(|s| s.to_string());

            Ok(SlideEntry {
                index: i as u32,
                title,
                body: String::new(),
                notes: String::new(),
                background_path: bg_str,
                rich_body: Vec::new(),
                word_count: 0,
                has_background_image: true,
                is_code_slide: false,
                layout_unsupported: false,
            })
        })();

        match result {
            Ok(entry) => {
                bus.publish(AppEvent::DeckImportProgress {
                    slide: n,
                    total: page_count,
                });
                slides.push(entry);
            }
            Err(reason) => {
                skipped += 1;
                bus.publish(AppEvent::DeckImportFailed {
                    slide: n,
                    reason: reason.to_string(),
                });
            }
        }
    }

    if slides.is_empty() {
        return Err(AppError::InvalidInput(
            "none of the pages in this PDF could be rendered".into(),
        ));
    }
    Ok(ImportReport {
        deck: SlideDeck { slides },
        skipped,
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn load_pdfium_without_the_library_is_an_error_not_a_panic() {
        let empty = tempfile::tempdir().expect("temp dir");

        let res = load_pdfium(&[empty.path().to_path_buf()], false);

        assert!(matches!(res, Err(AppError::Internal(m)) if m == PDFIUM_MISSING));
    }

    #[test]
    fn pdfium_candidates_cover_bundled_layouts_in_order() {
        let exe = Path::new("/opt/app/usr/bin");
        let stage = Path::new("/repo/resources/pdfium");

        let dirs = pdfium_candidate_dirs(exe, Some(stage));

        assert_eq!(dirs[0], exe);
        assert_eq!(dirs[1], exe.join("resources/pdfium"));
        assert!(dirs.contains(&exe.join("../lib/StreamDeck")));
        assert_eq!(dirs.last().map(PathBuf::as_path), Some(stage));
        assert!(
            !dirs.contains(&exe.join("..")),
            "parent only for cargo deps dir"
        );
    }

    #[test]
    fn pdfium_candidates_add_parent_for_cargo_test_binaries() {
        let exe = Path::new("/repo/target/debug/deps");

        let dirs = pdfium_candidate_dirs(exe, None);

        assert!(dirs.contains(&exe.join("..")));
        assert_eq!(dirs.len(), 5);
    }

    #[test]
    fn unsupported_extension_returns_invalid_input_error() {
        let path = std::path::Path::new("file.key");
        let result = DeckFormat::detect(path);
        assert!(matches!(result, Err(AppError::InvalidInput(_))));
    }

    // TODO: fixture-based tests (pptx_extracts_title_body_notes, pptx_background_image_extracted,
    // pptx_solid_fill, pdf_every_page_has_background_path) require fixture files at
    // src-tauri/tests/fixtures/ — add those files and tests in a follow-up once fixtures exist.
}
