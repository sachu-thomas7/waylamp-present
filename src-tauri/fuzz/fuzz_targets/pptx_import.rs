#![no_main]
use libfuzzer_sys::fuzz_target;

#[path = "../../src/error.rs"]
mod error;
#[path = "../../src/persist.rs"]
mod persist;
#[path = "../../src/pptx.rs"]
mod pptx;
#[path = "../../src/slide.rs"]
mod slide;

fuzz_target!(|data: &[u8]| {
    let Ok(archive) = zip::ZipArchive::new(std::io::Cursor::new(data)) else {
        return;
    };
    let limits = pptx::ImportLimits {
        max_xml_bytes: 1 << 20,
        max_media_bytes: 1 << 20,
        max_total_uncompressed: 16 << 20,
        ..Default::default()
    };
    let media_dir = std::env::temp_dir().join("streamdeck-fuzz-media");
    let _ = pptx::import_pptx(archive, &media_dir, &limits, &mut |_| {});
});
