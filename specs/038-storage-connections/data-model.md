# Data Model: Speicherverbindungen (S3)

**Feature**: [spec.md](./spec.md) | **Research**: [research.md](./research.md)

Migrationen (R3): `0027_passwords_owner` (`src-tauri/src/identity/migrations_passwords_owner.rs`, PR B) und
`0028_storage_connections` (`src-tauri/src/identity/migrations_storage.rs`, PR C). `HOLZI_TRIGGER_VERSION`
wird jeweils erhöht (neue Spalte in einer CRDT-Tabelle, neue CRDT-Tabellen).

## haex_storage_connections (synchronisiert)

Eine Speicherverbindung (Spec: Begriffe, FR-001, FR-006).

| Spalte                | Typ  | Regel                                                                             |
| --------------------- | ---- | --------------------------------------------------------------------------------- |
| `id`                  | TEXT | PK, UUIDv4                                                                        |
| `provider_name`       | TEXT | Pflicht, 1–80 Zeichen, vom Nutzer (z. B. „Hetzner“); in der Liste der Erweiterung |
| `provider_kind`       | TEXT | `aws`, `rustfs`, `other` (Vorbelegung von Endpunkt und Adressierung)              |
| `endpoint`            | TEXT | `https://…` oder `http://…` nur für lokale Adressen (R8); bei `aws` leer erlaubt  |
| `region`              | TEXT | Pflicht, 1–64 Zeichen                                                             |
| `addressing`          | TEXT | `path` oder `virtual`                                                             |
| `credentials_item_id` | TEXT | Kennung des Eintrags im Passwortmanager (`owner = 'storage'`), Pflicht            |
| `created_at`          | TEXT | ISO 8601                                                                          |
| `updated_at`          | TEXT | ISO 8601                                                                          |

Kein Fremdschlüssel auf den Eintrag (anderer Bereich, Löschen über den Dienst von 034). Fehlt der
Eintrag (vom Nutzer gelöscht), zeigt holzi „neue Zugangsdaten nötig“ (Edge Case).

## haex_storages (synchronisiert)

Ein Speicher = ein Bucket auf einer Verbindung (FR-002). Ziel der Berechtigung `remoteStorage`.

| Spalte          | Typ  | Regel                                                                 |
| --------------- | ---- | --------------------------------------------------------------------- |
| `id`            | TEXT | PK, UUIDv4; ist die `backendId` des SDK und das Ziel der Berechtigung |
| `connection_id` | TEXT | → `haex_storage_connections.id`                                       |
| `name`          | TEXT | Pflicht, 1–80 Zeichen                                                 |
| `bucket`        | TEXT | Pflicht, Bucket-Name nach S3 (3–63, Kleinbuchstaben, Ziffern, `.-`)   |
| `created_at`    | TEXT | ISO 8601                                                              |
| `updated_at`    | TEXT | ISO 8601                                                              |

Entfernen eines Speichers: Zeile löschen und die Berechtigungen aller Erweiterungen mit diesem Ziel
(`extension_permissions`, Art `remoteStorage`, Ziel = `id`) im selben Schreibvorgang (FR-007). Entfernen
einer Verbindung: alle ihre Speicher wie eben, dann die Verbindung, dann den Eintrag der Zugangsdaten über
den Dienst von 034 (`Caller::Internal { feature: "storage" }`), endgültig (nicht Papierkorb), wenn keine
andere Verbindung denselben Eintrag nutzt.

## storage_tests_no_sync (geräteeigen)

Letztes Testergebnis je Speicher auf diesem Gerät (FR-004, Edge Case „neue Zugangsdaten nötig“).

| Spalte       | Typ  | Regel                                                                    |
| ------------ | ---- | ------------------------------------------------------------------------ |
| `storage_id` | TEXT | PK                                                                       |
| `tested_at`  | TEXT | ISO 8601                                                                 |
| `outcome`    | TEXT | `passed`, `accessDenied`, `unreachable`, `bucketMissing`, `missingRight` |

Wird auch von einem Aufruf einer Erweiterung mit `accessDenied` gesetzt, damit die Einstellungen den
Hinweis zeigen.

## haex_passwords_item_details.owner (neue Spalte, Migration 0027)

| Spalte  | Typ  | Regel                                                                     |
| ------- | ---- | ------------------------------------------------------------------------- |
| `owner` | TEXT | `NULL` = Eintrag des Nutzers; sonst Name einer holzi-Funktion (`storage`) |

Regel Z14 (R2, Änderung an `specs/034-password-manager/contracts/access.md`): Ein Eintrag mit `owner` ist
für jeden Aufrufer außer `User` und `Internal { feature }` mit `feature == owner` `NotFound`, fehlt in
`list_headers` und den Agent-Kopfdaten und gilt beim Auflösen von Verweisen als nicht sichtbar.
`ItemState` bekommt dafür `owner: Option<&str>`. Nur holzi setzt `owner` (beim Anlegen über
`create_item` eines `Internal`-Aufrufers); kein Command und keine Bridge-Methode kann ihn setzen oder
ändern.

## Zugangsdaten-Eintrag

| Feld im Eintrag             | Inhalt                                       |
| --------------------------- | -------------------------------------------- |
| `title`                     | `S3: <provider_name>`                        |
| `username`                  | Zugangsschlüssel (Access Key ID)             |
| `password`                  | Geheimnis (Secret Access Key)                |
| eigenes Feld `sessionToken` | optional                                     |
| `url`                       | Endpunkt (für den Nutzer im Passwortmanager) |
| `owner`                     | `storage`                                    |

## Abgeleitet, nicht gespeichert

- **Bereich einer Erweiterung**: `holzi-ext/<extension_id>/` (R4).
- **Liste für eine Erweiterung** (FR-009a): `{ id, type: "s3", name, providerName, bucket }` je Speicher,
  den eine Leseberechtigung deckt.

## Zustände eines Speichers in den Einstellungen

`ungetestet` → (Test) → `bereit` | `Fehler <Grund>`; ein Aufruf mit `accessDenied` setzt `Fehler
accessDenied` (Hinweis „neue Zugangsdaten nötig“); neue Zugangsdaten → erneuter Test.
