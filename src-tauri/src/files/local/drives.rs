//! The drives of this device for the sidebar (spec 044 FR-002, research R6): on Linux the mounted
//! file systems that hold the user's files, skipping virtual ones; on Android the shared storage
//! and SD cards; on macOS `/` and `/Volumes`; on Windows the drive letters.

use std::path::{Path, PathBuf};

use serde::Serialize;
use ts_rs::TS;

/// A drive in the sidebar.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export, export_to = "../../src/types/bindings/")]
pub struct Drive {
    pub name: String,
    pub path: String,
    #[ts(type = "number | null")]
    pub total: Option<u64>,
    #[ts(type = "number | null")]
    pub free: Option<u64>,
}

/// File system types that never hold the user's files (research R6).
const VIRTUAL_TYPES: &[&str] = &[
    "proc",
    "sysfs",
    "devtmpfs",
    "devpts",
    "cgroup",
    "cgroup2",
    "securityfs",
    "debugfs",
    "tracefs",
    "configfs",
    "fusectl",
    "pstore",
    "bpf",
    "mqueue",
    "hugetlbfs",
    "autofs",
    "binfmt_misc",
    "efivarfs",
    "selinuxfs",
    "nsfs",
    "rpc_pipefs",
    "ramfs",
    "tmpfs",
    "overlay",
    "squashfs",
    "fuse.portal",
    "fuse.gvfsd-fuse",
];

/// One mount of `/proc/self/mountinfo`: its mount point and file system type.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Mount {
    pub point: PathBuf,
    pub fs_type: String,
}

/// The mounts of a `mountinfo` text: the mount point is field 5 (octal escapes such as `\040`
/// decoded), the type is the first field after ` - `.
pub fn parse_mountinfo(text: &str) -> Vec<Mount> {
    text.lines()
        .filter_map(|line| {
            let (left, right) = line.split_once(" - ")?;
            let point = left.split(' ').nth(4)?;
            let fs_type = right.split(' ').next()?;
            Some(Mount {
                point: PathBuf::from(unescape(point)),
                fs_type: fs_type.to_owned(),
            })
        })
        .collect()
}

/// `\040`, `\011`, `\012`, `\134` and other three-digit octal escapes of mountinfo.
fn unescape(field: &str) -> String {
    let bytes = field.as_bytes();
    let mut out = Vec::with_capacity(bytes.len());
    let mut i = 0;
    while i < bytes.len() {
        let octal = bytes
            .get(i + 1..i + 4)
            .filter(|digits| bytes[i] == b'\\' && digits.iter().all(|b| (b'0'..=b'7').contains(b)));
        match octal {
            Some(digits) => {
                let value = digits
                    .iter()
                    .fold(0u32, |acc, b| acc * 8 + u32::from(b - b'0'));
                out.push(u8::try_from(value).unwrap_or(b'?'));
                i += 4;
            }
            None => {
                out.push(bytes[i]);
                i += 1;
            }
        }
    }
    String::from_utf8_lossy(&out).into_owned()
}

/// Whether a Linux mount is a drive the user would look into: not virtual, and the root or a
/// removable/extra mount under the usual places.
pub fn is_user_drive(mount: &Mount) -> bool {
    if VIRTUAL_TYPES.contains(&mount.fs_type.as_str()) {
        return false;
    }
    let point = mount.point.as_path();
    point == Path::new("/")
        || ["/media", "/run/media", "/mnt", "/home"]
            .iter()
            .any(|base| point.starts_with(base))
}

/// The drives of this device.
pub fn drives() -> Vec<Drive> {
    let paths = drive_paths();
    paths
        .into_iter()
        .map(|path| {
            let (total, free) = space(&path).unzip();
            Drive {
                name: drive_name(&path),
                path: path.to_string_lossy().into_owned(),
                total,
                free,
            }
        })
        .collect()
}

fn drive_name(path: &Path) -> String {
    path.file_name()
        .map(|name| name.to_string_lossy().into_owned())
        .unwrap_or_else(|| path.to_string_lossy().into_owned())
}

#[cfg(target_os = "android")]
fn drive_paths() -> Vec<PathBuf> {
    let mut paths = vec![PathBuf::from("/storage/emulated/0")];
    // SD cards and USB drives appear as `/storage/XXXX-XXXX`.
    if let Ok(read) = std::fs::read_dir("/storage") {
        let mut cards: Vec<PathBuf> = read
            .filter_map(Result::ok)
            .map(|item| item.path())
            .filter(|path| {
                path.file_name()
                    .and_then(|n| n.to_str())
                    .is_some_and(is_volume_id)
            })
            .collect();
        cards.sort();
        paths.extend(cards);
    }
    paths
}

/// `XXXX-XXXX` in hex, the name Android gives an SD card or USB drive.
pub fn is_volume_id(name: &str) -> bool {
    let bytes = name.as_bytes();
    bytes.len() == 9
        && bytes[4] == b'-'
        && bytes
            .iter()
            .enumerate()
            .all(|(i, b)| i == 4 || b.is_ascii_hexdigit())
}

#[cfg(all(target_os = "linux", not(target_os = "android")))]
fn drive_paths() -> Vec<PathBuf> {
    let text = std::fs::read_to_string("/proc/self/mountinfo").unwrap_or_default();
    let mut seen = std::collections::BTreeSet::new();
    parse_mountinfo(&text)
        .into_iter()
        .filter(is_user_drive)
        .filter(|mount| mount.point != Path::new("/home"))
        .map(|mount| mount.point)
        .filter(|point| seen.insert(point.clone()))
        .collect()
}

#[cfg(target_os = "macos")]
fn drive_paths() -> Vec<PathBuf> {
    let mut paths = vec![PathBuf::from("/")];
    if let Ok(read) = std::fs::read_dir("/Volumes") {
        let mut volumes: Vec<PathBuf> = read
            .filter_map(Result::ok)
            .map(|item| item.path())
            // The boot volume appears in /Volumes as a link to `/`.
            .filter(|path| std::fs::canonicalize(path).map_or(true, |real| real != Path::new("/")))
            .collect();
        volumes.sort();
        paths.extend(volumes);
    }
    paths
}

#[cfg(windows)]
fn drive_paths() -> Vec<PathBuf> {
    (b'A'..=b'Z')
        .map(|letter| PathBuf::from(format!("{}:\\", letter as char)))
        .filter(|path| path.exists())
        .collect()
}

#[cfg(not(any(
    target_os = "android",
    target_os = "linux",
    target_os = "macos",
    windows
)))]
fn drive_paths() -> Vec<PathBuf> {
    vec![PathBuf::from("/")]
}

/// Total and free bytes of the file system holding `path`; `None` where unknown (Windows for now).
pub fn space(path: &Path) -> Option<(u64, u64)> {
    #[cfg(unix)]
    {
        use std::os::unix::ffi::OsStrExt;
        let c_path = std::ffi::CString::new(path.as_os_str().as_bytes()).ok()?;
        // SAFETY: `stats` is plain data the call fills; `c_path` is NUL-terminated and outlives it.
        let mut stats: libc::statvfs = unsafe { std::mem::zeroed() };
        if unsafe { libc::statvfs(c_path.as_ptr(), &mut stats) } != 0 {
            return None;
        }
        #[allow(clippy::unnecessary_cast)]
        let block = stats.f_frsize as u64;
        #[allow(clippy::unnecessary_cast)]
        Some((stats.f_blocks as u64 * block, stats.f_bavail as u64 * block))
    }
    #[cfg(not(unix))]
    {
        let _ = path;
        None
    }
}

#[cfg(test)]
#[path = "drives_tests.rs"]
mod tests;
