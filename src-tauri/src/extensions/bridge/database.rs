//! The database methods of the bridge (US2, T061): `extension_database_query`, `_execute`,
//! `_transaction` and `_register_migrations`. Every call runs as the calling frame's extension,
//! with its limits and at most `max_concurrent` at a time.

use std::sync::Arc;

use serde_json::{json, Value};

use super::dispatch::CallContext;
use crate::extensions::error::{BridgeError, ExtensionErrorCode};
use crate::extensions::host::SqlSlot;
use crate::extensions::ids::{ExtensionName, PublicKey, TablePrefix};
use crate::extensions::permissions::store::candidates;
use crate::extensions::permissions::PermissionKind;
use crate::extensions::sql::exec::{existing_tables, limits_of, prepare, run, Limits, SqlResult};
use crate::extensions::sql::migrate::{apply_pending, sql_sha256, MigrationError};
use crate::extensions::sql::policy::SqlPolicy;
use crate::extensions::sql::values::{params, statement_entry};
use crate::passwords::clock::unix_millis;
use crate::storage::query::Query;

pub const MODULE: &str = module_path!();

fn unavailable() -> BridgeError {
    BridgeError::new(ExtensionErrorCode::Database, "database unavailable")
}

fn invalid(message: &str) -> BridgeError {
    BridgeError::new(ExtensionErrorCode::Validation, message)
}

/// The calling extension's table prefix, from its registry row.
fn own_prefix(ctx: &CallContext) -> Result<TablePrefix, BridgeError> {
    let id = ctx.session.extension_id.to_string();
    let (key, name) = ctx
        .db
        .read_blocking(move |q| {
            q.query_row(
                "SELECT public_key, name FROM extensions WHERE id = ?1",
                &[&id],
                |r| Ok((r.get::<_, String>(0)?, r.get::<_, String>(1)?)),
            )
        })
        .map_err(|_| unavailable())?
        .ok_or_else(|| BridgeError::new(ExtensionErrorCode::NotFound, "not found"))?;
    Ok(TablePrefix {
        public_key: PublicKey::parse(&key).map_err(|_| unavailable())?,
        name: ExtensionName::parse(&name).map_err(|_| unavailable())?,
    })
}

/// What every SQL call needs: the policy, the limits and a running slot.
struct SqlCall {
    policy: Arc<SqlPolicy>,
    limits: Limits,
    _slot: SqlSlot,
}

/// The SQL policy of the calling extension: its prefix, its remembered `database` permissions for
/// this device and the decisions held in memory (read fresh on every call, so a change applies
/// to the next call of an open frame).
pub fn policy(ctx: &CallContext) -> Result<SqlPolicy, BridgeError> {
    let extension_id = ctx.session.extension_id;
    let device = ctx.device;
    let mut grants = ctx
        .db
        .read_blocking(move |q| {
            candidates(q, extension_id, PermissionKind::Database, device).map_err(Into::into)
        })
        .map_err(|_| unavailable())?;
    grants.extend(
        ctx.host
            .permissions
            .temporary(extension_id, PermissionKind::Database),
    );
    Ok(SqlPolicy {
        own: own_prefix(ctx)?,
        grants,
        device,
    })
}

fn sql_call(ctx: &CallContext) -> Result<SqlCall, BridgeError> {
    let extension_id = ctx.session.extension_id;
    let limits = ctx
        .db
        .read_blocking(move |q| limits_of(q, extension_id))
        .map_err(|_| unavailable())?;
    let slot = ctx
        .host
        .enter_sql(extension_id, limits.max_concurrent)
        .ok_or_else(|| BridgeError::new(ExtensionErrorCode::LimitExceeded, "too many requests"))?;
    Ok(SqlCall {
        policy: Arc::new(policy(ctx)?),
        limits,
        _slot: slot,
    })
}

fn result_json(result: SqlResult) -> Result<Value, BridgeError> {
    serde_json::to_value(result).map_err(|_| unavailable())
}

