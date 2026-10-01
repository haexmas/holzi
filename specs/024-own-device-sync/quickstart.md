# Quickstart: Validierung von Spec 024

Voraussetzungen: `nix develop` (Rust-Werkzeuge, Host-Bridge für WebKit), `pnpm install` im
Worktree (kein Symlink auf `node_modules`), gebaute E2E-App nach Spec 016. Die
haex-crdt-Revision mit E1/E2 (contracts/haex-crdt-upstream.md) ist in `src-tauri/Cargo.toml` gepinnt.

## Automatisch

| Schritt                   | Befehl                                                                                   | Erwartet                                                                                                                                                                                                                 |
| ------------------------- | ---------------------------------------------------------------------------------------- | ------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------ |
| A1 Unit-Tests Sync        | `cargo test --manifest-path src-tauri/Cargo.toml sync::`                                 | NIP-44-Vektoren, Schnorr, Ableitungen (R3, R7, R11), Regeln der Geräteliste inkl. gegenseitigem Entfernen, Liefern je Ursprung, Fortschritt erst nach dem Commit, Grenze eines entfernten Geräts, Prüfung je Gruppe grün |
| A2 Migrationen            | `cargo test --manifest-path src-tauri/Cargo.toml identity::`                             | Platzhalter → echte Identität, zwei Kopien desselben Platzhalters → dieselbe Identität; `privkey` nicht mehr in `vault_identity`                                                                                         |
| A3 Mehrgeräte-Integration | `cargo test --manifest-path src-tauri/Cargo.toml --test sync_devices`                    | zwei/drei In-Prozess-Geräte (iroh ohne iroh-Relay, `MemoryLookup`), US1, US2, US3                                                                                                                                        |
| A4 SC-003                 | `cargo test --manifest-path src-tauri/Cargo.toml --test sync_three_devices -- --ignored` | 1.000 Änderungen über wechselnde Wege mit Abbrüchen, alle gleich, jede mit ihrem Ursprungsgerät (SC-003, SC-014)                                                                                                         |
| A5 Präsenz                | `cargo test --manifest-path src-tauri/Cargo.toml --test sync_presence`                   | `MockRelay` als Nostr-Relay: Präsenz wird nur von eigenen Geräten entschlüsselt; Ein-Gerät-Vault veröffentlicht nichts (SC-007); Kopie wird gefunden                                                                     |
| A6 Verknüpfen             | `cargo test --manifest-path src-tauri/Cargo.toml --test sync_link`                       | falscher/abgelaufener/verbrauchter Code scheitert; ohne Rolle kein privater Schlüssel auf N (SC-009, SC-012); Abbruch hinterlässt nichts                                                                                 |
| A7 Frontend-Checks        | `pnpm check:settings && pnpm check:templates && pnpm typecheck && pnpm lint`             | Unteransicht „Geräte“, i18n de/en, keine direkten Tauri-Aufrufe außerhalb der Aktionen                                                                                                                                   |
| A8 E2E                    | `pnpm test:e2e --grep sync-`                                                             | zwei App-Prozesse: US1, US2, US3, US5 (SC-011)                                                                                                                                                                           |
| A9 Klartext-Prüfung       | Teil von A3: Mitschnitt der Rahmen                                                       | kein Tabellen-, Spaltenname oder Inhalt im Klartext (SC-006)                                                                                                                                                             |

## Manuell

Zwei Rechner (oder zwei Instanzen mit getrennten App-Daten-Verzeichnissen), holzi aus diesem Branch.

- **M1 Verknüpfen** (US5): Auf A eine Vault anlegen. In Einstellungen → Föderation → „Gerät
  verknüpfen“. Auf B auf der Startseite „Mit einer Vault verknüpfen“, Code eingeben, Gerätename und
  Passphrase vergeben. A zeigt den Namen und fragt nach der Rolle; „nein“ lassen, bestätigen.
  Erwartet: B öffnet die Vault mit allen Daten; beide zeigen B als „verknüpftes Gerät“, online.
- **M2 Sync** (US1): Auf A einen Chat beginnen → erscheint in wenigen Sekunden auf B. Auf B eine
  Einstellung ändern → gilt auf A. B beenden, auf A arbeiten, B öffnen → holt alles nach.
- **M3 Rollen und Identität** (US4): Auf beiden dieselbe öffentliche Vault-Identität (`npub…`),
  kopierbar, nicht änderbar. Auf B fehlen „Gerät verknüpfen“ und „Gerät entfernen“.
- **M4 Online-Stand** (US4): B beenden → auf A binnen 60 s „zuletzt online“ mit passender Zeit.
- **M5 Kopie** (US7): Datei von A auf C kopieren und öffnen → C ist Hauptgerät, Hinweis erscheint,
  synchronisiert. Datei von B auf D kopieren und öffnen → D zeigt „wartet auf Aufnahme“; auf A
  „Aufnehmen“ → D synchronisiert, auch seine Änderungen aus der Wartezeit.
- **M6 Entfernen** (US6): Auf A das Gerät D entfernen; die Folgen stehen vor der Bestätigung. D
  bekommt nichts Neues mehr und zeigt, dass es entfernt wurde. A, B, C synchronisieren weiter.
- **M7 Gegenseitiges Entfernen** (Edge Case): A und C offline nehmen, auf A C entfernen und auf C A
  entfernen, dann beide online. Erwartet: auf allen Geräten bleibt dasselbe der beiden Hauptgerät
  (das mit der Liste mit dem kleineren Hash), das andere ist entfernt und zeigt das; B hat
  weiterhin ein Hauptgerät.
- **M8 Sperren** (FR-031): Während eines großen Abgleichs die Vault auf A sperren → alle
  Verbindungen enden sofort; beim nächsten Öffnen läuft der Abgleich weiter, nichts fehlt.
- **M9 Server** (FR-008): In „Verbindungsserver“ alle Nostr-Relays abschalten (Kästchen) → Geräte finden sich nicht
  mehr, lokale Arbeit geht weiter (Constitution VII); wieder anschalten → finden sich wieder.
