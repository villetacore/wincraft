//! Small OS helpers for the shell: local time, disk space, system information, power.
use std::{path::Path, time::SystemTime};

pub struct LocalTime {
    pub year: i32,
    pub month: u32,
    pub day: u32,
    pub hour: u32,
    pub minute: u32,
}

pub fn local_time(t: SystemTime) -> LocalTime {
    let secs = t
        .duration_since(SystemTime::UNIX_EPOCH)
        .map(|d| d.as_secs() as libc::time_t)
        .unwrap_or(0);
    // SAFETY: localtime_r writes only into the provided tm structure.
    let tm = unsafe {
        let mut tm: libc::tm = std::mem::zeroed();
        libc::localtime_r(&secs, &mut tm);
        tm
    };
    LocalTime {
        year: tm.tm_year + 1900,
        month: tm.tm_mon as u32 + 1,
        day: tm.tm_mday as u32,
        hour: tm.tm_hour as u32,
        minute: tm.tm_min as u32,
    }
}

impl LocalTime {
    pub fn date(&self) -> String {
        format!("{:02}.{:02}.{}", self.day, self.month, self.year)
    }
    pub fn time(&self) -> String {
        format!("{:02}:{:02}", self.hour, self.minute)
    }
}

/// (free, total) bytes of the file system holding `path`.
pub fn disk_space(path: &Path) -> Option<(u64, u64)> {
    let c = std::ffi::CString::new(path.as_os_str().as_encoded_bytes()).ok()?;
    // SAFETY: statvfs writes only into the provided structure.
    unsafe {
        let mut s: libc::statvfs = std::mem::zeroed();
        if libc::statvfs(c.as_ptr(), &mut s) != 0 {
            return None;
        }
        let unit = s.f_frsize as u64;
        Some((s.f_bavail as u64 * unit, s.f_blocks as u64 * unit))
    }
}

pub struct SystemInfo {
    pub os: String,
    pub kernel: String,
    pub cpu: String,
    pub memory: String,
    pub wine: String,
}

pub fn system_info() -> SystemInfo {
    let read = |p: &str| std::fs::read_to_string(p).unwrap_or_default();
    let os = read("/etc/os-release")
        .lines()
        .find_map(|l| l.strip_prefix("PRETTY_NAME="))
        .map(|v| v.trim_matches('"').to_owned())
        .unwrap_or_else(|| "WinCraft OS".into());
    let cpu = read("/proc/cpuinfo")
        .lines()
        .find_map(|l| {
            l.strip_prefix("model name")
                .and_then(|v| v.split_once(':'))
                .map(|(_, v)| v.trim().to_owned())
        })
        .unwrap_or_else(|| "Unknown processor".into());
    let cores = read("/proc/cpuinfo")
        .lines()
        .filter(|l| l.starts_with("processor"))
        .count();
    let mem_kb: u64 = read("/proc/meminfo")
        .lines()
        .find_map(|l| l.strip_prefix("MemTotal:"))
        .and_then(|v| v.split_whitespace().next()?.parse().ok())
        .unwrap_or(0);
    let wine = std::process::Command::new("wine")
        .arg("--version")
        .output()
        .ok()
        .map(|o| String::from_utf8_lossy(&o.stdout).trim().to_owned())
        .filter(|v| !v.is_empty())
        .unwrap_or_else(|| "not installed".into());
    SystemInfo {
        os,
        kernel: format!("Linux {}", read("/proc/sys/kernel/osrelease").trim()),
        cpu: format!("{cpu} ({cores} cores)"),
        memory: format!("{:.1} GB", mem_kb as f64 / 1024.0 / 1024.0),
        wine,
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Power {
    ShutDown,
    Restart,
    SignOut,
}

/// Asks PID 1 (power) or winlogon (sign out) through the control directory.
pub fn request_power(action: Power) -> std::io::Result<()> {
    let name = match action {
        Power::ShutDown => "poweroff",
        Power::Restart => "reboot",
        Power::SignOut => "logoff",
    };
    std::fs::write(Path::new("/run/wincraft/control").join(name), b"")
}
