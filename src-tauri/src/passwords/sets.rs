//! The `SET` list of a partial update (spec 034, research R7): only the columns that really change
//! are written. haex-crdt merges per column, so two devices that change different fields of one
//! row both keep their change; an update that rewrote every column would hand the stale values of
//! the second device the newer clock and undo the first (FR-037).

use haex_crdt::rusqlite::ToSql;
use haex_crdt::CrdtTransaction;

use super::model::Patch;
use crate::error::Result;

/// Changed columns with their new values, in the order they were added.
#[derive(Default)]
pub struct Sets {
    columns: Vec<&'static str>,
    values: Vec<Box<dyn ToSql>>,
}

impl Sets {
    pub fn push(&mut self, column: &'static str, value: impl ToSql + 'static) {
        self.columns.push(column);
        self.values.push(Box::new(value));
    }

    /// A text column under a patch: `Keep` adds nothing, `Clear` writes `NULL` if there is a value,
    /// `Set` writes a value that differs from the stored one.
    pub fn text(&mut self, column: &'static str, current: &Option<String>, patch: &Patch<String>) {
        match patch {
            Patch::Keep => {}
            Patch::Clear => {
                if current.is_some() {
                    self.push(column, Option::<String>::None);
                }
            }
            Patch::Set(value) => {
                if current.as_deref() != Some(value.as_str()) {
                    self.push(column, value.clone());
                }
            }
        }
    }

    /// An integer column under a patch, like [`Self::text`].
    pub fn number(&mut self, column: &'static str, current: Option<i64>, patch: &Patch<i64>) {
        match patch {
            Patch::Keep => {}
            Patch::Clear => {
                if current.is_some() {
                    self.push(column, Option::<i64>::None);
                }
            }
            Patch::Set(value) => {
                if current != Some(*value) {
                    self.push(column, *value);
                }
            }
        }
    }

    pub fn is_empty(&self) -> bool {
        self.columns.is_empty()
    }

    /// Runs `UPDATE table SET … WHERE id_column = id`; nothing happens for an empty list.
    pub fn execute(
        &self,
        tx: &mut CrdtTransaction<'_>,
        table: &str,
        id_column: &str,
        id: &str,
    ) -> Result<()> {
        if self.is_empty() {
            return Ok(());
        }
        let assignments: Vec<String> = self
            .columns
            .iter()
            .enumerate()
            .map(|(i, column)| format!("{column} = ?{}", i + 1))
            .collect();
        let sql = format!(
            "UPDATE {table} SET {} WHERE {id_column} = ?{}",
            assignments.join(", "),
            self.columns.len() + 1
        );
        let id_owned = id.to_string();
        let mut bound: Vec<&dyn ToSql> = self.values.iter().map(|v| v.as_ref()).collect();
        bound.push(&id_owned);
        tx.execute(&sql, &bound)?;
        Ok(())
    }
}
