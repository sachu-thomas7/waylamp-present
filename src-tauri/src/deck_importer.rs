use std::path::Path;
use std::sync::Arc;
use crate::bus::EventBus;
use crate::error::AppError;
use crate::slide::{SlideDeck, SlideEntry, SlideTextBlock, SlideTextRun, TextAlignment};
use crate::events::AppEvent;

enum PhKind {
    Title,
    Body,
    Other,
}

// PowerPoint always writes <p:ph .../> as a self-closing (Empty) element, never Start/End —
// this must be called from both Event::Start and Event::Empty arms, not just Start.
fn classify_ph(e: &quick_xml::events::BytesStart) -> PhKind {
    let type_attr = e
        .attributes()
        .filter_map(|a| a.ok())
        .find(|a| a.key.local_name().as_ref() == b"type");
    match type_attr.as_ref().and_then(|a| std::str::from_utf8(&a.value).ok()) {
        Some("title") | Some("ctrTitle") => PhKind::Title,
        Some("body") | None => PhKind::Body,
        Some(_) => PhKind::Other,
    }
}

pub enum DeckFormat {
    Pptx,
    Pdf,
}

impl DeckFormat {
    pub fn detect(path: &Path) -> Result<Self, AppError> {
        match path.extension().and_then(|e| e.to_str()) {
            Some("pptx") => Ok(Self::Pptx),
            Some("pdf")  => Ok(Self::Pdf),
            Some(ext)    => Err(AppError::InvalidInput(
                format!("unsupported format: .{ext} — only .pptx and .pdf are supported")
            )),
            None => Err(AppError::InvalidInput("file has no extension".into())),
        }
    }
}

pub async fn import_deck(
    path: &Path,
    dest_dir: &Path,
    bus: Arc<EventBus>,
) -> Result<SlideDeck, AppError> {
    let format = DeckFormat::detect(path)?;
    let path = path.to_path_buf();
    let dest_dir = dest_dir.to_path_buf();

    tokio::task::spawn_blocking(move || {
        match format {
            DeckFormat::Pptx => import_pptx(&path, &dest_dir, &bus),
            DeckFormat::Pdf  => import_pdf(&path, &dest_dir, &bus),
        }
    })
    .await
    .map_err(|e| AppError::Internal(e.to_string()))?
}

fn import_pptx(path: &Path, dest_dir: &Path, bus: &Arc<EventBus>) -> Result<SlideDeck, AppError> {
    let file = std::fs::File::open(path)
        .map_err(|e| AppError::Internal(e.to_string()))?;
    let mut archive = zip::ZipArchive::new(file)
        .map_err(|e| AppError::Internal(e.to_string()))?;

    // Count slides by enumerating ppt/slides/slide*.xml entries
    let slide_count = (1..)
        .take_while(|n| archive.by_name(&format!("ppt/slides/slide{n}.xml")).is_ok())
        .count() as u32;

    let media_dir = dest_dir.join("media");
    std::fs::create_dir_all(&media_dir)
        .map_err(|e| AppError::Internal(e.to_string()))?;

    let mut slides = Vec::new();

    for n in 1..=slide_count {
        let result = parse_pptx_slide(&mut archive, n, &media_dir);
        match result {
            Ok(entry) => {
                bus.publish(AppEvent::DeckImportProgress { slide: n, total: slide_count });
                slides.push(entry);
            }
            Err(reason) => {
                bus.publish(AppEvent::DeckImportFailed { slide: n, reason: reason.to_string() });
            }
        }
    }

    Ok(SlideDeck { slides })
}

