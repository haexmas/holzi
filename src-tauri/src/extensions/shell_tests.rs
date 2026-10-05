//! Shells of extensions (spec 017, US11, T109) through the bridge, on a real PTY.

use std::path::Path;
use std::time::{Duration, Instant};

use super::*;
use crate::extensions::bridge::dispatch::call;
use crate::extensions::bridge::events::FRAME_EVENT;
use crate::extensions::commands::permissions::{set, PermissionSetArgs};
use crate::extensions::registry::effective::effective_bundle;
use crate::extensions::registry::install::install;
use crate::passwords::test_support::open_test_vault;
use crate::storage::known_devices;
use crate::vault_gate::{VaultDb, VaultGate};

#[derive(Default)]
struct Recorded(Mutex<Vec<(String, Value)>>);

impl Emit for Recorded {
    fn emit(&self, event: &str, payload: Value) {
        self.0.lock().unwrap().push((event.to_owned(), payload));
    }
}

impl Recorded {
    /// The `data` of every frame event of `kind` for `session`.
    fn of(&self, kind: &str, session: &str) -> Vec<Value> {
        self.0
            .lock()
            .unwrap()
            .iter()
            .filter(|(e, p)| {
                e == FRAME_EVENT && p["type"] == kind && p["data"]["sessionId"] == session
            })
            .map(|(_, p)| p["data"].clone())
            .collect()
    }

    fn output(&self, session: &str) -> String {
        self.of(OUTPUT, session)
            .iter()
            .filter_map(|d| d["data"].as_str().map(str::to_owned))
            .collect()
    }
}

fn wait_until(what: &str, mut done: impl FnMut() -> bool) {
    let started = Instant::now();
    while !done() {
        assert!(
            started.elapsed() < Duration::from_secs(10),
            "timed out: {what}"
        );
        std::thread::sleep(Duration::from_millis(20));
    }
}

fn alive(pid: i32) -> bool {
    // SAFETY: signal 0 only checks that the process exists.
    unsafe { libc::kill(pid, 0) == 0 }
}

struct Setup {
    _dir: tempfile::TempDir,
    vault: VaultDb,
    recorded: Arc<Recorded>,
    host: Arc<ExtensionHost>,
    notes: CallContext,
    minimal: CallContext,
}

fn installed(vault: &VaultDb, fixture: &str, device: Uuid) -> (Uuid, Uuid) {
    let bytes = std::fs::read(
        Path::new(env!("CARGO_MANIFEST_DIR"))
            .join("tests/fixtures/extension_bundles")
            .join(fixture),
    )
    .unwrap();
    let extension = install(vault, &bytes, vec![], false, device, 1)
        .unwrap()
        .ids
        .extension_id;
    let bundle = vault
        .read_blocking(move |q| effective_bundle(q, extension).map_err(Into::into))
        .unwrap()
        .unwrap()
        .bundle_id;
    (extension, bundle)
}

fn setup() -> Setup {
    let (dir, db) = open_test_vault();
    let vault = VaultGate::new().vault_db(Arc::new(db)).unwrap();
    let device = vault
        .read_blocking(|q| known_devices::list_devices(q))
        .unwrap()[0]
        .vault_device_uuid;
    let host = Arc::new(ExtensionHost::default());
    let recorded = Arc::new(Recorded::default());
    let context = |(extension, bundle): (Uuid, Uuid)| CallContext {
        db: vault.clone(),
        host: Arc::clone(&host),
        session: host.frames.open(extension, bundle, "tab"),
        device,
        emitter: Arc::clone(&recorded) as Arc<dyn Emit>,
    };
    let notes = context(installed(&vault, "good-notes-like.xt", device));
    let minimal = context(installed(&vault, "good-minimal.xt", device));
    Setup {
        _dir: dir,
        vault,
        recorded,
        host,
        notes,
        minimal,
    }
}

fn sh() -> PathBuf {
    resolve_program("/bin/sh").expect("/bin/sh")
}

impl Setup {
    fn allow(&self, ctx: &CallContext, program: &Path) {
        set(
            &self.vault,
            ctx.device,
            PermissionSetArgs {
                extension_id: ctx.session.extension_id.to_string(),
                kind: "shell".into(),
                action: "execute".into(),
                target: program.to_string_lossy().into_owned(),
                status: "granted".into(),
                all_devices: false,
                replaces: None,
            },
            2,
        )
        .unwrap();
    }

    fn start(&self, ctx: &CallContext) -> String {
        let started = call(
            ctx,
            "extension_shell_create",
            &json!({ "options": { "shell": "/bin/sh", "cols": 100, "rows": 30, "env": { "PROBE": "on" } } }),
        )
        .unwrap();
        assert_eq!(
            started["shellName"],
            sh().file_stem().unwrap().to_string_lossy().as_ref()
        );
        started["sessionId"].as_str().unwrap().to_owned()
    }

    fn write(&self, ctx: &CallContext, session: &str, data: &str) -> Result<Value, BridgeError> {
        call(
            ctx,
            "extension_shell_write",
            &json!({ "sessionId": session, "data": data }),
        )
    }
}

fn code(result: Result<Value, BridgeError>) -> u16 {
    result.map_or_else(|e| e.code.as_u16(), |_| 0)
}

