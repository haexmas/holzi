# Contract: Tauri-Befehle, Ereignisse und Aktionen

Alle Befehle laufen durch den Wrapper des Vault-Gates (Spec 013); außer `link_join_*` brauchen sie
eine aktive Vault-Session. Fehler kommen als `HolziError` mit festen Codes. Typen für die
Oberfläche erzeugt `ts-rs` wie bisher.

## Befehle

| Befehl                                     | Eingabe                                                                       | Ergebnis                                             | Wer                                           |
| ------------------------------------------ | ----------------------------------------------------------------------------- | ---------------------------------------------------- | --------------------------------------------- |
| `sync_status`                              | –                                                                             | `SyncStatus`                                         | jedes Gerät                                   |
| `list_vault_devices` (erweitert, Spec 023) | –                                                                             | `VaultDevice[]`                                      | jedes Gerät                                   |
| `vault_public_identity`                    | –                                                                             | `{ npub: string, hex: string }`                      | jedes Gerät                                   |
| `link_code_create`                         | –                                                                             | `{ code: string, qrSvg: string, expiresAt: number }` | nur Hauptgerät                                |
| `link_code_cancel`                         | –                                                                             | –                                                    | nur Hauptgerät                                |
| `link_confirm`                             | `{ asMainDevice: boolean }`                                                   | –                                                    | nur Hauptgerät, nur während eines Verknüpfens |
| `link_reject`                              | –                                                                             | –                                                    | nur Hauptgerät                                |
| `link_join_start`                          | `{ code: string, vaultName: string, deviceName: string, passphrase: string }` | `{ state: 'waiting_for_confirmation' }`              | Startseite, ohne offene Vault                 |
| `link_join_status`                         | –                                                                             | `LinkJoinState`                                      | Startseite                                    |
| `link_join_cancel`                         | –                                                                             | –                                                    | Startseite                                    |
| `admission_decide`                         | `{ devicePubkey: string, admit: boolean }`                                    | –                                                    | nur Hauptgerät                                |
| `device_remove`                            | `{ devicePubkey: string }`                                                    | –                                                    | nur Hauptgerät, nicht für sich selbst         |
| `sync_copy_notice_dismiss`                 | –                                                                             | –                                                    | jedes Gerät                                   |
| `sync_servers_get`                         | –                                                                             | `{ nostrRelays: string[], irohRelays: string[] }`    | jedes Gerät                                   |
| `sync_servers_set`                         | `{ nostrRelays: string[], irohRelays: string[] }`                             | –                                                    | jedes Gerät                                   |

„Nur Hauptgerät“ prüft das Backend (`NotMainDevice`), nicht nur die Oberfläche. `passphrase` in
`link_join_start` wird wie beim Anlegen einer Vault behandelt (`Passphrase`, nie protokolliert).

### Typen

```ts
type DeviceRole = 'main' | 'linked'
type DeviceProblem = 'incompatible_version' | 'duplicate' | null

interface VaultDevice {
  vaultDeviceUuid: string
  devicePubkey: string
  alias: string | null
  role: DeviceRole
  isCurrent: boolean
  online: boolean
  lastSeen: number | null // ms; null = "noch nie online gesehen"
  problem: DeviceProblem
}

interface SyncStatus {
  thisDevice: 'main' | 'linked' | 'awaiting_admission' | 'removed'
  openAdmissions: { devicePubkey: string; name: string; requestedAt: number }[]
  linking: null | {
    stage: 'code_shown' | 'awaiting_confirmation'
    newDeviceName?: string
  }
  // Diese Vault-Datei ist die Kopie eines Hauptgeräts und dieses Gerät hat sich als Hauptgerät
  // eingetragen; die Nutzerin hat den Hinweis (FR-044) noch nicht gelesen.
  copyEnrolledAsMain: boolean
}

type LinkJoinState =
  | { state: 'searching' }
  | { state: 'waiting_for_confirmation' }
  | { state: 'transferring'; progress: number }
  | { state: 'done'; vaultName: string }
  | {
      state: 'failed'
      reason:
        | 'expired'
        | 'used'
        | 'wrong_code'
        | 'rejected'
        | 'incompatible'
        | 'connection_lost'
    }
```

## Ereignisse (Backend → Oberfläche)

| Ereignis                  | Nutzdaten               | Wann                                                                                              |
| ------------------------- | ----------------------- | ------------------------------------------------------------------------------------------------- |
| `sync-devices-changed`    | –                       | Geräteliste, Online-Stand, Name, Problem oder Aufnahmeanfragen ändern sich (FR-034)               |
| `sync-data-changed`       | `{ tables: string[] }`  | eine empfangene Seite wurde angewendet; offene Ansichten laden die betroffenen Daten neu (FR-032) |
| `link-host-state-changed` | `SyncStatus['linking']` | Fortschritt eines Verknüpfens auf dem Hauptgerät                                                  |
| `link-join-state-changed` | `LinkJoinState`         | Fortschritt eines Verknüpfens auf der neuen Installation                                          |

`sync-data-changed` ersetzt kein eigenes Neuladen der Stores; die vorhandenen Stores (Chat,
Einstellungen, Sitzung) hören darauf und laden gezielt.

## Aktionen (Katalog, Spec 020)

| Aktion                              | Agenten            | Wirkung                                       |
| ----------------------------------- | ------------------ | --------------------------------------------- |
| `settings.devices.list` (erweitert) | ja (lesen, FR-036) | Geräteliste mit Rolle, online, zuletzt online |
| `settings.devices.identity`         | ja (lesen)         | öffentlicher Schlüssel der Vault-Identität    |
| `settings.devices.link`             | nein               | öffnet die Verknüpfen-Ansicht                 |
| `settings.devices.admit`            | nein               | Aufnahmeanfrage annehmen oder ablehnen        |
| `settings.devices.remove`           | nein               | Gerät entfernen (mit Rückfrage)               |
| `settings.sync.servers.set`         | nein               | Verbindungsserver ändern                      |

Keine Aktion gibt private Schlüssel, Verknüpfungscodes oder Umschläge heraus (FR-036).

## Orte (Spec 020, Einstellungen)

- `/federation` bleibt der Einstieg und zeigt die Unteransicht „Geräte“ (Spec 023 FR-022).
- `/federation/devices/link` — Code anzeigen, danach Bestätigung mit Rollenfrage.
- `/federation/devices/:devicePubkey/remove` — Rückfrage mit den Folgen (FR-026).
