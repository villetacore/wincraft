//! Classifies files by their content, the way the shell decides how to open them.
//!
//! Only headers are inspected (at most [`HEADER_LEN`] bytes); nothing is executed.
//! Every offset read from the file is bounds-checked: malformed input yields
//! [`Format::Unknown`] or a weaker classification, never a panic.
use std::{fs::File, io::Read, path::Path};

/// How many leading bytes [`classify`] needs to recognise every supported format.
pub const HEADER_LEN: usize = 4096;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Arch {
    X86,
    X64,
    Arm64,
    Other(u16),
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Subsystem {
    Gui,
    Console,
    Native,
    Other(u16),
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub enum Format {
    /// Win32/Win64 Portable Executable.
    Pe {
        arch: Arch,
        subsystem: Subsystem,
        dll: bool,
        /// .NET assembly (has a CLR runtime header).
        dotnet: bool,
    },
    /// 16-bit Windows "New Executable".
    Ne,
    /// Plain DOS MZ executable without a Windows header.
    Dos,
    /// Windows Installer package or other OLE compound document.
    CompoundDocument,
    /// Windows shell link (.lnk).
    Shortcut,
    /// Native Linux program.
    Elf {
        bits64: bool,
    },
    /// Text file starting with `#!`.
    Script {
        interpreter: String,
    },
    Unknown,
}

impl Format {
    /// Human-readable type, as Explorer shows it in the "Type" column.
    pub fn description(&self) -> &'static str {
        match self {
            Format::Pe { dll: true, .. } => "Application extension",
            Format::Pe {
                arch: Arch::X86, ..
            } => "Application (32-bit)",
            Format::Pe { .. } => "Application",
            Format::Ne => "16-bit Windows application",
            Format::Dos => "DOS application",
            Format::CompoundDocument => "Windows Installer package",
            Format::Shortcut => "Shortcut",
            Format::Elf { .. } => "Linux program",
            Format::Script { .. } => "Script",
            Format::Unknown => "File",
        }
    }

    pub fn is_windows_program(&self) -> bool {
        matches!(
            self,
            Format::Pe { dll: false, .. } | Format::Ne | Format::Dos
        )
    }
}

fn u16_at(b: &[u8], at: usize) -> Option<u16> {
    Some(u16::from_le_bytes(b.get(at..at + 2)?.try_into().ok()?))
}

fn u32_at(b: &[u8], at: usize) -> Option<u32> {
    Some(u32::from_le_bytes(b.get(at..at + 4)?.try_into().ok()?))
}

const OLE_MAGIC: [u8; 8] = [0xD0, 0xCF, 0x11, 0xE0, 0xA1, 0xB1, 0x1A, 0xE1];
const LNK_MAGIC: [u8; 20] = [
    0x4C, 0, 0, 0, 0x01, 0x14, 0x02, 0, 0, 0, 0, 0, 0xC0, 0, 0, 0, 0, 0, 0, 0x46,
];

/// Classifies a file from its first bytes (pass up to [`HEADER_LEN`]).
pub fn classify(header: &[u8]) -> Format {
    if header.starts_with(b"\x7fELF") {
        return Format::Elf {
            bits64: header.get(4) == Some(&2),
        };
    }
    if header.starts_with(&OLE_MAGIC) {
        return Format::CompoundDocument;
    }
    if header.starts_with(&LNK_MAGIC) {
        return Format::Shortcut;
    }
    if header.starts_with(b"#!") {
        let line = header[2..]
            .split(|&c| c == b'\n')
            .next()
            .unwrap_or_default();
        let interpreter = String::from_utf8_lossy(line).trim().to_owned();
        return Format::Script { interpreter };
    }
    if header.starts_with(b"MZ") {
        return classify_mz(header);
    }
    Format::Unknown
}

fn classify_mz(b: &[u8]) -> Format {
    let Some(lfanew) = u32_at(b, 0x3C).map(|v| v as usize) else {
        return Format::Dos;
    };
    if !(0x40..=HEADER_LEN - 4).contains(&lfanew) {
        return Format::Dos;
    }
    match b.get(lfanew..lfanew + 4) {
        Some(sig) if sig == b"PE\0\0" => classify_pe(b, lfanew).unwrap_or(Format::Dos),
        Some(sig) if &sig[..2] == b"NE" => Format::Ne,
        _ => Format::Dos,
    }
}

fn classify_pe(b: &[u8], pe: usize) -> Option<Format> {
    let coff = pe + 4;
    let arch = match u16_at(b, coff)? {
        0x14c => Arch::X86,
        0x8664 => Arch::X64,
        0xAA64 => Arch::Arm64,
        other => Arch::Other(other),
    };
    let optional_size = u16_at(b, coff + 16)? as usize;
    let characteristics = u16_at(b, coff + 18)?;
    let optional = coff + 20;
    let magic = u16_at(b, optional)?;
    let subsystem = match u16_at(b, optional + 68)? {
        1 => Subsystem::Native,
        2 => Subsystem::Gui,
        3 => Subsystem::Console,
        other => Subsystem::Other(other),
    };
    // Data directory 14 is the CLR runtime header; its offset differs for PE32 / PE32+.
    let directories = optional + if magic == 0x20B { 112 } else { 96 };
    let clr = directories + 14 * 8;
    let dotnet = clr + 8 <= optional + optional_size && u32_at(b, clr).is_some_and(|rva| rva != 0);
    Some(Format::Pe {
        arch,
        subsystem,
        dll: characteristics & 0x2000 != 0,
        dotnet,
    })
}

/// Reads the header of `path` and classifies it.
pub fn classify_file(path: &Path) -> std::io::Result<Format> {
    let mut header = Vec::with_capacity(HEADER_LEN);
    File::open(path)?
        .take(HEADER_LEN as u64)
        .read_to_end(&mut header)?;
    Ok(classify(&header))
}

#[cfg(test)]
mod tests {
    use super::*;

    fn pe(machine: u16, magic: u16, subsystem: u16, characteristics: u16) -> Vec<u8> {
        let mut b = vec![0u8; 1024];
        b[..2].copy_from_slice(b"MZ");
        b[0x3C..0x40].copy_from_slice(&0x80u32.to_le_bytes());
        b[0x80..0x84].copy_from_slice(b"PE\0\0");
        b[0x84..0x86].copy_from_slice(&machine.to_le_bytes());
        let optional_size: u16 = if magic == 0x20B { 240 } else { 224 };
        b[0x94..0x96].copy_from_slice(&optional_size.to_le_bytes());
        b[0x96..0x98].copy_from_slice(&characteristics.to_le_bytes());
        b[0x98..0x9A].copy_from_slice(&magic.to_le_bytes());
        b[0x98 + 68..0x98 + 70].copy_from_slice(&subsystem.to_le_bytes());
        b
    }

    #[test]
    fn recognises_gui_and_console_pe() {
        assert_eq!(
            classify(&pe(0x8664, 0x20B, 2, 0x22)),
            Format::Pe {
                arch: Arch::X64,
                subsystem: Subsystem::Gui,
                dll: false,
                dotnet: false
            }
        );
        assert_eq!(
            classify(&pe(0x14c, 0x10B, 3, 0x102)),
            Format::Pe {
                arch: Arch::X86,
                subsystem: Subsystem::Console,
                dll: false,
                dotnet: false
            }
        );
    }

    #[test]
    fn recognises_dll_and_dotnet() {
        let mut b = pe(0x14c, 0x10B, 2, 0x2102);
        let clr = 0x98 + 96 + 14 * 8;
        b[clr..clr + 4].copy_from_slice(&0x2008u32.to_le_bytes());
        assert!(matches!(
            classify(&b),
            Format::Pe {
                dll: true,
                dotnet: true,
                ..
            }
        ));
    }

    #[test]
    fn malformed_headers_never_panic() {
        assert_eq!(classify(b"MZ"), Format::Dos);
        let mut b = pe(0x8664, 0x20B, 2, 0);
        b[0x3C..0x40].copy_from_slice(&u32::MAX.to_le_bytes());
        assert_eq!(classify(&b), Format::Dos);
        b[0x3C..0x40].copy_from_slice(&0x80u32.to_le_bytes());
        assert_eq!(classify(&b[..0x90]), Format::Dos, "truncated COFF header");
        for len in 0..b.len() {
            let _ = classify(&b[..len]);
        }
    }

    #[test]
    fn recognises_other_formats() {
        assert_eq!(classify(b"\x7fELF\x02\x01"), Format::Elf { bits64: true });
        assert_eq!(
            classify(b"#!/bin/sh -e\necho"),
            Format::Script {
                interpreter: "/bin/sh -e".into()
            }
        );
        assert_eq!(classify(&OLE_MAGIC), Format::CompoundDocument);
        assert_eq!(classify(&LNK_MAGIC), Format::Shortcut);
        let mut ne = vec![0u8; 0x100];
        ne[..2].copy_from_slice(b"MZ");
        ne[0x3C..0x40].copy_from_slice(&0x80u32.to_le_bytes());
        ne[0x80..0x82].copy_from_slice(b"NE");
        assert_eq!(classify(&ne), Format::Ne);
        assert_eq!(classify(b"hello"), Format::Unknown);
    }
}
