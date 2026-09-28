# Implementation Plan: Vault-Identität, Geräteliste und Datensync zwischen eigenen Geräten

**Branch**: `024-own-device-sync-plan` | **Date**: 2026-09-28 | **Spec**: [spec.md](./spec.md)

**Input**: Feature specification from `/specs/024-own-device-sync/spec.md`

## Summary

holzi bekommt echte Schlüssel, eine signierte Geräteliste und den direkten Datensync zwischen den
Geräten einer Vault. Die erste Instanz einer Vault ist ein Hauptgerät; weitere Geräte kommen
bevorzugt über Verknüpfen per Code dazu, als verknüpftes Gerät ohne den privaten Schlüssel der
Vault-Identität. Geräte finden sich über verschlüsselte Präsenzmeldungen auf Nostr-Relays, verbinden
sich direkt über iroh und tauschen verschlüsselte, signierte Änderungspakete mit lückenlosen
Laufnummern je Ursprungsgerät. Die Unteransicht „Geräte“ zeigt Rollen, Online-Stand und die
öffentliche Vault-Identität. Relay-Postfächer und Wiederherstellung folgen mit Spec 026.

Technischer Ansatz (Begründungen in [research.md](./research.md)):

- **Schlüssel** (R2, R3): Vault-Identität öffentlich synchronisiert, privat nur in
  `vault_identity_secret_no_sync` auf Hauptgeräten; Geräte- und iroh-Schlüssel je Installation in
  `device_keys_no_sync`; alte Vaults leiten ihre Identität deterministisch aus dem Platzhalter ab.
- **Paketprotokoll beim Ursprung** (R4): haex-crdt vergibt je lokaler Transaktion eine lückenlose
  Laufnummer; ein Versiegler macht daraus nach jedem Commit ein signiertes, verschlüsseltes
  Änderungspaket und legt es unveränderlich ab. Weitergegeben werden nur diese Pakete.
- **Prüfen beim Empfang** (R5): eigene `ApplyPolicy`, atomar je Paket, je Transaktionsgruppe bei
  Momentaufnahmen; Fortschritt und Paket im selben Commit.
- **Transport** (R6): ein iroh-Endpunkt je Vault-Session ohne n0-Adresssuche, ALPN
  `holzi-sync/1` und `holzi-link/1`, gegenseitiger Handshake mit Geräteschlüsseln, postcard-Rahmen.
- **Präsenz** (R7): flüchtige Nostr-Ereignisse im NIP-59-Aufbau an einen täglich wechselnden
  Postfach-Schlüssel aus dem Inhaltsschlüssel.
- **Geräteliste** (R8) als unveränderliche, signierte Zeilen; höchste Generation, dann kleinster
  Hash; entfernt bleibt entfernt.
- **Verknüpfen** (R11) mit 128-Bit-Code, Treffpunkt über Nostr, HMAC-Nachweis, Übertragung als
  Momentaufnahme.
- **haex-crdt**: vier kleine, allgemeine Änderungen upstream
  ([contracts/haex-crdt-upstream.md](./contracts/haex-crdt-upstream.md)), danach neue Revision
  pinnen.

## Technical Context

**Language/Version**: Rust (MSRV mindestens 1.91 wegen iroh 1.2), Tauri 2; TypeScript 6 (strict),
Vue 3.5, Nuxt 4.5 (SPA)

**Primary Dependencies**: neu `iroh` 1.2 (`tls-ring`), `nostr` 0.45 (`nip44`, `nip59`),
`nostr-sdk` 0.45, `secp256k1` 0.30, `hkdf` 0.13, `chacha20poly1305` 0.10, `postcard` 1, `qrcode`
0.14 (R1); vorhanden haex-crdt (neue Revision nach U1–U4), tokio, tokio-util, zeroize, sha2, serde,
ts-rs; Frontend nur Vorhandenes (haex-ui Group/Row/OptionRow, Pinia, vue-i18n)

**Storage**: SQLCipher-Vault über haex-crdt; neue Tabellen laut [data-model.md](./data-model.md)
(synchronisiert: `device_lists`, `vault_key_generations`, `vault_key_envelopes`,
`admission_requests`; gerätelokal: `vault_identity_secret_no_sync`, `device_keys_no_sync`,
`vault_content_keys_no_sync`, `sync_packages_no_sync`, `sync_progress_no_sync`,
`sync_peer_progress_no_sync`, `sync_sealer_no_sync`, `device_presence_no_sync`); Migration von
`vault_identity`

