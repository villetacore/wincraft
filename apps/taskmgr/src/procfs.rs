//! Process and system statistics from /proc.
use std::{collections::HashMap, fs};

#[derive(Clone, Debug)]
pub struct Process {
    pub pid: u32,
    pub name: String,
    pub user: String,
    /// Percent of total CPU capacity since the previous sample.
    pub cpu: f32,
    pub memory: u64,
    /// Windows program running in the Win32 subsystem.
    pub windows: bool,
}

pub struct Sampler {
    previous: HashMap<u32, u64>,
    previous_total: u64,
    previous_idle: u64,
    users: HashMap<u32, String>,
    page_size: u64,
}

impl Default for Sampler {
    fn default() -> Self {
        let users = fs::read_to_string("/etc/passwd")
            .unwrap_or_default()
            .lines()
            .filter_map(|l| {
                let f: Vec<&str> = l.split(':').collect();
                Some((f.get(2)?.parse().ok()?, f.first()?.to_string()))
            })
            .collect();
        // SAFETY: sysconf has no preconditions.
        let page_size = unsafe { libc::sysconf(libc::_SC_PAGESIZE) }.max(4096) as u64;
        Self {
            previous: HashMap::new(),
            previous_total: 0,
            previous_idle: 0,
            users,
            page_size,
        }
    }
}

pub struct Snapshot {
    pub processes: Vec<Process>,
    /// Whole-system CPU use, percent.
    pub cpu: f32,
    pub memory_used: u64,
    pub memory_total: u64,
}

fn cpu_totals() -> (u64, u64) {
    let stat = fs::read_to_string("/proc/stat").unwrap_or_default();
    let values: Vec<u64> = stat
        .lines()
        .next()
        .unwrap_or_default()
        .split_whitespace()
        .skip(1)
        .filter_map(|v| v.parse().ok())
        .collect();
    let total = values.iter().sum();
    let idle = values.get(3).copied().unwrap_or(0) + values.get(4).copied().unwrap_or(0);
    (total, idle)
}

impl Sampler {
    pub fn sample(&mut self) -> Snapshot {
        let (total, idle) = cpu_totals();
        let elapsed = total.saturating_sub(self.previous_total).max(1);
        let busy = elapsed.saturating_sub(idle.saturating_sub(self.previous_idle));
        let cpu = busy as f32 * 100.0 / elapsed as f32;
        let mut current = HashMap::new();
        let mut processes = vec![];
        for entry in fs::read_dir("/proc").into_iter().flatten().flatten() {
            let Ok(pid) = entry.file_name().to_string_lossy().parse::<u32>() else {
                continue;
            };
            let Ok(stat) = fs::read_to_string(entry.path().join("stat")) else {
                continue;
            };
            // The command name is in parentheses and may itself contain spaces.
            let (Some(open), Some(close)) = (stat.find('('), stat.rfind(')')) else {
                continue;
            };
            let mut name = stat[open + 1..close].to_owned();
            let fields: Vec<&str> = stat[close + 2..].split_whitespace().collect();
            let ticks: u64 = fields.get(11).and_then(|v| v.parse().ok()).unwrap_or(0)
                + fields.get(12).and_then(|v| v.parse().ok()).unwrap_or(0);
            let rss: u64 = fields.get(21).and_then(|v| v.parse().ok()).unwrap_or(0);
            if rss == 0 {
                continue; // kernel threads
            }
            current.insert(pid, ticks);
            let delta = ticks.saturating_sub(self.previous.get(&pid).copied().unwrap_or(ticks));
            let uid: u32 = fs::read_to_string(entry.path().join("status"))
                .ok()
                .and_then(|s| {
                    s.lines()
                        .find(|l| l.starts_with("Uid:"))?
                        .split_whitespace()
                        .nth(1)?
                        .parse()
                        .ok()
                })
                .unwrap_or(0);
            // Wine processes show their Windows image name, like Windows itself would.
            let cmdline = fs::read(entry.path().join("cmdline")).unwrap_or_default();
            let first = cmdline.split(|&b| b == 0).next().unwrap_or_default();
            let first = String::from_utf8_lossy(first);
            let windows = first.to_ascii_lowercase().ends_with(".exe");
            if windows {
                name = first
                    .rsplit(['\\', '/'])
                    .next()
                    .unwrap_or(&first)
                    .to_owned();
            }
            processes.push(Process {
                pid,
                name,
                user: self
                    .users
                    .get(&uid)
                    .cloned()
                    .unwrap_or_else(|| uid.to_string()),
                cpu: delta as f32 * 100.0 / elapsed as f32,
                memory: rss * self.page_size,
                windows,
            });
        }
        self.previous = current;
        self.previous_total = total;
        self.previous_idle = idle;
        let meminfo = fs::read_to_string("/proc/meminfo").unwrap_or_default();
        let field = |key: &str| -> u64 {
            meminfo
                .lines()
                .find_map(|l| l.strip_prefix(key))
                .and_then(|v| v.split_whitespace().next()?.parse().ok())
                .unwrap_or(0)
                * 1024
        };
        let memory_total = field("MemTotal:");
        Snapshot {
            processes,
            cpu,
            memory_used: memory_total.saturating_sub(field("MemAvailable:")),
            memory_total,
        }
    }
}
