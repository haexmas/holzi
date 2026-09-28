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

| Nachricht        | Richtung             | Inhalt                                                                                                   | Antwort                                     |
| ---------------- | -------------------- | -------------------------------------------------------------------------------------------------------- | ------------------------------------------- |
| `DeviceListPush` | beide                | `{ payload, signature }`                                                                                 | –                                           |
| `Progress`       | beide                | `{ contiguous: Vec<(origin, seq)>, have_beyond: Vec<(origin, Vec<seq>)>, last_seen: Vec<(device, ms)> }` | `Progress` der Gegenseite                   |
| `Want`           | beide                | `{ ranges: Vec<(origin, from_seq, to_seq)> }`                                                            | `Packages` (seitenweise, ≤ 4 MiB je Rahmen) |
| `Packages`       | beide                | `{ packages: Vec<SealedPackage>, more: bool }`                                                           | –                                           |
| `Push`           | Ursprung → verbunden | `{ packages: Vec<SealedPackage> }` (neu versiegelte eigene Pakete, sofort)                               | –                                           |
| `Envelopes`      | beide                | neue Zeilen aus `vault_key_envelopes`/`vault_key_generations` für die Gegenseite                         | –                                           |

Ablauf nach dem Handshake: beide senden `Progress`; jede Seite berechnet aus dem Vergleich, was ihr
fehlt (auch Lücken), und schickt `Want`; die Antworten kommen als `Packages`. Neu versiegelte
eigene Pakete gehen als `Push` an jedes verbundene Gerät. Weitergeleitete Pakete sind die
gespeicherten Bytes aus `sync_packages_no_sync`, unverändert (FR-012, FR-021).

`SealedPackage = { scope: "vault", key_id: [u8;16], origin: [u8;32], seq: u64, nonce: [u8;24],
ciphertext: Vec<u8> }`. Der Empfänger:

1. verwirft, wenn `key_id` unbekannt ist und sich auch nach dem nächsten `Envelopes` nicht
   entpacken lässt (Paket bleibt liegen und wird erneut angefordert),
2. prüft, dass `origin` auf der geltenden Geräteliste steht oder `seq ≤ limit` seiner Entfernung,
3. verwirft als Fälschung, wenn unter (`origin`, `seq`) schon ein anderes Paket liegt,
4. entschlüsselt (AEAD mit `scope ‖ key_id ‖ origin ‖ seq`), prüft jede Zellsignatur (auf direkten,
   geprüften Verbindungen im Bereich „Vault“ darf das entfallen, FR-021),
5. wendet über `apply_remote_changes_with_policy` atomar an und schreibt im selben Commit Paket und
   Fortschritt.

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
- `snapshot` sind Seiten von Zellen des Bereichs „Vault“ mit ihren Originalsignaturen, ohne
  gerätelokale Tabellen. N prüft je Transaktionsgruppe (FR-013) und wendet an.
- Bricht die Verbindung vor `LinkDone` ab, löscht N die angelegte Vault; H hat nichts
  veröffentlicht (FR-025).
- `vault_secret` wird nur bei gewählter Hauptgerät-Rolle gesendet und nur auf dieser Verbindung
  (FR-038).