**Testing**: `cargo test` mit Unit-Tests in `*_tests.rs` und Integrationstests
`src-tauri/tests/sync_*.rs` (mehrere In-Prozess-Geräte, iroh ohne Relay, `MockRelay` für Nostr);
Tests in haex-crdt für U1–U4; Frontend `check:settings`, `check:templates`, `typecheck`, `lint`,
`format:check`; E2E (Spec 016) mit zwei App-Prozessen; manuell nach [quickstart.md](./quickstart.md)

**Target Platform**: Tauri-Desktop (Linux, macOS, Windows)

**Project Type**: desktop-app (Nuxt-SPA + Rust-Backend in einem Tauri-Projekt) plus eine
Upstream-Änderung an der Bibliothek haex-crdt

**Performance Goals**: SC-001 ≤ 5 s bis zur Anzeige auf dem anderen Gerät (95 %), SC-002 ≤ 30 s bis
zur Verbindung, SC-008 Online-Wechsel ≤ 60 s, SC-009 Verknüpfen < 2 min

**Constraints**: Sync blockiert nie die Bedienung (FR-032) und nie lokale Arbeit (Constitution
VII); Ende der Vault-Session beendet alles innerhalb der Fristen von Spec 013 (FR-031); private
Schlüssel nie in Protokollen (FR-002); Dateien ≤ 500 Zeilen; Rahmen ≤ 4 MiB

**Scale/Scope**: 1–10 Geräte je Vault; 1 neues Rust-Modul `sync/` mit rund 16 Dateien; 12 neue
Tabellen; 13 Befehle; 3 Ereignisse; 5 neue Aktionen; 2 Oberflächen (Unteransicht „Geräte“,
„Mit einer Vault verknüpfen“)

## Constitution Check

_GATE: Muss vor Phase 0 bestehen. Nach Phase 1 erneut geprüft — Ergebnis unten._

Geprüft gegen `.specify/memory/constitution.md` (v1.4.0) und die spaex-Constitution
`.spaex/constitution.md`.

| Prinzip / Vorgabe                                                          | Status | Begründung                                                                                                                                               |
| -------------------------------------------------------------------------- | ------ | -------------------------------------------------------------------------------------------------------------------------------------------------------- |
| I Keine Geheimnisse in Git                                                 | ✅     | Schlüssel entstehen zur Laufzeit; Testvektoren sind öffentliche NIP-44-Vektoren, keine echten Schlüssel                                                  |
| II Keine lokalen absoluten Pfade                                           | ✅     | Nur repo-relative Pfade                                                                                                                                  |
| III Projektidentität geräteunabhängig                                      | ✅     | Berührt nicht                                                                                                                                            |
| IV Cross-Repo-Referenzen gepinnt                                           | ✅     | haex-vault `8dce379d94e18fcd42c3b73686a06f984ca3f574`, haex-crdt `ed230d2c3f58c1b10710b6025ea0ce6c20b8d009` als Grundlage; nach U1–U4 die neue volle SHA |
| V Externe Quellen nur per Opt-in                                           | ✅     | Keine neue Harness-Quelle; neue Crates sind Bibliotheken, keine Harness-Inhalte                                                                          |
| VI Selbstverändernde Anweisungen review-pflichtig                          | ✅     | Keine Änderung an Constitution oder Skills                                                                                                               |
| VII Relay-Ausfall blockiert lokale Arbeit nicht                            | ✅     | Ohne Nostr- oder iroh-Relay findet der Sync keine Geräte; lokale Arbeit läuft unverändert (quickstart M9)                                                |
| VIII Keine Verheimlichung in Agent-Ausgaben                                | ✅     | –                                                                                                                                                        |
| Workflow: speckit-Stufen, PR auf `main`, Conventional Commits, kein Squash | ✅     | Spec über #156; Plan im Topic-Branch `024-own-device-sync-plan`                                                                                          |
| ADR bei prinzipienrelevanter Entscheidung                                  | ✅     | Keine Entscheidung berührt ein Prinzip; das Identitätsmodell steht im Entwurf (D26–D32)                                                                  |
| Neue Crates nur bei konkretem Problem                                      | ✅     | Jede neue Abhängigkeit löst eine Vorgabe der Spec, die das Vorhandene nicht kann (R1); Verworfenes in R1                                                 |
| Test-Code in separaten Dateien                                             | ✅     | `*_tests.rs` je Modul, Integrationstests unter `src-tauri/tests/`                                                                                        |
| Keine willkürlichen Wartezeiten in async-Tests                             | ✅     | Warten auf Ereignisse mit Zeitgrenze (R16)                                                                                                               |
| Kein blockierendes Arbeiten auf dem async-Executor                         | ✅     | SQLite-Zugriffe des Sync über `spawn_blocking` wie im übrigen Backend                                                                                    |
| Worktree je Änderung                                                       | ✅     | `.worktrees/024-own-device-sync-plan`                                                                                                                    |
| 500-LoC-Grenze                                                             | ✅     | `sync/` in kleine Dateien je Aufgabe; `FederationView.vue` wird aufgeteilt                                                                               |
| Graphify vor neuen benannten Artefakten                                    | ⚠️     | Abfragen in R18; der Graph (21.09.) ist älter als 022/023, Kandidaten zusätzlich im Code geprüft; zur manuellen Nachprüfung vermerkt                     |
| Nicht-triviale Logik hinterlässt einen ausführbaren Check                  | ✅     | Unit- und Integrationstests, E2E-Szenarien                                                                                                               |
| Keine Selbstreferenzen von Agenten in Artefakten/Commits                   | ✅     | Wird bei Commits eingehalten                                                                                                                             |
| **Phasen-Disziplin**                                                       | ✅     | Setzt 013, 016, 020, 022, 023 voraus, alle auf `main` und im Einsatz                                                                                     |

