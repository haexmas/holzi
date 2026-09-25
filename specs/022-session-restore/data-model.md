# Data Model: Sitzung wiederherstellen (wählbar)

## Einstellung „Sitzung wiederherstellen“

Gespeichert in der bestehenden, synchronisierten Tabelle `preferences`
(Migration 0011, `src-tauri/src/storage/preferences.rs`).

| Feld   | Wert                                                                |
| ------ | ------------------------------------------------------------------- |
| Key    | `wm.session_restore`                                                |
| Scopes | Vault (`VAULT_SCOPE_UUID`) und optional Gerät (`vault_device_uuid`) |
| Wert   | `'true'` oder `'false'`; eine fehlende Zeile heißt „nicht gesetzt“  |

**Geltender Wert** (FR-002, FR-003): Gerätewert, falls gesetzt; sonst
Vault-Wert, falls gesetzt; sonst `false`. Ein Wert, der weder `'true'` noch
`'false'` ist, zählt als nicht gesetzt.

`SessionRestoreState` (Rückgabe der Befehle, siehe
[contracts/wm-session.md](./contracts/wm-session.md)):

```ts
type SessionRestoreState = {
  device: boolean | null // null = nicht gesetzt
  vault: boolean | null
  effective: boolean
}
```

## Gespeicherte Sitzung

Tabelle `wm_sessions_no_sync` (Migration 0020). Die Endung `_no_sync` nimmt sie
von der Synchronisierung aus (research R1).

| Spalte              | Typ                | Regel                                                               |
| ------------------- | ------------------ | ------------------------------------------------------------------- |
| `vault_device_uuid` | `TEXT PRIMARY KEY` | Gerät, dem die Sitzung gehört; von Rust ermittelt, nie vom Frontend |
| `session_json`      | `TEXT NOT NULL`    | `WmSession` als JSON, höchstens 1 MiB                               |
| `updated_at`        | `TEXT NOT NULL`    | RFC 3339, nur zur Diagnose                                          |

Kein Fremdschlüssel (research R1). Gelesen und geschrieben wird immer nur die
Zeile des eigenen Geräts.

**Lebenszyklus**:

```text
          Einstellung gilt nicht                 Einstellung gilt
 ┌──────────────────────────────┐   an   ┌──────────────────────────────┐
 │ keine Zeile                   │ ─────▶ │ Zeile wird bei jeder Änderung │
 │ (load/set löschen eine alte)  │ ◀───── │ überschrieben (save)          │
 └──────────────────────────────┘   aus  └──────────────────────────────┘
                                   (set löscht sofort, FR-007)
```

## `WmSession` (Frontend-Typ, Inhalt von `session_json`)

Ersetzt `PersistedLayout` in `src/lib/wm/types.ts`; `hydrate` nimmt ihn
unverändert entgegen.

```ts
type WmSession = {
  version: 1
  workspaces: { id: string; position: number }[]
  windows: {
    id: string
    workspaceId: string
    x: number
    y: number
    width: number
    height: number
    minimized: boolean
    maximized: boolean
    stack: number
    tabs: { id: string; appId: string }[]
    activeTabId: string
  }[]
  activeWorkspaceId: string
}
```

Nicht enthalten, wie in Spec 015 und 020: Tab-Inhalte, Tab-Orte und
Vor-/Zurück-Historie, Titel, Aufmerksamkeitsmarken, Schließwächter.

**Beim Laden** (FR-012, Spec 015 FR-025): Ist das JSON ungültig, hat es eine
andere `version` oder fehlen Pflichtfelder, startet holzi leer und
protokolliert den Fehler. Unbekannte Apps, doppelte Einzelinstanzen und
Fenster außerhalb des sichtbaren Bereichs behandelt `hydrate` wie bisher.

## Wartungsaufgaben

Tabelle `holzi_maintenance_no_sync` (Migration 0020), eine Zeile je offener
Aufgabe.

| Spalte | Typ                | Regel                                          |
| ------ | ------------------ | ---------------------------------------------- |
| `task` | `TEXT PRIMARY KEY` | derzeit nur `vacuum_after_legacy_wm_drop` (R6) |

Die Migration legt die Aufgabe an; der erste erfolgreiche Start nach dem Öffnen
arbeitet sie ab und löscht die Zeile.

## Entfernte Altdaten (Migration 0020 und Start)

| Was                                                            | Wie                                          | Rest (FR-011)                                              |
| -------------------------------------------------------------- | -------------------------------------------- | ---------------------------------------------------------- |
| `shell_window_tabs`, `shell_windows`, `workspaces`             | `DROP TABLE` in 0020, Kinder zuerst          | keiner; alte Löschvermerke aus 015 bleiben (nur Kennungen) |
| `preferences` mit Key `shell.active_workspace_id`, alle Geräte | `DELETE` bei jedem Start (idempotent)        | Löschvermerk mit Gerätekennung und Key                     |
| freie Seiten und WAL                                           | einmal `VACUUM` + `wal_checkpoint(TRUNCATE)` | keiner                                                     |
