//! The Windows view of the file system that the shell shows to the user.
//!
//! Underneath is an ordinary Unix tree. The mapping follows Wine, so Windows programs and
//! the shell agree on every path:
//!
//! | Windows                     | Unix                                   |
//! |-----------------------------|----------------------------------------|
//! | `C:\`                       | `$WINEPREFIX/drive_c`                  |
//! | `C:\users\<user>\Documents` | `$HOME/Documents` (Wine links these)   |
//! | `D:\`                       | `/apps`, portable programs             |
//! | `Z:\`                       | `/`, the whole system                  |
//!
//! Lookups ignore case like Windows does; the real on-disk spelling is preserved.
mod atomic;
pub use atomic::write_atomic;

use std::{
    ffi::OsString,
    path::{Component, Path, PathBuf},
};

#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum KnownFolder {
    Profile,
    Desktop,
    Documents,
    Downloads,
    Music,
    Pictures,
    Videos,
}

impl KnownFolder {
    pub const USER_FOLDERS: [KnownFolder; 6] = [
        KnownFolder::Desktop,
        KnownFolder::Downloads,
        KnownFolder::Documents,
        KnownFolder::Pictures,
        KnownFolder::Music,
        KnownFolder::Videos,
    ];

    /// Directory name under the profile (also the Wine/XDG name).
    pub fn dir_name(self) -> &'static str {
        match self {
            KnownFolder::Profile => "",
            KnownFolder::Desktop => "Desktop",
            KnownFolder::Documents => "Documents",
            KnownFolder::Downloads => "Downloads",
            KnownFolder::Music => "Music",
            KnownFolder::Pictures => "Pictures",
            KnownFolder::Videos => "Videos",
        }
    }

    pub fn from_name(name: &str) -> Option<Self> {
        let all = [KnownFolder::Profile].into_iter().chain(Self::USER_FOLDERS);
        all.into_iter().find(|f| {
            f.dir_name().eq_ignore_ascii_case(name)
                || (*f == KnownFolder::Profile && name.eq_ignore_ascii_case("Profile"))
        })
    }
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Drive {
    pub letter: char,
    pub label: &'static str,
    pub root: PathBuf,
}

impl Drive {
    /// "Local Disk (C:)"
    pub fn display_name(&self) -> String {
        format!("{} ({}:)", self.label, self.letter)
    }
}

#[derive(Clone, Debug)]
pub struct Namespace {
    pub user: String,
    pub home: PathBuf,
    pub wine_prefix: PathBuf,
    pub apps: PathBuf,
}

impl Namespace {
    pub fn new(user: impl Into<String>, home: impl Into<PathBuf>) -> Self {
        let home = home.into();
        Self {
            user: user.into(),
            wine_prefix: home.join(".wine"),
            home,
            apps: PathBuf::from("/apps"),
        }
    }

    /// From `USER`, `HOME` and `WINEPREFIX`, as set up by winlogon.
    pub fn from_env() -> Self {
        let home = std::env::var_os("HOME")
            .map(PathBuf::from)
            .unwrap_or_else(|| "/".into());
        let user = std::env::var("USER").unwrap_or_else(|_| "user".into());
        let mut ns = Self::new(user, home);
        if let Some(prefix) = std::env::var_os("WINEPREFIX") {
            ns.wine_prefix = prefix.into();
        }
        ns
    }

    pub fn drive_c(&self) -> PathBuf {
        self.wine_prefix.join("drive_c")
    }

    pub fn drives(&self) -> Vec<Drive> {
        vec![
            Drive {
                letter: 'C',
                label: "Local Disk",
                root: self.drive_c(),
            },
            Drive {
                letter: 'D',
                label: "Applications",
                root: self.apps.clone(),
            },
            Drive {
                letter: 'Z',
                label: "WinCraft System",
                root: PathBuf::from("/"),
            },
        ]
    }

    pub fn known_folder(&self, folder: KnownFolder) -> PathBuf {
        match folder {
            KnownFolder::Profile => self.home.clone(),
            other => self.home.join(other.dir_name()),
        }
    }

    /// Windows profile directory as Windows programs see it: `C:\users\<user>`.
    fn profile_windows(&self) -> String {
        format!("C:\\users\\{}", self.user)
    }

