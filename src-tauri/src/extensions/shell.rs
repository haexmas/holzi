//! Shells of extensions (spec 017, US11, T110, FR-057–FR-059, research R21): the SDK's
//! `client.shell` on a PTY (`portable-pty`, Unix PTYs and Windows ConPTY).
//!
//! Every program needs a `shell` permission for its canonical path (`execute`); the question in
//! holzi's window warns that the extension can then do anything the user can on this device. A
//! session belongs to the extension that started it: only it writes, resizes or closes it, and its
//! output and its end reach only that extension's frames (`shell:output`, `shell:exit`). A session
//! ends with the extension's last frame, when it is disabled or removed, and with the vault: the
//! whole process group, registered with the vault's [`ChildRegistry`]. Mobile devices have no
//! shell (8001).

use std::collections::HashMap;
use std::io::{Read, Write};
use std::path::{Path, PathBuf};
use std::sync::{Arc, Mutex, MutexGuard, OnceLock, PoisonError};

use serde_json::{json, Value};
use uuid::Uuid;

use crate::extensions::bridge::dispatch::{CallContext, Emit};
use crate::extensions::bridge::events::emit_to_frames;
use crate::extensions::error::{BridgeError, ExtensionErrorCode};
use crate::extensions::host::ExtensionHost;
use crate::extensions::permissions::store::candidates;
use crate::extensions::permissions::{
    evaluate, Action, Decision, PermissionKind, PermissionRequest, RequestTarget,
};
use crate::vault_gate::{ChildGuard, ChildRegistry};

pub const MODULE: &str = module_path!();

pub const OUTPUT: &str = "shell:output";
pub const EXIT: &str = "shell:exit";

/// Bytes of one `write`.
pub const MAX_WRITE_BYTES: usize = 64 * 1024;
/// Columns and rows of a terminal.
pub const MAX_SIZE: u16 = 1000;
/// Environment variables of one session, and the bytes of each name and value.
pub const MAX_ENV: usize = 128;
pub const MAX_ENV_BYTES: usize = 4096;
/// Sessions one extension may have open at a time.
pub const MAX_SESSIONS: usize = 16;

fn invalid(message: &str) -> BridgeError {
    BridgeError::new(ExtensionErrorCode::Validation, message)
}

fn shell_error(message: impl Into<String>) -> BridgeError {
    BridgeError::new(ExtensionErrorCode::Shell, message)
}

fn not_found() -> BridgeError {
    BridgeError::new(ExtensionErrorCode::NotFound, "not found")
}

/// Turns PTY output into text: a character split across two reads stays whole, an invalid byte
/// becomes U+FFFD.
#[derive(Default)]
pub struct Utf8Stream {
    pending: Vec<u8>,
}

impl Utf8Stream {
    /// The text of `bytes` that is complete so far; an unfinished character waits for the next.
    pub fn push(&mut self, bytes: &[u8]) -> String {
        self.pending.extend_from_slice(bytes);
        let mut text = String::new();
        let mut rest: &[u8] = &self.pending;
        loop {
            match std::str::from_utf8(rest) {
                Ok(valid) => {
                    text.push_str(valid);
                    rest = &[];
                    break;
                }
                Err(error) => {
                    let (valid, after) = rest.split_at(error.valid_up_to());
                    // `from_utf8` checked this prefix.
                    text.push_str(std::str::from_utf8(valid).unwrap_or_default());
                    match error.error_len() {
                        Some(bad) => {
                            text.push(char::REPLACEMENT_CHARACTER);
                            rest = &after[bad..];
                        }
                        None => {
                            rest = after;
                            break;
                        }
                    }
                }
            }
        }
        self.pending = rest.to_vec();
        text
    }
}

/// One running shell.
struct Session {
    extension_id: Uuid,
    master: Box<dyn portable_pty::MasterPty + Send>,
    writer: Box<dyn Write + Send>,
    pid: Option<u32>,
    /// Keeps the process group registered with the vault while it runs.
    _child: Option<ChildGuard>,
}

/// How long a shell has to end its jobs after the hangup before its group is killed.
const HANGUP_GRACE: std::time::Duration = std::time::Duration::from_millis(500);

