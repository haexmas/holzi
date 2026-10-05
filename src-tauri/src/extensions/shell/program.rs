//! The programs a shell session can start (spec 017, US11, T110): found by name on `PATH` or by
//! path, always as a canonical path, the form a `shell` permission names.

use std::path::{Component, Path, PathBuf};

/// `program` as an existing file's canonical path: a bare name is looked up on `PATH`.
pub fn resolve_program(program: &str) -> Option<PathBuf> {
    let path = Path::new(program);
    let candidate = if path.is_absolute() {
        Some(path.to_path_buf())
    } else if path.components().count() == 1 {
        let path_var = std::env::var_os("PATH")?;
        std::env::split_paths(&path_var).find_map(|dir| {
            let names: Vec<String> = if cfg!(windows) && path.extension().is_none() {
                vec![format!("{program}.exe"), format!("{program}.cmd")]
            } else {
                vec![program.to_owned()]
            };
            names
                .into_iter()
                .map(|name| dir.join(name))
                .find(|p| p.is_file())
        })
    } else {
        None
    };
    std::fs::canonicalize(candidate?)
        .ok()
        .filter(|p| p.is_file())
}

/// An absolute `program` without `.` and `..`, taken by its text alone: the target a question
/// names for a program that is not there. `None` for a name that is not absolute.
pub(super) fn lexical_absolute(program: &str) -> Option<PathBuf> {
    let path = Path::new(program);
    if !path.is_absolute() {
        return None;
    }
    let mut clean = PathBuf::new();
    for component in path.components() {
        match component {
            Component::CurDir => {}
            Component::ParentDir => {
                clean.pop();
            }
            other => clean.push(other),
        }
    }
    Some(clean)
}

/// The program a session starts when the SDK names none: the user's shell.
pub(super) fn default_program() -> String {
    if cfg!(windows) {
        std::env::var("COMSPEC").unwrap_or_else(|_| "cmd.exe".to_owned())
    } else {
        std::env::var("SHELL").unwrap_or_else(|_| "/bin/sh".to_owned())
    }
}

pub(super) fn home() -> Option<PathBuf> {
    std::env::var_os(if cfg!(windows) { "USERPROFILE" } else { "HOME" }).map(PathBuf::from)
}

/// The shells of this device: `/etc/shells` and `$SHELL` on Unix, cmd and PowerShell on Windows.
pub(super) fn available() -> Vec<(String, PathBuf)> {
    let mut names: Vec<String> = if cfg!(windows) {
        vec![default_program(), "powershell".into(), "pwsh".into()]
    } else {
        let listed = std::fs::read_to_string("/etc/shells").unwrap_or_default();
        listed
            .lines()
            .map(str::trim)
            .filter(|l| l.starts_with('/'))
            .map(str::to_owned)
            .chain(std::iter::once(default_program()))
            .collect()
    };
    names.dedup();
    let mut seen = Vec::new();
    let mut shells = Vec::new();
    for name in names {
        let Some(path) = resolve_program(&name) else {
            continue;
        };
        if seen.contains(&path) {
            continue;
        }
        seen.push(path.clone());
        let label = Path::new(&name)
            .file_stem()
            .map_or_else(|| name.clone(), |s| s.to_string_lossy().into_owned());
        shells.push((label, path));
    }
    shells
}
