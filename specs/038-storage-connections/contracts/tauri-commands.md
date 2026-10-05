# Contract: Tauri-Commands der Speicherverbindungen

Für die Einstellungen von holzi (Kategorie „Speicher“) und den Dialog aus R6. Alle in
`src-tauri/src/remote_storage/commands.rs`, Typen mit ts-rs nach `src/types/bindings/`. Kein Command gibt
ein Geheimnis zurück; das Geheimnis zeigt der Passwortmanager auf eigene Aktion des Nutzers (FR-005).

| Command                     | Argumente                                                                                                                                                          | Antwort                                                                                                                                                                       |
| --------------------------- | ------------------------------------------------------------------------------------------------------------------------------------------------------------------ | ----------------------------------------------------------------------------------------------------------------------------------------------------------------------------- |
| `storage_list`              | —                                                                                                                                                                  | `StorageOverview { connections: [ConnectionView], storages: [StorageView] }`                                                                                                  |
| `storage_connection_save`   | `ConnectionInput { id?, providerName, providerKind, endpoint?, region, addressing, credentials?: { accessKeyId, secretAccessKey, sessionToken? }, bucketForTest }` | `ConnectionView`; bei neuer Verbindung oder neuen Zugangsdaten erst nach bestandenem Test mit `bucketForTest` (FR-003), sonst Fehler mit `TestOutcome` und nichts gespeichert |
| `storage_connection_remove` | `{ id }`                                                                                                                                                           | `()`; vorher `storage_removal_preview`                                                                                                                                        |
| `storage_save`              | `StorageInput { id?, connectionId, name, bucket }`                                                                                                                 | `StorageView`; neuer Bucket → Test vorher                                                                                                                                     |
| `storage_remove`            | `{ id }`                                                                                                                                                           | `()`                                                                                                                                                                          |
| `storage_removal_preview`   | `{ connectionId? , storageId? }`                                                                                                                                   | `{ storages: [name], extensions: [name] }` (FR-007)                                                                                                                           |
| `storage_test`              | `{ storageId }`                                                                                                                                                    | `TestOutcome` (`passed` oder Grund), gespeichert in `storage_tests_no_sync`                                                                                                   |
| `storage_dialog_resolve`    | `{ requestId, answer: Cancel \| Confirm { connectionId? , credentials?, name?, bucket? } }`                                                                        | `()`; beantwortet einen wartenden Dialog einer Erweiterung (R6)                                                                                                               |

Ereignis an das Fenster: `extension-storage-request { requestId, frame, kind: add | update | test |
remove, proposal, extensionName, otherExtensions }`.

`ConnectionView` = `{ id, providerName, providerKind, endpoint, region, addressing, insecure: bool,
credentialsMissing: bool }`. `StorageView` = `{ id, connectionId, name, bucket, lastTest?: {at, outcome},
extensions: [name] }`.
