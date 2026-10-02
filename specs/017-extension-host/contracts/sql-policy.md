# Vertrag: SQL von Erweiterungen

Begründung: [research.md](../research.md) R6–R8. Gilt für `extension_database_query`, `_execute`,
`_transaction` und für Migrationen.

## Begriffe

- **Präfix** einer Erweiterung: `<publicKey>__<name>__`. Ein Tabellenname wird exakt in genau drei Teile an
  `__` zerlegt (ohne Rücksicht auf Groß-/Kleinschreibung); Name der Erweiterung und Tabelle enthalten kein `__`.
- **Eigene Tabelle**: Präfix = Präfix der aufrufenden Erweiterung.
- **Fremde Tabelle**: Präfix einer anderen installierten Erweiterung.
- **Kerntabelle**: alles andere, einschließlich `sqlite_*`, `haex_*`, `pragma_*`-Funktionen.

## Laufzeit

### Vorprüfung (holzi)

| Regel                                                                              | Ergebnis bei Verstoß              |
| ---------------------------------------------------------------------------------- | --------------------------------- |
| genau eine Anweisung (`parse_sql_statements(...).len() == 1`)                      | 1000                              |
| Art ∈ {Query, Insert, Update, Delete}                                              | 1000                              |
| keine Sync-Spalten (`haex_*`) gesetzt oder gelesen per Name                        | 1000                              |
| Qualifizierer nur `main` oder keiner                                               | 1000                              |
| Bezeichner nur ASCII                                                               | 1000                              |
| kein `WITH`-Name gleich einem echten Tabellennamen                                 | 1000                              |
| SQL-Länge ≤ `max_sql_bytes`                                                        | 7000                              |
| jede Tabelle aus `visit_relations` (ohne eigene `WITH`-Namen) ist eigen oder fremd | Kerntabelle → 1000 ohne Rückfrage |
| fremde Tabelle: Leseberechtigung (Query) bzw. Lesen und Schreiben (DML)            | fehlt → 1004 (Anfrage) bzw. 1002  |

### Authorizer (haex-crdt `SqlGuard`, entscheidet)

Oberste Ebene (`accessor = None`):

| Aktion                       | erlaubt, wenn                                                          |
| ---------------------------- | ---------------------------------------------------------------------- |
| `Read`, `Select`             | `database = main` und Tabelle eigen oder fremd mit Leseberechtigung    |
| `Insert`, `Update`, `Delete` | `database = main` und Tabelle eigen oder fremd mit Lesen und Schreiben |
| `Function`                   | Funktion in der Erlaubtliste (unten)                                   |
| `Recursive`                  | immer (rekursive `WITH`)                                               |
| alles andere                 | nie                                                                    |

In Triggern: erlaubt nur, wenn `accessor` `z_dirty_<T>_(insert|update|delete)` ist und T eine Tabelle, die
diese Anweisung schreiben darf.

**Erlaubte Funktionen**: Kernfunktionen von SQLite (`abs`, `coalesce`, `ifnull`, `iif`, `instr`, `length`,
`lower`, `upper`, `ltrim`, `rtrim`, `trim`, `max`, `min`, `nullif`, `printf`, `format`, `quote`, `random`,
`randomblob`, `replace`, `round`, `sign`, `substr`, `substring`, `typeof`, `unicode`, `char`, `hex`, `unhex`,
`zeroblob`, `glob`, `like`, `likely`, `unlikely`, `changes`, `last_insert_rowid`, `total_changes`),
Aggregate und Fensterfunktionen, Datum und Zeit, Mathefunktionen, JSON-Funktionen einschließlich
`json_each`/`json_tree` (Tabellenwertfunktionen nur, wenn der Test zeigt, dass sie nicht über Kerntabellen
lesen können). Funktionen, die der CRDT-Transformer selbst in die Anweisung einsetzt, erlaubt der Authorizer
nur, wenn sie aus dem Transformer stammen (Prüfaufgabe: ob der Transformer HLC-Werte als Literal oder als
Funktionsaufruf einsetzt); eine Erweiterung, die sie selbst aufruft, wird in der Vorprüfung abgelehnt. **Nie**: `load_extension`, `fts3_tokenizer`, `sqlite_compileoption_*`,
`sqlite_offset`, `sqlcipher_*`, `pragma_*`, unbekannte UDFs.

Lehnt der Authorizer etwas ab, was die Vorprüfung durchgelassen hat: 1000 „Form nicht zuordenbar“.

### Ausführung

