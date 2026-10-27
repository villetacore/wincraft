//! Saving documents without risking the previous version (FIX-04).
//!
//! The data goes to a temporary file in the same directory (same file system), is synced to
//! disk, and only then replaces the document with a rename. A failure at any point — full
//! disk, permissions, power loss — leaves the old document as it was.
use std::{
    fs, io,
    io::Write,
    path::{Path, PathBuf},
};

/// The file a save really replaces: a symlinked document is updated at its target, so the
/// link stays a link.
fn real_target(path: &Path) -> PathBuf {
    match fs::symlink_metadata(path) {
        Ok(meta) if meta.file_type().is_symlink() => {
            fs::canonicalize(path).unwrap_or_else(|_| path.to_path_buf())
        }
        _ => path.to_path_buf(),
    }
}

/// Writes `data` to `path` atomically; existing permissions are kept.
pub fn write_atomic(path: &Path, data: &[u8]) -> io::Result<()> {
    let target = real_target(path);
    let dir = target
        .parent()
        .filter(|d| !d.as_os_str().is_empty())
        .unwrap_or(Path::new("."));
    let name = target
        .file_name()
        .ok_or_else(|| io::Error::new(io::ErrorKind::InvalidInput, "no file name"))?;
    let temp = dir.join(format!(
        ".{}.saving-{}",
        name.to_string_lossy(),
        std::process::id()
    ));
    let result = (|| {
        let mut file = fs::OpenOptions::new()
            .write(true)
            .create_new(true)
            .open(&temp)?;
        file.write_all(data)?;
        if let Ok(meta) = fs::metadata(&target) {
            fs::set_permissions(&temp, meta.permissions())?;
        }
        file.sync_all()?;
        drop(file);
        fs::rename(&temp, &target)?;
        // The rename itself must reach the disk too.
        #[cfg(unix)]
        if let Ok(d) = fs::File::open(dir) {
            let _ = d.sync_all();
        }
        Ok(())
    })();
    if result.is_err() {
        let _ = fs::remove_file(&temp);
    }
    result
}

#[cfg(test)]
mod tests {
    use super::*;

    fn temp_dir(name: &str) -> PathBuf {
        let dir = std::env::temp_dir().join(format!("wc-atomic-{name}-{}", std::process::id()));
        let _ = fs::remove_dir_all(&dir);
        fs::create_dir_all(&dir).unwrap();
        dir
    }

    #[test]
    fn creates_and_replaces_without_leftovers() {
        let dir = temp_dir("replace");
        let doc = dir.join("note.txt");
        write_atomic(&doc, b"first").unwrap();
        write_atomic(&doc, b"second").unwrap();
        assert_eq!(fs::read(&doc).unwrap(), b"second");
        let names: Vec<_> = fs::read_dir(&dir)
            .unwrap()
            .flatten()
            .map(|e| e.file_name())
            .collect();
        assert_eq!(names.len(), 1, "no temporary files remain: {names:?}");
        let _ = fs::remove_dir_all(dir);
    }

    #[test]
    fn a_failed_save_keeps_the_old_document() {
        let dir = temp_dir("fail");
        let doc = dir.join("note.txt");
        write_atomic(&doc, b"old").unwrap();
        // A directory where the temporary file would go makes the save fail before rename.
        let blocker = dir.join(format!(".note.txt.saving-{}", std::process::id()));
        fs::create_dir(&blocker).unwrap();
        assert!(write_atomic(&doc, b"new").is_err());
        assert_eq!(fs::read(&doc).unwrap(), b"old");
        let _ = fs::remove_dir_all(dir);
    }

    #[cfg(unix)]
    #[test]
    fn keeps_permissions_and_symlinks() {
        use std::os::unix::fs::{symlink, PermissionsExt};
        let dir = temp_dir("links");
        let real = dir.join("real.txt");
        write_atomic(&real, b"a").unwrap();
        fs::set_permissions(&real, fs::Permissions::from_mode(0o600)).unwrap();
        let link = dir.join("link.txt");
        symlink(&real, &link).unwrap();
        write_atomic(&link, b"b").unwrap();
        assert!(fs::symlink_metadata(&link)
            .unwrap()
            .file_type()
            .is_symlink());
        assert_eq!(fs::read(&real).unwrap(), b"b");
        assert_eq!(
            fs::metadata(&real).unwrap().permissions().mode() & 0o777,
            0o600
        );
        let _ = fs::remove_dir_all(dir);
    }
}
