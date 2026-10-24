//! `sdmedia` custom URI scheme: serves slide backgrounds and scene media from the app data dir to the webviews.
use std::io::{Read, Seek, SeekFrom};
use std::path::{Path, PathBuf};

pub const SCHEME: &str = "sdmedia";
const MAX_CHUNK: u64 = 16 * 1024 * 1024;

#[derive(Debug, PartialEq)]
pub struct MediaResponse {
    pub status: u16,
    pub headers: Vec<(&'static str, String)>,
    pub body: Vec<u8>,
}

impl MediaResponse {
    fn status(status: u16) -> Self {
        Self {
            status,
            headers: vec![("Cache-Control", "no-store".into())],
            body: Vec::new(),
        }
    }
}

fn content_type(path: &Path) -> Option<&'static str> {
    match path.extension()?.to_str()?.to_lowercase().as_str() {
        "png" => Some("image/png"),
        "jpg" | "jpeg" => Some("image/jpeg"),
        "webp" => Some("image/webp"),
        "mp4" => Some("video/mp4"),
        "webm" => Some("video/webm"),
        _ => None,
    }
}

/// Maps a request path (percent-encoded absolute file path) to a file inside `data_dir` (or the one staged video), or `None`.
pub fn resolve(data_dir: &Path, staged_video: Option<&Path>, uri_path: &str) -> Option<PathBuf> {
    let decoded = percent_encoding::percent_decode_str(uri_path.trim_start_matches('/'))
        .decode_utf8()
        .ok()?;
    let root = std::fs::canonicalize(data_dir).ok()?;
    let file = std::fs::canonicalize(Path::new(decoded.as_ref())).ok()?;
    let allowed = file.starts_with(&root)
        || staged_video
            .and_then(|v| std::fs::canonicalize(v).ok())
            .is_some_and(|v| v == file);
    (allowed && file.is_file()).then_some(file)
}

/// Inclusive byte range for a `Range` header against a file of `len` bytes; `None` means 416.
pub fn parse_range(header: &str, len: u64) -> Option<(u64, u64)> {
    let spec = header.strip_prefix("bytes=")?;
    if spec.contains(',') || len == 0 {
        return None;
    }
    let (a, b) = spec.split_once('-')?;
    let (start, end) = if a.is_empty() {
        let n: u64 = b.parse().ok()?;
        (len.saturating_sub(n), len - 1)
    } else {
        let start: u64 = a.parse().ok()?;
        let end = if b.is_empty() {
            len - 1
        } else {
            b.parse::<u64>().ok()?.min(len - 1)
        };
        (start, end)
    };
    (start <= end && start < len).then_some((start, end))
}

pub fn respond(
    data_dir: &Path,
    staged_video: Option<&Path>,
    uri_path: &str,
    range: Option<&str>,
) -> MediaResponse {
    let Some(file) = resolve(data_dir, staged_video, uri_path) else {
        return MediaResponse::status(404);
    };
    let Some(mime) = content_type(&file) else {
        return MediaResponse::status(404);
    };
    let Ok(mut handle) = std::fs::File::open(&file) else {
        return MediaResponse::status(404);
    };
    let Ok(len) = handle.metadata().map(|m| m.len()) else {
        return MediaResponse::status(404);
    };

    let (start, end, partial) = match range {
        Some(h) => match parse_range(h, len) {
            Some((s, e)) => (s, e.min(s + MAX_CHUNK - 1), true),
            None => {
                let mut r = MediaResponse::status(416);
                r.headers.push(("Content-Range", format!("bytes */{len}")));
                return r;
            }
        },
        None if len > MAX_CHUNK => (0, MAX_CHUNK - 1, true),
        None => (0, len.saturating_sub(1), false),
    };

    let mut body = Vec::new();
    if len > 0 {
        let take = end - start + 1;
        if handle.seek(SeekFrom::Start(start)).is_err()
            || handle.take(take).read_to_end(&mut body).is_err()
        {
            return MediaResponse::status(500);
        }
    }
    let mut r = MediaResponse::status(if partial { 206 } else { 200 });
    r.headers.push(("Content-Type", mime.into()));
    r.headers.push(("Accept-Ranges", "bytes".into()));
    r.headers.push(("Content-Length", body.len().to_string()));
    if partial {
        r.headers
            .push(("Content-Range", format!("bytes {start}-{end}/{len}")));
    }
    r.body = body;
    r
}

#[cfg(test)]
mod tests {
    use super::*;

