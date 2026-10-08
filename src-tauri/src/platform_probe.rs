//! Platform probe for the CI on Windows, macOS and Android (spec 044, T007, research R4).
//!
//! Built only with the feature `platform-probe` and active only when asked for: `HOLZI_PROBE=1` on
//! desktops, the system property `debug.holzi.probe=1` on Android (set with `adb shell setprop`).
//! It answers one question before any media code exists: may holzi's web view reach a server on
//! `127.0.0.1`? Chromium's Local Network Access could forbid that in WebView2 and Android WebView.
//!
//! The probe starts a tiny HTTP server on `127.0.0.1`, adds `http://127.0.0.1:*` to the
//! `connect-src` of holzi's document (the shipped policy has no loopback yet, T016), and hands the
//! port to the window. The window fetches `/__probe` and reports through IPC, which no network rule
//! can block, so a failed fetch still arrives with its reason. The probe prints one line starting
//! with [`RESULT_MARKER`] and ends the process: 0 when every step passed, 1 when one failed, 2 when
//! no report came within [`REPORT_TIMEOUT`].

use std::collections::VecDeque;
use std::io::{BufRead, BufReader, Write};
use std::net::{TcpListener, TcpStream};
use std::sync::atomic::{AtomicBool, Ordering};
use std::time::Duration;

use serde::Deserialize;
use serde_json::json;
use tauri::{AppHandle, Runtime};

/// Every result line starts with this, so the CI finds it in stdout or in logcat.
pub const RESULT_MARKER: &str = "HOLZI_PROBE_RESULT";

/// The route the window fetches.
pub const HEALTH_PATH: &str = "/__probe";

/// The body of a healthy answer.
pub const HEALTH_BODY: &str = "ok";

/// How long the probe waits for the window's report before it gives up.
pub const REPORT_TIMEOUT: Duration = Duration::from_secs(90);

const LOOPBACK_SOURCE: &str = "http://127.0.0.1:*";
const MAX_SEEN: usize = 200;

static REPORTED: AtomicBool = AtomicBool::new(false);

/// Every request line the health server received, so a failed fetch shows whether the web view
/// reached the server at all (and whether it sent a preflight).
static SEEN: std::sync::Mutex<VecDeque<String>> = std::sync::Mutex::new(VecDeque::new());

fn note_seen(line: String) {
    if let Ok(mut lines) = SEEN.lock() {
        if lines.len() == MAX_SEEN {
            lines.pop_front();
        }
        lines.push_back(line);
    }
}

fn seen() -> Vec<String> {
    SEEN.lock()
        .map(|lines| lines.iter().cloned().collect())
        .unwrap_or_default()
}

/// Whether this run was asked to probe.
pub fn requested() -> bool {
    #[cfg(target_os = "android")]
    {
        std::process::Command::new("getprop")
            .arg("debug.holzi.probe")
            .output()
            .map(|out| String::from_utf8_lossy(&out.stdout).trim() == "1")
            .unwrap_or(false)
    }
    #[cfg(not(target_os = "android"))]
    {
        std::env::var("HOLZI_PROBE").as_deref() == Ok("1")
    }
}

/// Starts the health server and the report watchdog; returns the port for the window.
pub fn start<R: Runtime>(app: &AppHandle<R>) -> std::io::Result<u16> {
    let listener = TcpListener::bind("127.0.0.1:0")?;
    let port = listener.local_addr()?.port();
    note(app, STARTED_FILE, &format!("port {port}"));
    std::thread::spawn(move || {
        for stream in listener.incoming().flatten() {
            note_seen(format!(
                "connect from {}",
                stream
                    .peer_addr()
                    .map_or_else(|_| "?".to_owned(), |a| a.to_string())
            ));
            std::thread::spawn(move || {
                if let Err(error) = serve(stream) {
                    log::warn!("platform probe: health request failed: {error}");
                }
            });
        }
    });
    // Whether the server is reachable in this process at all, apart from the web view.
    std::thread::spawn(move || {
        let reached = TcpStream::connect(("127.0.0.1", port)).and_then(|mut stream| {
            stream.write_all(b"GET /__probe?v=rust HTTP/1.1\r\nHost: 127.0.0.1\r\n\r\n")
        });
        if let Err(error) = reached {
            note_seen(format!("rust self-connect failed: {error}"));
        }
    });
    let app = app.clone();
    std::thread::spawn(move || {
        std::thread::sleep(REPORT_TIMEOUT);
        if !REPORTED.swap(true, Ordering::SeqCst) {
            let line = result_line(&json!({
                "ok": false,
                "steps": [{ "name": "report", "ok": false, "detail": "no report from the window" }],
                "serverSaw": seen(),
            }));
            emit(&app, &line);
            app.exit(2);
        }
    });
    Ok(port)
}

/// The script that tells the window where to look.
pub fn init_script(port: u16) -> String {
    format!("window.__HOLZI_PROBE__ = Object.freeze({{ port: {port} }});")
}

/// `csp` with [`LOOPBACK_SOURCE`] in its `connect-src` directive. A policy without one gets it with
/// the sources of `default-src`, so nothing else loses what it had.
pub fn with_loopback_connect(csp: &str) -> String {
    let mut directives: Vec<String> = csp
        .split(';')
        .map(str::trim)
        .filter(|d| !d.is_empty())
        .map(str::to_owned)
        .collect();
    let named = |directives: &[String], wanted: &str| {
        directives.iter().position(|directive| {
            directive
                .split_whitespace()
                .next()
                .is_some_and(|name| name.eq_ignore_ascii_case(wanted))
        })
    };
    if let Some(index) = named(&directives, "connect-src") {
        directives[index] = format!("{} {LOOPBACK_SOURCE}", directives[index]);
    } else {
        let fallback = named(&directives, "default-src")
            .map(|index| {
                directives[index]
                    .split_whitespace()
                    .skip(1)
                    .map(|source| format!("{source} "))
                    .collect::<String>()
            })
            .unwrap_or_default();
        directives.push(format!("connect-src {fallback}{LOOPBACK_SOURCE}"));
    }
    directives.join("; ")
}

