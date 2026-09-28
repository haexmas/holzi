//! Queries that run both inside a write transaction and on the read-only view.
//!
//! Storage readers take `&mut impl Query`, so the same function serves
//! [`crate::vault_gate::VaultDb::read`] and a read inside
//! [`crate::vault_gate::VaultDb::write`] (read, then write, in one transaction).

use haex_crdt::rusqlite::{OptionalExtension, Row, ToSql};
use haex_crdt::{CrdtTransaction, Database, ReadOnlyConnection};

/// Runs `f` on haex-crdt's read-only view of `db` through the [`Query`] interface.
pub fn read<R>(
    db: &Database,
    f: impl FnOnce(&mut Reader<'_, '_>) -> haex_crdt::Result<R>,
) -> haex_crdt::Result<R> {
    db.read(|conn| f(&mut Reader::new(conn)))
}

/// A connection that can run queries: a [`CrdtTransaction`] or a [`Reader`].
pub trait Query {
    /// Runs a query expected to return at most one row; `None` when it returns none.
    fn query_row<T, F>(
        &mut self,
        sql: &str,
        params: &[&dyn ToSql],
        f: F,
    ) -> haex_crdt::Result<Option<T>>
    where
        F: FnOnce(&Row<'_>) -> haex_crdt::rusqlite::Result<T>;

    /// Runs a query and maps every returned row.
    fn query_map<T, F>(
        &mut self,
        sql: &str,
        params: &[&dyn ToSql],
        f: F,
    ) -> haex_crdt::Result<Vec<T>>
    where
        F: FnMut(&Row<'_>) -> haex_crdt::rusqlite::Result<T>;
}

impl Query for CrdtTransaction<'_> {
    fn query_row<T, F>(
        &mut self,
        sql: &str,
        params: &[&dyn ToSql],
        f: F,
    ) -> haex_crdt::Result<Option<T>>
    where
        F: FnOnce(&Row<'_>) -> haex_crdt::rusqlite::Result<T>,
    {
        CrdtTransaction::query_row(self, sql, params, f)
    }

    fn query_map<T, F>(
        &mut self,
        sql: &str,
        params: &[&dyn ToSql],
        f: F,
    ) -> haex_crdt::Result<Vec<T>>
    where
        F: FnMut(&Row<'_>) -> haex_crdt::rusqlite::Result<T>,
    {
        CrdtTransaction::query_map(self, sql, params, f)
    }
}

/// haex-crdt's read-only view behind the [`Query`] interface.
pub struct Reader<'a, 'c> {
    conn: &'a ReadOnlyConnection<'c>,
}

impl<'a, 'c> Reader<'a, 'c> {
    /// Wraps the view `Database::read` hands its closure.
    pub fn new(conn: &'a ReadOnlyConnection<'c>) -> Self {
        Self { conn }
    }
}

impl Query for Reader<'_, '_> {
    fn query_row<T, F>(
        &mut self,
        sql: &str,
        params: &[&dyn ToSql],
        f: F,
    ) -> haex_crdt::Result<Option<T>>
    where
        F: FnOnce(&Row<'_>) -> haex_crdt::rusqlite::Result<T>,
    {
        Ok(self.conn.query_row(sql, params, f).optional()?)
    }

    fn query_map<T, F>(
        &mut self,
        sql: &str,
        params: &[&dyn ToSql],
        f: F,
    ) -> haex_crdt::Result<Vec<T>>
    where
        F: FnMut(&Row<'_>) -> haex_crdt::rusqlite::Result<T>,
    {
        Ok(self.conn.query_map(sql, params, f)?)
    }
}
