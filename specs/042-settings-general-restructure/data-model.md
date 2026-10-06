# Data Model: Allgemein mit Grundeinstellung und Erscheinungsbild

Keine Migration: Beide neuen Werte sind Zeilen der vorhandenen, synchronisierten Key/Value-Tabelle
`preferences(vault_device_uuid, key, value TEXT)` (`src-tauri/src/identity/migrations.rs:264-275`) im
Vault-Scope (`PrefScope::Vault`).

## Vault-Sprache

| Feld      | Wert                                                                        |
| --------- | --------------------------------------------------------------------------- |
| Schlüssel | `general.language`                                                          |
| Scope     | Vault (synchronisiert)                                                      |
| Werte     | `de` \| `en`                                                                |
| Fehlt     | noch nie gesetzt → beim nächsten Öffnen wird die aktive Sprache geschrieben |
| Ungültig  | wie „fehlt“ behandelt (überschrieben mit der aktiven Sprache)               |

## Workspace-Hintergrund

| Feld      | Wert                                                                                                                                               |
| --------- | -------------------------------------------------------------------------------------------------------------------------------------------------- |
| Schlüssel | `appearance.background`                                                                                                                            |
| Scope     | Vault (synchronisiert)                                                                                                                             |
| Wert      | `data:image/webp;base64,<…>`, lange Kante ≤ 2560 px                                                                                                |
| Fehlt     | Standard-Hintergrund (`bg-muted/10`)                                                                                                               |
| Prüfung   | Backend `validate_value`: Präfix `data:image/webp;base64,`, Länge ≤ 4 MiB; sonst Ablehnung. Frontend zeigt beim Lesen nur Werte mit diesem Präfix. |
| Entfernen | Zeile löschen (`clear_pref`)                                                                                                                       |

## Vault-Passwort (kein gespeicherter Wert)

Nicht in der Datenbank. Der SQLCipher-Schlüssel der Datei dieses Geräts.

| Regel                                             | Quelle                                                     |
| ------------------------------------------------- | ---------------------------------------------------------- |
| Neues Passwort ≥ `MIN_PASSPHRASE_LEN` (8) Zeichen | `Passphrase::validate_new`, gleiche Regel wie beim Anlegen |
| Neues ≠ aktuelles                                 | `change_vault_passphrase`                                  |
| Aktuelles muss die Datei öffnen                   | zweite, nur lesende Connection                             |

Zustandsübergang: `Passwort A` → (Prüfung A ok, Rekey) → `Passwort B`. Jeder Fehler vor dem Rekey lässt A
unverändert; der Rekey selbst ist im DELETE-Journal atomar.
