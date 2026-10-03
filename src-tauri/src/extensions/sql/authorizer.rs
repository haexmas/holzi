//! The authorizer of extension SQL (contracts/sql-policy.md §Authorizer): it judges what SQLite
//! really accesses while it prepares the statement, so it decides even where the pre-check could
//! be wrong. Built per statement from the [`SqlPolicy`].

use std::collections::HashSet;
use std::sync::Arc;

use haex_crdt::{AuthAction, AuthContext, Authorization, SqlAuthorizer};

use super::function_allowed;
use super::policy::SqlPolicy;

fn allow(yes: bool) -> Authorization {
    if yes {
        Authorization::Allow
    } else {
        Authorization::Deny
    }
}

/// `z_dirty_<table>_(insert|update|delete)`: the change trigger haex-crdt keeps for `table`.
fn trigger_table(accessor: &str) -> Option<String> {
    let rest = accessor.to_ascii_lowercase();
    let rest = rest.strip_prefix("z_dirty_")?.to_owned();
    ["_insert", "_update", "_delete"]
        .iter()
        .find_map(|suffix| rest.strip_suffix(suffix).map(str::to_owned))
}

fn in_main(context: &AuthContext<'_>) -> bool {
    context
        .database_name
        .is_some_and(|db| db.eq_ignore_ascii_case("main"))
}

/// Functions haex-crdt's change triggers call besides the allowlist.
const TRIGGER_FUNCTIONS: &[&str] = &["gen_uuid", "current_hlc"];

/// Whether a change trigger of `trigger_table` may run at all: the one of a table this statement
/// writes, or haex-crdt's own trigger on one of its tables (`haex_*`, e.g. `haex_deleted_rows`
/// when a delete leaves its marker), which fires only from an allowed trigger because writing a
/// `haex_*` table directly is denied.
fn trigger_allowed(trigger_table: &str, writable: &HashSet<String>) -> bool {
    writable.contains(trigger_table) || trigger_table.starts_with("haex_")
}

/// What a change trigger may do: its own table and haex-crdt's tables, nothing else.
fn trigger_action(context: &AuthContext<'_>, writable: &HashSet<String>) -> Authorization {
    let table_ok = |table: &str| {
        let table = table.to_ascii_lowercase();
        in_main(context) && (writable.contains(&table) || table.starts_with("haex_"))
    };
    allow(match context.action {
        AuthAction::Select => true,
        AuthAction::Function { function_name } => {
            function_allowed(function_name)
                || TRIGGER_FUNCTIONS.contains(&function_name.to_ascii_lowercase().as_str())
        }
        AuthAction::Read { table_name, .. }
        | AuthAction::Insert { table_name }
        | AuthAction::Update { table_name, .. }
        | AuthAction::Delete { table_name } => table_ok(table_name),
        _ => false,
    })
}

/// The run-time authorizer. `writable` are the lower-case names of the tables this statement may
/// write; `ctes` the lower-case names of its `WITH` tables.
///
/// SQLite reports the body of a `WITH` with the CTE's name as accessor and a read of a CTE with
/// no database: both are judged like top-level SQL, so the tables a CTE reads are checked, and a
/// CTE named like a change trigger gains nothing.
pub fn runtime(
    policy: Arc<SqlPolicy>,
    writable: HashSet<String>,
    ctes: HashSet<String>,
) -> SqlAuthorizer {
    Arc::new(move |context: &AuthContext<'_>| {
        let accessor = context
            .accessor
            .map(str::to_ascii_lowercase)
            .filter(|a| !ctes.contains(a));
        if let Some(accessor) = accessor {
            return match trigger_table(&accessor) {
                Some(table) if trigger_allowed(&table, &writable) => {
                    trigger_action(context, &writable)
                }
                _ => Authorization::Deny,
            };
        }
        match context.action {
            AuthAction::Select | AuthAction::Recursive => Authorization::Allow,
            AuthAction::Function { function_name } => allow(function_allowed(function_name)),
            AuthAction::Read { table_name, .. } if context.database_name.is_none() => {
                allow(ctes.contains(&table_name.to_ascii_lowercase()))
            }
            AuthAction::Read { table_name, .. } => {
                allow(in_main(context) && policy.allows(table_name, false))
            }
            AuthAction::Insert { table_name }
            | AuthAction::Update { table_name, .. }
            | AuthAction::Delete { table_name } => allow(
                in_main(context)
                    && policy.allows(table_name, true)
                    && writable.contains(&table_name.to_ascii_lowercase()),
            ),
            _ => Authorization::Deny,
        }
    })
}

#[cfg(test)]
#[path = "authorizer_tests.rs"]
mod tests;