**Ergebnis vor Phase 0**: kein Verstoß; die Graphify-Einschränkung ist eine Warnung nach der Regel
für abgeschnittene Abfragen.

**Ergebnis nach Phase 1**: unverändert. Die Upstream-Änderungen an haex-crdt sind allgemein und
werden dort geprüft und gepinnt, bevor holzi sie nutzt.

## Project Structure

### Documentation (this feature)

```text
specs/024-own-device-sync/
├── plan.md                    # Dieses Dokument
├── research.md                # Phase 0 (R1–R18)
├── data-model.md              # Phase 1: Tabellen, Werte, Zustände
├── quickstart.md              # Phase 1: automatische (A1–A9) und manuelle (M1–M9) Validierung
├── contracts/
│   ├── sync-protocol.md       # holzi-sync/1, holzi-link/1: Handshake, Nachrichten, Pakete
│   ├── nostr-events.md        # Präsenz, Aufnahmeanfrage, Treffpunkt
│   ├── tauri-commands.md      # Befehle, Ereignisse, Aktionen, Orte
│   └── haex-crdt-upstream.md  # U1–U4
├── checklists/requirements.md # Spec-Qualitätscheckliste
└── tasks.md                   # Phase 2 — NICHT von /speckit-plan erzeugt
```

### Source Code (repository root)

