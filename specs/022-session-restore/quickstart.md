# Quickstart: Sitzung wiederherstellen (wählbar)

## 1. Automatische Prüfungen

```sh
nix develop --command pnpm typecheck
nix develop --command pnpm lint
nix develop --command pnpm format:check
nix develop --command pnpm check:wm-state        # enthält die Speicher-Warteschlange (useWmSession)
nix develop --command pnpm check:wm-navigation
nix develop --command pnpm check:chat-state
nix develop --command pnpm check:templates
nix develop --command scripts/with-nix-host-bridge.sh cargo fmt --manifest-path src-tauri/Cargo.toml --check
nix develop --command scripts/with-nix-host-bridge.sh pnpm lint:rust
nix develop --command scripts/with-nix-host-bridge.sh cargo test --manifest-path src-tauri/Cargo.toml
```

Erwartet: alles grün; die Rust-Tests enthalten die neuen `wm_session_*`-Tests
und die Migrationstests für 0020 (frische und aktualisierte Vault).

## 2. Manuelle Szenarien

Mit `pnpm tauri:dev` (oder `tauri:dev:cuda`). „Neu öffnen“ heißt: holzi
beenden und die Vault erneut öffnen, oder sperren und entsperren.

| Nr. | Schritte                                                                                                                                                                               | Erwartet                                                                                                             | Deckt           |
| --- | -------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------- | -------------------------------------------------------------------------------------------------------------------- | --------------- |
| S1  | Frische Vault. Zwei Arbeitsbereiche, Chat und Einstellungen als Fenster, ein Fenster verschieben. Neu öffnen.                                                                          | Ein Arbeitsbereich, kein Fenster.                                                                                    | US1, SC-001     |
| S2  | Wie S1, aber sperren und entsperren statt beenden.                                                                                                                                     | Ein Arbeitsbereich, kein Fenster.                                                                                    | US1 AS2         |
| S3  | Einstellungen öffnen, Zeit messen bis „Sitzung wiederherstellen“ für dieses Gerät eingeschaltet ist.                                                                                   | Unter 30 Sekunden; Anzeige „Dieses Gerät: an“, „Gilt hier: an“.                                                      | US2, SC-005     |
| S4  | Nach S3 zwei Arbeitsbereiche mit Fenstern und Tabs einrichten, einen minimieren, einen maximieren, den zweiten Arbeitsbereich aktiv lassen. Neu öffnen.                                | Alles wie verlassen (Spec 015 US5); Chat-Tab beginnt mit neuer Unterhaltung, kein Tab hat eine Vor-/Zurück-Historie. | US2, SC-002     |
| S5  | Nach S4 die Einstellung für dieses Gerät ausschalten.                                                                                                                                  | Fenster bleiben offen. Neu öffnen: leer.                                                                             | US4, FR-007     |
| S6  | Vault-Wert „an“, Gerätewert zurücksetzen. Fenster öffnen, neu öffnen.                                                                                                                  | Sitzung kommt zurück (Vault-Wert gilt).                                                                              | US3 AS1, AS5    |
| S7  | Vault-Wert „an“, Gerätewert „aus“. Fenster öffnen, neu öffnen.                                                                                                                         | Leer.                                                                                                                | US3 AS2         |
| S8  | Zweites Gerät (oder zweite Installation mit eigener Installationskennung) mit derselben Vault-Datei, Vault-Wert „an“. Auf beiden Geräten unterschiedliche Fenster, jeweils neu öffnen. | Jedes Gerät sieht nur seine Sitzung.                                                                                 | US3 AS4, SC-004 |
| S9  | Vault, die mit dem Stand vor 022 eine Sitzung gespeichert hat (z. B. `main` vor dem Merge), mit 022 öffnen.                                                                            | Leer. Siehe Prüfung P1.                                                                                              | US5, FR-009     |
| S10 | Mit Deep-Link starten (`/workspace/<vault>?open=system.settings`), Einstellung aus.                                                                                                    | Nur die Einstellungen offen.                                                                                         | US1 AS4, FR-014 |
| S11 | Wie S10, Einstellung an und gespeicherte Sitzung vorhanden.                                                                                                                            | Wiederhergestellte Sitzung plus die Einstellungen.                                                                   | FR-014          |
| S12 | Einstellung an, Fenster öffnen, holzi hart beenden (`kill -9`). Neu öffnen.                                                                                                            | Sitzung kommt zurück, höchstens die letzte halbe Sekunde fehlt.                                                      | Edge Case       |
| S13 | Zeit vom Klick auf „Entsperren“ bis zum Arbeitsbereich mit S9 (einmalige Bereinigung) und danach ohne Bereinigung vergleichen.                                                         | Unterschied höchstens eine Sekunde.                                                                                  | SC-006          |

### P1: Vault-Datei untersuchen (SC-003, FR-010, FR-011)

Nach S5 und nach S9, bei beendetem holzi, die Vault mit `sqlcipher` und der
Passphrase öffnen:

```sql
PRAGMA key = '<passphrase>';
SELECT name FROM sqlite_master WHERE name IN ('workspaces','shell_windows','shell_window_tabs');  -- leer
SELECT count(*) FROM wm_sessions_no_sync;              -- 0 nach S5 für dieses Gerät
SELECT count(*) FROM preferences WHERE key = 'shell.active_workspace_id';  -- 0
SELECT * FROM holzi_maintenance_no_sync;               -- leer
SELECT table_name, row_pks FROM haex_deleted_rows
 WHERE table_name IN ('workspaces','shell_windows','shell_window_tabs','preferences');
-- nur Kennungen, keine App-Namen, keine Positionen
```

Freie Seiten: Direkt nach S9 muss `PRAGMA freelist_count;` 0 ergeben (das
einmalige `VACUUM` hat die Datei neu geschrieben) und die WAL-Datei neben der
Vault leer oder nicht vorhanden sein. Dass spätere Löschungen (S5) ihre Seiten
überschreiben, sichert `PRAGMA secure_delete = ON` auf der Verbindung; das prüft
ein Rust-Test (`PRAGMA secure_delete` liefert 1 nach dem Öffnen), weil man
überschriebene freie Seiten von außen nicht sinnvoll ansehen kann.
