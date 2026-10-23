//! PPTX parsing. Depends only on `error` and `slide` so the fuzz target can compile it alone.
use crate::error::AppError;
use crate::slide::{SlideDeck, SlideEntry, SlideTextBlock, SlideTextRun, TextAlignment};
use std::io::{Read, Seek};
use std::path::Path;

/// Caps that keep a hostile or corrupt archive from exhausting memory, disk or time.
#[derive(Debug, Clone)]
pub struct ImportLimits {
    pub max_file_bytes: u64,
    pub max_entries: usize,
    pub max_xml_bytes: u64,
    pub max_media_bytes: u64,
    pub max_total_uncompressed: u64,
    pub max_ratio: u64,
    pub max_slides: u32,
}

impl Default for ImportLimits {
    fn default() -> Self {
        Self {
            max_file_bytes: 200 * 1024 * 1024,
            max_entries: 10_000,
            max_xml_bytes: 16 * 1024 * 1024,
            max_media_bytes: 64 * 1024 * 1024,
            max_total_uncompressed: 1024 * 1024 * 1024,
            max_ratio: 100,
            max_slides: 500,
        }
    }
}

const RATIO_FLOOR_BYTES: u64 = 1024 * 1024;

fn too_big(what: &str, limit: u64) -> AppError {
    AppError::InvalidInput(format!(
        "this file looks unsafe or too large to import ({what} exceeds {} MB)",
        limit / (1024 * 1024)
    ))
}

/// Reads at most `limit` bytes; a stream that would exceed it (whatever its header claims) is rejected.
fn read_capped(r: &mut impl Read, limit: u64) -> Result<Vec<u8>, AppError> {
    let mut buf = Vec::new();
    r.take(limit + 1)
        .read_to_end(&mut buf)
        .map_err(|e| AppError::Internal(e.to_string()))?;
    if buf.len() as u64 > limit {
        return Err(too_big("an archive entry", limit));
    }
    Ok(buf)
}

fn read_text_capped(r: &mut impl Read, limit: u64) -> Result<String, AppError> {
    String::from_utf8(read_capped(r, limit)?)
        .map_err(|_| AppError::InvalidInput("archive XML is not valid UTF-8".into()))
}

/// Rejects zip bombs from the central directory alone, before anything is decompressed.
pub fn check_archive<R: Read + Seek>(
    archive: &mut zip::ZipArchive<R>,
    limits: &ImportLimits,
) -> Result<(), AppError> {
    if archive.len() > limits.max_entries {
        return Err(AppError::InvalidInput(format!(
            "this file has too many parts ({} > {})",
            archive.len(),
            limits.max_entries
        )));
    }
    let mut total: u64 = 0;
    for i in 0..archive.len() {
        let entry = archive
            .by_index_raw(i)
            .map_err(|e| AppError::InvalidInput(e.to_string()))?;
        let (size, packed) = (entry.size(), entry.compressed_size());
        if size > limits.max_media_bytes.max(limits.max_xml_bytes) {
            return Err(too_big("an archive entry", limits.max_media_bytes));
        }
        if size > RATIO_FLOOR_BYTES && size / packed.max(1) > limits.max_ratio {
            return Err(AppError::InvalidInput(
                "this file looks unsafe to import (suspicious compression ratio)".into(),
            ));
        }
        total = total.saturating_add(size);
        if total > limits.max_total_uncompressed {
            return Err(too_big(
                "the uncompressed contents",
                limits.max_total_uncompressed,
            ));
        }
    }
    Ok(())
}

pub enum SlideOutcome {
    Done { n: u32, total: u32 },
    Failed { n: u32, reason: String },
}