fn parse_pptx_slide(
    archive: &mut zip::ZipArchive<std::fs::File>,
    n: u32,
    media_dir: &Path,
) -> Result<SlideEntry, AppError> {
    use quick_xml::events::Event;
    use quick_xml::Reader;
    use std::io::Read;

    let mut xml_content = String::new();
    {
        let mut slide_file = archive.by_name(&format!("ppt/slides/slide{n}.xml"))
            .map_err(|e| AppError::Internal(e.to_string()))?;
        slide_file.read_to_string(&mut xml_content)
            .map_err(|e| AppError::Internal(e.to_string()))?;
    }

    let mut reader = Reader::from_str(&xml_content);
    reader.config_mut().trim_text(true);

    let mut title = String::new();
    let mut body_lines: Vec<String> = Vec::new();
    let notes = String::new();
    let mut rich_body: Vec<SlideTextBlock> = Vec::new();
    let mut layout_unsupported = false;

    // State tracking
    let mut in_title_ph = false;
    let mut in_body_ph = false;
    let mut current_runs: Vec<SlideTextRun> = Vec::new();
    let mut current_bold = false;
    let mut current_italic = false;
    let mut current_font_size: Option<u32> = None;
    let mut current_color: Option<String> = None;
    let mut current_font_family: Option<String> = None;
    let mut current_indent: u8 = 0;
    let mut current_align = TextAlignment::Left;
    let mut buf = Vec::new();

    loop {
        match reader.read_event_into(&mut buf) {
            Ok(Event::Start(ref e)) => {
                let local = e.local_name();
                match local.as_ref() {
                    b"sp" => {
                        // Detect placeholder type via ph element inside spPr/nvSpPr
                    }
                    b"ph" => match classify_ph(e) {
                        PhKind::Title => { in_title_ph = true; in_body_ph = false; }
                        PhKind::Body => { in_body_ph = true; in_title_ph = false; }
                        PhKind::Other => { layout_unsupported = true; }
                    },
                    b"rPr" if in_title_ph || in_body_ph => {
                        current_bold = e.attributes().filter_map(|a| a.ok())
                            .any(|a| a.key.local_name().as_ref() == b"b" && a.value.as_ref() == b"1");
                        current_italic = e.attributes().filter_map(|a| a.ok())
                            .any(|a| a.key.local_name().as_ref() == b"i" && a.value.as_ref() == b"1");
                        current_font_size = e.attributes().filter_map(|a| a.ok())
                            .find(|a| a.key.local_name().as_ref() == b"sz")
                            .and_then(|a| std::str::from_utf8(&a.value).ok()
                                .and_then(|v| v.parse::<u32>().ok().map(|v| v / 100)));
                    }
                    b"pPr" if in_body_ph => {
                        current_indent = e.attributes().filter_map(|a| a.ok())
                            .find(|a| a.key.local_name().as_ref() == b"lvl")
                            .and_then(|a| std::str::from_utf8(&a.value).ok()
                                .and_then(|v| v.parse::<u8>().ok()))
                            .unwrap_or(0);
                        current_align = e.attributes().filter_map(|a| a.ok())
                            .find(|a| a.key.local_name().as_ref() == b"algn")
                            .map(|a| match a.value.as_ref() {
                                b"ctr" => TextAlignment::Center,
                                b"r" => TextAlignment::Right,
                                b"just" => TextAlignment::Justify,
                                _ => TextAlignment::Left,
                            })
                            .unwrap_or(TextAlignment::Left);
                    }
                    _ => {}
                }
            }
            Ok(Event::Empty(ref e)) => {
                match e.local_name().as_ref() {
                    b"latin" if in_title_ph || in_body_ph => {
                        current_font_family = e.attributes().filter_map(|a| a.ok())
                            .find(|a| a.key.local_name().as_ref() == b"typeface")
                            .and_then(|a| String::from_utf8(a.value.into_owned()).ok());
                    }
                    b"ph" => match classify_ph(e) {
                        PhKind::Title => { in_title_ph = true; in_body_ph = false; }
                        PhKind::Body => { in_body_ph = true; in_title_ph = false; }
                        PhKind::Other => { layout_unsupported = true; }
                    },
                    _ => {}
                }
            }
            Ok(Event::Text(ref e)) => {
                // quick-xml 0.41 split the old combined `.unescape()` into decode + unescape.
                let text = e
                    .decode()
                    .ok()
                    .and_then(|d| quick_xml::escape::unescape(&d).ok().map(|u| u.into_owned()))
                    .unwrap_or_default();
                if text.is_empty() { buf.clear(); continue; }
                if in_title_ph && title.is_empty() {
                    title = text.clone();
                } else if in_body_ph {
                    body_lines.push(text.clone());
                    current_runs.push(SlideTextRun {
                        text,
                        bold: current_bold,
                        italic: current_italic,
                        font_size_pt: current_font_size,
                        color_hex: current_color.clone(),
                        font_family: current_font_family.clone(),
                    });
                }
            }
            Ok(Event::End(ref e)) => {
                match e.local_name().as_ref() {
                    b"p" if in_body_ph && !current_runs.is_empty() => {
                        rich_body.push(SlideTextBlock {
                            runs: std::mem::take(&mut current_runs),
                            alignment: match current_align {
                                TextAlignment::Left    => TextAlignment::Left,
                                TextAlignment::Center  => TextAlignment::Center,
                                TextAlignment::Right   => TextAlignment::Right,
                                TextAlignment::Justify => TextAlignment::Justify,
                            },
                            indent_level: current_indent,
                        });
                        current_indent = 0;
                        current_align = TextAlignment::Left;
                    }
                    b"sp" => {
                        in_title_ph = false;
                        in_body_ph = false;
                        current_bold = false;
                        current_italic = false;
                        current_font_size = None;
                        current_color = None;
                        current_font_family = None;
                    }
                    _ => {}
                }
            }
            Ok(Event::Eof) => break,
            Err(e) => return Err(AppError::Internal(e.to_string())),
            _ => {}
        }
        buf.clear();
    }

    // Extract background image if present in ppt/media/ referenced by slide rels
    let background_path = extract_pptx_background(archive, n, media_dir)?;
    let has_background_image = background_path.is_some();
    let body = body_lines.join("\n");
    let word_count = body.split_whitespace().count() as u32;
    let code_slide = is_code_slide(&rich_body);

    Ok(SlideEntry {
        index: n - 1,
        title,
        body,
        notes,
        background_path,
        rich_body,
        word_count,
        has_background_image,
        is_code_slide: code_slide,
        layout_unsupported,
    })
}

