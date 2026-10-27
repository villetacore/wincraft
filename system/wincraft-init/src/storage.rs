//! Finds file systems by label without external tools.
use std::{
    fs::File,
    io::{Read, Seek, SeekFrom},
    path::PathBuf,
};

/// ext2/3/4 superblock: at byte 1024; magic at +56, volume label (16 bytes) at +120.
pub fn ext_label(device: &str) -> Option<String> {
    let mut file = File::open(device).ok()?;
    let mut sb = [0u8; 136];
    file.seek(SeekFrom::Start(1024)).ok()?;
    file.read_exact(&mut sb).ok()?;
    if u16::from_le_bytes([sb[56], sb[57]]) != 0xEF53 {
        return None;
    }
    let raw = &sb[120..136];
    let end = raw.iter().position(|&b| b == 0).unwrap_or(raw.len());
    Some(String::from_utf8_lossy(&raw[..end]).into_owned())
}

/// Block devices and partitions (whole disks first), excluding loop and RAM devices.
pub fn block_devices() -> Vec<PathBuf> {
    let mut names: Vec<String> = std::fs::read_dir("/sys/class/block")
        .map(|d| {
            d.flatten()
                .map(|e| e.file_name().to_string_lossy().into_owned())
                .collect()
        })
        .unwrap_or_default();
    names.retain(|n| !n.starts_with("loop") && !n.starts_with("ram") && !n.starts_with("sr"));
    names.sort();
    names
        .into_iter()
        .map(|n| PathBuf::from("/dev").join(n))
        .collect()
}

pub fn find_by_label(label: &str) -> Option<PathBuf> {
    block_devices()
        .into_iter()
        .find(|d| ext_label(&d.to_string_lossy()).is_some_and(|l| l == label))
}