- Ergebnis-Anweisungen (`SELECT`, `RETURNING`) über `query_map`, sonst `execute`.
- Ergebnis `{rows, columns, rowsAffected, lastInsertId}`; Sync-Spalten entfernt.
- Werte: BLOB → Base64-Text; INTEGER → Zahl; REAL NaN/Inf → `null`. Parameter: Zahl, Text, `null`,
  Wahrheitswert → 0/1, `{"$bytes": b64}` → BLOB, Array/Objekt → JSON-Text.
- Grenzen: Zeilen und Bytes im Callback, Laufzeit über Fortschritts-Callback (`SQLITE_INTERRUPT`),
  gleichzeitige Anfragen per Semaphore. Überschreitung → 7000, die Transaktion wird zurückgerollt.
- Transaktion: alle Anweisungen erst vorgeprüft, dann in einem `write_guarded`; `rowsAffected` = Summe.
- Schreibanweisungen laufen durch den CRDT-Transformer (Zeitstempel, Löschmarken, Größengrenze), FR-028.

## Migrationen

Zusätzlich zur Laufzeitprüfung je Anweisung:

| erlaubt                                             | Bedingung                                                                                                                                |
| --------------------------------------------------- | ---------------------------------------------------------------------------------------------------------------------------------------- |
| `CREATE TABLE`                                      | eigener Name oder `__new_<eigenes Präfix>…`; kein `AS SELECT`; keine `TEMP`; keine Spalte `haex_*`; `REFERENCES` nur auf eigene Tabellen |
| `CREATE [UNIQUE] INDEX`                             | auf eigener Tabelle                                                                                                                      |
| `ALTER TABLE … ADD/RENAME/DROP COLUMN`, `RENAME TO` | eigene Tabelle; kein Umbenennen über die Grenze `_no_sync`                                                                               |
| `DROP TABLE`, `DROP INDEX`                          | eigene Tabelle bzw. Index darauf                                                                                                         |
| `INSERT`/`UPDATE`/`DELETE`                          | nur eigene Tabellen (auch der Umbau `INSERT INTO __new_… SELECT … FROM …`)                                                               |
| `PRAGMA foreign_keys = OFF/ON`                      | nur als Steuerung des Schema-Modus, wird nicht ausgeführt                                                                                |

Verboten: `VIEW`, `TRIGGER`, `VIRTUAL`, `ATTACH`, `DETACH`, jedes andere `PRAGMA`, `VACUUM`, Transaktionssteuerung.

Ablauf je Migration in einem `write_guarded` im Schema-Modus (Fremdschlüssel aus, `foreign_key_check` vor dem
Commit): Anweisungen ausführen → für jede geänderte synchronisierte Tabelle Trigger neu anlegen → Umbau mit
unveränderten Sync-Spalten → Journalzeile schreiben. Scheitert etwas, wird alles zurückgerollt (FR-034).

## Umgehungssammlung (SC-002, `src-tauri/tests/extension_sql_bypass.rs`)

Jeder Fall muss von der Vorprüfung **und** vom Authorizer jeweils allein abgelehnt werden. Mindestens:

- `WITH x AS (SELECT * FROM chat_threads) SELECT * FROM x`; `WITH` mit eigenem Präfix als Name über eine
  Kerntabelle; `WITH` mit dem Namen einer Kerntabelle
- `EXISTS`, Unterabfragen in `SELECT`, `WHERE`, `HAVING`, `ORDER BY`, `LIMIT`, `VALUES`, `RETURNING`,
  `ON CONFLICT … DO UPDATE SET x = (SELECT …)`, `JOIN … ON (SELECT …)`, `CASE`, Funktionsargumente
- `main.chat_threads`, `temp.x`, `"Chat_Threads"`, `[chat_threads]`, `` `chat_threads` ``
- Präfix-Kollision `a` gegen `a__b`; Tabelle einer nicht installierten Erweiterung
- `sqlite_master`, `sqlite_schema`, `pragma_table_info('chat_threads')`, `json_each((SELECT … FROM
chat_threads))`
- `load_extension(...)`, `sqlcipher_export(...)`, `fts3_tokenizer(...)`
- `BEGIN`, `COMMIT`, `SAVEPOINT`, `ATTACH`, zwei Anweisungen in einer Zeichenkette, Kommentar-Tricks
- Schreiben in `haex_hlc_no_sync` und andere Sync-Spalten
- in Migrationen: `CREATE VIEW`, `CREATE TRIGGER`, `CREATE VIRTUAL TABLE`, `CREATE TEMP TABLE`, Spalte
  `haex_hlc_no_sync` in einer `_no_sync`-Tabelle, `REFERENCES chat_threads`, `PRAGMA writable_schema`,
  `INSERT INTO chat_threads …`, `UPDATE haex_crdt_configs_no_sync …`
- `DELETE` in einer eigenen `_no_sync`-Tabelle erzeugt keine Löschmarke (FR-029)