/// `{sql | query, params}`: one statement.
fn one_statement(ctx: &CallContext, call_params: &Value) -> Result<Value, BridgeError> {
    let sql = call_params
        .get("sql")
        .or_else(|| call_params.get("query"))
        .and_then(Value::as_str)
        .ok_or_else(|| invalid("sql is missing"))?;
    let call = sql_call(ctx)?;
    let existing = existing_tables(&ctx.db)?;
    let checked = prepare(
        sql,
        params(call_params.get("params"))?,
        &call.policy,
        &call.limits,
        &existing,
    )?;
    result_json(run(
        &ctx.db,
        Arc::clone(&call.policy),
        &call.limits,
        vec![checked],
    )?)
}

pub fn query(ctx: &CallContext, params: &Value) -> Result<Value, BridgeError> {
    one_statement(ctx, params)
}

pub fn execute(ctx: &CallContext, params: &Value) -> Result<Value, BridgeError> {
    one_statement(ctx, params)
}

/// `{statements: [[sql, params], …]}`: all or nothing.
pub fn transaction(ctx: &CallContext, call_params: &Value) -> Result<Value, BridgeError> {
    let entries = call_params
        .get("statements")
        .and_then(Value::as_array)
        .filter(|s| !s.is_empty())
        .ok_or_else(|| invalid("statements must be a non-empty array"))?;
    let call = sql_call(ctx)?;
    let existing = existing_tables(&ctx.db)?;
    let checked = entries
        .iter()
        .map(|entry| {
            let (sql, entry_params) = statement_entry(entry)?;
            prepare(
                sql,
                params(entry_params)?,
                &call.policy,
                &call.limits,
                &existing,
            )
        })
        .collect::<Result<Vec<_>, _>>()?;
    result_json(run(
        &ctx.db,
        Arc::clone(&call.policy),
        &call.limits,
        checked,
    )?)
}

/// `{extensionVersion, migrations: [{name, sql}]}`: only migrations of the installed bundle are
/// accepted (same name and SQL); pending ones are applied. Answers the SDK's `MigrationResult`.
pub fn register_migrations(ctx: &CallContext, call_params: &Value) -> Result<Value, BridgeError> {
    let migrations = call_params
        .get("migrations")
        .and_then(Value::as_array)
        .ok_or_else(|| invalid("migrations must be an array"))?;
    let extension_id = ctx.session.extension_id;
    let ext = extension_id.to_string();
    let known: Vec<(String, String)> = ctx
        .db
        .read_blocking(move |q| {
            q.query_map(
                "SELECT name, sql_sha256 FROM extension_migrations WHERE extension_id = ?1",
                &[&ext],
                |r| Ok((r.get(0)?, r.get(1)?)),
            )
        })
        .map_err(|_| unavailable())?;
    for migration in migrations {
        let name = migration.get("name").and_then(Value::as_str);
        let sql = migration.get("sql").and_then(Value::as_str);
        let (Some(name), Some(sql)) = (name, sql) else {
            return Err(invalid("a migration needs name and sql"));
        };
        let hash = sql_sha256(sql);
        if !known.iter().any(|(n, h)| n == name && *h == hash) {
            return Err(BridgeError::new(
                ExtensionErrorCode::SecurityViolation,
                "migration is not part of the installed bundle",
            ));
        }
    }
    let own = own_prefix(ctx)?;
    let applied = apply_pending(
        &ctx.db,
        extension_id,
        &own,
        unix_millis(std::time::SystemTime::now()),
    )
    .map_err(|error| match error {
        MigrationError::Refused { error, .. } => *error,
        MigrationError::Unavailable => unavailable(),
        _ => BridgeError::new(ExtensionErrorCode::Database, "migration failed"),
    })?;
    Ok(json!({
        "appliedCount": applied.len(),
        "alreadyAppliedCount": migrations.len().saturating_sub(applied.len()),
        "appliedMigrations": applied,
    }))
}
