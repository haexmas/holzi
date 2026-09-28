# Data Model: Vault-Identität, Geräteliste und Datensync zwischen eigenen Geräten

**Spec**: [spec.md](./spec.md) | **Research**: [research.md](./research.md)

Tabellen mit Endung `_no_sync` bleiben auf dem Gerät (haex-crdt nimmt sie vom Sync aus); alle
anderen sind CRDT-Tabellen und reisen mit dem Sync. Migrationsnummern schließen an die letzte
vorhandene an (heute `0020_wm_session_no_sync` und folgende; beim Umsetzen prüfen).
`HOLZI_TRIGGER_VERSION` steigt, weil die Trigger von haex-crdt die Transaktionstabelle füllen
(U2).

## Tabellen

### `vault_identity` (geändert, synchronisiert)

| Spalte   | Typ        | Regeln                                                  |
| -------- | ---------- | ------------------------------------------------------- |
| `id`     | INTEGER PK | immer `1`                                               |
| `pubkey` | BLOB(32)   | x-only secp256k1, unveränderlich nach dem Anlegen (D26) |

Die Spalte `privkey` entfällt. Migration: Liegt noch der Platzhalter vor, wird daraus nach R3 die
echte Identität abgeleitet; `pubkey` kommt hierher, der private Schlüssel nach
`vault_identity_secret_no_sync`. Die Zeile bekommt dabei zum ersten Mal einen HLC-Zeitstempel und
wird damit gesynct.

### `vault_identity_secret_no_sync` (neu, gerätelokal)

| Spalte    | Typ        | Regeln                                                                 |
| --------- | ---------- | ---------------------------------------------------------------------- |
| `id`      | INTEGER PK | immer `1`                                                              |
| `privkey` | BLOB(32)   | nur auf Hauptgeräten; nie in Protokollen, Oberfläche, Agenten (FR-002) |

Eine Zeile gibt es genau dann, wenn dieses Gerät ein Hauptgerät ist. Sie entsteht beim Anlegen
der Vault, beim Verknüpfen mit Hauptgerät-Rolle (FR-024) und kommt mit der Kopie eines Hauptgeräts
mit (FR-044).

### `device_keys_no_sync` (neu, gerätelokal)

| Spalte              | Typ      | Regeln                                     |
| ------------------- | -------- | ------------------------------------------ |
| `installation_uuid` | TEXT PK  | aus `<AppLocalData>/installation-id`       |
| `device_secret`     | BLOB(32) | secp256k1, verlässt das Gerät nie (FR-003) |
| `device_pubkey`     | BLOB(32) | x-only                                     |
| `endpoint_secret`   | BLOB(32) | ed25519 für iroh                           |
| `endpoint_id`       | BLOB(32) | öffentlicher iroh-Schlüssel                |
| `created_at`        | INTEGER  | ms seit Epoch                              |

Beim Öffnen: gibt es keine Zeile zur eigenen Installation, werden Zeilen anderer Installationen
gelöscht und neue Schlüssel erzeugt (Kopie, FR-006, R12).

### `device_lists` (neu, synchronisiert, nur Einfügen)

| Spalte       | Typ         | Regeln                                                            |
| ------------ | ----------- | ----------------------------------------------------------------- |
| `list_hash`  | BLOB(32) PK | `SHA-256(payload)`                                                |
| `generation` | INTEGER     | ≥ 1                                                               |
| `payload`    | BLOB        | kanonische Geräteliste (postcard), siehe unten                    |
| `signature`  | BLOB(64)    | Schnorr der Vault-Identität über `holzi-device-list/v1 ‖ payload` |

Zeilen werden nie geändert oder gelöscht. Geltende Liste: höchste Generation, bei Gleichstand
kleinster `list_hash` (FR-043). Ein Gerät ist entfernt, wenn irgendeine gültige Zeile es als
entfernt führt (FR-005).

**Payload `DeviceList`**:

| Feld         | Inhalt                                                                                                                                                                                              |
| ------------ | --------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------- |
| `vault`      | öffentlicher Schlüssel der Vault-Identität                                                                                                                                                          |
| `generation` | Generation                                                                                                                                                                                          |
| `devices[]`  | `device_pubkey`, `endpoint_id`, `role` (`main` \| `linked`), `vault_device_uuid`, `name_sealed` (Name, verschlüsselt mit einem aus dem Inhaltsschlüssel abgeleiteten Schlüssel, FR-005), `added_at` |
| `removed[]`  | `device_pubkey`, `limit` (höchste lückenlose Laufnummer, Grenze beim Entfernen, FR-028), `removed_at`                                                                                               |
| `issued_by`  | Geräteschlüssel des ausstellenden Hauptgeräts                                                                                                                                                       |
| `issued_at`  | ms seit Epoch                                                                                                                                                                                       |

