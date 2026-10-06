//! Data of another extension (spec 017, US6, T088): two signed extensions, `calendar` with a table
//! of events and `week`, which declares reading the calendar's tables. Every call goes through the
//! bridge as a frame of `week` would make it.

// The tests write registry rows directly to set up a state.
#![allow(clippy::disallowed_methods)]

use std::sync::{Arc, Mutex};

use haex_crdt::Database;
use serde_json::json;

use super::*;
use crate::extensions::bridge::dispatch::{call, CallContext, Emit};
use crate::extensions::commands::permissions::{set, PermissionSetArgs};
use crate::extensions::registry::effective::effective_bundle;
use crate::extensions::registry::install::{install, PermissionChoice};
use crate::extensions::registry::lifecycle::reconcile;
use crate::extensions::registry::remove::remove;
use crate::passwords::test_support::open_test_vault;
use crate::storage::known_devices;
use crate::vault_gate::VaultGate;

/// Test-only publisher keys.
fn key(seed: u8) -> ed25519_dalek::SigningKey {
    ed25519_dalek::SigningKey::from_bytes(&[seed; 32])
}

fn public_key(seed: u8) -> String {
    key(seed)
        .verifying_key()
        .as_bytes()
        .iter()
        .map(|b| format!("{b:02x}"))
        .collect()
}

const CALENDAR: u8 = 7;
const WEEK: u8 = 8;

/// The calendar's table, unquoted (as a permission names it).
fn events_name() -> String {
    format!("{}__calendar__events", public_key(CALENDAR))
}

/// `name` quoted for SQL (a publisher key may start with a digit).
fn q(name: &str) -> String {
    format!("\"{name}\"")
}

fn events() -> String {
    q(&events_name())
}

fn notes() -> String {
    q(&format!("{}__week__notes", public_key(WEEK)))
}

fn calendar_tables() -> String {
    format!("{}__calendar__*", public_key(CALENDAR))
}

fn bundle(seed: u8, name: &str, permissions: &str, migration: &str) -> Vec<u8> {
    let manifest = haex_bundle::jcs::parse_restricted(&format!(
        r#"{{"name":"{name}","version":"1.0.0","displayName":"{name}","migrationsDir":"db","permissions":{permissions}}}"#
    ))
    .unwrap();
    let files = vec![
        haex_bundle::Entry {
            path: "index.html".into(),
            data: b"<!doctype html><title>t</title>".to_vec(),
        },
        haex_bundle::Entry {
            path: "db/0000_init.sql".into(),
            data: migration.as_bytes().to_vec(),
        },
    ];
    haex_bundle::build_archive(files, manifest, &key(seed)).unwrap()
}

#[derive(Default)]
struct Recorded(Mutex<Vec<(String, serde_json::Value)>>);

impl Emit for Recorded {
    fn emit(&self, event: &str, payload: serde_json::Value) {
        self.0.lock().unwrap().push((event.to_owned(), payload));
    }
}

impl Recorded {
    fn questions(&self) -> usize {
        self.0
            .lock()
            .unwrap()
            .iter()
            .filter(|(e, _)| e == "extension-permission-request")
            .count()
    }
}

struct Setup {
    _dir: tempfile::TempDir,
    _db: Database,
    vault: VaultDb,
    host: Arc<ExtensionHost>,
    device: Uuid,
    recorded: Arc<Recorded>,
    calendar: Uuid,
    week: Uuid,
}

fn setup() -> Setup {
    let (dir, db) = open_test_vault();
    let vault = VaultGate::new().vault_db(Arc::new(db.clone())).unwrap();
    let device = vault
        .read_blocking(|q| known_devices::list_devices(q))
        .unwrap()[0]
        .vault_device_uuid;
    let calendar = bundle(
        CALENDAR,
        "calendar",
        "{}",
        &format!(
            "CREATE TABLE `{}` (`id` text PRIMARY KEY NOT NULL, `title` text);",
            events_name()
        ),
    );
    let week = bundle(
        WEEK,
        "week",
        &format!(
            r#"{{"database":[{{"target":"{}","operation":"read"}}]}}"#,
            calendar_tables()
        ),
        &format!(
            "CREATE TABLE `{}__week__notes` (`id` text PRIMARY KEY NOT NULL, `event` text, `text` text);",
            public_key(WEEK)
        ),
    );
    let calendar = install(&vault, &calendar, vec![], false, device, 1)
        .unwrap()
        .ids
        .extension_id;
    let choice = PermissionChoice {
        kind: "database".into(),
        action: "read".into(),
        target: calendar_tables(),
        granted: true,
    };
    let week = install(&vault, &week, vec![choice], false, device, 2)
        .unwrap()
        .ids
        .extension_id;
    let host = Arc::new(ExtensionHost::default());
    reconcile(&vault, &host, device, 3).unwrap();
    let s = Setup {
        _dir: dir,
        _db: db,
        vault,
        host,
        device,
        recorded: Arc::new(Recorded::default()),
        calendar,
        week,
    };
    let added = s.sql(
        s.calendar,
        &format!(
            "INSERT INTO {} (id, title) VALUES ('e1', 'Standup')",
            events()
        ),
    );
    assert!(added.is_ok(), "{added:?}");
    s
}

