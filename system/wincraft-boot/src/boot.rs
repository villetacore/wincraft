use std::{
    ffi::CString,
    fs,
    io::{self, Read},
    os::unix::io::AsRawFd,
    path::Path,
    thread,
    time::{Duration, Instant},
};

const INIT: &str = "/usr/bin/wincraft-init";
const MEDIUM: &str = "/media/medium";
const LOWER: &str = "/media/lower";
const RW: &str = "/media/rw";
const NEWROOT: &str = "/newroot";

struct Cmdline {
    media: Option<String>,
    debug: bool,
    verify: bool,
}

fn cmdline() -> Cmdline {
    let raw = fs::read_to_string("/proc/cmdline").unwrap_or_default();
    let words: Vec<&str> = raw.split_whitespace().collect();
    Cmdline {
        media: words
            .iter()
            .find_map(|w| w.strip_prefix("wincraft.media="))
            .map(str::to_owned),
        debug: words.contains(&"wincraft.debug"),
        verify: words.contains(&"wincraft.verify"),
    }
}

fn cstr(s: &str) -> CString {
    CString::new(s).expect("no NUL in paths")
}

fn mount(
    source: &str,
    target: &str,
    fstype: &str,
    flags: libc::c_ulong,
    data: &str,
) -> io::Result<()> {
    let _ = fs::create_dir_all(target);
    let (source, target, fstype, data) = (cstr(source), cstr(target), cstr(fstype), cstr(data));
    // SAFETY: all pointers are valid NUL-terminated strings for the duration of the call.
    let rc = unsafe {
        libc::mount(
            source.as_ptr(),
            target.as_ptr(),
            fstype.as_ptr(),
            flags,
            data.as_ptr().cast(),
        )
    };
    if rc == 0 {
        Ok(())
    } else {
        Err(io::Error::last_os_error())
    }
}

fn move_mount(from: &str, to: &str) -> io::Result<()> {
    let _ = fs::create_dir_all(to);
    let (from, to) = (cstr(from), cstr(to));
    // SAFETY: valid strings; MS_MOVE ignores fstype and data.
    let rc = unsafe {
        libc::mount(
            from.as_ptr(),
            to.as_ptr(),
            std::ptr::null(),
            libc::MS_MOVE,
            std::ptr::null(),
        )
    };
    if rc == 0 {
        Ok(())
    } else {
        Err(io::Error::last_os_error())
    }
}

fn umount(target: &str) {
    let t = cstr(target);
    // SAFETY: valid string.
    unsafe { libc::umount2(t.as_ptr(), libc::MNT_DETACH) };
}

/// Block devices that can hold the live medium: CD/DVD, USB/SATA disks, virtio, NVMe.
fn candidate_devices() -> Vec<String> {
    let mut out: Vec<String> = fs::read_dir("/sys/class/block")
        .map(|d| {
            d.flatten()
                .map(|e| e.file_name().to_string_lossy().into_owned())
                .collect()
        })
        .unwrap_or_default();
    out.retain(|n| ["sr", "sd", "vd", "nvme"].iter().any(|p| n.starts_with(p)));
    out.sort_by_key(|n| (!n.starts_with("sr"), n.clone()));
    out.into_iter().map(|n| format!("/dev/{n}")).collect()
}

fn find_medium(media: Option<&str>) -> Option<String> {
    let deadline = Instant::now() + Duration::from_secs(30);
    while Instant::now() < deadline {
        for dev in candidate_devices() {
            if mount(&dev, MEDIUM, "iso9660", libc::MS_RDONLY, "").is_err() {
                continue;
            }
            let image = Path::new(MEDIUM).join("wincraft/rootfs.squashfs");
            let id = fs::read_to_string(Path::new(MEDIUM).join("wincraft/id")).unwrap_or_default();
            if image.is_file() && media.is_none_or(|m| id.trim() == m) {
                return Some(dev);
            }
            umount(MEDIUM);
        }
        thread::sleep(Duration::from_millis(500));
    }
    None
}

fn verify_image() -> Result<(), String> {
    use sha2::{Digest, Sha256};
    let sums = fs::read_to_string(Path::new(MEDIUM).join("wincraft/SHA256SUMS"))
        .map_err(|e| e.to_string())?;
    let expected = sums
        .split_whitespace()
        .next()
        .ok_or("empty SHA256SUMS")?
        .to_ascii_lowercase();
    let mut file = fs::File::open(Path::new(MEDIUM).join("wincraft/rootfs.squashfs"))
        .map_err(|e| e.to_string())?;
    let mut hasher = Sha256::new();
    let mut buf = vec![0u8; 1 << 20];
    loop {
        let n = file.read(&mut buf).map_err(|e| e.to_string())?;
        if n == 0 {
            break;
        }
        hasher.update(&buf[..n]);
    }
    let actual: String = hasher
        .finalize()
        .iter()
        .map(|b| format!("{b:02x}"))
        .collect();
    if actual == expected {
        Ok(())
    } else {
        Err(format!("checksum mismatch: {actual}"))
    }
}