Prüfregeln: Signatur passt zur Vault-Identität; kein Gerät zugleich in `devices` und `removed`;
kein Geräteschlüssel und keine `vault_device_uuid` doppelt; `generation` größer als die jeder
Liste, die der Aussteller beim Ausstellen kannte. Eine Liste ohne Hauptgerät ist gültig (Edge Case:
zwei Hauptgeräte entfernen sich gegenseitig).

### `vault_key_generations` (neu, synchronisiert, nur Einfügen)

| Spalte       | Typ         | Regeln                                          |
| ------------ | ----------- | ----------------------------------------------- |
| `key_id`     | BLOB(16) PK | `SHA-256("holzi-key-id/v1" ‖ schlüssel)[0..16]` |
| `scope`      | TEXT        | in 024 immer `vault`                            |
| `generation` | INTEGER     | steigt mit jedem Entfernen (FR-015)             |
| `created_by` | BLOB(32)    | Geräteschlüssel des Ausstellers                 |
| `created_at` | INTEGER     | ms                                              |

### `vault_key_envelopes` (neu, synchronisiert, nur Einfügen)

| Spalte      | Typ                     | Regeln                                               |
| ----------- | ----------------------- | ---------------------------------------------------- |
| `key_id`    | BLOB(16)                | → `vault_key_generations`                            |
| `recipient` | BLOB(32)                | Geräteschlüssel des Empfängers                       |
| `sender`    | BLOB(32)                | Geräteschlüssel des Ausstellers                      |
| `envelope`  | TEXT                    | NIP-44 v2, Inhalt `{scope, generation, key_id, key}` |
| PK          | (`key_id`, `recipient`) |                                                      |

### `vault_content_keys_no_sync` (neu, gerätelokal)

| Spalte       | Typ         | Regeln                                                |
| ------------ | ----------- | ----------------------------------------------------- |
| `key_id`     | BLOB(16) PK |                                                       |
| `generation` | INTEGER     |                                                       |
| `key`        | BLOB(32)    | entpackt aus dem eigenen Umschlag; nie in Protokollen |

### `sync_packages_no_sync` (neu, gerätelokal)

| Spalte         | Typ               | Regeln                                                                            |
| -------------- | ----------------- | --------------------------------------------------------------------------------- |
| `origin`       | BLOB(32)          | Geräteschlüssel des Ursprungsgeräts                                               |
| `seq`          | INTEGER           | Laufnummer, ≥ 1                                                                   |
| `package_hash` | BLOB(32)          | `SHA-256(sealed)`                                                                 |
| `hlc`          | TEXT              | Zeitstempel der Transaktion (nur für den Ursprung lesbar; für fremde Pakete leer) |
| `scope`        | TEXT              | `vault`                                                                           |
| `key_id`       | BLOB(16)          |                                                                                   |
| `sealed`       | BLOB              | Änderungspaket, wie es auf dem Draht reist                                        |
| `received_at`  | INTEGER           | ms                                                                                |
| PK             | (`origin`, `seq`) | ein zweites, anderes Paket unter demselben Schlüssel ist eine Fälschung (FR-019)  |

Aufräumen: löschbar, sobald jedes Gerät der geltenden Geräteliste laut `sync_peer_progress_no_sync`
die Laufnummer hat (R4).

### `sync_progress_no_sync` (neu, gerätelokal)

| Spalte       | Typ         | Regeln                                                                      |
| ------------ | ----------- | --------------------------------------------------------------------------- |
| `origin`     | BLOB(32) PK |                                                                             |
| `contiguous` | INTEGER     | höchste Laufnummer, bis zu der alle Pakete dieses Ursprungs angewendet sind |

Lücken ergeben sich aus `sync_packages_no_sync`: jede fehlende Laufnummer zwischen `contiguous` und
der höchsten vorhandenen. Pakete jenseits einer Lücke werden angewendet (FR-019), zählen aber nicht.

### `sync_peer_progress_no_sync` (neu, gerätelokal)

| Spalte        | Typ                | Regeln                                            |
| ------------- | ------------------ | ------------------------------------------------- |
| `peer`        | BLOB(32)           | Geräteschlüssel des anderen Geräts                |
| `origin`      | BLOB(32)           |                                                   |
| `contiguous`  | INTEGER            | zuletzt gemeldeter Fortschritt des anderen Geräts |
| `reported_at` | INTEGER            | ms                                                |
| PK            | (`peer`, `origin`) |                                                   |