/// Parses every slide of an already-opened PPTX archive, writing background images into `media_dir`.
pub fn import_pptx<R: Read + Seek>(
    mut archive: zip::ZipArchive<R>,
    media_dir: &Path,
    limits: &ImportLimits,
    on_slide: &mut dyn FnMut(SlideOutcome),
) -> Result<SlideDeck, AppError> {
    check_archive(&mut archive, limits)?;

    let slide_count = (1..=limits.max_slides + 1)
        .take_while(|n| archive.by_name(&format!("ppt/slides/slide{n}.xml")).is_ok())
        .count() as u32;
    if slide_count > limits.max_slides {
        return Err(AppError::InvalidInput(format!(
            "this deck has more than {} slides",
            limits.max_slides
        )));
    }
    if slide_count == 0 {
        return Err(AppError::InvalidInput(
            "no slides found in this file".into(),
        ));
    }

    std::fs::create_dir_all(media_dir).map_err(|e| AppError::Internal(e.to_string()))?;

    let mut slides = Vec::new();
    for n in 1..=slide_count {
        match parse_pptx_slide(&mut archive, n, media_dir, limits) {
            Ok(entry) => {
                on_slide(SlideOutcome::Done {
                    n,
                    total: slide_count,
                });
                slides.push(entry);
            }
            Err(reason) => on_slide(SlideOutcome::Failed {
                n,
                reason: reason.to_string(),
            }),
        }
    }
    if slides.is_empty() {
        return Err(AppError::InvalidInput(
            "none of the slides in this file could be read".into(),
        ));
    }
    Ok(SlideDeck { slides })
}

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
        .find(|a| a.key.local_name().as_ref() == "type");
    match type_attr.as_ref().map(|a| a.value.as_ref()) {
        Some("title") | Some("ctrTitle") => PhKind::Title,
        Some("body") | None => PhKind::Body,
        Some(_) => PhKind::Other,
    }
}

fn parse_pptx_slide<R: Read + Seek>(
    archive: &mut zip::ZipArchive<R>,
    n: u32,
    media_dir: &Path,
    limits: &ImportLimits,
) -> Result<SlideEntry, AppError> {
    use quick_xml::events::Event;
    use quick_xml::Reader;

    let xml_content = {
        let mut slide_file = archive
            .by_name(&format!("ppt/slides/slide{n}.xml"))
            .map_err(|e| AppError::Internal(e.to_string()))?;
        read_text_capped(&mut slide_file, limits.max_xml_bytes)?
    };

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
                    "sp" => {
                        // Detect placeholder type via ph element inside spPr/nvSpPr
                    }
                    "ph" => match classify_ph(e) {
                        PhKind::Title => {
                            in_title_ph = true;
                            in_body_ph = false;
                        }
                        PhKind::Body => {
                            in_body_ph = true;
                            in_title_ph = false;
                        }
                        PhKind::Other => {
                            layout_unsupported = true;
                        }
                    },
                    "rPr" if in_title_ph || in_body_ph => {
                        current_bold = e
                            .attributes()
                            .filter_map(|a| a.ok())
                            .any(|a| a.key.local_name().as_ref() == "b" && a.value.as_ref() == "1");
                        current_italic = e
                            .attributes()
                            .filter_map(|a| a.ok())
                            .any(|a| a.key.local_name().as_ref() == "i" && a.value.as_ref() == "1");
                        current_font_size = e
                            .attributes()
                            .filter_map(|a| a.ok())
                            .find(|a| a.key.local_name().as_ref() == "sz")
                            .and_then(|a| {
                                Some(a.value.as_ref())
                                    .and_then(|v| v.parse::<u32>().ok().map(|v| v / 100))
                            });
                    }
                    "pPr" if in_body_ph => {
                        current_indent = e
                            .attributes()
                            .filter_map(|a| a.ok())
                            .find(|a| a.key.local_name().as_ref() == "lvl")
                            .and_then(|a| Some(a.value.as_ref()).and_then(|v| v.parse::<u8>().ok()))
                            .unwrap_or(0);
                        current_align = e
                            .attributes()
                            .filter_map(|a| a.ok())
                            .find(|a| a.key.local_name().as_ref() == "algn")
                            .map(|a| match a.value.as_ref() {
                                "ctr" => TextAlignment::Center,
                                "r" => TextAlignment::Right,
                                "just" => TextAlignment::Justify,
                                _ => TextAlignment::Left,
                            })
                            .unwrap_or(TextAlignment::Left);
                    }
                    _ => {}
                }
            }
            Ok(Event::Empty(ref e)) => match e.local_name().as_ref() {
                "latin" if in_title_ph || in_body_ph => {
                    current_font_family = e
                        .attributes()
                        .filter_map(|a| a.ok())
                        .find(|a| a.key.local_name().as_ref() == "typeface")
                        .map(|a| a.value.into_owned());
                }
                "ph" => match classify_ph(e) {
                    PhKind::Title => {
                        in_title_ph = true;
                        in_body_ph = false;
                    }
                    PhKind::Body => {
                        in_body_ph = true;
                        in_title_ph = false;
                    }
                    PhKind::Other => {
                        layout_unsupported = true;
                    }
                },
                _ => {}
            },
            Ok(Event::Text(ref e)) => {
                let text = e.xml10_content().into_owned();

                if text.is_empty() {
                    buf.clear();
                    continue;
                }
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
            Ok(Event::End(ref e)) => match e.local_name().as_ref() {
                "p" if in_body_ph && !current_runs.is_empty() => {
                    rich_body.push(SlideTextBlock {
                        runs: std::mem::take(&mut current_runs),
                        alignment: match current_align {
                            TextAlignment::Left => TextAlignment::Left,
                            TextAlignment::Center => TextAlignment::Center,
                            TextAlignment::Right => TextAlignment::Right,
                            TextAlignment::Justify => TextAlignment::Justify,
                        },
                        indent_level: current_indent,
                    });
                    current_indent = 0;
                    current_align = TextAlignment::Left;
                }
                "sp" => {
                    in_title_ph = false;
                    in_body_ph = false;
                    current_bold = false;
                    current_italic = false;
                    current_font_size = None;
                    current_color = None;
                    current_font_family = None;
                }
                _ => {}
            },
            Ok(Event::Eof) => break,
            Err(e) => return Err(AppError::Internal(e.to_string())),
            _ => {}
        }
        buf.clear();
    }

    // Extract background image if present in ppt/media/ referenced by slide rels
    let background_path = extract_pptx_background(archive, n, media_dir, limits)?;
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

