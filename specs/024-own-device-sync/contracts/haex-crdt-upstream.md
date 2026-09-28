# Contract: Änderungen an haex-crdt

Repository `https://github.com/haexmas/haex-crdt`, Grundlage Revision
`ed230d2c3f58c1b10710b6025ea0ce6c20b8d009` (heute in holzi gepinnt). Die Änderungen kommen als PR
in haex-crdt; holzi pinnt danach die neue Revision (Constitution IV). Sie sind allgemein und
enthalten nichts holzi-Eigenes.

## U1 Ursprungsgerät in `ColumnChange.device_id`

- Heute füllt der Scanner `device_id` mit dem scannenden Gerät (`src/database/mod.rs`,
  `src/crdt/scanner/emit.rs`), für weitergeleitete Zellen falsch.
- Neu: `device_id` ist der Knoten des HLC-Zeitstempels der Zelle, dekodiert wie in `src/crdt/hlc.rs`
  (`parse_hlc_node_hex`, little-endian UUID). Der Parameter `device_id` des Scanners entfällt oder
  wird ignoriert; der Test in `scanner/tests.rs`, der das alte Verhalten festhält, wird angepasst.
- Rückwärtsverträglich für haex-vault: dort ist der Knoten ebenfalls die Geräte-UUID.

## U2 Laufnummer je lokal begonnener Transaktion

- Neue Crate-Migration: Tabelle `haex_crdt_local_tx_no_sync(hlc TEXT PRIMARY KEY, node TEXT NOT
NULL, seq INTEGER NOT NULL, UNIQUE(node, seq))`.
- Die Trigger für INSERT, UPDATE und DELETE fügen, nur wenn `triggers_enabled = '1'`, ein:
  `INSERT INTO haex_crdt_local_tx_no_sync(hlc, node, seq) SELECT <hlc>, <node>,
COALESCE((SELECT MAX(seq) FROM haex_crdt_local_tx_no_sync WHERE node = <node>), 0) + 1 ON
CONFLICT(hlc) DO NOTHING`. Eine Transaktion teilt sich einen HLC und bekommt so genau eine
  Nummer; ein Rollback entfernt die Zeile, die Folge bleibt lückenlos.
- Lesehilfe: `Database::local_transactions_after(node, after_seq) -> Vec<(hlc, seq)>`.
- Die Trigger-Version steigt; Verbraucher (holzi `HOLZI_TRIGGER_VERSION`) installieren neu.

## U3 Anwenden mit eigener Policy

- Neu: `Database::apply_remote_changes_with_policy(&self, changes: Vec<ColumnChange>, policy: &mut
dyn ApplyPolicy) -> Result<ApplyOutcome>`; `apply_remote_changes` ruft es mit der bisherigen
  `SignatureApplyPolicy` auf.
- Zweck: holzi prüft Autor, Laufnummer und Atomarität in `preflight`, `begin`, `prepare_row` und
  `before_commit` und schreibt im selben Commit Fortschritt und Paket.

## U4 Commit-Beobachter

- Neu: `Database::set_commit_observer(Box<dyn Fn() + Send + Sync>)`, über den `commit_hook` von
  rusqlite; ruft nach jedem erfolgreichen Commit den Beobachter auf, ohne Argumente und ohne auf der
  Verbindung zu arbeiten.
- Zweck: der Versiegler in holzi wacht nach lokalen Schreibvorgängen auf, ohne dass jede der
  Schreibstellen in holzi daran denken muss. Commits von `apply` wecken ihn auch; er findet dann
  nichts Neues in `haex_crdt_local_tx_no_sync` und schläft weiter.

## Tests in haex-crdt

- U1: eine Zelle von Knoten A, über Knoten B gescannt, trägt `device_id` = A.
- U2: zwei Transaktionen → `seq` 1 und 2; Rollback dazwischen → keine Lücke; `apply` erzeugt keine
  Zeile.
- U3: eine Policy, die in `before_commit` einen Fehler liefert, hinterlässt keine Zeile.
- U4: der Beobachter läuft genau einmal je Commit, nicht bei Rollback.
