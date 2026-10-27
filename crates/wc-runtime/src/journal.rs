//! Launch journal (APP-01): one line per event of every program started through the
//! launcher, so "it did not start" can be diagnosed afterwards. Plain text, append-only,
//! `key=value` fields separated by tabs; values are escaped and never span lines.
use std::{
    fs, io,
    io::Write,
    path::{Path, PathBuf},
    time::SystemTime,
};

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Event {
    Start,
    Exit,
    Error,
}

impl Event {
    fn name(self) -> &'static str {
        match self {
            Event::Start => "start",
            Event::Exit => "exit",
            Event::Error => "error",
        }
    }
}

#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct Record {
    pub program: String,
    pub args: Vec<String>,
    pub cwd: String,
    /// Runtime that ran it (`wine-11.0`), empty for native programs.
    pub runtime: String,
    pub pid: Option<u32>,
    /// Exit code, or `-signal` when the process was killed.
    pub status: Option<i32>,
    pub detail: String,
}

/// `~/.local/state/wincraft/launches.log` (XDG state directory).
pub fn path(home: &Path) -> PathBuf {
    home.join(".local/state/wincraft/launches.log")
}

fn escape(value: &str) -> String {
    let mut out = String::with_capacity(value.len());
    for c in value.chars() {
        match c {
            '\\' => out.push_str("\\\\"),
            '\t' => out.push_str("\\t"),
            '\n' => out.push_str("\\n"),
            '\r' => out.push_str("\\r"),
            c => out.push(c),
        }
    }
    out
}

/// One journal line (without the newline).
pub fn line(time: u64, event: Event, record: &Record) -> String {
    let mut fields = vec![
        format!("time={time}"),
        format!("event={}", event.name()),
        format!("program={}", escape(&record.program)),
    ];
    for (i, arg) in record.args.iter().enumerate() {
        fields.push(format!("arg{i}={}", escape(arg)));
    }
    if !record.cwd.is_empty() {
        fields.push(format!("cwd={}", escape(&record.cwd)));
    }
    if !record.runtime.is_empty() {
        fields.push(format!("runtime={}", escape(&record.runtime)));
    }
    if let Some(pid) = record.pid {
        fields.push(format!("pid={pid}"));
    }
    if let Some(status) = record.status {
        fields.push(format!("status={status}"));
    }
    if !record.detail.is_empty() {
        fields.push(format!("detail={}", escape(&record.detail)));
    }
    fields.join("\t")
}

/// Appends an event; the directory is created on first use.
pub fn append(journal: &Path, event: Event, record: &Record) -> io::Result<()> {
    if let Some(dir) = journal.parent() {
        fs::create_dir_all(dir)?;
    }
    let time = SystemTime::now()
        .duration_since(SystemTime::UNIX_EPOCH)
        .map(|d| d.as_secs())
        .unwrap_or(0);
    let mut file = fs::OpenOptions::new()
        .create(true)
        .append(true)
        .open(journal)?;
    writeln!(file, "{}", line(time, event, record))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn lines_are_single_line_and_complete() {
        let record = Record {
            program: "/apps/My App.exe".into(),
            args: vec!["--name=a\tb".into(), "c:\\new\nline".into()],
            cwd: "/apps".into(),
            runtime: "wine-11.0".into(),
            pid: Some(42),
            status: Some(-9),
            detail: String::new(),
        };
        let text = line(7, Event::Exit, &record);
        assert!(!text.contains('\n'));
        assert_eq!(text.split('\t').count(), 9);
        assert!(text.contains("program=/apps/My App.exe"));
        assert!(text.contains("arg0=--name=a\\tb"));
        assert!(text.contains("arg1=c:\\\\new\\nline"));
        assert!(text.contains("status=-9"));
        assert!(!text.contains("detail="));
    }

    #[test]
    fn appends_to_a_new_journal() {
        let home = std::env::temp_dir().join(format!("wc-journal-{}", std::process::id()));
        let _ = fs::remove_dir_all(&home);
        let journal = path(&home);
        append(
            &journal,
            Event::Start,
            &Record {
                program: "a.exe".into(),
                ..Default::default()
            },
        )
        .unwrap();
        append(
            &journal,
            Event::Error,
            &Record {
                program: "a.exe".into(),
                detail: "timeout".into(),
                ..Default::default()
            },
        )
        .unwrap();
        let text = fs::read_to_string(&journal).unwrap();
        assert_eq!(text.lines().count(), 2);
        assert!(text.lines().nth(1).unwrap().contains("event=error"));
        let _ = fs::remove_dir_all(home);
    }
}