```text
haex-crdt (eigenes Repository, PR vor holzi)
├── src/crdt/scanner/emit.rs, src/database/mod.rs   # U1 Ursprungsgerät
├── src/crdt/trigger/mod.rs, src/db/migrations/     # U2 haex_crdt_local_tx_no_sync
├── src/database/mod.rs                             # U3 apply_remote_changes_with_policy, U4 Commit-Beobachter
└── tests/                                          # Tests zu U1–U4

src-tauri/
├── Cargo.toml                         # neue Crates (R1), neue haex-crdt-Revision
├── src/sync/                          # NEU
│   ├── mod.rs                         # SyncService: Start nach dem Öffnen, Ende am Gate-Token
│   ├── keys.rs, keys_tests.rs         # Vault-Identität, Geräte-/iroh-Schlüssel, Ableitungen (R2, R3)
│   ├── signing.rs, signing_tests.rs   # Domänen-Tags, Preimages, Schnorr (R10)
│   ├── device_list.rs, _tests.rs      # Geräteliste: prüfen, wählen, zusammenführen (R8)
│   ├── content_keys.rs, _tests.rs     # Generationen, Umschläge (R9)
│   ├── package.rs, package_tests.rs   # SealedPackage: versiegeln, öffnen (R9)
│   ├── sealer.rs, sealer_tests.rs     # Versiegler nach Commit, Paketprotokoll (R4)
│   ├── progress.rs, progress_tests.rs # Fortschritt, Lücken, Fälschung (FR-019)
│   ├── policy.rs, policy_tests.rs     # ApplyPolicy/SignatureProvider (R5)
│   ├── endpoint.rs                    # iroh-Endpunkt, Router, MemoryLookup (R6)
│   ├── wire.rs, wire_tests.rs         # Rahmen, Nachrichten (contracts/sync-protocol.md)
│   ├── handshake.rs, _tests.rs        # gegenseitiger Nachweis (R6)
│   ├── session.rs                     # Austausch mit einem verbundenen Gerät
│   ├── presence.rs, presence_tests.rs # Nostr-Präsenz, Aufnahmeanfragen empfangen (R7, R12)
│   ├── link.rs, link_tests.rs         # Verknüpfen beider Seiten (R11)
│   ├── admission.rs                   # Kopie erkennen, Anfrage, Aufnehmen (R12)
│   ├── removal.rs, removal_tests.rs   # Entfernen (R13)
│   ├── servers.rs                     # Verbindungsserver aus den Einstellungen (FR-008)
│   ├── commands.rs, commands_tests.rs # Tauri-Befehle (contracts/tauri-commands.md)
│   └── events.rs                      # sync-devices-changed, sync-data-changed, link-state-changed
├── src/identity/bootstrap.rs          # echte Schlüssel statt Platzhalter, erste Geräteliste
├── src/identity/migrations.rs         # neue Migrationen, HOLZI_TRIGGER_VERSION +1
├── src/instances/open.rs, create.rs   # Sync nach dem Öffnen starten, Kopie erkennen; Vault aus Verknüpfen anlegen
├── src/instances/vault_config.rs      # eigener SignatureProvider statt Noop
├── src/storage/known_devices.rs       # Liste mit Rolle, online, zuletzt online
├── src/device/commands.rs             # list_vault_devices erweitert
├── src/lib.rs                         # Befehle registrieren
└── tests/
    ├── common/sync_fixture.rs         # NEU: In-Prozess-Geräte, MockRelay
    ├── sync_devices.rs                # NEU: US1, US2, US3
    ├── sync_three_devices.rs          # NEU: SC-003
    ├── sync_presence.rs               # NEU: Präsenz, SC-007
    └── sync_link.rs                   # NEU: Verknüpfen, Kopie, Entfernen

src/
├── composables/useSync.ts             # NEU: Befehle, Ereignisse
├── stores/syncDevices.ts              # NEU: Geräteliste live
├── lib/sync/deviceStatus.ts           # NEU: rein, „online“/„zuletzt online“, sortieren
├── lib/actions/settingsActions.ts     # neue Aktionen (contracts/tauri-commands.md)
├── stores/settingsActionHandlers.ts   # Handler dazu
├── components/settings/
│   ├── FederationView.vue             # Unteransicht „Geräte“ (Kopf, Liste, Knöpfe)
│   ├── DeviceRow.vue                  # NEU
│   ├── VaultIdentityRow.vue           # NEU
│   ├── LinkDeviceView.vue             # NEU: Code, Rollenfrage
│   ├── RemoveDeviceView.vue           # NEU: Rückfrage mit Folgen
│   ├── AdmissionRequests.vue          # NEU
│   └── SyncServersGroup.vue           # NEU
├── components/onboarding/LinkSheet.vue # NEU: „Mit einer Vault verknüpfen“
├── pages/index.vue                    # Knopf auf der Startseite
├── lib/settings/registry.ts           # Orte /federation/devices/link, …/remove
└── i18n/locales/{de,en}.json          # alle neuen Texte (FR-037)

scripts/e2e/scenarios/sync-*.ts        # NEU: US1, US2, US3, US5 mit zwei App-Prozessen
scripts/e2e/lib/nostr-relay.ts         # NEU: startet das Test-Relay
src-tauri/src/bin/e2e_nostr_relay.rs   # NEU, nur mit Feature `e2e`: MockRelay als Prozess
CONTEXT.md                             # Begriffe Hauptgerät, verknüpftes Gerät, Geräteliste, Änderungspaket, Laufnummer
plans/README.md                        # Zeile 024
```

**Structure Decision**: Ein neues Backend-Modul `src-tauri/src/sync/` neben `identity/` und
`instances/`; es hängt an Abbruch-Token und Task-Tracker des Vault-Gates (Spec 013). Die
Oberfläche erweitert die vorhandene Kategorie „Föderation“ (Spec 023) statt einer eigenen App.
Reine Anzeige-Logik unter `src/lib/sync/` (unter Node testbar wie `src/lib/settings/`).

## Umsetzungsreihenfolge

Jeder Schritt ist ein eigener PR und für sich prüfbar.