impl Setup {
    fn ctx(&self, extension: Uuid) -> CallContext {
        let bundle = self
            .vault
            .read_blocking(move |q| effective_bundle(q, extension).map_err(Into::into))
            .unwrap()
            .unwrap()
            .bundle_id;
        CallContext {
            db: self.vault.clone(),
            host: Arc::clone(&self.host),
            session: self.host.frames.open(extension, bundle, "tab"),
            device: self.device,
            emitter: Arc::clone(&self.recorded) as Arc<dyn Emit>,
        }
    }

    fn sql(&self, extension: Uuid, sql: &str) -> Result<serde_json::Value, BridgeError> {
        let method = if sql.trim_start().starts_with("SELECT") {
            "extension_database_query"
        } else {
            "extension_database_execute"
        };
        call(&self.ctx(extension), method, &json!({ "sql": sql }))
    }

    fn code(&self, extension: Uuid, sql: &str) -> u16 {
        self.sql(extension, sql)
            .map_or_else(|e| e.code.as_u16(), |_| 0)
    }

    fn grant(&self, action: &str, target: &str) {
        set(
            &self.vault,
            self.device,
            PermissionSetArgs {
                extension_id: self.week.to_string(),
                kind: "database".into(),
                action: action.into(),
                target: target.into(),
                status: "granted".into(),
                replaces: None,
            },
            4,
        )
        .unwrap();
    }

    fn remove_calendar(&self, delete_data: bool) {
        remove(&self.vault, self.calendar, delete_data, 5).unwrap();
        reconcile(&self.vault, &self.host, self.device, 6).unwrap();
    }
}

fn rows(answer: &serde_json::Value) -> &Vec<serde_json::Value> {
    answer["rows"].as_array().unwrap()
}

#[test]
fn with_read_it_reads_the_tables_of_the_other_extension_also_joined_with_its_own() {
    let s = setup();
    let read = s
        .sql(s.week, &format!("SELECT title FROM {}", events()))
        .unwrap();
    assert_eq!(rows(&read), &vec![json!(["Standup"])]);

    s.sql(
        s.week,
        &format!(
            "INSERT INTO {} (id, event, text) VALUES ('n1', 'e1', 'prepare')",
            notes()
        ),
    )
    .unwrap();
    let joined = s
        .sql(
            s.week,
            &format!(
                "SELECT e.title, n.text FROM {} n JOIN {} e ON e.id = n.event \
                 WHERE EXISTS (SELECT 1 FROM {} WHERE id = 'e1')",
                notes(),
                events(),
                events()
            ),
        )
        .unwrap();
    assert_eq!(rows(&joined), &vec![json!(["Standup", "prepare"])]);
    assert_eq!(s.recorded.questions(), 0);
}

#[test]
fn a_write_asks_for_read_and_write_and_passes_once_granted() {
    let s = setup();
    let insert = format!(
        "INSERT INTO {} (id, title) VALUES ('e2', 'from week')",
        events()
    );
    let asked = s.sql(s.week, &insert).unwrap_err();
    assert_eq!(asked.code.as_u16(), 1004);
    assert_eq!(
        asked.details,
        Some(json!({"resourceType": "database", "action": "readWrite", "target": events_name()}))
    );
    assert_eq!(s.recorded.questions(), 1);
    assert_eq!(
        s.code(s.week, &format!("DELETE FROM {}", events())),
        1004,
        "every write"
    );

    s.grant("readWrite", &events_name());
    s.sql(s.week, &insert).unwrap();
    let seen = s
        .sql(
            s.calendar,
            &format!("SELECT id FROM {} ORDER BY id", events()),
        )
        .unwrap();
    assert_eq!(rows(&seen), &vec![json!(["e1"]), json!(["e2"])]);
}

#[test]
fn a_grant_on_one_table_covers_only_that_table() {
    let s = setup();
    let other = q(&format!("{}__calendar__people", public_key(CALENDAR)));
    s.grant("readWrite", &events_name());
    assert_eq!(
        s.code(s.week, &format!("UPDATE {} SET title = 'x'", events())),
        0
    );
    assert_eq!(
        s.code(s.week, &format!("INSERT INTO {other} (id) VALUES ('p')")),
        1004
    );
}

