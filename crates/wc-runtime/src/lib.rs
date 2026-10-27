//! The Win32 runtime as a managed platform (FIX-01, APP-01, groundwork for APP-03/04/10).
//!
//! Windows API itself is Wine's job (ADR-003); WinCraft manages it: whether the prefix is
//! ready, which runtime build prepared it, and what happened to every launch.
//!
//! - [`prefix`]: preparing/ready/failed state with a reason and a bounded wait;
//! - [`journal`]: the launch journal (`~/.local/state/wincraft/launches.log`);
//! - [`runtime_id`] / [`prefix_runtime`]: which Wine build a prefix belongs to, so a new
//!   build updates the prefix (`wineboot --update`) instead of running on a stale one.
pub mod journal;
pub mod prefix;

use std::{fs, io, path::Path};

/// Runtime id from `wine --version` output ("wine-11.0", "wine-11.0 (Staging)" -> "wine-11.0").
pub fn runtime_id(version_output: &str) -> Option<String> {
    let first = version_output.split_whitespace().next()?;
    first.starts_with("wine-").then(|| first.to_owned())
}

const STAMP: &str = ".wincraft-runtime";

/// The runtime a prefix was last prepared with (None for prefixes from before 0.4).
pub fn prefix_runtime(prefix: &Path) -> Option<String> {
    fs::read_to_string(prefix.join(STAMP))
        .ok()
        .map(|s| s.trim().to_owned())
        .filter(|s| !s.is_empty())
}

pub fn set_prefix_runtime(prefix: &Path, runtime: &str) -> io::Result<()> {
    fs::write(prefix.join(STAMP), format!("{runtime}\n"))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn runtime_ids() {
        assert_eq!(runtime_id("wine-11.0\n"), Some("wine-11.0".into()));
        assert_eq!(runtime_id("wine-11.0 (Staging)"), Some("wine-11.0".into()));
        assert_eq!(runtime_id(""), None);
        assert_eq!(runtime_id("bash: wine: not found"), None);
    }

    #[test]
    fn prefix_stamp() {
        let dir = std::env::temp_dir().join(format!("wc-stamp-{}", std::process::id()));
        let _ = fs::remove_dir_all(&dir);
        fs::create_dir_all(&dir).unwrap();
        assert_eq!(prefix_runtime(&dir), None);
        set_prefix_runtime(&dir, "wine-11.0").unwrap();
        assert_eq!(prefix_runtime(&dir), Some("wine-11.0".into()));
        let _ = fs::remove_dir_all(dir);
    }
}
