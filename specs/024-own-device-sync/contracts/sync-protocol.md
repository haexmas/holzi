# Contract: Sync-Protokoll zwischen eigenen Geräten (`holzi-sync/1`, `holzi-link/1`)

Transport: iroh 1.2, QUIC. Jede Anfrage ist ein eigener Bi-Stream. Rahmen: `u32 BE Länge ‖
postcard(Nachricht)`, höchstens 4 MiB, die Länge wird vor dem Anlegen des Puffers geprüft. Nach
dem letzten Rahmen einer Richtung `finish()`. Höchstens 8 gleichzeitige Bi-Streams je Verbindung.
Unbekannte Nachrichten oder zu große Rahmen schließen die Verbindung mit einem Fehlercode.

## ALPN `holzi-sync/1`

### 1. Handshake (erster Bi-Stream, vom Annehmenden geöffnet)

```text
A = annehmendes Gerät, D = wählendes Gerät

A → D  Challenge   { v: 1, nonce_a: [u8;32], endpoint_a: [u8;32] }
D → A  Response    { v: 1, device_d: [u8;32], vault: [u8;32], nonce_d: [u8;32],
                     schema: SchemaVersion, list: ListRef, sig_d: [u8;64] }
A → D  Accept      { device_a: [u8;32], schema: SchemaVersion, list: ListRef, sig_a: [u8;64] }
       | Reject    { code: RejectCode }
```

- `sig_x = schnorr(device_x, SHA-256("holzi-device-auth/v1" ‖ lp(nonce_a) ‖ lp(nonce_d) ‖
lp(endpoint_x) ‖ lp(endpoint_y) ‖ lp(vault)))`, `lp` = `u32 BE Länge ‖ Bytes`.
- `ListRef = { generation, list_hash }`. Kennt eine Seite eine höhere oder gleich hohe Liste mit
  kleinerem Hash, sendet sie diese auf einem eigenen Stream (`DeviceListPush`), bevor irgendetwas
  anderes fließt; die andere prüft und übernimmt sie nach FR-005.
- Jede Seite prüft: `endpoint` des Gegenübers gleich `Connection::remote_id()`, Signatur gültig,
  Geräteschlüssel steht mit genau diesem `endpoint` auf der geltenden Geräteliste der eigenen Vault
  und gilt nicht als entfernt, `vault` gleich der eigenen, `schema` verträglich.
- `RejectCode`: `ForeignVault`, `NotOnList`, `Removed`, `BadSignature`, `Incompatible`,
  `Duplicate`, `Closing`. Vor `Accept` fließt nur die Geräteliste (FR-009).

`SchemaVersion = { protocol: 1, holzi_migration: u32, crdt_trigger: u32 }`.

### 2. Nachrichten nach dem Handshake

| Nachricht        | Richtung | Inhalt                                                                           | Antwort                              |
| ---------------- | -------- | -------------------------------------------------------------------------------- | ------------------------------------ |
| `DeviceListPush` | beide    | `{ payload, signature }`                                                         | –                                    |
| `Progress`       | beide    | `{ vector: Vec<(origin: Uuid, max_hlc: String)>, last_seen: Vec<(device, ms)> }` | –                                    |
| `Pull`           | beide    | `{ vector: Vec<(origin, max_hlc)> }` (eigener Fortschrittsstand des Anfragenden) | `Page`, dann weitere `Page` bis Ende |
| `Page`           | beide    | `{ changes: Vec<ColumnChange>, more: bool }` (≤ 4 MiB je Rahmen)                 | –                                    |
| `Envelopes`      | beide    | neue Zeilen aus `vault_key_envelopes`/`vault_key_generations` für die Gegenseite | –                                    |

Ablauf:

1. Nach dem Handshake senden beide `Progress`. Nach jedem lokalen Commit (research R19) und nach
   jeder angewendeten Seite sendet ein Gerät erneut `Progress` an jedes verbundene Gerät, dicht
   folgende Anstöße zusammengefasst.
2. Wer im `Progress` der Gegenseite für irgendeinen Ursprung einen höheren `max_hlc` sieht als im
   eigenen Stand, schickt `Pull` mit seinem eigenen Stand. Höchstens ein `Pull` je Verbindung
   gleichzeitig.
3. Der Sender scannt jede CRDT-Tabelle und das Lösch-Log ab dem kleinsten Cursor im `Pull`, behält
   je Zelle nur, was jenseits des Cursors ihres Ursprungs liegt (ein fehlender Ursprung heißt
   „alles“), sortiert nach HLC aufsteigend über alle Ursprünge und schickt `Page`s, die nie eine
   Transaktionsgruppe teilen (`paginate_changes`). Gerätelokale Tabellen scannt er nie.
4. Der Empfänger prüft jede Seite je Transaktionsgruppe (research R5), wendet sie in einer
   Transaktion an und erhöht danach je Ursprung seinen Fortschritt auf den höchsten HLC dieses
   Ursprungs in der Seite. Bricht die Verbindung ab, gilt der Stand der zuletzt angewendeten Seite.

Daten gehen nie ungefragt über die Leitung; so entsteht beim Empfänger keine Lücke (FR-019).
`ColumnChange.device_id` wird nicht gesendet oder beim Empfang ignoriert; der Ursprung ist der
Knoten im `hlc_timestamp`.

## ALPN `holzi-link/1` (Verknüpfen)

```text
H = Hauptgerät, N = neue Installation; beide kennen den Code c

N findet H über den Treffpunkt (contracts/nostr-events.md) und wählt H an.
H → N  LinkHello   { nonce_h, endpoint_h, device_h }
N → H  LinkProof   { nonce_n, endpoint_n, device_n, name, schema, mac_n }
H → N  LinkProof   { mac_h }
       -- H zeigt Name und Rollenfrage; wartet auf die Nutzerin --
H → N  LinkDecision { accepted: bool }
H → N  LinkTransfer { snapshot pages…, progress, key_generations, envelopes_for_n,
                      device_list (neu, noch nicht veröffentlicht),
                      vault_secret: Option<[u8;32]> (nur bei Rolle Hauptgerät) }
N → H  LinkDone    { applied: true }
       -- H veröffentlicht die neue Geräteliste --
```

- `k = HKDF-SHA256(c, info = "holzi/link/proof/v1")`,
  `mac_x = HMAC-SHA256(k, "holzi-link/v1" ‖ lp(nonce_h) ‖ lp(nonce_n) ‖ lp(endpoint_h) ‖
lp(endpoint_n) ‖ lp(device_h) ‖ lp(device_n) ‖ rolle_x)`, `rolle_x` ∈ {`H`, `N`}.
- Falscher `mac`, abgelaufener oder verbrauchter Code: Verbindung zu, nichts übertragen (FR-024).
  Der Code ist verbraucht, sobald `LinkHello` gesendet wurde.
- `snapshot` sind `Page`s wie in Abschnitt 2 mit dem Stand „nichts“ für jeden Ursprung: alle Zellen
  des Bereichs „Vault“ mit ihren ursprünglichen HLCs, samt Lösch-Log, ohne gerätelokale Tabellen.
  `progress` ist der Fortschrittsstand von H nach der letzten Seite. N prüft je Transaktionsgruppe
  (FR-013) und wendet an.
- Bricht die Verbindung vor `LinkDone` ab, löscht N die angelegte Vault; H hat nichts
  veröffentlicht (FR-025).
- `vault_secret` wird nur bei gewählter Hauptgerät-Rolle gesendet und nur auf dieser Verbindung
  (FR-038).