#[test]
fn the_schema_of_the_other_extension_is_refused_with_any_permission() {
    let s = setup();
    s.grant("readWrite", &calendar_tables());
    let questions = s.recorded.questions();
    for ddl in [
        format!(
            "CREATE TABLE \"{}__calendar__extra\" (id TEXT)",
            public_key(CALENDAR)
        ),
        format!("ALTER TABLE {} ADD COLUMN x TEXT", events()),
        format!("DROP TABLE {}", events()),
        format!(
            "CREATE INDEX \"{}__calendar__idx\" ON {} (title)",
            public_key(CALENDAR),
            events()
        ),
        format!("DROP INDEX \"{}__calendar__idx\"", public_key(CALENDAR)),
    ] {
        assert_eq!(s.code(s.week, &ddl), 1000, "{ddl}");
    }
    assert_eq!(s.recorded.questions(), questions, "nothing to ask");
    let read = s
        .sql(s.week, &format!("SELECT count(*) FROM {}", events()))
        .unwrap();
    assert_eq!(rows(&read), &vec![json!([1])]);
}

#[test]
fn core_tables_are_refused_without_a_question_whatever_is_granted() {
    let s = setup();
    s.grant("readWrite", &calendar_tables());
    for sql in [
        "SELECT * FROM chat_threads".to_owned(),
        format!(
            "SELECT * FROM {} e JOIN extension_permissions p ON p.target = e.id",
            events()
        ),
        format!("INSERT INTO {} (id) SELECT id FROM extensions", events()),
    ] {
        assert_eq!(s.code(s.week, &sql), 1000, "{sql}");
    }
    assert_eq!(s.recorded.questions(), 0);
}

/// The answer `week` gets for `sql` after the calendar is removed.
fn after_removal(delete_data: bool, sql: &str) -> BridgeError {
    let s = setup();
    s.remove_calendar(delete_data);
    let kept = s.vault.read_blocking(|q| {
        q.query_row(
            "SELECT count(*) FROM sqlite_master WHERE name = ?1",
            &[&events_name()],
            |r| r.get::<_, i64>(0),
        )
    });
    assert_eq!(kept.unwrap(), Some(i64::from(!delete_data)));
    let error = s.sql(s.week, sql).unwrap_err();
    assert_eq!(s.recorded.questions(), 0, "nothing to ask");
    error
}

#[test]
fn after_the_other_extension_is_removed_its_tables_answer_as_missing_kept_or_not() {
    let sql = format!("SELECT title FROM {}", events());
    let deleted = after_removal(true, &sql);
    let kept = after_removal(false, &sql);
    assert_eq!(deleted.code.as_u16(), 2000);
    assert_eq!(deleted.message, format!("no such table: {}", events_name()));
    assert_eq!(
        (kept.code, kept.message, kept.details),
        (deleted.code, deleted.message, deleted.details),
        "kept data must not show"
    );

    // The same for any spelling.
    let shouted = format!(
        "SELECT title FROM MAIN.{}",
        q(&events_name().to_uppercase())
    );
    let deleted = after_removal(true, &shouted);
    let kept = after_removal(false, &shouted);
    assert_eq!(kept.message, deleted.message);
}

#[test]
fn a_with_name_does_not_tell_whether_another_extension_has_a_table() {
    let s = setup();
    let probe = |table: &str| {
        s.sql(
            s.week,
            &format!("WITH {table} AS (SELECT 1 AS x) SELECT x FROM {table}"),
        )
        .map(drop)
        .map_err(|e| (e.code, e.message))
    };
    let existing = probe(&events());
    let missing = probe(&q(&format!("{}__calendar__nothing", public_key(CALENDAR))));
    assert!(existing.is_err());
    assert_eq!(existing, missing);
}

#[test]
fn a_database_target_that_is_not_an_extension_is_never_stored() {
    let s = setup();
    for target in ["chat_*", "chat_threads", "*", "extensions"] {
        let stored = set(
            &s.vault,
            s.device,
            PermissionSetArgs {
                extension_id: s.week.to_string(),
                kind: "database".into(),
                action: "read".into(),
                target: target.into(),
                status: "granted".into(),
                replaces: None,
            },
            4,
        );
        assert!(stored.is_err(), "{target} was stored");
    }
    let week = s.week.to_string();
    let rows = s.vault.read_blocking(move |q| {
        q.query_row(
            "SELECT count(*) FROM extension_permissions WHERE extension_id = ?1",
            &[&week],
            |r| r.get::<_, i64>(0),
        )
    });
    assert_eq!(rows.unwrap(), Some(1), "only the declared calendar grant");
}
