//! Who may read and write which extension table (contracts/sql-policy.md, research R6): the one
//! rule for the pre-check and the authorizer. An allowlist: the own tables always, a table of
//! another extension with a matching `database` permission, nothing else — there is no list of
//! forbidden core tables to keep complete.
//!
//! A table of an extension that is not installed (never, or removed with "keep data") does not
//! exist for others, even with a remembered permission (spec 017, US6 scenario 5).

use std::collections::HashSet;

use uuid::Uuid;

use super::{classify, TableClass};
use crate::extensions::ids::{ExtensionTable, TablePrefix};
use crate::extensions::permissions::{
    evaluate, Action, Decision, Permission, PermissionKind, PermissionRequest, RequestTarget,
};

#[derive(Debug, Clone)]
pub struct SqlPolicy {
    pub own: TablePrefix,
    /// The calling extension's remembered and temporary `database` permissions.
    pub grants: Vec<Permission>,
    /// This device, for device-scoped rows.
    pub device: Uuid,
    /// The prefixes of the installed extensions; another prefix's tables do not exist.
    pub installed: HashSet<TablePrefix>,
}

impl SqlPolicy {
    /// A policy that allows only the own tables.
    pub fn own_only(own: TablePrefix, device: Uuid) -> Self {
        Self {
            own,
            grants: Vec::new(),
            device,
            installed: HashSet::new(),
        }
    }

    /// Whether the tables of `prefix` exist for the caller: its own, or an installed extension's.
    pub fn is_present(&self, prefix: &TablePrefix) -> bool {
        *prefix == self.own || self.installed.contains(prefix)
    }

    /// The decision for one extension table: own tables always, another extension's by its
    /// permission (`read` for reading, `readWrite` for writing).
    pub fn decide(&self, table: &ExtensionTable, write: bool) -> Decision {
        if table.prefix == self.own {
            return Decision::Allow;
        }
        let request = PermissionRequest {
            kind: PermissionKind::Database,
            action: if write {
                Action::ReadWrite
            } else {
                Action::Read
            },
            target: RequestTarget::Table(table.clone()),
        };
        evaluate(&self.grants, &request, self.device)
    }

    /// Whether SQLite may access the table `name` (as SQLite reports it) now.
    pub fn allows(&self, name: &str, write: bool) -> bool {
        match classify(name, &self.own) {
            TableClass::Own(_) => true,
            TableClass::Foreign(table) => {
                self.is_present(&table.prefix) && self.decide(&table, write) == Decision::Allow
            }
            TableClass::Core => false,
        }
    }
}

#[cfg(test)]
#[path = "policy_tests.rs"]
mod tests;
