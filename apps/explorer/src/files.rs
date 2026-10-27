//! Directory listings and file operations, including the Recycle Bin
//! (stored as the freedesktop.org Trash, so Linux tools see the same bin).
use std::{
    fs, io,
    path::{Path, PathBuf},
    time::SystemTime,
};

#[derive(Clone, Debug)]
pub struct Entry {
    pub path: PathBuf,
    pub name: String,
    pub is_dir: bool,
    pub size: u64,
    pub modified: Option<SystemTime>,
    pub hidden: bool,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum SortKey {
    Name,
    Modified,
    Type,
    Size,
}

pub fn list(dir: &Path) -> io::Result<Vec<Entry>> {
    let mut entries: Vec<Entry> = fs::read_dir(dir)?
        .flatten()
        .map(|e| {
            let path = e.path();
            let meta = fs::metadata(&path).ok();
            let name = e.file_name().to_string_lossy().into_owned();
            Entry {
                hidden: name.starts_with('.'),
                is_dir: meta.as_ref().is_some_and(|m| m.is_dir()),
                size: meta.as_ref().map_or(0, |m| m.len()),
                modified: meta.and_then(|m| m.modified().ok()),
                name,
                path,
            }
        })
        .collect();
    sort(&mut entries, SortKey::Name, true);
    Ok(entries)
}

/// Folders first, then the chosen key; names compare case-insensitively like Windows.
pub fn sort(entries: &mut [Entry], key: SortKey, ascending: bool) {
    entries.sort_by(|a, b| {
        let by_key = match key {
            SortKey::Name => a.name.to_lowercase().cmp(&b.name.to_lowercase()),
            SortKey::Modified => a.modified.cmp(&b.modified),
            SortKey::Type => type_name(&a.name, a.is_dir).cmp(&type_name(&b.name, b.is_dir)),
            SortKey::Size => a.size.cmp(&b.size),
        };
        let by_key = if ascending { by_key } else { by_key.reverse() };
        b.is_dir.cmp(&a.is_dir).then(by_key)
    });
}

/// "Type" column text, by extension (contents are inspected only in Properties).
pub fn type_name(name: &str, is_dir: bool) -> String {
    if is_dir {
        return "File folder".into();
    }
    let ext = name
        .rsplit_once('.')
        .map(|(_, e)| e.to_ascii_lowercase())
        .unwrap_or_default();
    let known = match ext.as_str() {
        "exe" => "Application",
        "dll" => "Application extension",
        "msi" => "Windows Installer Package",
        "lnk" => "Shortcut",
        "txt" => "Text Document",
        "log" => "Log file",
        "ini" | "inf" | "cfg" | "conf" => "Configuration settings",
        "bat" | "cmd" => "Windows Batch File",
        "sh" => "Shell script",
        "md" => "Markdown document",
        "png" | "jpg" | "jpeg" | "gif" | "bmp" => "Image",
        "mp3" | "wav" | "ogg" | "flac" => "Audio",
        "mp4" | "mkv" | "avi" | "webm" => "Video",
        "zip" | "7z" | "rar" | "tar" | "gz" => "Compressed archive",
        "pdf" => "PDF Document",
        "" => "File",
        _ => return format!("{} File", ext.to_ascii_uppercase()),
    };
    known.into()
}

/// Creates "New folder", "New folder (2)", ... like Windows.
pub fn create_unique(dir: &Path, base: &str, ext: &str, folder: bool) -> io::Result<PathBuf> {
    for i in 1..1000 {
        let name = if i == 1 {
            format!("{base}{ext}")
        } else {
            format!("{base} ({i}){ext}")
        };
        let path = dir.join(name);
        if path.exists() {
            continue;
        }
        if folder {
            fs::create_dir(&path)?
        } else {
            fs::File::create_new(&path).map(|_| ())?
        }
        return Ok(path);
    }
    Err(io::Error::new(
        io::ErrorKind::AlreadyExists,
        "too many items with this name",
    ))
}

/// "Extract All": unpacks `zip` into a new folder named after it, next to it, and returns
/// the folder. busybox unzip refuses entries that would escape it (`..`, absolute paths).
pub fn extract_zip(zip: &Path) -> Result<PathBuf, String> {
    let dir = zip.parent().ok_or("the archive has no folder")?;
    let stem = zip
        .file_stem()
        .map(|s| s.to_string_lossy().into_owned())
        .unwrap_or_else(|| "Archive".into());
    let dest = create_unique(dir, &stem, "", true)
        .map_err(|e| format!("Cannot create a folder for the files: {e}"))?;
    let status = std::process::Command::new("unzip")
        .arg("-q")
        .arg(zip)
        .arg("-d")
        .arg(&dest)
        .status();
    match status {
        Ok(s) if s.success() => Ok(dest),
        result => {
            let _ = fs::remove_dir_all(&dest);
            Err(match result {
                Ok(s) => format!("The archive could not be extracted (unzip: {s})."),
                Err(e) => format!("The archive could not be extracted: {e}"),
            })
        }
    }
}

fn trash_dir() -> PathBuf {
    let home = std::env::var_os("HOME")
        .map(PathBuf::from)
        .unwrap_or_else(|| "/tmp".into());
    home.join(".local/share/Trash")
}

pub fn recycle_bin_files() -> PathBuf {
    trash_dir().join("files")
}

/// Moves across file systems when rename cannot (RAM overlay <-> data disk).
fn move_path(from: &Path, to: &Path) -> io::Result<()> {
    match fs::rename(from, to) {
        Ok(()) => Ok(()),
        Err(e) if e.raw_os_error() == Some(libc::EXDEV) => {
            copy_recursive(from, to)?;
            if from.is_dir() {
                fs::remove_dir_all(from)
            } else {
                fs::remove_file(from)
            }
        }
        Err(e) => Err(e),
    }
}

fn copy_recursive(from: &Path, to: &Path) -> io::Result<()> {
    if fs::symlink_metadata(from)?.is_dir() {
        fs::create_dir_all(to)?;
        for entry in fs::read_dir(from)? {
            let entry = entry?;
            copy_recursive(&entry.path(), &to.join(entry.file_name()))?;
        }
        Ok(())
    } else {
        fs::copy(from, to).map(|_| ())
    }
}

/// Delete key: the item goes to the Recycle Bin, restorable later.
pub fn recycle(path: &Path) -> io::Result<()> {
    let files = recycle_bin_files();
    let info = trash_dir().join("info");
    fs::create_dir_all(&files)?;
    fs::create_dir_all(&info)?;
    let name = path
        .file_name()
        .map(|n| n.to_string_lossy().into_owned())
        .unwrap_or_else(|| "item".into());
    let mut target_name = name.clone();
    let mut i = 2;
    while files.join(&target_name).exists()
        || info.join(format!("{target_name}.trashinfo")).exists()
    {
        target_name = format!("{name}.{i}");
        i += 1;
    }
    let now = crate::system::local_time(SystemTime::now());
    let stamp = format!(
        "{:04}-{:02}-{:02}T{:02}:{:02}:00",
        now.year, now.month, now.day, now.hour, now.minute
    );
    fs::write(
        info.join(format!("{target_name}.trashinfo")),
        format!(
            "[Trash Info]\nPath={}\nDeletionDate={stamp}\n",
            path.display()
        ),
    )?;
    move_path(path, &files.join(&target_name))
}

/// Original location of an item in the Recycle Bin.
pub fn original_location(trashed: &Path) -> Option<PathBuf> {
    let name = trashed.file_name()?.to_string_lossy().into_owned();
    let info =
        fs::read_to_string(trash_dir().join("info").join(format!("{name}.trashinfo"))).ok()?;
    info.lines()
        .find_map(|l| l.strip_prefix("Path="))
        .map(PathBuf::from)
}

pub fn restore(trashed: &Path) -> io::Result<PathBuf> {
    let target = original_location(trashed)
        .ok_or_else(|| io::Error::new(io::ErrorKind::NotFound, "original location unknown"))?;
    if target.exists() {
        return Err(io::Error::new(
            io::ErrorKind::AlreadyExists,
            format!("{} already exists", target.display()),
        ));
    }
    if let Some(parent) = target.parent() {
        fs::create_dir_all(parent)?;
    }
    move_path(trashed, &target)?;
    let name = trashed
        .file_name()
        .unwrap_or_default()
        .to_string_lossy()
        .into_owned();
    let _ = fs::remove_file(trash_dir().join("info").join(format!("{name}.trashinfo")));
    Ok(target)
}

/// Permanently deletes everything in the Recycle Bin (asked for explicitly by the user).
pub fn empty_recycle_bin() -> io::Result<()> {
    for sub in ["files", "info"] {
        let dir = trash_dir().join(sub);
        if dir.exists() {
            fs::remove_dir_all(&dir)?;
            fs::create_dir_all(&dir)?;
        }
    }
    Ok(())
}

pub fn format_time(t: Option<SystemTime>) -> String {
    t.map(|t| {
        let l = crate::system::local_time(t);
        format!("{} {}", l.date(), l.time())
    })
    .unwrap_or_default()
}