1. **haex-crdt U1–U4** im Repository haex-crdt, danach neue Revision in holzi pinnen.
2. **Schlüssel und Geräteliste** ohne Netz: Migrationen, Bootstrap, Ableitung aus dem Platzhalter,
   Geräteliste, Inhaltsschlüssel, Umschläge (FR-001 bis FR-006, FR-015, FR-038).
3. **Pakete, Versiegler, Fortschritt, Policy** mit In-Prozess-Austausch ohne Transport (FR-011 bis
   FR-022).
4. **iroh-Transport und Handshake** (FR-009, FR-010, FR-029 bis FR-032).
5. **Präsenz über Nostr** (FR-007, FR-008).
6. **Verknüpfen** (FR-023 bis FR-025).
7. **Kopie und Aufnahmeanfrage** (FR-044, FR-045).
8. **Entfernen** (FR-026 bis FR-028).
9. **Oberfläche** (FR-033 bis FR-037, FR-046).
10. **E2E-Szenarien** (SC-011).

## Anforderungen → Umsetzung

| Anforderungen                                             | Umsetzung                                                                                           |
| --------------------------------------------------------- | --------------------------------------------------------------------------------------------------- |
| FR-001 bis FR-004 (Identitäten, Platzhalter)              | `sync/keys.rs`, `identity/bootstrap.rs`, Migration (R2, R3)                                         |
| FR-005, FR-043 (Geräteliste, gleiche Generation)          | `sync/device_list.rs`, Tabelle `device_lists` (R8)                                                  |
| FR-006, FR-044, FR-045 (Kopie, Aufnahme)                  | `sync/admission.rs`, `instances/open.rs` (R12)                                                      |
| FR-007, FR-008, FR-010 (Präsenz, Server, Wiederaufbau)    | `sync/presence.rs`, `sync/servers.rs`, `sync/endpoint.rs` (R6, R7)                                  |
| FR-009 (Handshake)                                        | `sync/handshake.rs` (R6, contracts/sync-protocol.md)                                                |
| FR-011 bis FR-014 (Bereiche, Pakete, atomar)              | `sync/package.rs`, `sync/policy.rs` (R5, R9)                                                        |
| FR-015 (Inhaltsschlüssel)                                 | `sync/content_keys.rs` (R9)                                                                         |
| FR-016 bis FR-018 (was synchronisiert wird, Positivliste) | haex-crdt `_no_sync`; Positivliste als leere, geprüfte Konstante für andere Bereiche (für 027/028)  |
| FR-019, FR-020 (lückenloser Fortschritt)                  | haex-crdt U2, `sync/sealer.rs`, `sync/progress.rs` (R4)                                             |
| FR-021 (Autor)                                            | haex-crdt U1, `sig`-Eintrag, `sync/signing.rs` (R4, R10)                                            |
| FR-022 (Ersteller)                                        | Spalten-Regel in `sync/policy.rs`, in 024 ohne Tabelle                                              |
| FR-023 bis FR-025 (Verknüpfen)                            | `sync/link.rs`, `LinkDeviceView.vue`, `LinkSheet.vue` (R11)                                         |
| FR-026 bis FR-028 (Entfernen)                             | `sync/removal.rs`, `RemoveDeviceView.vue` (R13)                                                     |
| FR-029, FR-030 (Versionen, doppelt)                       | `sync/handshake.rs`, `sync/presence.rs` (R14)                                                       |
| FR-031, FR-032 (Session-Ende, Hintergrund)                | `sync/mod.rs` am Gate-Token, Ereignis `sync-data-changed`                                           |
| FR-033 bis FR-035, FR-046 (Oberfläche)                    | `FederationView.vue` und neue Komponenten (R15)                                                     |
| FR-036 (Agenten)                                          | Aktionen mit `agentCallable` nur für Lesen                                                          |
| FR-037 (de, en)                                           | i18n; `check:settings` prüft beide Sprachen                                                         |
| FR-038 (Nur-direkt-Daten)                                 | `vault_identity_secret_no_sync`, `link.rs` sendet ihn nur bei Rolle Hauptgerät                      |
| FR-039 bis FR-042 (Grundlagen für 027/028)                | Datenstrukturen und Regeln in `device_list.rs`, `content_keys.rs`, `policy.rs`; ohne Nutzung in 024 |
| Edge Case gegenseitiges Entfernen                         | `device_list.rs` (entfernt bleibt entfernt), Test in `device_list_tests.rs`, quickstart M7          |

## Complexity Tracking

Keine Einträge.