    fn enc(p: &Path) -> String {
        format!(
            "/{}",
            percent_encoding::utf8_percent_encode(
                &p.to_string_lossy(),
                percent_encoding::NON_ALPHANUMERIC
            )
        )
    }

    fn fixture() -> (tempfile::TempDir, PathBuf) {
        let dir = tempfile::tempdir().unwrap();
        let f = dir.path().join("media dir").join("a b.png");
        std::fs::create_dir_all(f.parent().unwrap()).unwrap();
        std::fs::write(&f, b"0123456789").unwrap();
        (dir, f)
    }

    #[test]
    fn serves_file_inside_data_dir_with_encoded_path() {
        let (dir, f) = fixture();
        let r = respond(dir.path(), None, &enc(&f), None);
        assert_eq!(r.status, 200);
        assert_eq!(r.body, b"0123456789");
        assert!(r
            .headers
            .contains(&("Content-Type", "image/png".to_string())));
    }

    #[test]
    fn refuses_files_outside_data_dir() {
        let (dir, _) = fixture();
        let outside = tempfile::tempdir().unwrap();
        let o = outside.path().join("x.png");
        std::fs::write(&o, b"secret").unwrap();
        assert_eq!(respond(dir.path(), None, &enc(&o), None).status, 404);
    }

    #[test]
    fn refuses_dotdot_traversal() {
        let (dir, f) = fixture();
        let outside = tempfile::tempdir().unwrap();
        std::fs::write(outside.path().join("x.png"), b"secret").unwrap();
        let sneaky = dir
            .path()
            .join("..")
            .join(outside.path().file_name().unwrap())
            .join("x.png");
        assert_eq!(respond(dir.path(), None, &enc(&sneaky), None).status, 404);
        assert_eq!(respond(dir.path(), None, &enc(&f), None).status, 200);
    }

    #[test]
    fn refuses_disallowed_extension_even_inside_data_dir() {
        let (dir, _) = fixture();
        let toml = dir.path().join("waylamp-present.toml");
        std::fs::write(&toml, b"x=1").unwrap();
        assert_eq!(respond(dir.path(), None, &enc(&toml), None).status, 404);
    }

    #[test]
    fn missing_file_and_garbage_path_are_404() {
        let (dir, _) = fixture();
        assert_eq!(respond(dir.path(), None, "/%FF%FE", None).status, 404);
        assert_eq!(respond(dir.path(), None, "/nope.png", None).status, 404);
    }

    #[test]
    fn range_requests_return_206_slices() {
        let (dir, f) = fixture();
        let r = respond(dir.path(), None, &enc(&f), Some("bytes=2-4"));
        assert_eq!((r.status, r.body.as_slice()), (206, &b"234"[..]));
        assert!(r
            .headers
            .contains(&("Content-Range", "bytes 2-4/10".to_string())));
        let tail = respond(dir.path(), None, &enc(&f), Some("bytes=8-"));
        assert_eq!(tail.body, b"89");
        let suffix = respond(dir.path(), None, &enc(&f), Some("bytes=-3"));
        assert_eq!(suffix.body, b"789");
    }

    #[test]
    fn unsatisfiable_range_is_416() {
        let (dir, f) = fixture();
        assert_eq!(
            respond(dir.path(), None, &enc(&f), Some("bytes=50-")).status,
            416
        );
        assert_eq!(
            respond(dir.path(), None, &enc(&f), Some("items=1-2")).status,
            416
        );
        assert_eq!(
            respond(dir.path(), None, &enc(&f), Some("bytes=1-2,4-5")).status,
            416
        );
    }
}

#[cfg(test)]
mod staged_video_tests {
    use super::*;

    #[test]
    fn only_the_exact_staged_video_is_served_from_outside_the_data_dir() {
        let data = tempfile::tempdir().unwrap();
        let ext = tempfile::tempdir().unwrap();
        let clip = ext.path().join("clip.mp4");
        let other = ext.path().join("other.mp4");
        std::fs::write(&clip, b"v").unwrap();
        std::fs::write(&other, b"v").unwrap();
        let enc = |p: &Path| {
            format!(
                "/{}",
                percent_encoding::utf8_percent_encode(
                    &p.to_string_lossy(),
                    percent_encoding::NON_ALPHANUMERIC
                )
            )
        };
        assert_eq!(
            respond(data.path(), Some(&clip), &enc(&clip), None).status,
            200
        );
        assert_eq!(
            respond(data.path(), Some(&clip), &enc(&other), None).status,
            404
        );
        assert_eq!(respond(data.path(), None, &enc(&clip), None).status, 404);
    }
}