fn extract_pptx_background(
    archive: &mut zip::ZipArchive<std::fs::File>,
    n: u32,
    media_dir: &Path,
) -> Result<Option<String>, AppError> {
    use std::io::Read;

    let rels_name = format!("ppt/slides/_rels/slide{n}.xml.rels");
    let mut rels_content = String::new();
    {
        let mut rels_file = match archive.by_name(&rels_name) {
            Ok(f) => f,
            Err(_) => return Ok(None),
        };
        rels_file.read_to_string(&mut rels_content)
            .map_err(|e| AppError::Internal(e.to_string()))?;
    }

    // Look for image relationship targets (blipFill/background image)
    // This is a simplified heuristic: real background images appear as relationships
    // with type ending in "image" and target in ../media/
    use quick_xml::events::Event;
    use quick_xml::Reader;
    let mut reader = Reader::from_str(&rels_content);
    let mut buf = Vec::new();
    let mut image_target: Option<String> = None;

    loop {
        match reader.read_event_into(&mut buf) {
            Ok(Event::Empty(ref e)) if e.local_name().as_ref() == b"Relationship" => {
                let rel_type = e.attributes().filter_map(|a| a.ok())
                    .find(|a| a.key.local_name().as_ref() == b"Type")
                    .and_then(|a| String::from_utf8(a.value.into_owned()).ok());
                let target = e.attributes().filter_map(|a| a.ok())
                    .find(|a| a.key.local_name().as_ref() == b"Target")
                    .and_then(|a| String::from_utf8(a.value.into_owned()).ok());

                if let (Some(rt), Some(tgt)) = (rel_type, target) {
                    if rt.ends_with("/image") && tgt.contains("media/") {
                        image_target = Some(tgt);
                        break;
                    }
                }
            }
            Ok(Event::Eof) => break,
            Err(e) => return Err(AppError::Internal(e.to_string())),
            _ => {}
        }
        buf.clear();
    }

    let Some(target) = image_target else {
        return Ok(None);
    };

    // target is like "../media/image1.png" — normalize to "ppt/media/image1.png"
    let filename = target.split('/').next_back().unwrap_or("");
    if filename.is_empty() { return Ok(None); }

    let ext = std::path::Path::new(filename)
        .extension()
        .and_then(|e| e.to_str())
        .unwrap_or("");
    if !matches!(ext.to_lowercase().as_str(), "png" | "jpg" | "jpeg") {
        return Ok(None);
    }

    let archive_key = format!("ppt/media/{filename}");
    let mut media_bytes = Vec::new();
    {
        let mut media_file = match archive.by_name(&archive_key) {
            Ok(f) => f,
            Err(_) => return Ok(None),
        };
        use std::io::Read;
        media_file.read_to_end(&mut media_bytes)
            .map_err(|e| AppError::Internal(e.to_string()))?;
    }

    let dest = media_dir.join(format!("slide_{n}_bg.{ext}"));
    std::fs::write(&dest, &media_bytes)
        .map_err(|e| AppError::Internal(e.to_string()))?;

    Ok(dest.to_str().map(|s| s.to_string()))
}