impl Session {
    /// Ends the shell as a closed terminal does: a hangup, which an interactive shell passes on to
    /// all its jobs (each in a process group of its own), then, after [`HANGUP_GRACE`], a kill of
    /// its group for whatever is left.
    fn end(&self) {
        let Some(pid) = self.pid else {
            return;
        };
        #[cfg(unix)]
        if let Some(group) = i32::try_from(pid).ok().filter(|p| *p > 1) {
            // SAFETY: FFI call with a plain group id above 1 (never every process or our own
            // group) and a signal constant.
            unsafe {
                libc::kill(-group, libc::SIGHUP);
            }
        }
        let _ = std::thread::Builder::new()
            .name("extension-shell-end".into())
            .spawn(move || {
                std::thread::sleep(HANGUP_GRACE);
                crate::vault_gate::kill_process_tree(pid);
            });
    }
}

/// The shells of this process, by session id.
#[derive(Default)]
pub struct ShellState {
    sessions: Mutex<HashMap<String, Session>>,
    children: OnceLock<ChildRegistry>,
}

impl ShellState {
    fn lock(&self) -> MutexGuard<'_, HashMap<String, Session>> {
        self.sessions.lock().unwrap_or_else(PoisonError::into_inner)
    }

    /// The vault's registry of child processes; set once, at start.
    pub fn set_children(&self, children: ChildRegistry) {
        let _ = self.children.set(children);
    }

    /// Ends every shell of `extension_id` (its last frame closed, disabled, removed).
    pub fn end_all(&self, extension_id: Uuid) {
        let ended: Vec<Session> = {
            let mut sessions = self.lock();
            let ids: Vec<String> = sessions
                .iter()
                .filter(|(_, s)| s.extension_id == extension_id)
                .map(|(id, _)| id.clone())
                .collect();
            ids.iter().filter_map(|id| sessions.remove(id)).collect()
        };
        for session in ended {
            session.end();
        }
    }

    /// How many shells `extension_id` has open.
    pub fn count(&self, extension_id: Uuid) -> usize {
        self.lock()
            .values()
            .filter(|s| s.extension_id == extension_id)
            .count()
    }
}

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

/// The program a session starts when the SDK names none: the user's shell.
fn default_program() -> String {
    if cfg!(windows) {
        std::env::var("COMSPEC").unwrap_or_else(|_| "cmd.exe".to_owned())
    } else {
        std::env::var("SHELL").unwrap_or_else(|_| "/bin/sh".to_owned())
    }
}

fn home() -> Option<PathBuf> {
    std::env::var_os(if cfg!(windows) { "USERPROFILE" } else { "HOME" }).map(PathBuf::from)
}

