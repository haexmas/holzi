# Vertrag: Tauri-Commands und Ereignisse für holzis Oberfläche

Nur holzis eigenes Frontend ruft diese Commands auf. Eine Erweiterung erreicht ausschließlich
`extension_bridge_call` und nur über die Weiterleitung im Frontend ([bridge.md](./bridge.md)). Alle Commands
laufen über `VaultDb` und sind nach dem Schließen der Vault gesperrt (ADR-0003). Typen per ts-rs nach
`src/types/bindings/`. Fehlerart `HolziError::Extension*` ohne Daten der Erweiterung im Text.

## Installation und Verwaltung (L1, L3)

| Command                     | Eingabe                                                  | Ausgabe                                                                                                                                                                                               | Hinweise                                            |
| --------------------------- | -------------------------------------------------------- | ----------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------- | --------------------------------------------------- |
| `extension_install_preview` | `{path}` (aus dem Datei-Dialog)                          | `InstallPreview {name, displayName, version, publisherFingerprint, signatureValid, error?, declared: DeclaredPermission[], unsupportedCategories, existing?: {version, isDowngrade, newPermissions}}` | liest die Datei in Rust; kein Schreiben             |
| `extension_install`         | `{path, accepted: PermissionChoice[], confirmDowngrade}` | `ExtensionSummary`                                                                                                                                                                                    | prüft erneut, schreibt BLOBs einzeln, dann Registry |
| `extension_list`            | –                                                        | `ExtensionSummary[]` (inkl. Zustand je Gerät, wirksame Fassung, Größe der Daten)                                                                                                                      | auch Erweiterungen mit behaltenen Daten             |
| `extension_set_enabled`     | `{extensionId, enabled}`                                 | –                                                                                                                                                                                                     | L3                                                  |
| `extension_remove`          | `{extensionId, deleteData}`                              | –                                                                                                                                                                                                     | L3; setzt Grabstein (R11)                           |
| `extension_purge_kept_data` | `{extensionId}`                                          | –                                                                                                                                                                                                     | L3; behaltene Daten entfernter Erweiterungen        |
| `extension_icon`            | `{extensionId}`                                          | `data:`-URL                                                                                                                                                                                           | für Launcher und Einstellungen                      |

## Berechtigungen (L1)

| Command                        | Eingabe                                                                    | Ausgabe                                                                    |
| ------------------------------ | -------------------------------------------------------------------------- | -------------------------------------------------------------------------- |
| `extension_permissions_list`   | `{extensionId}`                                                            | `PermissionView[]` (gemerkt mit Geltungsbereich und Gerätename, vorläufig) |
| `extension_permission_set`     | `{extensionId, kind, action, target, status, scope}`                       | –                                                                          |
| `extension_permission_remove`  | `{permissionId}` oder `{temporaryKey}`                                     | –                                                                          |
| `extension_permission_resolve` | `{requestId, decision: allow \| deny, remember, allDevices}`               | –                                                                          |
| `extension_permission_cancel`  | `{requestId}` (Dialog geschlossen; die nächste gleiche Frage kommt wieder) | –                                                                          |

Ereignis `extension-permission-request` (siehe [permissions.md](./permissions.md)).

## Grenzwerte, Protokolle, Entwicklermodus (L3)

| Command                                         | Eingabe                                   | Ausgabe                                                   |
| ----------------------------------------------- | ----------------------------------------- | --------------------------------------------------------- |
| `extension_limits_get` / `extension_limits_set` | `{extensionId}` / `{extensionId, limits}` | `Limits` / –                                              |
| `extension_logs_read`                           | `{extensionId, level?, limit, before?}`   | `LogEntry[]`                                              |
| `extension_dev_mode_set`                        | `{enabled}`                               | – (gerätebezogene Einstellung; lädt das Hauptfenster neu) |
| `extension_dev_load`                            | `{projectPath}`                           | `InstallPreview`                                          |
| `extension_dev_confirm`                         | `{projectPath, accepted}`                 | `ExtensionSummary`                                        |
| `extension_dev_unload`                          | `{devExtensionId, deleteData}`            | –                                                         |

## Rahmen (L1)

| Command                 | Eingabe                       | Ausgabe                                                                            |
| ----------------------- | ----------------------------- | ---------------------------------------------------------------------------------- |
| `extension_frame_open`  | `{extensionId, tabId}`        | `{frame, url}` (URL mit Start-Token) oder Fehler `disabled` / `not_ready {status}` |
| `extension_frame_close` | `{frame}`                     | – (beendet Beobachtungen und Shells, wenn es der letzte Rahmen war)                |
| `extension_bridge_call` | `{frame, id, method, params}` | `{id, result}` oder `{id, error}` (SDK-Form)                                       |

Ereignis `extension-frame-event {frame, type, data, timestamp}`.

`extension_host_context_set {theme, locale}` (nur holzis Fenster): Farbschema (`light` | `dark` | `system`) und
Sprache, wie holzis Oberfläche sie anwendet; Grundlage von `extension_context_get`. So liest nur das Frontend
die Einstellungen, und Rust kennt sie trotzdem.

## Weitere Ereignisse

| Ereignis                   | Daten                           | Zweck                                                             |
| -------------------------- | ------------------------------- | ----------------------------------------------------------------- |
| `extensions-changed`       | `{extensionIds}`                | Liste neu laden (Launcher, Einstellungen, App-Liste des wm)       |
| `extension-status-changed` | `{extensionId, status, error?}` | Anzeige „wird übertragen“, Fehler, Neuladen offener Tabs (FR-038) |
