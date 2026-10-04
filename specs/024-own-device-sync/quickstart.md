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
| A8 E2E                    | `pnpm test:e2e --grep sync-`                                                             | mehrere App-Prozesse: US1 bis US7, M1 bis M9 (SC-011)                                                                                                                                                                    |
| A9 Klartext-Prüfung       | Teil von A3: Mitschnitt der Rahmen                                                       | kein Tabellen-, Spaltenname oder Inhalt im Klartext (SC-006)                                                                                                                                                             |

## Manuell

Keine mehr. M1 bis M9 laufen als Szenarien im E2E-Job (Spec 033) und lokal mit
`pnpm test:e2e --grep sync-`. Was jedes Szenario erwartet, steht in FR-002 bis FR-010 von Spec 033.

| Prüfung                    | Szenario                |
| -------------------------- | ----------------------- |
| M1 Verknüpfen              | `sync-link`             |
| M2 Sync                    | `sync-two-devices`      |
| M3 Rollen und Identität    | `sync-identity`         |
| M4 Online-Stand            | `sync-presence`         |
| M5 Kopie                   | `sync-copy`             |
| M6 Entfernen               | `sync-remove-device`    |
| M7 Gegenseitiges Entfernen | `sync-mutual-removal`   |
| M8 Sperren                 | `sync-lock-during-sync` |
| M9 Server                  | `sync-servers-off`      |

`--grep sync-` startet außerdem `sync-relay-return` (Nostr-Relay kommt unter derselben Adresse
zurück, Gate G1 von Spec 033), `sync-away-through-key-change` (ein Gerät war beim Schlüsselwechsel
weg), `sync-indirect`, `sync-own-devices-only`, `sync-two-users`, `appearance-sync-two-devices` und
`passwords-sync-two-devices`, zusammen 16 Szenarien.