fn extract_pptx_background<R: Read + Seek>(
    archive: &mut zip::ZipArchive<R>,
    n: u32,
    media_dir: &Path,
    limits: &ImportLimits,
) -> Result<Option<String>, AppError> {
    let rels_name = format!("ppt/slides/_rels/slide{n}.xml.rels");
    let rels_content = {
        let mut rels_file = match archive.by_name(&rels_name) {
            Ok(f) => f,
            Err(_) => return Ok(None),
        };
        read_text_capped(&mut rels_file, limits.max_xml_bytes)?
    };

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
            Ok(Event::Empty(ref e)) if e.local_name().as_ref() == "Relationship" => {
                let rel_type = e
                    .attributes()
                    .filter_map(|a| a.ok())
                    .find(|a| a.key.local_name().as_ref() == "Type")
                    .map(|a| a.value.into_owned());
                let target = e
                    .attributes()
                    .filter_map(|a| a.ok())
                    .find(|a| a.key.local_name().as_ref() == "Target")
                    .map(|a| a.value.into_owned());

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
    if filename.is_empty() {
        return Ok(None);
    }

    let ext = std::path::Path::new(filename)
        .extension()
        .and_then(|e| e.to_str())
        .unwrap_or("");
    if !matches!(ext.to_lowercase().as_str(), "png" | "jpg" | "jpeg") {
        return Ok(None);
    }

    let archive_key = format!("ppt/media/{filename}");
    let media_bytes = {
        let mut media_file = match archive.by_name(&archive_key) {
            Ok(f) => f,
            Err(_) => return Ok(None),
        };
        read_capped(&mut media_file, limits.max_media_bytes)?
    };

    let dest = media_dir.join(format!("slide_{n}_bg.{ext}"));
    std::fs::write(&dest, &media_bytes).map_err(|e| AppError::Internal(e.to_string()))?;

    Ok(dest.to_str().map(|s| s.to_string()))
}

pub fn is_code_slide(blocks: &[SlideTextBlock]) -> bool {
    const MONOSPACE_FAMILIES: &[&str] = &[
        "Courier New",
        "Consolas",
        "Monaco",
        "Fira Code",
        "JetBrains Mono",
        "Source Code Pro",
    ];
    blocks.iter().any(|block| {
        block.runs.iter().any(|run| {
            run.font_family
                .as_deref()
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
        SlideTextBlock {
            runs: vec![run],
            alignment: TextAlignment::Left,
            indent_level: 0,
        }
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

    // --- import limits ---

    use std::io::{Cursor, Write};

    fn zip_of(entries: &[(&str, Vec<u8>)]) -> zip::ZipArchive<Cursor<Vec<u8>>> {
        let mut w = zip::ZipWriter::new(Cursor::new(Vec::new()));
        let opts = zip::write::SimpleFileOptions::default()
            .compression_method(zip::CompressionMethod::Deflated);
        for (name, data) in entries {
            w.start_file(*name, opts).unwrap();
            w.write_all(data).unwrap();
        }
        zip::ZipArchive::new(Cursor::new(w.finish().unwrap().into_inner())).unwrap()
    }

    const SLIDE: &str = r#"<p:sld><p:sp><p:ph type="title"/><a:t>Hi</a:t></p:sp></p:sld>"#;

    fn run(entries: &[(&str, Vec<u8>)], limits: &ImportLimits) -> Result<SlideDeck, AppError> {
        let dir = tempfile::tempdir().unwrap();
        import_pptx(zip_of(entries), dir.path(), limits, &mut |_| {})
    }

    fn invalid(r: Result<SlideDeck, AppError>) -> String {
        match r {
            Err(AppError::InvalidInput(m)) => m,
            other => panic!("expected InvalidInput, got {other:?}"),
        }
    }

    #[test]
    fn decompression_bomb_is_rejected_by_ratio_before_any_slide_is_read() {
        let bomb = vec![0u8; 8 * 1024 * 1024];
        let msg = invalid(run(
            &[("ppt/slides/slide1.xml", bomb)],
            &ImportLimits::default(),
        ));
        assert!(msg.contains("compression ratio"), "{msg}");
    }

    #[test]
    fn oversized_total_entry_count_and_entry_size_are_rejected() {
        let slide = || ("ppt/slides/slide1.xml", SLIDE.as_bytes().to_vec());
        let tight_total = ImportLimits {
            max_total_uncompressed: 10,
            ..Default::default()
        };
        assert!(invalid(run(&[slide()], &tight_total)).contains("uncompressed contents"));
        let few = ImportLimits {
            max_entries: 1,
            ..Default::default()
        };
        let two = [slide(), ("a.txt", b"x".to_vec())];
        assert!(invalid(run(&two, &few)).contains("too many parts"));
        let small = ImportLimits {
            max_xml_bytes: 8,
            max_media_bytes: 8,
            ..Default::default()
        };
        assert!(invalid(run(&[slide()], &small)).contains("too large"));
    }

    #[test]
    fn capped_read_stops_a_stream_that_lies_about_its_size() {
        let mut endless = std::io::repeat(b'a');
        let err = read_capped(&mut endless, 1024).unwrap_err();
        assert!(matches!(err, AppError::InvalidInput(_)));
        assert_eq!(
            read_capped(&mut Cursor::new(vec![1u8; 1024]), 1024)
                .unwrap()
                .len(),
            1024
        );
    }

    #[test]
    fn slide_count_cap_and_empty_decks_are_rejected() {
        let three: Vec<(String, Vec<u8>)> = (1..=3)
            .map(|n| {
                (
                    format!("ppt/slides/slide{n}.xml"),
                    SLIDE.as_bytes().to_vec(),
                )
            })
            .collect();
        let refs: Vec<(&str, Vec<u8>)> =
            three.iter().map(|(n, d)| (n.as_str(), d.clone())).collect();
        let cap = ImportLimits {
            max_slides: 2,
            ..Default::default()
        };
        assert!(invalid(run(&refs, &cap)).contains("more than 2 slides"));
        assert_eq!(
            run(&refs, &ImportLimits::default()).unwrap().slides.len(),
            3
        );
        assert!(invalid(run(
            &[("readme.txt", b"x".to_vec())],
            &ImportLimits::default()
        ))
        .contains("no slides"));
    }

    #[test]
    fn a_deck_where_every_slide_fails_is_an_error_not_an_empty_deck() {
        let bad = ("ppt/slides/slide1.xml", vec![0xff, 0xfe, 0xfd]);
        let msg = invalid(run(&[bad], &ImportLimits::default()));
        assert!(
            msg.contains("could be read") || msg.contains("UTF-8"),
            "{msg}"
        );
    }
}