### `sync_sealer_no_sync` (neu, gerätelokal)

| Spalte       | Typ        | Regeln                                |
| ------------ | ---------- | ------------------------------------- |
| `id`         | INTEGER PK | immer `1`                             |
| `cursor_hlc` | TEXT       | letzte versiegelte eigene Transaktion |
| `next_seq`   | INTEGER    | nächste zu versiegelnde Laufnummer    |

### `admission_requests` (neu, synchronisiert)

| Spalte          | Typ         | Regeln                                          |
| --------------- | ----------- | ----------------------------------------------- |
| `device_pubkey` | BLOB(32) PK | der Kopie                                       |
| `endpoint_id`   | BLOB(32)    |                                                 |
| `name`          | TEXT        |                                                 |
| `requested_at`  | INTEGER     | ms                                              |
| `signature`     | BLOB(64)    | Schnorr der Kopie über `holzi-admission/v1 ‖ …` |
| `state`         | TEXT        | `open` \| `admitted` \| `rejected`              |

Übergänge: `open → admitted` (Hauptgerät, „Aufnehmen“, gleichzeitig neue Geräteliste) und `open →
rejected` („Ablehnen“). Keine Rückkehr.

### `device_presence_no_sync` (neu, gerätelokal)

| Spalte          | Typ         | Regeln                                                                 |
| --------------- | ----------- | ---------------------------------------------------------------------- |
| `device_pubkey` | BLOB(32) PK |                                                                        |
| `last_seen`     | INTEGER     | jüngster Zeitpunkt aus Verbindung, Präsenz oder drittem Gerät (FR-033) |
| `endpoint_addr` | BLOB        | letzte bekannte Erreichbarkeit (postcard `EndpointAddr`)               |
| `problem`       | TEXT NULL   | `incompatible_version` \| `duplicate` \| NULL                          |

„Zuletzt online“ über ein drittes Gerät: Geräte tauschen im Handshake ihre `last_seen`-Werte aus
und übernehmen den jeweils jüngeren.

### `known_devices` (vorhanden, synchronisiert)

Bleibt die Quelle für Installation ↔ `vault_device_uuid` und den änderbaren Namen (Spec 023). Neu
ist keine Spalte; die Verbindung zum Geräteschlüssel führt die Geräteliste über
`vault_device_uuid`.

### `haex_crdt_local_tx_no_sync` (neu in haex-crdt, U2)

| Spalte | Typ     | Regeln                                         |
| ------ | ------- | ---------------------------------------------- |
| `hlc`  | TEXT PK | Zeitstempel einer lokal begonnenen Transaktion |
| `node` | TEXT    | HLC-Knoten (= `vault_device_uuid`)             |
| `seq`  | INTEGER | lückenlos je `node`, `UNIQUE(node, seq)`       |

## Werte, die nicht in Tabellen liegen

- **Änderungspaket** (`SealedPackage`): `scope`, `key_id`, `origin`, `seq`, `nonce(24)`,
  `ciphertext`. Klartext: `{changes: [ColumnChange mit sig]}`, alle Zellen einer Transaktion.
  Zusätzliche Daten der Verschlüsselung: `scope ‖ key_id ‖ origin ‖ seq` (R9).
- **Zellsignatur** (`sig`-JSON in haex-crdt): `{v: 1, dev: device_pubkey, vault: vault_pubkey, seq,
sig: schnorr}` über den Preimage aus R10.
- **Präsenzmeldung**: flüchtiges Ereignis Art 21059 an `mb_pk` (R7), Inhalt `{device_pubkey,
endpoint_id, relay_url, direct_addrs, device_list_generation, ts, nonce}`.
- **Verknüpfungscode**: 16 Byte Zufall, `expires_at`, `used`; nur im Speicher des Hauptgeräts.

## Zustände

**Rolle dieses Geräts** (abgeleitet, nicht gespeichert):

```
auf geltender Geräteliste, Zeile in vault_identity_secret_no_sync → Hauptgerät
auf geltender Geräteliste, keine Zeile                            → verknüpftes Gerät
nicht auf der Liste, offene Aufnahmeanfrage                       → wartet auf Aufnahme
in irgendeiner gültigen Liste als entfernt                        → entfernt (kein Sync mehr)
```

**Verbindung zu einem anderen Gerät**: `unbekannt → gefunden (Präsenz) → verbindet → geprüft
(Handshake, FR-009) → synchronisiert ↔ getrennt`; von jedem Zustand nach `abgewiesen` (nicht auf
der Liste, entfernt, falsche Signatur, Version, doppelt).
