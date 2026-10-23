//! Crash-safe persistence: atomic writes with a `.bak`, and recovery that never aborts startup.
use std::fs;
use std::io::Write;
use std::path::{Path, PathBuf};

use crate::error::AppError;

fn sibling(path: &Path, suffix: &str) -> PathBuf {
    let mut name = path.file_name().unwrap_or_default().to_os_string();
    name.push(suffix);
    path.with_file_name(name)
}

pub fn backup_path(path: &Path) -> PathBuf {
    sibling(path, ".bak")
}

/// Writes via temp file + fsync + rename, copying the previous version to `<path>.bak` first.
pub fn atomic_write(path: &Path, bytes: &[u8]) -> Result<(), AppError> {
    let io = |e: std::io::Error| AppError::Internal(format!("write {}: {e}", path.display()));
    if let Some(parent) = path.parent().filter(|p| !p.as_os_str().is_empty()) {
        fs::create_dir_all(parent).map_err(io)?;
    }
    let tmp = sibling(path, ".tmp");
    {
        let mut f = fs::File::create(&tmp).map_err(io)?;
        f.write_all(bytes).map_err(io)?;
        f.sync_all().map_err(io)?;
    }
    if path.exists() {
        fs::copy(path, backup_path(path)).map_err(io)?;
    }
    fs::rename(&tmp, path).map_err(io)
}

#[derive(Debug, PartialEq)]
pub enum Recovery {
    /// Primary file parsed fine, or did not exist.
    Clean,
    /// Primary was unreadable; the `.bak` was used. The broken file was kept at `broken`.
    FromBackup { broken: PathBuf },
    /// Neither parsed; defaults are in use. The broken file was kept at `broken`.
    Defaults { broken: PathBuf },
}

impl Recovery {
    pub fn notice(&self, what: &str) -> Option<String> {
        match self {
            Recovery::Clean => None,
            Recovery::FromBackup { broken } => Some(format!(
                "{what} was damaged; the previous saved copy was restored. The damaged file was kept as {}.",
                broken.display()
            )),
            Recovery::Defaults { broken } => Some(format!(
                "{what} was damaged and no usable backup existed; defaults are in use. The damaged file was kept as {}.",
                broken.display()
            )),
        }
    }
}

fn set_aside(path: &Path) -> PathBuf {
    let ts = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map(|d| d.as_secs())
        .unwrap_or(0);
    let broken = sibling(path, &format!(".broken-{ts}"));
    if fs::rename(path, &broken).is_err() {
        let _ = fs::copy(path, &broken);
        let _ = fs::remove_file(path);
    }
    broken
}

/// Reads and parses `path`. A missing file is `Clean` with `None`; a damaged one is set aside,
/// then the `.bak` is tried, then `None` (caller supplies defaults). Never returns an error.
pub fn load_recovering<T>(path: &Path, parse: impl Fn(&str) -> Option<T>) -> (Option<T>, Recovery) {
    if !path.exists() {
        return (None, Recovery::Clean);
    }
    if let Some(v) = fs::read_to_string(path).ok().and_then(|s| parse(&s)) {
        return (Some(v), Recovery::Clean);
    }
    let broken = set_aside(path);
    let bak = backup_path(path);
    match fs::read_to_string(&bak).ok().and_then(|s| parse(&s)) {
        Some(v) => {
            let _ = fs::copy(&bak, path);
            (Some(v), Recovery::FromBackup { broken })
        }
        None => (None, Recovery::Defaults { broken }),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn parse_num(s: &str) -> Option<u32> {
        s.trim().parse().ok()
    }

    #[test]
    fn atomic_write_replaces_content_keeps_bak_and_leaves_no_temp() {
        let d = tempfile::tempdir().unwrap();
        let p = d.path().join("sub").join("a.json");
        atomic_write(&p, b"1").unwrap();
        assert!(!backup_path(&p).exists(), "no bak on first write");
        atomic_write(&p, b"2").unwrap();
        assert_eq!(fs::read_to_string(&p).unwrap(), "2");
        assert_eq!(fs::read_to_string(backup_path(&p)).unwrap(), "1");
        assert!(!sibling(&p, ".tmp").exists());
    }

    #[test]
    fn a_leftover_temp_from_a_crash_never_replaces_the_real_file() {
        let d = tempfile::tempdir().unwrap();
        let p = d.path().join("a.json");
        atomic_write(&p, b"7").unwrap();
        fs::write(sibling(&p, ".tmp"), b"half wri").unwrap();
        assert_eq!(load_recovering(&p, parse_num), (Some(7), Recovery::Clean));
        atomic_write(&p, b"8").unwrap();
        assert_eq!(fs::read_to_string(&p).unwrap(), "8");
    }

    #[test]
    fn missing_file_is_clean_and_empty() {
        let d = tempfile::tempdir().unwrap();
        assert_eq!(
            load_recovering(&d.path().join("x"), parse_num),
            (None, Recovery::Clean)
        );
    }

    #[test]
    fn damaged_file_recovers_from_bak_and_keeps_the_broken_copy() {
        let d = tempfile::tempdir().unwrap();
        let p = d.path().join("a.json");
        atomic_write(&p, b"41").unwrap();
        atomic_write(&p, b"42").unwrap();
        fs::write(&p, "42 truncat").unwrap();
        let (v, r) = load_recovering(&p, parse_num);
        assert_eq!(v, Some(41));
        let Recovery::FromBackup { broken } = r else {
            panic!("expected FromBackup")
        };
        assert_eq!(fs::read_to_string(broken).unwrap(), "42 truncat");
        assert_eq!(fs::read_to_string(&p).unwrap(), "41", "primary restored");
    }

    #[test]
    fn damaged_file_without_usable_bak_yields_defaults_and_never_errors() {
        let d = tempfile::tempdir().unwrap();
        let p = d.path().join("a.json");
        fs::write(&p, "garbage").unwrap();
        fs::write(backup_path(&p), "also garbage").unwrap();
        let (v, r) = load_recovering(&p, parse_num);
        assert_eq!(v, None);
        assert!(matches!(r, Recovery::Defaults { .. }));
        assert!(!p.exists(), "damaged file is moved aside");
        assert!(r
            .notice("Settings")
            .unwrap()
            .contains("defaults are in use"));
    }

    #[test]
    fn clean_recovery_has_no_notice() {
        assert_eq!(Recovery::Clean.notice("x"), None);
    }
}
