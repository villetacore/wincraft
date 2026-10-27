//! The session account and privilege dropping (no setuid helpers or shell wrappers).
use std::{fs, io, os::unix::process::CommandExt, path::PathBuf, process::Command};

#[derive(Clone, Debug)]
pub struct Account {
    pub name: String,
    pub uid: u32,
    pub gid: u32,
    pub home: PathBuf,
    pub shell: String,
    pub groups: Vec<u32>,
}

impl Account {
    /// Looks the account up in /etc/passwd and its supplementary groups in /etc/group.
    pub fn lookup(name: &str) -> io::Result<Account> {
        let passwd = fs::read_to_string("/etc/passwd")?;
        let fields: Vec<&str> = passwd
            .lines()
            .map(|l| l.split(':').collect::<Vec<_>>())
            .find(|f| f.first() == Some(&name) && f.len() >= 7)
            .ok_or_else(|| io::Error::new(io::ErrorKind::NotFound, format!("no account {name}")))?;
        let parse = |s: &str| {
            s.parse::<u32>()
                .map_err(|e| io::Error::new(io::ErrorKind::InvalidData, e))
        };
        let (uid, gid) = (parse(fields[2])?, parse(fields[3])?);
        let mut groups = vec![gid];
        for line in fs::read_to_string("/etc/group").unwrap_or_default().lines() {
            let g: Vec<&str> = line.split(':').collect();
            if g.len() >= 4 && g[3].split(',').any(|m| m == name) {
                if let Ok(id) = g[2].parse() {
                    groups.push(id);
                }
            }
        }
        Ok(Account {
            name: name.into(),
            uid,
            gid,
            home: fields[5].into(),
            shell: fields[6].into(),
            groups,
        })
    }

    fn base_environment(&self, runtime_dir: &str) -> Vec<(String, String)> {
        let home = self.home.display().to_string();
        vec![
            (
                "PATH".into(),
                "/usr/local/bin:/usr/bin:/bin:/usr/sbin:/sbin".into(),
            ),
            ("HOME".into(), home),
            ("USER".into(), self.name.clone()),
            ("LOGNAME".into(), self.name.clone()),
            ("SHELL".into(), self.shell.clone()),
            ("LANG".into(), "C.UTF-8".into()),
            ("XDG_RUNTIME_DIR".into(), runtime_dir.into()),
            ("XDG_SESSION_TYPE".into(), "wayland".into()),
            (
                "XDG_CURRENT_DESKTOP".into(),
                "WinCraft:labwc:wlroots".into(),
            ),
            // Keyboard layouts: English and Russian, Alt+Shift switches (as on Windows).
            ("XKB_DEFAULT_LAYOUT".into(), "us,ru".into()),
            ("XKB_DEFAULT_OPTIONS".into(), "grp:alt_shift_toggle".into()),
            ("XCURSOR_THEME".into(), "whiteglass".into()),
            ("XCURSOR_SIZE".into(), "24".into()),
        ]
    }

    /// Environment of the compositor (labwc on wlroots, with seatd for the devices).
    pub fn compositor_environment(&self, runtime_dir: &str) -> Vec<(String, String)> {
        let mut env = self.base_environment(runtime_dir);
        env.extend([
            ("LIBSEAT_BACKEND".into(), "seatd".into()),
            // QEMU's display adapters have no 3D: render on the CPU; clients still use
            // Mesa (llvmpipe) through EGL. GPU rendering comes with GPU-01.
            ("WLR_RENDERER".into(), "pixman".into()),
            ("WLR_NO_HARDWARE_CURSORS".into(), "1".into()),
        ]);
        env
    }

    /// Environment of the desktop session: the compositor's sockets and the Win32 prefix.
    pub fn environment(
        &self,
        runtime_dir: &str,
        sockets: &[(String, String)],
    ) -> Vec<(String, String)> {
        let home = self.home.display().to_string();
        let mut env = self.base_environment(runtime_dir);
        env.extend(sockets.iter().cloned());
        env.extend([
            ("WINEPREFIX".into(), format!("{home}/.wine")),
            ("WINEDEBUG".into(), "-all".into()),
            // WinCraft shows Windows shortcuts (.lnk) itself; Wine's menu builder would add
            // Linux .desktop duplicates of every installed program to the desktop.
            ("WINEDLLOVERRIDES".into(), "winemenubuilder.exe=d".into()),
            ("WINCRAFT_GUEST".into(), "1".into()),
            // Toolkits that can do both prefer Wayland and fall back to XWayland.
            ("GDK_BACKEND".into(), "wayland,x11".into()),
            ("QT_QPA_PLATFORM".into(), "wayland;xcb".into()),
            // No 3D device in QEMU: go straight to llvmpipe instead of probing GPU drivers
            // (removed with GPU-01).
            ("LIBGL_ALWAYS_SOFTWARE".into(), "1".into()),
        ]);
        env
    }

    /// Configures `command` to run as this account with a clean environment.
    pub fn apply(&self, command: &mut Command, env: &[(String, String)]) {
        command
            .env_clear()
            .envs(env.iter().cloned())
            .current_dir(&self.home);
        let (uid, gid, groups) = (self.uid, self.gid, self.groups.clone());
        // SAFETY: only async-signal-safe libc calls between fork and exec. Group, gid and
        // uid are dropped in that order; std's own uid/gid handling would run before this
        // closure and make setgroups fail, so it is not used.
        unsafe {
            command.pre_exec(move || {
                if libc::setgroups(groups.len(), groups.as_ptr()) != 0
                    || libc::setgid(gid) != 0
                    || libc::setuid(uid) != 0
                {
                    return Err(io::Error::last_os_error());
                }
                Ok(())
            });
        }
    }
}