    fn expand_variables(&self, input: &str) -> String {
        let profile = self.profile_windows();
        let vars: [(&str, &str); 6] = [
            ("%USERPROFILE%", &profile),
            ("%HOMEDRIVE%", "C:"),
            ("%SYSTEMDRIVE%", "C:"),
            ("%SYSTEMROOT%", "C:\\windows"),
            ("%WINDIR%", "C:\\windows"),
            ("%PROGRAMFILES%", "C:\\Program Files"),
        ];
        let mut out = input.to_owned();
        for (name, value) in vars {
            while let Some(at) = find_ignore_case(&out, name) {
                out.replace_range(at..at + name.len(), value);
            }
        }
        out
    }

    /// Resolves user input from the address bar or the Run dialog to a Unix path.
    /// Accepts `C:\...`, `%USERPROFILE%\...`, `shell:Documents`, `~/...` and Unix paths.
    /// Missing components are kept as typed, so the result may not exist.
    pub fn to_unix(&self, input: &str) -> Option<PathBuf> {
        let input = input.trim().trim_matches('"');
        if input.is_empty() {
            return None;
        }
        if let Some(name) = strip_prefix_ignore_case(input, "shell:") {
            return KnownFolder::from_name(name).map(|f| self.known_folder(f));
        }
        if input == "~" {
            return Some(self.home.clone());
        }
        if let Some(rest) = input.strip_prefix("~/") {
            return Some(self.home.join(rest));
        }
        if input.starts_with('/') {
            return Some(PathBuf::from(input));
        }
        let expanded = self.expand_variables(input);
        let bytes = expanded.as_bytes();
        if bytes.len() < 2 || bytes[1] != b':' || !bytes[0].is_ascii_alphabetic() {
            return None;
        }
        let letter = (bytes[0] as char).to_ascii_uppercase();
        let rest: Vec<&str> = expanded[2..]
            .split(['\\', '/'])
            .filter(|c| !c.is_empty() && *c != ".")
            .collect();
        // The profile and its known folders live in $HOME, not inside drive_c.
        if letter == 'C'
            && rest.len() >= 2
            && rest[0].eq_ignore_ascii_case("users")
            && rest[1].eq_ignore_ascii_case(&self.user)
        {
            return Some(resolve_case_insensitive(&self.home, &rest[2..]));
        }
        let drive = self.drives().into_iter().find(|d| d.letter == letter)?;
        Some(resolve_case_insensitive(&drive.root, &rest))
    }

    /// The Windows spelling of a Unix path, for the address bar and title bars.
    pub fn to_windows(&self, path: &Path) -> String {
        // drive_c lives inside $HOME/.wine, so it must win over the profile mapping.
        if let Ok(rest) = path.strip_prefix(self.drive_c()) {
            return join_windows("C:", rest);
        }
        if let Ok(rest) = path.strip_prefix(&self.home) {
            return join_windows(&self.profile_windows(), rest);
        }
        let mut best: Option<(Drive, usize)> = None;
        for drive in self.drives() {
            if path.starts_with(&drive.root) {
                let depth = drive.root.components().count();
                if best.as_ref().is_none_or(|(_, d)| depth > *d) {
                    best = Some((drive, depth));
                }
            }
        }
        match best {
            Some((drive, _)) => {
                let rest = path.strip_prefix(&drive.root).unwrap_or(path);
                join_windows(&format!("{}:", drive.letter), rest)
            }
            None => path.display().to_string(),
        }
    }

    /// Short name for a folder as Explorer shows it in titles and the navigation pane.
    pub fn display_name(&self, path: &Path) -> String {
        if path == self.home {
            return self.user.clone();
        }
        for drive in self.drives() {
            if path == drive.root {
                return drive.display_name();
            }
        }
        path.file_name()
            .map(|n| n.to_string_lossy().into_owned())
            .unwrap_or_else(|| self.to_windows(path))
    }
}

fn join_windows(base: &str, rest: &Path) -> String {
    let mut out = base.to_owned();
    for component in rest.components() {
        if let Component::Normal(part) = component {
            out.push('\\');
            out.push_str(&part.to_string_lossy());
        }
    }
    if out.len() == 2 {
        out.push('\\');
    }
    out
}

fn find_ignore_case(haystack: &str, needle: &str) -> Option<usize> {
    haystack
        .to_ascii_lowercase()
        .find(&needle.to_ascii_lowercase())
}

fn strip_prefix_ignore_case<'a>(s: &'a str, prefix: &str) -> Option<&'a str> {
    (s.len() >= prefix.len() && s[..prefix.len()].eq_ignore_ascii_case(prefix))
        .then(|| &s[prefix.len()..])
}