/// The shells of this device: `/etc/shells` and `$SHELL` on Unix, cmd and PowerShell on Windows.
fn available() -> Vec<(String, PathBuf)> {
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

fn check_program(ctx: &CallContext, program: &Path) -> Result<(), BridgeError> {
    let (extension_id, device) = (ctx.session.extension_id, ctx.device);
    let mut grants = ctx
        .db
        .read_blocking(move |q| {
            candidates(q, extension_id, PermissionKind::Shell, device).map_err(Into::into)
        })
        .map_err(|_| BridgeError::new(ExtensionErrorCode::Database, "database unavailable"))?;
    grants.extend(
        ctx.host
            .permissions
            .temporary(extension_id, PermissionKind::Shell),
    );
    let request = PermissionRequest {
        kind: PermissionKind::Shell,
        action: Action::Execute,
        target: RequestTarget::Program(program.to_path_buf()),
    };
    let code = match evaluate(&grants, &request, device) {
        Decision::Allow => return Ok(()),
        Decision::Deny => ExtensionErrorCode::PermissionDenied,
        Decision::Prompt => ExtensionErrorCode::PermissionPromptRequired,
    };
    Err(
        BridgeError::new(code, "permission required").with_details(json!({
            "resourceType": "shell",
            "action": "execute",
            "target": program.to_string_lossy(),
        })),
    )
}

fn size(value: Option<&Value>, default: u16) -> Result<u16, BridgeError> {
    match value {
        None | Some(Value::Null) => Ok(default),
        Some(v) => v
            .as_u64()
            .and_then(|n| u16::try_from(n).ok())
            .filter(|n| (1..=MAX_SIZE).contains(n))
            .ok_or_else(|| invalid("cols and rows must be numbers from 1 to 1000")),
    }
}

fn environment(value: Option<&Value>) -> Result<Vec<(String, String)>, BridgeError> {
    let Some(map) = value.filter(|v| !v.is_null()) else {
        return Ok(Vec::new());
    };
    let map = map
        .as_object()
        .filter(|m| m.len() <= MAX_ENV)
        .ok_or_else(|| invalid("env must be an object of at most 128 strings"))?;
    map.iter()
        .map(|(name, value)| {
            let value = value
                .as_str()
                .ok_or_else(|| invalid("env values must be strings"))?;
            let fits = |s: &str| s.len() <= MAX_ENV_BYTES && !s.contains('\0');
            if name.is_empty() || name.contains('=') || !fits(name) || !fits(value) {
                return Err(invalid("env name or value not allowed"));
            }
            Ok((name.clone(), value.to_owned()))
        })
        .collect()
}

/// Reads a session's output until it ends, then reports its exit code.
fn pump(
    mut reader: Box<dyn Read + Send>,
    mut child: Box<dyn portable_pty::Child + Send + Sync>,
    host: Arc<ExtensionHost>,
    emitter: Arc<dyn Emit>,
    extension_id: Uuid,
    session_id: String,
) {
    let mut stream = Utf8Stream::default();
    let mut buffer = [0u8; 8192];
    loop {
        match reader.read(&mut buffer) {
            Ok(0) | Err(_) => break,
            Ok(n) => {
                let data = stream.push(&buffer[..n]);
                if !data.is_empty() {
                    emit_to_frames(
                        &*emitter,
                        &host,
                        extension_id,
                        OUTPUT,
                        &json!({ "sessionId": session_id, "data": data }),
                    );
                }
            }
        }
    }
    let exit_code = child.wait().ok().map(|status| status.exit_code());
    host.shells.lock().remove(&session_id);
    emit_to_frames(
        &*emitter,
        &host,
        extension_id,
        EXIT,
        &json!({ "sessionId": session_id, "exitCode": exit_code }),
    );
}

fn desktop_only() -> Result<(), BridgeError> {
    if cfg!(desktop) {
        Ok(())
    } else {
        Err(BridgeError::not_available())
    }
}

/// `extension_shell_list_available` → `[{name, path}]`; needs no permission.
pub fn list_available(_ctx: &CallContext, _params: &Value) -> Result<Value, BridgeError> {
    desktop_only()?;
    Ok(Value::Array(
        available()
            .into_iter()
            .map(|(name, path)| json!({ "name": name, "path": path.to_string_lossy() }))
            .collect(),
    ))
}

/// `extension_shell_create {options: {shell?, cwd?, cols?, rows?, env?}}` → `{sessionId, shellName}`.
pub fn create(ctx: &CallContext, params: &Value) -> Result<Value, BridgeError> {
    desktop_only()?;
    let options = match params.get("options") {
        None | Some(Value::Null) => json!({}),
        Some(o) if o.is_object() => o.clone(),
        Some(_) => return Err(invalid("options must be an object")),
    };
    let named = match options.get("shell") {
        None | Some(Value::Null) => default_program(),
        Some(Value::String(s)) if !s.is_empty() => s.clone(),
        Some(_) => return Err(invalid("shell must be a string")),
    };
    let program = resolve_program(&named).ok_or_else(|| shell_error("program not found"))?;
    let cwd = match options.get("cwd") {
        None | Some(Value::Null) => home(),
        Some(Value::String(dir)) => Some(
            std::fs::canonicalize(dir)
                .ok()
                .filter(|d| d.is_dir())
                .ok_or_else(|| shell_error("working directory not found"))?,
        ),
        Some(_) => return Err(invalid("cwd must be a string")),
    };
    let cols = size(options.get("cols"), 80)?;
    let rows = size(options.get("rows"), 24)?;
    let env = environment(options.get("env"))?;
    check_program(ctx, &program)?;
    let extension_id = ctx.session.extension_id;
    if ctx.host.shells.count(extension_id) >= MAX_SESSIONS {
        return Err(BridgeError::new(
            ExtensionErrorCode::LimitExceeded,
            "too many shells",
        ));
    }

    let pair = portable_pty::native_pty_system()
        .openpty(portable_pty::PtySize {
            rows,
            cols,
            pixel_width: 0,
            pixel_height: 0,
        })
        .map_err(|_| shell_error("no terminal available"))?;
    let mut command = portable_pty::CommandBuilder::new(&program);
    if let Some(cwd) = cwd {
        command.cwd(cwd);
    }
    for (name, value) in env {
        command.env(name, value);
    }
    let child = pair
        .slave
        .spawn_command(command)
        .map_err(|_| shell_error("the program did not start"))?;
    drop(pair.slave);
    let pid = child.process_id();
    let guard = pid.and_then(|pid| ctx.host.shells.children.get().map(|c| c.register(pid)));
    let (reader, writer) = match (pair.master.try_clone_reader(), pair.master.take_writer()) {
        (Ok(reader), Ok(writer)) => (reader, writer),
        _ => {
            if let Some(pid) = pid {
                crate::vault_gate::kill_process_tree(pid);
            }
            return Err(shell_error("no terminal available"));
        }
    };
    let session_id = Uuid::new_v4().to_string();
    ctx.host.shells.lock().insert(
        session_id.clone(),
        Session {
            extension_id,
            master: pair.master,
            writer,
            pid,
            _child: guard,
        },
    );
    let (host, emitter, id) = (
        Arc::clone(&ctx.host),
        Arc::clone(&ctx.emitter),
        session_id.clone(),
    );
    std::thread::Builder::new()
        .name("extension-shell".into())
        .spawn(move || pump(reader, child, host, emitter, extension_id, id))
        .map_err(|_| shell_error("the program did not start"))?;
    let shell_name = program
        .file_stem()
        .map_or_else(String::new, |s| s.to_string_lossy().into_owned());
    Ok(json!({ "sessionId": session_id, "shellName": shell_name }))
}

fn session_id(params: &Value) -> Result<String, BridgeError> {
    params
        .get("sessionId")
        .and_then(Value::as_str)
        .map(str::to_owned)
        .ok_or_else(|| invalid("sessionId must be a string"))
}

/// Runs `f` on a session of the calling extension; another extension's is not found (FR-062).
fn with_own<T>(
    ctx: &CallContext,
    params: &Value,
    f: impl FnOnce(&mut Session) -> Result<T, BridgeError>,
) -> Result<T, BridgeError> {
    desktop_only()?;
    let id = session_id(params)?;
    let mut sessions = ctx.host.shells.lock();
    match sessions.get_mut(&id) {
        Some(session) if session.extension_id == ctx.session.extension_id => f(session),
        _ => Err(not_found()),
    }
}

/// `extension_shell_write {sessionId, data}`.
pub fn write(ctx: &CallContext, params: &Value) -> Result<Value, BridgeError> {
    let data = params
        .get("data")
        .and_then(Value::as_str)
        .ok_or_else(|| invalid("data must be a string"))?;
    if data.len() > MAX_WRITE_BYTES {
        return Err(BridgeError::new(
            ExtensionErrorCode::LimitExceeded,
            "data too large",
        ));
    }
    with_own(ctx, params, |session| {
        session
            .writer
            .write_all(data.as_bytes())
            .and_then(|()| session.writer.flush())
            .map_err(|_| shell_error("the shell has ended"))
    })?;
    Ok(Value::Null)
}

/// `extension_shell_resize {sessionId, cols, rows}`.
pub fn resize(ctx: &CallContext, params: &Value) -> Result<Value, BridgeError> {
    let cols = size(params.get("cols"), 0)?;
    let rows = size(params.get("rows"), 0)?;
    with_own(ctx, params, |session| {
        session
            .master
            .resize(portable_pty::PtySize {
                rows,
                cols,
                pixel_width: 0,
                pixel_height: 0,
            })
            .map_err(|_| shell_error("the shell has ended"))
    })?;
    Ok(Value::Null)
}

/// `extension_shell_close {sessionId}`: ends the process group; `shell:exit` follows.
pub fn close(ctx: &CallContext, params: &Value) -> Result<Value, BridgeError> {
    desktop_only()?;
    let id = session_id(params)?;
    let session = {
        let mut sessions = ctx.host.shells.lock();
        match sessions.get(&id) {
            Some(s) if s.extension_id == ctx.session.extension_id => sessions.remove(&id),
            _ => None,
        }
    };
    session.ok_or_else(not_found)?.end();
    Ok(Value::Null)
}

#[cfg(all(test, unix))]
#[path = "shell_tests.rs"]
mod tests;