#[test]
fn a_granted_shell_runs_echo_resizes_and_reports_its_end() {
    let s = setup();
    s.allow(&s.notes, &sh());
    let session = s.start(&s.notes);
    s.write(&s.notes, &session, "echo \"probe-$PROBE\"; stty size\n")
        .unwrap();
    wait_until("the echo", || {
        s.recorded.output(&session).contains("probe-on")
    });
    wait_until("the size", || {
        s.recorded.output(&session).contains("30 100")
    });

    call(
        &s.notes,
        "extension_shell_resize",
        &json!({ "sessionId": session, "cols": 120, "rows": 40 }),
    )
    .unwrap();
    s.write(&s.notes, &session, "stty size; exit 3\n").unwrap();
    wait_until("the new size", || {
        s.recorded.output(&session).contains("40 120")
    });
    wait_until("the end", || !s.recorded.of(EXIT, &session).is_empty());
    assert_eq!(s.recorded.of(EXIT, &session)[0]["exitCode"], 3);
    assert_eq!(s.host.shells.count(s.notes.session.extension_id), 0);
    assert_eq!(code(s.write(&s.notes, &session, "x")), 1001, "gone");
}

#[test]
fn without_a_permission_holzi_asks_for_the_canonical_program() {
    let s = setup();
    let asked = call(
        &s.notes,
        "extension_shell_create",
        &json!({ "options": { "shell": "sh" } }),
    )
    .unwrap_err();
    assert_eq!(asked.code.as_u16(), 1004);
    assert_eq!(
        asked.details,
        Some(json!({
            "resourceType": "shell",
            "action": "execute",
            "target": resolve_program("sh").unwrap().to_string_lossy(),
        }))
    );
    s.allow(&s.notes, Path::new("/bin/false-not-sh"));
    assert_eq!(
        code(call(
            &s.notes,
            "extension_shell_create",
            &json!({ "options": { "shell": "/bin/sh" } })
        )),
        1004,
        "another program's permission does not count"
    );
    assert_eq!(
        code(call(
            &s.notes,
            "extension_shell_create",
            &json!({ "options": { "shell": "/no/such/shell" } })
        )),
        2003
    );
}

#[test]
fn a_session_belongs_to_its_extension() {
    let s = setup();
    s.allow(&s.notes, &sh());
    let session = s.start(&s.notes);
    assert_eq!(code(s.write(&s.minimal, &session, "echo x\n")), 1001);
    assert_eq!(
        code(call(
            &s.minimal,
            "extension_shell_resize",
            &json!({ "sessionId": session, "cols": 10, "rows": 10 })
        )),
        1001
    );
    assert_eq!(
        code(call(
            &s.minimal,
            "extension_shell_close",
            &json!({ "sessionId": session })
        )),
        1001
    );
    assert_eq!(s.host.shells.count(s.notes.session.extension_id), 1);
    call(
        &s.notes,
        "extension_shell_close",
        &json!({ "sessionId": session }),
    )
    .unwrap();
    wait_until("the end", || !s.recorded.of(EXIT, &session).is_empty());
    let heard: Vec<String> = s
        .recorded
        .0
        .lock()
        .unwrap()
        .iter()
        .filter(|(_, p)| p["data"]["sessionId"] == session.as_str())
        .map(|(_, p)| p["frame"].as_str().unwrap().to_owned())
        .collect();
    assert!(
        heard.iter().all(|f| *f == s.notes.session.frame),
        "only the frames of the owning extension hear it"
    );
}

#[test]
fn ending_all_shells_of_an_extension_kills_the_whole_process_group() {
    let s = setup();
    s.allow(&s.notes, &sh());
    let session = s.start(&s.notes);
    s.write(&s.notes, &session, "sleep 100 & echo \"child=$!\"\n")
        .unwrap();
    // The terminal echoes the typed line too; the pid is in the last `child=`.
    let pid = |output: String| -> Option<i32> {
        output
            .rsplit("child=")
            .next()
            .and_then(|rest| rest.split_whitespace().next())
            .and_then(|pid| pid.parse().ok())
    };
    wait_until("the background child", || {
        pid(s.recorded.output(&session)).is_some()
    });
    let child = pid(s.recorded.output(&session)).unwrap();
    assert!(alive(child));
    s.host.shells.end_all(s.notes.session.extension_id);
    wait_until("the background child to end", || !alive(child));
    wait_until("the end", || !s.recorded.of(EXIT, &session).is_empty());
}

#[test]
fn the_list_of_shells_names_existing_programs() {
    let s = setup();
    let shells = call(&s.notes, "extension_shell_list_available", &json!({})).unwrap();
    let shells = shells.as_array().unwrap();
    assert!(!shells.is_empty());
    for shell in shells {
        assert!(
            Path::new(shell["path"].as_str().unwrap()).is_file(),
            "{shell}"
        );
    }
}

#[test]
fn malformed_options_are_refused() {
    let s = setup();
    s.allow(&s.notes, &sh());
    for options in [
        json!({ "shell": 7 }),
        json!({ "cols": 0 }),
        json!({ "rows": 5000 }),
        json!({ "env": { "A=B": "x" } }),
        json!({ "env": { "A": 1 } }),
    ] {
        assert_eq!(
            code(call(
                &s.notes,
                "extension_shell_create",
                &json!({ "options": options.clone() })
            )),
            3001,
            "{options}"
        );
    }
    assert_eq!(
        code(call(
            &s.notes,
            "extension_shell_create",
            &json!({ "options": { "cwd": "/no/such/dir" } })
        )),
        2003
    );
}

#[test]
fn output_decodes_characters_split_across_reads() {
    let text = "grün ✓ 🦀";
    let bytes = text.as_bytes();
    for split in 0..=bytes.len() {
        let mut stream = Utf8Stream::default();
        let mut out = stream.push(&bytes[..split]);
        out.push_str(&stream.push(&bytes[split..]));
        assert_eq!(out, text, "split at {split}");
    }
    let mut stream = Utf8Stream::default();
    assert_eq!(stream.push(&[b'a', 0xff, b'b']), "a\u{fffd}b");
}
