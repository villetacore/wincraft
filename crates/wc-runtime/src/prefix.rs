//! Wine prefix readiness as an explicit state (FIX-01): preparing, ready or failed with a
//! reason. winlogon writes it while it prepares the prefix; launchers wait for "ready" with
//! a deadline and report "failed" instead of waiting forever for a marker file.
use std::{
    fs, io,
    path::{Path, PathBuf},
    time::{Duration, Instant},
};

#[derive(Clone, Debug, PartialEq, Eq)]
pub enum State {
    /// Preparation started (`since`: Unix time in seconds).
    Preparing { since: u64 },
    /// The prefix works with this runtime (see [`crate::runtime_id`]).
    Ready { runtime: String },
    /// Preparation failed; `reason` is shown to the user.
    Failed { reason: String },
}

impl State {
    /// One `key=value` per line; reasons are kept on one line.
    pub fn encode(&self) -> String {
        match self {
            State::Preparing { since } => format!("state=preparing\nsince={since}\n"),
            State::Ready { runtime } => format!("state=ready\nruntime={}\n", one_line(runtime)),
            State::Failed { reason } => format!("state=failed\nreason={}\n", one_line(reason)),
        }
    }

    pub fn decode(text: &str) -> Option<State> {
        let field = |key: &str| {
            text.lines()
                .find_map(|l| l.strip_prefix(key).and_then(|v| v.strip_prefix('=')))
                .map(str::to_owned)
        };
        Some(match field("state")?.as_str() {
            "preparing" => State::Preparing {
                since: field("since").and_then(|v| v.parse().ok()).unwrap_or(0),
            },
            "ready" => State::Ready {
                runtime: field("runtime").unwrap_or_default(),
            },
            "failed" => State::Failed {
                reason: field("reason").unwrap_or_else(|| "unknown error".into()),
            },
            _ => return None,
        })
    }
}

fn one_line(text: &str) -> String {
    text.split_whitespace().collect::<Vec<_>>().join(" ")
}

/// Where the session keeps the state of the default prefix (one per boot, in the runtime dir).
pub fn state_file(runtime_dir: &Path) -> PathBuf {
    runtime_dir.join("wine-prefix.state")
}

pub fn read(path: &Path) -> Option<State> {
    fs::read_to_string(path)
        .ok()
        .and_then(|t| State::decode(&t))
}

/// Replaces the state atomically: readers never see a half-written file.
pub fn write(path: &Path, state: &State) -> io::Result<()> {
    let partial = path.with_extension("state.tmp");
    fs::write(&partial, state.encode())?;
    fs::rename(&partial, path)
}

/// A prefix whose registry exists and is not truncated (a power cut can leave it empty).
pub fn is_valid(prefix: &Path) -> bool {
    let nonempty = |name: &str| fs::metadata(prefix.join(name)).is_ok_and(|m| m.len() > 0);
    nonempty("system.reg")
        && nonempty("user.reg")
        && fs::read_to_string(prefix.join("system.reg"))
            .is_ok_and(|s| s.starts_with("WINE REGISTRY"))
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub enum WaitError {
    Failed(String),
    Timeout(Duration),
}

impl std::fmt::Display for WaitError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            WaitError::Failed(reason) => write!(f, "The Windows environment could not be prepared: {reason}"),
            WaitError::Timeout(t) => write!(
                f,
                "The Windows environment is still being prepared after {} s. Try again in a moment.",
                t.as_secs()
            ),
        }
    }
}

/// Waits until the state is "ready" and returns the runtime, polling every `poll`.
/// A missing file counts as "preparing" (the session has not started preparation yet).
pub fn wait_ready(path: &Path, timeout: Duration, poll: Duration) -> Result<String, WaitError> {
    let deadline = Instant::now() + timeout;
    loop {
        match read(path) {
            Some(State::Ready { runtime }) => return Ok(runtime),
            Some(State::Failed { reason }) => return Err(WaitError::Failed(reason)),
            _ if Instant::now() >= deadline => return Err(WaitError::Timeout(timeout)),
            _ => std::thread::sleep(poll),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn temp_dir(name: &str) -> PathBuf {
        let dir = std::env::temp_dir().join(format!("wc-runtime-{name}-{}", std::process::id()));
        let _ = fs::remove_dir_all(&dir);
        fs::create_dir_all(&dir).unwrap();
        dir
    }

    #[test]
    fn states_round_trip() {
        for state in [
            State::Preparing { since: 42 },
            State::Ready {
                runtime: "wine-11.0".into(),
            },
            State::Failed {
                reason: "wineboot exited with status 1".into(),
            },
        ] {
            assert_eq!(State::decode(&state.encode()), Some(state));
        }
        // A multi-line reason cannot break the format.
        let failed = State::Failed {
            reason: "line one\nline two".into(),
        };
        assert_eq!(
            State::decode(&failed.encode()),
            Some(State::Failed {
                reason: "line one line two".into()
            })
        );
        assert_eq!(State::decode("state=bogus\n"), None);
        assert_eq!(State::decode(""), None);
    }

    #[test]
    fn waiting_reports_ready_failed_and_timeout() {
        let dir = temp_dir("wait");
        let path = state_file(&dir);
        let quick = Duration::from_millis(5);
        // No file yet: preparing until the deadline.
        assert_eq!(
            wait_ready(&path, Duration::from_millis(20), quick),
            Err(WaitError::Timeout(Duration::from_millis(20)))
        );
        write(&path, &State::Preparing { since: 1 }).unwrap();
        assert!(matches!(
            wait_ready(&path, Duration::from_millis(20), quick),
            Err(WaitError::Timeout(_))
        ));
        write(
            &path,
            &State::Failed {
                reason: "disk full".into(),
            },
        )
        .unwrap();
        assert_eq!(
            wait_ready(&path, Duration::from_secs(5), quick),
            Err(WaitError::Failed("disk full".into()))
        );
        write(
            &path,
            &State::Ready {
                runtime: "wine-11.0".into(),
            },
        )
        .unwrap();
        assert_eq!(
            wait_ready(&path, Duration::from_secs(5), quick),
            Ok("wine-11.0".into())
        );
        let _ = fs::remove_dir_all(dir);
    }

    #[test]
    fn registry_presence_is_not_enough() {
        let dir = temp_dir("valid");
        assert!(!is_valid(&dir));
        fs::write(dir.join("system.reg"), "").unwrap();
        fs::write(dir.join("user.reg"), "WINE REGISTRY Version 2").unwrap();
        assert!(!is_valid(&dir), "an empty system.reg is a damaged prefix");
        fs::write(dir.join("system.reg"), "garbage").unwrap();
        assert!(!is_valid(&dir));
        fs::write(dir.join("system.reg"), "WINE REGISTRY Version 2").unwrap();
        assert!(is_valid(&dir));
        let _ = fs::remove_dir_all(dir);
    }
}
