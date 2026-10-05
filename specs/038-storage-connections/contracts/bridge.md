# Contract: Bridge-Methoden `extension_remote_storage_*`

Die neun Methoden des vault-sdk (`src/api/remoteStorage.ts`), in holzi in
`src-tauri/src/extensions/remote_storage.rs`, registriert in `bridge/dispatch.rs` (aus der Liste `LATER`
entfernt). Fehlercodes wie in Spec 017 (`contracts/bridge.md`). „Lesen“/„Lesen und Schreiben“ =
Berechtigung der Art `remoteStorage` mit dem Speicher (`backendId`) oder `*` als Ziel; ohne passende
Berechtigung 1004 für eine Berechtigung im Zustand „fragen“, sonst 1002 (wie Spec 017).

| Methode                                   | Parameter                                                                                         | Recht                                                           | Antwort                                                                                                          | Dialog                                                                                                                                |
| ----------------------------------------- | ------------------------------------------------------------------------------------------------- | --------------------------------------------------------------- | ---------------------------------------------------------------------------------------------------------------- | ------------------------------------------------------------------------------------------------------------------------------------- |
| `extension_remote_storage_list_backends`  | `{}`                                                                                              | Lesen je Speicher                                               | `[{id, type: "s3", name, providerName, bucket}]` nur für gedeckte Speicher; ohne jede Leseberechtigung 1004/1002 | nein                                                                                                                                  |
| `extension_remote_storage_add_backend`    | `{request: {name, type: "s3", config: {endpoint?, region, bucket, pathStyle?}, sameProviderAs?}}` | keins vorab; `sameProviderAs` braucht Lesen auf diesen Speicher | Eintrag wie in der Liste                                                                                         | ja: Vorschlag prüfen, Zugangsdaten eingeben (oder vorhandene Verbindung wählen), Test; danach Lesen und Schreiben für die Erweiterung |
| `extension_remote_storage_update_backend` | `{request: {backendId, name?, config?: {bucket?}}}`                                               | Lesen und Schreiben                                             | Eintrag wie in der Liste                                                                                         | ja: Änderung bestätigen, neue Zugangsdaten nur dort; Test bei neuem Bucket oder neuen Zugangsdaten                                    |
| `extension_remote_storage_test_backend`   | `{backendId}`                                                                                     | Lesen und Schreiben                                             | `null` oder 2002 mit `kind`                                                                                      | ja: Bestätigen des Tests                                                                                                              |
| `extension_remote_storage_remove_backend` | `{backendId}`                                                                                     | Lesen und Schreiben                                             | `null`                                                                                                           | ja: nennt andere Erweiterungen mit Berechtigung                                                                                       |
| `extension_remote_storage_upload`         | `{request: {backendId, key, data (Base64)}}`                                                      | Lesen und Schreiben                                             | `null`                                                                                                           | nein                                                                                                                                  |
| `extension_remote_storage_download`       | `{request: {backendId, key}}`                                                                     | Lesen                                                           | Base64                                                                                                           | nein                                                                                                                                  |
| `extension_remote_storage_list`           | `{request: {backendId, prefix?}}`                                                                 | Lesen                                                           | `[{key, size, lastModified?}]`, Schlüssel ohne Präfix der Erweiterung                                            | nein                                                                                                                                  |
| `extension_remote_storage_delete`         | `{request: {backendId, key}}`                                                                     | Lesen und Schreiben                                             | `null`                                                                                                           | nein                                                                                                                                  |

## Regeln

- **Zugangsdaten nie über die Bridge** (FR-012, FR-013a): enthält `config` eines der Felder
  `accessKeyId`, `secretAccessKey`, `sessionToken`, antwortet holzi 3001 `credentials are entered in
holzi`, bevor ein Dialog erscheint. Keine Antwort und keine Fehlermeldung enthält Endpunkt, Region,
  Zugangsdaten, Kopfzeilen oder Antworttext des Anbieters.
- **Bereich** (FR-010, R4): jeder `key` und `prefix` wird nach R5 geprüft (sonst 3001) und mit
  `holzi-ext/<extension_id>/` versehen; Schlüssel in Antworten ohne dieses Präfix.
- **Unbekannter oder nicht gedeckter Speicher**: 1001 für einen unbekannten, 1004/1002 für einen nicht
  gedeckten (eine Erweiterung ohne Leseberechtigung erfährt nicht, ob es ihn gibt: auch hier 1004/1002,
  nie 1001).
- **Grenzen** (R7): Hochladen über `max_response_bytes` und Herunterladen über `max_response_bytes / 4 *
3` → 7000; Frist `timeout_ms` je Aufruf → 7000; Auflisten über `max_rows` Objekte → 7000 mit Hinweis
  auf ein engeres Präfix.
- **Fehler beim Anbieter** (R7): 1001 `not found`; 2002 `{kind: "accessDenied"}`; 2002 `{kind:
"network"}`.
- **Dialog** (R6): Abbruch, Ablauf (300 s) oder Schließen des Rahmens → 1002 `cancelled by the user`;
  ein gescheiterter Test im Dialog → 2002 mit `kind`, nichts angelegt.
- **Plattformen**: auf allen Plattformen gleich (FR-016); kein `cfg(desktop)`.
- **Entwicklerversionen**: wie installierte (Präfix aus `extension_id`, R4).