/// Appends `parts` to `base`, matching each existing directory entry without regard to case.
pub fn resolve_case_insensitive(base: &Path, parts: &[&str]) -> PathBuf {
    let mut path = base.to_path_buf();
    for (i, part) in parts.iter().enumerate() {
        if *part == ".." {
            path.pop();
            continue;
        }
        // Prefer the exact name, otherwise the on-disk spelling of a case-insensitive match
        // (also on case-insensitive host file systems, which would accept any spelling).
        let wanted = part.to_lowercase();
        let names: Vec<OsString> = std::fs::read_dir(&path)
            .map(|entries| entries.flatten().map(|e| e.file_name()).collect())
            .unwrap_or_default();
        let found = names
            .iter()
            .find(|n| n.as_os_str() == std::ffi::OsStr::new(part))
            .or_else(|| {
                names
                    .iter()
                    .find(|n| n.to_string_lossy().to_lowercase() == wanted)
            })
            .cloned();
        match found {
            Some(name) => path.push(name),
            None => {
                // Not on disk: keep the rest exactly as typed.
                for rest in &parts[i..] {
                    path.push(rest);
                }
                return path;
            }
        }
    }
    path
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::fs;

    fn sandbox() -> (PathBuf, Namespace) {
        let root = std::env::temp_dir().join(format!(
            "wc-paths-{}-{:?}",
            std::process::id(),
            std::thread::current().id()
        ));
        let _ = fs::remove_dir_all(&root);
        let home = root.join("home/user");
        fs::create_dir_all(home.join("Documents/Reports")).unwrap();
        fs::create_dir_all(home.join(".wine/drive_c/Program Files/Game")).unwrap();
        let mut ns = Namespace::new("user", &home);
        ns.apps = root.join("apps");
        fs::create_dir_all(&ns.apps).unwrap();
        (root, ns)
    }

    #[test]
    fn windows_paths_resolve_case_insensitively() {
        let (root, ns) = sandbox();
        assert_eq!(
            ns.to_unix(r"c:\PROGRAM FILES\game").unwrap(),
            ns.drive_c().join("Program Files/Game")
        );
        assert_eq!(
            ns.to_unix(r"C:\Users\USER\documents\reports").unwrap(),
            ns.home.join("Documents/Reports")
        );
        assert_eq!(
            ns.to_unix(r"%USERPROFILE%\Documents").unwrap(),
            ns.home.join("Documents")
        );
        assert_eq!(
            ns.to_unix("shell:documents").unwrap(),
            ns.home.join("Documents")
        );
        assert_eq!(ns.to_unix(r"D:\").unwrap(), ns.apps);
        #[cfg(unix)]
        assert_eq!(
            ns.to_unix(r"Z:\etc\hosts").unwrap(),
            PathBuf::from("/etc/hosts")
        );
        assert_eq!(
            ns.to_unix(r"C:\new\Folder").unwrap(),
            ns.drive_c().join("new/Folder")
        );
        assert_eq!(ns.to_unix("notepad"), None);
        fs::remove_dir_all(root).unwrap();
    }

    #[test]
    fn unix_paths_get_windows_names() {
        let (root, ns) = sandbox();
        assert_eq!(
            ns.to_windows(&ns.home.join("Documents/a.txt")),
            r"C:\users\user\Documents\a.txt"
        );
        assert_eq!(ns.to_windows(&ns.drive_c()), r"C:\");
        assert_eq!(ns.to_windows(&ns.drive_c().join("windows")), r"C:\windows");
        assert_eq!(ns.to_windows(&ns.apps.join("demo.exe")), r"D:\demo.exe");
        #[cfg(unix)]
        assert_eq!(ns.to_windows(Path::new("/usr/bin")), r"Z:\usr\bin");
        assert_eq!(ns.display_name(&ns.drive_c()), "Local Disk (C:)");
        assert_eq!(ns.display_name(&ns.home.join("Documents")), "Documents");
        fs::remove_dir_all(root).unwrap();
    }

    #[test]
    fn round_trip_through_both_spellings() {
        let (root, ns) = sandbox();
        for p in [
            ns.home.join("Documents/Reports"),
            ns.drive_c().join("Program Files/Game"),
        ] {
            assert_eq!(ns.to_unix(&ns.to_windows(&p)).unwrap(), p);
        }
        fs::remove_dir_all(root).unwrap();
    }
}
