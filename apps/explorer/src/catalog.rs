//! What the Start menu lists: built-in apps, installed Windows programs (their Start Menu
//! shortcuts inside the Wine prefix) and portable programs on the Applications drive.
use std::path::{Path, PathBuf};
use wc_ui::Icon;

#[derive(Clone, Debug, PartialEq, Eq)]
pub enum Action {
    ThisPc,
    OpenPath(PathBuf),
    Command(&'static str, &'static [&'static str]),
    RunDialog,
    Settings,
    /// A Windows shortcut or program, opened through ShellExecute.
    Launch(PathBuf),
}

#[derive(Clone, Debug)]
pub struct App {
    pub name: String,
    pub icon: Icon,
    pub action: Action,
}

fn builtin(name: &str, icon: Icon, action: Action) -> App {
    App {
        name: name.into(),
        icon,
        action,
    }
}

/// Pinned tiles, in Start menu order.
pub fn pinned(ns: &wc_paths::Namespace) -> Vec<App> {
    use wc_paths::KnownFolder as K;
    vec![
        builtin(
            "WinCraft Browser",
            Icon::Browser,
            Action::Command("browser", &[]),
        ),
        builtin("File Explorer", Icon::Folder, Action::ThisPc),
        builtin(
            "Documents",
            Icon::FolderDocuments,
            Action::OpenPath(ns.known_folder(K::Documents)),
        ),
        builtin("Notepad", Icon::Notepad, Action::Command("notepad", &[])),
        builtin(
            "Terminal",
            Icon::Terminal,
            Action::Command("foot", &["--title", "Terminal"]),
        ),
        builtin(
            "Task Manager",
            Icon::TaskManager,
            Action::Command("taskmgr", &[]),
        ),
        builtin("Settings", Icon::Settings, Action::Settings),
        builtin(
            "Applications (D:)",
            Icon::Drive,
            Action::OpenPath(ns.apps.clone()),
        ),
        builtin(
            "Command Prompt",
            Icon::Terminal,
            Action::Command(
                "foot",
                &["--title", "Command Prompt", "start", "--wine", "cmd"],
            ),
        ),
        builtin(
            "Registry Editor",
            Icon::Wine,
            Action::Command("start", &["--wine", "regedit"]),
        ),
        builtin(
            "Wine configuration",
            Icon::Wine,
            Action::Command("start", &["--wine", "winecfg"]),
        ),
        builtin(
            "Downloads",
            Icon::FolderDownloads,
            Action::OpenPath(ns.known_folder(K::Downloads)),
        ),
        builtin("Run", Icon::Run, Action::RunDialog),
    ]
}

fn collect_shortcuts(dir: &Path, depth: usize, out: &mut Vec<App>) {
    let Ok(entries) = std::fs::read_dir(dir) else {
        return;
    };
    for entry in entries.flatten() {
        let path = entry.path();
        if path.is_dir() && depth < 4 {
            collect_shortcuts(&path, depth + 1, out);
        } else if path
            .extension()
            .is_some_and(|e| e.eq_ignore_ascii_case("lnk"))
        {
            let name = path
                .file_stem()
                .unwrap_or_default()
                .to_string_lossy()
                .into_owned();
            // Uninstallers clutter the list; Windows hides most of them too.
            if !name.to_lowercase().starts_with("uninstall") {
                out.push(App {
                    name,
                    icon: Icon::Application,
                    action: Action::Launch(path),
                });
            }
        }
    }
}

/// Everything for "All apps", sorted by name.
pub fn all_apps(ns: &wc_paths::Namespace) -> Vec<App> {
    let mut apps = pinned(ns);
    apps.retain(|a| !matches!(a.action, Action::OpenPath(_) | Action::RunDialog));
    let start_menus = [
        ns.drive_c()
            .join("ProgramData/Microsoft/Windows/Start Menu/Programs"),
        ns.drive_c().join(format!(
            "users/{}/AppData/Roaming/Microsoft/Windows/Start Menu/Programs",
            ns.user
        )),
    ];
    for dir in &start_menus {
        collect_shortcuts(dir, 0, &mut apps);
    }
    if let Ok(entries) = std::fs::read_dir(&ns.apps) {
        for path in entries.flatten().map(|e| e.path()) {
            if path
                .extension()
                .is_some_and(|e| e.eq_ignore_ascii_case("exe"))
            {
                let name = path
                    .file_stem()
                    .unwrap_or_default()
                    .to_string_lossy()
                    .into_owned();
                apps.push(App {
                    name,
                    icon: Icon::Application,
                    action: Action::Launch(path),
                });
            }
        }
    }
    apps.sort_by_key(|a| a.name.to_lowercase());
    apps.dedup_by(|a, b| a.name == b.name);
    apps
}
