//! Test support of the extension SQL tests: a vault with own tables and a policy.

// The setup creates extension tables directly and the tests read what the CRDT layer wrote.
#![allow(clippy::disallowed_methods)]

use std::sync::Arc;

use haex_crdt::{AuthContext, Authorization, Database, GuardedWriteOptions, SqlGuard};

use super::exec::{existing_tables, prepare, run, Limits, SqlResult};
use super::policy::SqlPolicy;
use super::values::params as to_params;
use crate::extensions::error::BridgeError;
use crate::extensions::ids::{ExtensionName, PublicKey, TablePrefix};
use crate::extensions::permissions::{
    Action, GrantScope, Permission, PermissionKind, PermissionStatus, Target,
};
use crate::passwords::test_support::open_test_vault;
use crate::vault_gate::{VaultDb, VaultGate};

pub(crate) fn own() -> TablePrefix {
    TablePrefix {
        public_key: PublicKey::parse(&"a".repeat(64)).unwrap(),
        name: ExtensionName::parse("notes").unwrap(),
    }
}

/// `t:` stands for the own prefix.
pub(crate) fn t(sql: &str) -> String {
    sql.replace("t:", &own().to_string())
}

pub(crate) struct Setup {
    pub _dir: tempfile::TempDir,
    pub db: Database,
    pub vault: VaultDb,
    pub policy: Arc<SqlPolicy>,
}

/// Another installed extension, `cal`.
pub(crate) fn foreign() -> TablePrefix {
    TablePrefix {
        public_key: PublicKey::parse(&"b".repeat(64)).unwrap(),
        name: ExtensionName::parse("cal").unwrap(),
    }
}

/// `t:` stands for the own prefix, `f:` for the one of [`foreign`].
pub(crate) fn tf(sql: &str) -> String {
    t(sql).replace("f:", &foreign().to_string())
}

/// Creates a table like a migration does: guarded, in schema mode.
fn create(vault: &VaultDb, ddl: String) {
    let everything = SqlGuard {
        authorizer: Arc::new(|_: &AuthContext<'_>| Authorization::Allow),
        progress: None,
        max_value_bytes: None,
    };
    vault
        .write_guarded_blocking(
            &everything,
            GuardedWriteOptions {
                schema_mode: true,
                local: false,
            },
            move |tx| tx.execute(&ddl, &[]).map(drop),
        )
        .unwrap();
}

/// A vault with the own tables `pages` (synced) and `cache_no_sync` (device-local).
pub(crate) fn setup() -> Setup {
    let (dir, db) = open_test_vault();
    let vault = VaultGate::new().vault_db(Arc::new(db.clone())).unwrap();
    for ddl in [
        "CREATE TABLE t:pages (id TEXT PRIMARY KEY, body TEXT, n INTEGER, data BLOB)",
        "CREATE TABLE t:cache_no_sync (key TEXT PRIMARY KEY, value TEXT)",
    ] {
        create(&vault, t(ddl));
    }
    Setup {
        _dir: dir,
        db,
        vault,
        policy: Arc::new(SqlPolicy::own_only(own(), uuid::Uuid::new_v4())),
    }
}

/// [`setup`] and the table `events` of the installed extension [`foreign`] with one row, which
/// the policy may read but not write.
pub(crate) fn setup_reading_foreign() -> Setup {
    let mut s = setup();
    create(
        &s.vault,
        tf("CREATE TABLE f:events (id TEXT PRIMARY KEY, title TEXT)"),
    );
    let insert = tf("INSERT INTO f:events (id, title) VALUES ('e1', 'kept')");
    s.vault
        .write_blocking(move |tx| tx.execute(&insert, &[]).map(drop))
        .unwrap();
    let mut policy = SqlPolicy::own_only(own(), s.policy.device);
    policy.installed.insert(foreign());
    policy.grants.push(Permission {
        kind: PermissionKind::Database,
        action: Action::Read,
        target: Target::ExtensionTables(foreign()),
        status: PermissionStatus::Granted,
        scope: GrantScope::Vault,
    });
    s.policy = Arc::new(policy);
    s
}

impl Setup {
    pub(crate) fn sql(
        &self,
        sql: &str,
        params: serde_json::Value,
    ) -> Result<SqlResult, BridgeError> {
        self.sql_with(sql, params, &Limits::default())
    }

    pub(crate) fn sql_with(
        &self,
        sql: &str,
        params: serde_json::Value,
        limits: &Limits,
    ) -> Result<SqlResult, BridgeError> {
        let existing = existing_tables(&self.vault)?;
        let checked = prepare(
            &t(sql),
            to_params(Some(&params))?,
            &self.policy,
            limits,
            &existing,
        )?;
        run(&self.vault, Arc::clone(&self.policy), limits, vec![checked])
    }

    pub(crate) fn transaction(&self, statements: &[&str]) -> Result<SqlResult, BridgeError> {
        let existing = existing_tables(&self.vault)?;
        let limits = Limits::default();
        let checked = statements
            .iter()
            .map(|s| prepare(&t(s), Vec::new(), &self.policy, &limits, &existing))
            .collect::<Result<Vec<_>, _>>()?;
        run(&self.vault, Arc::clone(&self.policy), &limits, checked)
    }

    pub(crate) fn count(&self, sql: &str) -> i64 {
        let sql = t(sql);
        self.db
            .with_connection(move |c| Ok(c.query_row(&sql, [], |r| r.get(0))?))
            .unwrap()
    }
}