fn import_pdf(path: &Path, dest_dir: &Path, bus: &Arc<EventBus>) -> Result<SlideDeck, AppError> {
    use pdfium_render::prelude::*;

    let media_dir = dest_dir.join("media");
    std::fs::create_dir_all(&media_dir)
        .map_err(|e| AppError::Internal(e.to_string()))?;

    let pdfium = Pdfium::default();
    let document = pdfium.load_pdf_from_file(path, None)
        .map_err(|e| AppError::Internal(e.to_string()))?;

    let page_count = document.pages().len() as u32;
    let mut slides = Vec::new();

    for (i, page) in document.pages().iter().enumerate() {
        let n = i as u32 + 1;
        let result = (|| -> Result<SlideEntry, AppError> {
            let render_config = PdfRenderConfig::new()
                .set_target_width(1920)
                .set_target_height(1080);

            let image = page.render_with_config(&render_config)
                .map_err(|e| AppError::Internal(e.to_string()))?
                .as_image();

            let bg_path = media_dir.join(format!("slide_{n}_bg.png"));
            image.save(&bg_path)
                .map_err(|e| AppError::Internal(e.to_string()))?;

            let raw_text = page.text()
                .map_err(|e| AppError::Internal(e.to_string()))?
                .all();

            let title = raw_text.lines()
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
                bus.publish(AppEvent::DeckImportProgress { slide: n, total: page_count });
                slides.push(entry);
            }
            Err(reason) => {
                bus.publish(AppEvent::DeckImportFailed { slide: n, reason: reason.to_string() });
            }
        }
    }

    Ok(SlideDeck { slides })
}

pub fn is_code_slide(blocks: &[SlideTextBlock]) -> bool {
    const MONOSPACE_FAMILIES: &[&str] = &[
        "Courier New", "Consolas", "Monaco",
        "Fira Code", "JetBrains Mono", "Source Code Pro",
    ];
    blocks.iter().any(|block| {
        block.runs.iter().any(|run| {
            run.font_family.as_deref()
                .map(|f| MONOSPACE_FAMILIES.contains(&f))
                .unwrap_or(false)
        })
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::slide::{SlideTextBlock, SlideTextRun, TextAlignment};

    fn make_run(font_family: Option<&str>) -> SlideTextRun {
        SlideTextRun {
            text: "hello".into(),
            bold: false,
            italic: false,
            font_size_pt: None,
            color_hex: None,
            font_family: font_family.map(|s| s.to_string()),
        }
    }

    fn make_block(run: SlideTextRun) -> SlideTextBlock {
        SlideTextBlock { runs: vec![run], alignment: TextAlignment::Left, indent_level: 0 }
    }

    #[test]
    fn is_code_slide_true_for_monospace_font_run() {
        let blocks = vec![make_block(make_run(Some("Consolas")))];
        assert!(is_code_slide(&blocks));
    }

    #[test]
    fn is_code_slide_false_for_sans_serif_run() {
        let blocks = vec![make_block(make_run(Some("Arial")))];
        assert!(!is_code_slide(&blocks));
    }

    fn empty_ph_event(xml: &str) -> quick_xml::events::BytesStart<'static> {
        use quick_xml::events::Event;
        use quick_xml::Reader;
        let mut reader = Reader::from_str(xml);
        let mut buf = Vec::new();
        loop {
            match reader.read_event_into(&mut buf).unwrap() {
                Event::Empty(e) => return e.into_owned(),
                Event::Eof => panic!("no empty element found in {xml}"),
                _ => {}
            }
            buf.clear();
        }
    }

    #[test]
    fn classify_ph_title_for_title_type() {
        let e = empty_ph_event(r#"<p:ph type="title"/>"#);
        assert!(matches!(classify_ph(&e), PhKind::Title));
    }

    #[test]
    fn classify_ph_title_for_ctr_title_type() {
        let e = empty_ph_event(r#"<p:ph type="ctrTitle"/>"#);
        assert!(matches!(classify_ph(&e), PhKind::Title));
    }

    #[test]
    fn classify_ph_body_for_body_type() {
        let e = empty_ph_event(r#"<p:ph type="body" idx="1"/>"#);
        assert!(matches!(classify_ph(&e), PhKind::Body));
    }

    #[test]
    fn classify_ph_body_when_type_attribute_absent() {
        let e = empty_ph_event(r#"<p:ph idx="1"/>"#);
        assert!(matches!(classify_ph(&e), PhKind::Body));
    }

    #[test]
    fn classify_ph_other_for_unrecognised_type() {
        let e = empty_ph_event(r#"<p:ph type="subTitle"/>"#);
        assert!(matches!(classify_ph(&e), PhKind::Other));
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