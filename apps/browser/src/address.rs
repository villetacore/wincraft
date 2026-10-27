//! What the address bar does with what the user typed, and download file names.
//! Pure functions, tested on the host.
use std::path::{Path, PathBuf};

/// Start page and search engine: no account, no tracking by default.
pub const HOME: &str = "https://duckduckgo.com/";
const SEARCH: &str = "https://duckduckgo.com/?q=";

fn percent_encode(text: &str) -> String {
    let mut out = String::new();
    for b in text.bytes() {
        match b {
            b'A'..=b'Z' | b'a'..=b'z' | b'0'..=b'9' | b'-' | b'_' | b'.' | b'~' => {
                out.push(b as char)
            }
            b' ' => out.push('+'),
            _ => out.push_str(&format!("%{b:02X}")),
        }
    }
    out
}

fn looks_like_host(text: &str) -> bool {
    let host = text.split(['/', '?', '#']).next().unwrap_or_default();
    let name = host
        .rsplit_once(':')
        .filter(|(_, port)| port.chars().all(|c| c.is_ascii_digit()))
        .map_or(host, |(h, _)| h);
    let labels: Vec<&str> = name.split('.').collect();
    let ipv4 = labels.len() == 4
        && labels
            .iter()
            .all(|l| !l.is_empty() && l.len() <= 3 && l.chars().all(|c| c.is_ascii_digit()));
    // A real top-level domain is alphabetic: "example.org", not "v1.2".
    let domain = labels.len() >= 2
        && labels
            .iter()
            .all(|l| !l.is_empty() && l.chars().all(|c| c.is_alphanumeric() || c == '-'))
        && labels
            .last()
            .is_some_and(|tld| tld.chars().count() >= 2 && tld.chars().all(char::is_alphabetic));
    name == "localhost" || ipv4 || domain
}

/// A URL to load for the address bar input: an address, a local path or a web search.
pub fn resolve(input: &str) -> String {
    let text = input.trim();
    if text.is_empty() {
        return HOME.into();
    }
    let lower = text.to_ascii_lowercase();
    if ["http://", "https://", "file://", "about:", "data:"]
        .iter()
        .any(|s| lower.starts_with(s))
    {
        return text.into();
    }
    if text.starts_with('/') {
        return format!("file://{text}");
    }
    if !text.contains(char::is_whitespace) && looks_like_host(text) {
        return format!("https://{text}");
    }
    format!("{SEARCH}{}", percent_encode(text))
}

/// A file name that is safe to create in the Downloads folder.
pub fn safe_file_name(suggested: &str) -> String {
    let base = suggested.rsplit(['/', '\\']).next().unwrap_or_default();
    let cleaned: String = base.chars().filter(|c| !c.is_control()).collect();
    let cleaned = cleaned.trim().trim_start_matches('.').to_owned();
    if cleaned.is_empty() {
        "download".into()
    } else {
        cleaned
    }
}

/// `dir/name`, or `dir/name (1).ext`, `(2)`, ... when a file of that name exists — the way
/// Windows browsers keep earlier downloads.
pub fn unique_path(dir: &Path, name: &str, exists: impl Fn(&Path) -> bool) -> PathBuf {
    let first = dir.join(name);
    if !exists(&first) {
        return first;
    }
    let (stem, ext) = match name.rsplit_once('.') {
        Some((stem, ext)) if !stem.is_empty() => (stem, format!(".{ext}")),
        _ => (name, String::new()),
    };
    (1..)
        .map(|n| dir.join(format!("{stem} ({n}){ext}")))
        .find(|p| !exists(p))
        .expect("an unused name exists")
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn addresses_paths_and_searches() {
        assert_eq!(resolve(""), HOME);
        assert_eq!(resolve("https://example.org/a"), "https://example.org/a");
        assert_eq!(resolve("example.org"), "https://example.org");
        assert_eq!(
            resolve("www.7-zip.org/download.html"),
            "https://www.7-zip.org/download.html"
        );
        assert_eq!(resolve("localhost:8080"), "https://localhost:8080");
        assert_eq!(
            resolve("/home/user/page.html"),
            "file:///home/user/page.html"
        );
        assert_eq!(
            resolve("free old games"),
            "https://duckduckgo.com/?q=free+old+games"
        );
        assert_eq!(resolve("c++ &"), "https://duckduckgo.com/?q=c%2B%2B+%26");
        assert_eq!(
            resolve("игры"),
            "https://duckduckgo.com/?q=%D0%B8%D0%B3%D1%80%D1%8B"
        );
        assert_eq!(
            resolve("v1.2"),
            "https://duckduckgo.com/?q=v1.2",
            "a version number is not an address"
        );
        assert_eq!(resolve("192.168.1.10/admin"), "https://192.168.1.10/admin");
        assert_eq!(resolve("hello"), "https://duckduckgo.com/?q=hello");
    }

    #[test]
    fn download_names() {
        assert_eq!(safe_file_name("7z2409-x64.exe"), "7z2409-x64.exe");
        assert_eq!(safe_file_name("../../etc/passwd"), "passwd");
        assert_eq!(safe_file_name("..\\..\\x.exe"), "x.exe");
        assert_eq!(safe_file_name(".hidden"), "hidden");
        assert_eq!(safe_file_name(""), "download");
        let taken = [
            PathBuf::from("/d/setup.exe"),
            PathBuf::from("/d/setup (1).exe"),
        ];
        assert_eq!(
            unique_path(Path::new("/d"), "setup.exe", |p| taken
                .iter()
                .any(|t| t == p)),
            PathBuf::from("/d/setup (2).exe")
        );
        assert_eq!(
            unique_path(Path::new("/d"), "new.exe", |p| taken.iter().any(|t| t == p)),
            PathBuf::from("/d/new.exe")
        );
        assert_eq!(
            unique_path(Path::new("/d"), "README", |_| false),
            PathBuf::from("/d/README")
        );
    }
}