/// The hook on holzi's web resources while probing: the document may fetch from the loopback.
pub fn allow_loopback(response: &mut tauri::http::Response<std::borrow::Cow<'static, [u8]>>) {
    // The diagnosis variants also load an image from the loopback (`img-src`).
    let headers = response.headers_mut();
    let Some(policy) = headers.get_mut("Content-Security-Policy") else {
        return;
    };
    let Ok(text) = policy.to_str() else {
        return;
    };
    let patched =
        with_loopback_connect(text).replacen("img-src ", &format!("img-src {LOOPBACK_SOURCE} "), 1);
    if let Ok(value) = tauri::http::HeaderValue::from_str(&patched) {
        *policy = value;
    }
}

/// One step the window checked.
#[derive(Debug, Deserialize, PartialEq)]
pub struct Step {
    pub name: String,
    pub ok: bool,
    #[serde(default)]
    pub detail: Option<String>,
}

/// What the window reports.
#[derive(Debug, Deserialize, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct Report {
    pub steps: Vec<Step>,
    #[serde(default)]
    pub user_agent: Option<String>,
}

/// The exit code for a report: 0 only when there is at least one step and every step passed.
pub fn exit_code(report: &Report) -> i32 {
    if !report.steps.is_empty() && report.steps.iter().all(|step| step.ok) {
        0
    } else {
        1
    }
}

/// The line the CI looks for.
pub fn result_line(value: &serde_json::Value) -> String {
    format!("{RESULT_MARKER} {value}")
}

/// The window's report: printed once, then the process ends with [`exit_code`].
#[tauri::command]
pub fn platform_probe_report<R: Runtime>(app: AppHandle<R>, report: Report) {
    if REPORTED.swap(true, Ordering::SeqCst) {
        return;
    }
    let code = exit_code(&report);
    let steps: Vec<_> = report
        .steps
        .iter()
        .map(|step| json!({ "name": step.name, "ok": step.ok, "detail": step.detail }))
        .collect();
    let line = result_line(&json!({
        "ok": code == 0,
        "steps": steps,
        "userAgent": report.user_agent,
        "platform": std::env::consts::OS,
        "serverSaw": seen(),
    }));
    emit(&app, &line);
    app.exit(code);
}

/// Files in holzi's cache directory that the Android job reads with `adb shell run-as`, because a
/// GUI app's stdout reaches no one there and logcat proved unreliable on the emulator.
pub const STARTED_FILE: &str = "platform-probe-started.txt";
pub const RESULT_FILE: &str = "platform-probe-result.txt";

fn emit<R: Runtime>(app: &AppHandle<R>, line: &str) {
    println!("{line}");
    let _ = std::io::stdout().flush();
    log::info!(target: "holzi-probe", "{line}");
    note(app, RESULT_FILE, line);
}

fn note<R: Runtime>(app: &AppHandle<R>, name: &str, content: &str) {
    use tauri::Manager;
    let Ok(dir) = app.path().app_cache_dir() else {
        return;
    };
    if std::fs::create_dir_all(&dir).is_ok() {
        if let Err(error) = std::fs::write(dir.join(name), format!("{content}\n")) {
            log::warn!("platform probe: could not write {name}: {error}");
        }
    }
}

/// Answers one request: the health route, its preflight, or 404.
fn serve(mut stream: TcpStream) -> std::io::Result<()> {
    stream.set_read_timeout(Some(Duration::from_secs(5)))?;
    let mut reader = BufReader::new(stream.try_clone()?);
    let mut request_line = String::new();
    reader.read_line(&mut request_line)?;
    note_seen(request_line.trim_end().to_owned());
    // Drain the headers; the probe needs none of them.
    loop {
        let mut header = String::new();
        if reader.read_line(&mut header)? == 0 || header == "\r\n" || header == "\n" {
            break;
        }
    }
    stream.write_all(response_for(&request_line).as_bytes())?;
    stream.flush()
}

/// The raw HTTP answer for a request line.
pub fn response_for(request_line: &str) -> String {
    let mut parts = request_line.split_whitespace();
    let method = parts.next().unwrap_or_default();
    let path = parts
        .next()
        .unwrap_or_default()
        .split('?')
        .next()
        .unwrap_or_default();
    let cors = "Access-Control-Allow-Origin: *\r\nAccess-Control-Allow-Headers: *\r\nAccess-Control-Allow-Methods: GET, OPTIONS\r\nAccess-Control-Allow-Private-Network: true\r\n";
    match (method, path) {
        ("OPTIONS", HEALTH_PATH) => {
            format!("HTTP/1.1 204 No Content\r\n{cors}Content-Length: 0\r\nConnection: close\r\n\r\n")
        }
        ("GET", HEALTH_PATH) => format!(
            "HTTP/1.1 200 OK\r\n{cors}Content-Type: text/plain\r\nCache-Control: no-store\r\nContent-Length: {}\r\nConnection: close\r\n\r\n{HEALTH_BODY}",
            HEALTH_BODY.len()
        ),
        _ => "HTTP/1.1 404 Not Found\r\nContent-Length: 0\r\nConnection: close\r\n\r\n".to_owned(),
    }
}

#[cfg(test)]
#[path = "platform_probe_tests.rs"]
mod tests;