// <linux/loop.h>
const LOOP_SET_FD: libc::Ioctl = 0x4C00;
const LOOP_CTL_GET_FREE: libc::Ioctl = 0x4C82;

/// Attaches `image` read-only to a free loop device and returns its path.
fn attach_loop(image: &Path) -> io::Result<String> {
    let control = fs::OpenOptions::new()
        .read(true)
        .write(true)
        .open("/dev/loop-control")?;
    // SAFETY: LOOP_CTL_GET_FREE takes no argument and returns a device number.
    let n = unsafe { libc::ioctl(control.as_raw_fd(), LOOP_CTL_GET_FREE) };
    if n < 0 {
        return Err(io::Error::last_os_error());
    }
    let device = format!("/dev/loop{n}");
    let backing = fs::File::open(image)?; // read-only fd => read-only loop device
    let loop_dev = fs::OpenOptions::new().read(true).open(&device)?;
    // SAFETY: LOOP_SET_FD takes the backing file descriptor by value.
    let rc = unsafe { libc::ioctl(loop_dev.as_raw_fd(), LOOP_SET_FD, backing.as_raw_fd()) };
    if rc < 0 {
        return Err(io::Error::last_os_error());
    }
    Ok(device)
}

fn fail(reason: &str, debug: bool) -> ! {
    println!("WINCRAFT_EARLY_FAILED: {reason}");
    if debug {
        println!("Debug boot: kernel log follows on the serial console; the machine waits.");
    } else {
        println!("Boot failed. Restart and choose the debug entry for details.");
    }
    loop {
        thread::sleep(Duration::from_secs(3600));
    }
}

fn switch_root() -> io::Result<()> {
    std::env::set_current_dir(NEWROOT)?;
    move_mount(".", "/")?;
    let dot = cstr(".");
    // SAFETY: valid string; we are PID 1 with CAP_SYS_CHROOT.
    if unsafe { libc::chroot(dot.as_ptr()) } != 0 {
        return Err(io::Error::last_os_error());
    }
    std::env::set_current_dir("/")?;
    let init = cstr(INIT);
    let argv = [init.as_ptr(), std::ptr::null()];
    // SAFETY: argv is NULL-terminated and outlives the call; execv only returns on error.
    unsafe { libc::execv(init.as_ptr(), argv.as_ptr()) };
    Err(io::Error::last_os_error())
}

pub fn main() {
    for (src, target, fstype) in [
        ("proc", "/proc", "proc"),
        ("sysfs", "/sys", "sysfs"),
        ("devtmpfs", "/dev", "devtmpfs"),
    ] {
        if let Err(e) = mount(src, target, fstype, libc::MS_NOSUID, "") {
            println!("WINCRAFT_EARLY_FAILED: mount {target}: {e}");
        }
    }
    let args = cmdline();
    println!(
        "WINCRAFT_EARLY media={} debug={}",
        args.media.as_deref().unwrap_or("any"),
        args.debug as u8
    );

    let Some(device) = find_medium(args.media.as_deref()) else {
        fail("live medium not found", args.debug);
    };
    println!("WINCRAFT_MEDIUM {device}");
    if args.verify {
        println!("Verifying system image checksum...");
        if let Err(e) = verify_image() {
            fail(&format!("system image {e}"), args.debug);
        }
        println!("WINCRAFT_MEDIA_VERIFIED");
    }
    let image = Path::new(MEDIUM).join("wincraft/rootfs.squashfs");
    let loop_dev =
        attach_loop(&image).unwrap_or_else(|e| fail(&format!("loop device: {e}"), args.debug));
    if let Err(e) = mount(&loop_dev, LOWER, "squashfs", libc::MS_RDONLY, "") {
        fail(&format!("mount system image: {e}"), args.debug);
    }
    if let Err(e) = mount("tmpfs", RW, "tmpfs", 0, "mode=0755") {
        fail(&format!("RAM layer: {e}"), args.debug);
    }
    let _ = fs::create_dir_all(format!("{RW}/upper"));
    let _ = fs::create_dir_all(format!("{RW}/work"));
    let options = format!("lowerdir={LOWER},upperdir={RW}/upper,workdir={RW}/work");
    if let Err(e) = mount("overlay", NEWROOT, "overlay", 0, &options) {
        fail(&format!("overlay root: {e}"), args.debug);
    }
    if !Path::new(NEWROOT)
        .join(INIT.trim_start_matches('/'))
        .is_file()
    {
        fail("system init missing in image", args.debug);
    }
    for (from, name) in [(MEDIUM, "medium"), (LOWER, "lower"), (RW, "rw")] {
        if let Err(e) = move_mount(from, &format!("{NEWROOT}/media/wincraft-{name}")) {
            println!("WINCRAFT_WARN move {from}: {e}");
        }
    }
    let _ = move_mount("/dev", &format!("{NEWROOT}/dev"));
    umount("/proc");
    umount("/sys");
    println!("WINCRAFT_EARLY_HANDOVER");
    if let Err(e) = switch_root() {
        fail(&format!("switch_root: {e}"), args.debug);
    }
}
