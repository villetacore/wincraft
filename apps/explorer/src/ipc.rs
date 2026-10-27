//! Single-instance shell: requests from later `explorer` invocations.
use std::{
    io::{BufRead, BufReader, Write},
    os::unix::net::{UnixListener, UnixStream},
    path::PathBuf,
    sync::mpsc::Sender,
};

#[derive(Clone, Debug, PartialEq, Eq)]
pub enum Request {
    /// Plain `explorer`: become the shell (or do nothing if one runs).
    Shell,
    StartMenu,
    /// A click elsewhere in the shell: close the Start menu if it is open.
    CloseStart,
    Run,
    Settings,
    /// Open a folder window or a file, given as typed (Windows or Unix spelling).
    Open(String),
}

impl Request {
    pub fn from_args(mut args: impl Iterator<Item = String>) -> Request {
        match args.next().as_deref() {
            None => Request::Shell,
            Some("--start") => Request::StartMenu,
            Some("--run") => Request::Run,
            Some("--settings") | Some("ms-settings:") => Request::Settings,
            Some(target) => Request::Open(target.to_owned()),
        }
    }

    fn encode(&self) -> String {
        match self {
            Request::Shell => "shell".into(),
            Request::StartMenu => "start".into(),
            Request::CloseStart => "close-start".into(),
            Request::Run => "run".into(),
            Request::Settings => "settings".into(),
            Request::Open(target) => format!("open {target}"),
        }
    }

    fn decode(line: &str) -> Option<Request> {
        Some(
            match line.split_once(' ').map_or((line, ""), |(a, b)| (a, b)) {
                ("shell", _) => Request::Shell,
                ("start", _) => Request::StartMenu,
                ("close-start", _) => Request::CloseStart,
                ("run", _) => Request::Run,
                ("settings", _) => Request::Settings,
                ("open", target) if !target.is_empty() => Request::Open(target.to_owned()),
                _ => return None,
            },
        )
    }
}

fn socket_path() -> PathBuf {
    std::env::var_os("XDG_RUNTIME_DIR")
        .map(PathBuf::from)
        .unwrap_or_else(std::env::temp_dir)
        .join("explorer.sock")
}

/// Hands the request to a running shell. Fails if there is none.
pub fn forward(request: &Request) -> std::io::Result<()> {
    let mut stream = UnixStream::connect(socket_path())?;
    writeln!(stream, "{}", request.encode())
}

/// Accepts requests in a background thread; `wake` asks the UI to repaint.
pub fn serve(sender: Sender<Request>, wake: impl Fn() + Send + 'static) -> std::io::Result<()> {
    let path = socket_path();
    let _ = std::fs::remove_file(&path); // stale socket of a crashed shell
    let listener = UnixListener::bind(&path)?;
    std::thread::spawn(move || {
        for stream in listener.incoming().flatten() {
            for line in BufReader::new(stream).lines().map_while(Result::ok) {
                if let Some(request) = Request::decode(line.trim()) {
                    if sender.send(request).is_err() {
                        return;
                    }
                    wake();
                }
            }
        }
    });
    Ok(())
}
