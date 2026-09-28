# Contract: Erweiterung von haex-crdt für mehrteilige CRDT-Schreibungen

Repository `https://github.com/haexmas/haex-crdt`, Grundlage Revision
`ed230d2c3f58c1b10710b6025ea0ce6c20b8d009` (heute in holzi gepinnt). Die Änderung kommt als PR in
haex-crdt; holzi pinnt danach die neue Revision mit voller SHA (Constitution IV). Sie ist allgemein,
enthält nichts holzi-Eigenes und ändert das Verhalten für haex-vault nicht. Begründung: research R19.

## E1 Transaktion mit mehreren CRDT-Anweisungen

```rust
impl Database {
    pub fn write<R>(
        &self,
        f: impl FnOnce(&mut CrdtTransaction<'_>) -> Result<R>,
    ) -> Result<R>;
    pub fn read<R>(&self, f: impl FnOnce(&ReadTransaction<'_>) -> Result<R>) -> Result<R>;
}

impl CrdtTransaction<'_> {
    pub fn execute_with_crdt(&mut self, sql: &str, params: &[SqlValue]) -> Result<usize>;
    pub fn query_with_crdt(&mut self, sql: &str, params: &[SqlValue]) -> Result<Vec<Row>>;
    pub fn execute_local(&mut self, sql: &str, params: &[SqlValue]) -> Result<usize>;
    pub fn select(&self, sql: &str, params: &[SqlValue]) -> Result<Vec<Row>>;
}
```

- `write` öffnet eine Transaktion (`IMMEDIATE`), ruft `f` auf und committet nur bei `Ok`; bei `Err`
  oder Panik Rollback. Alle Anweisungen darin teilen einen HLC (`tx_scoped_hlc`, wie heute).
- `execute_with_crdt`/`query_with_crdt` laufen durch den vorhandenen `CrdtTransformer` wie
  `execute_internal`/`query_internal` heute, samt Verbot für CRDT-Metaspalten und `PostWriteHook`s
  je Anweisung in derselben Transaktion.
- `execute_local` führt eine Anweisung ohne Transformer aus und weist sie ab, wenn ihre Zieltabelle
  eine CRDT-Tabelle ist (`extract_primary_table_name_from_sql` gegen die installierten
  CRDT-Tabellen). So kann keine synchronisierte Tabelle versehentlich ohne HLC beschrieben werden.
- `SqlValue` ist `rusqlite::types::Value`, BLOBs eingeschlossen. Die JSON-Parameter des heutigen
  `execute_with_crdt` bleiben für haex-vault unverändert.
- Das heutige freie `execute_with_crdt(sql, params, connection, …)` bleibt und ruft intern dieselbe
  Logik mit einer Anweisung auf.

## E2 Einstellbare Grenze je Transaktion

- `DatabaseConfig.max_transaction_bytes: usize`, Standard `MAX_CRDT_TRANSACTION_BYTES` (100 MiB).
- `CrdtTransaction` zählt die serialisierte Größe aller Parameter aller Anweisungen einer
  Transaktion und bricht mit `DatabaseError::TransactionTooLarge` ab, sobald die Summe die Grenze
  übersteigt, bevor die überschreitende Anweisung schreibt.
- `Database::max_transaction_bytes()` gibt den Wert heraus; holzi nutzt ihn als Obergrenze für eine
  Transaktionsgruppe im Sync (contracts/sync-protocol.md).

## Tests in haex-crdt

- Zwei `execute_with_crdt` in einem `write` → dieselbe `haex_hlc_no_sync` in beiden Zeilen, eine
  Transaktionsgruppe im Scanner.
- `Err` im Closure → keine Zeile, kein Eintrag im Lösch-Log.
- `execute_local` auf eine CRDT-Tabelle → Fehler; auf eine `_no_sync`-Tabelle → geschrieben.
- BLOB-Parameter kommen unverändert an.
- Summe über der eingestellten Grenze → `TransactionTooLarge`, nichts geschrieben.
- Das bestehende freie `execute_with_crdt` verhält sich wie vorher (vorhandene Tests grün).
