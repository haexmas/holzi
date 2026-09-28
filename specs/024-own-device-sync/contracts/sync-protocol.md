# Contract: Sync-Protokoll zwischen eigenen Geräten (`holzi-sync/1`, `holzi-link/1`)

Transport: iroh 1.2, QUIC. Jede Anfrage ist ein eigener Bi-Stream. Rahmen: `u32 BE Länge ‖
postcard(Nachricht)`, höchstens 4 MiB, die Länge wird vor dem Anlegen des Puffers geprüft. Eine
vollständig serialisierte Transaktionsgruppe darf höchstens `3 MiB` groß sein; so bleiben für
Rahmen- und Postcard-Metadaten mindestens `1 MiB` Reserve. `paginate_changes` misst jede komplette
Gruppe vor dem Aufteilen. Ist sie größer, bricht es mit `TransactionGroupTooLarge` ab, statt die
Gruppe zu teilen. Nach dem letzten Rahmen einer Richtung `finish()`. Höchstens 8 gleichzeitige
Bi-Streams je Verbindung. Unbekannte Nachrichten oder zu große Rahmen schließen die Verbindung
mit einem Fehlercode.

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
  kleinerem Hash, sendet sie diese auf einem eigenen Kontroll-Stream (`DeviceListPush`), bevor
  irgendetwas anderes fließt. Der Empfänger wartet auf den vollständigen Stream, prüft Hash,
  Vault-Signatur, Generation, `issued_by`, Basisliste und Inhalt nach FR-005 und übernimmt die Liste
  atomar. Erst danach wird die geltende Liste neu gewählt.
- Vor dem Listenabgleich prüft jede Seite weiterhin `endpoint` des Gegenübers gleich
  `Connection::remote_id()`, die Authentifizierungssignatur, `vault` gleich der eigenen und
  `schema` verträglich. Nach dem vollständigen Listenabgleich prüft sie mit der aktualisierten
  geltenden Liste erneut, dass der Geräteschlüssel mit genau diesem `endpoint` eingetragen und
  nicht entfernt ist. Erst dann wird `Accept` gesendet; andernfalls gilt `NotOnList` oder `Removed`.
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

Ablauf:

1. Nach dem Handshake senden beide `Progress`. Nach jedem lokalen Commit (research R19) und nach
   jeder angewendeten Seite sendet ein Gerät erneut `Progress` an jedes verbundene Gerät, dicht
   folgende Anstöße zusammengefasst.
2. Wer im `Progress` der Gegenseite für irgendeinen Ursprung einen höheren `max_hlc` sieht als im
   eigenen Stand, schickt `Pull` mit seinem eigenen Stand. Höchstens ein `Pull` je Verbindung
   gleichzeitig.
3. Der Sender scannt jede CRDT-Tabelle und das Lösch-Log ab dem kleinsten Cursor im `Pull`,
   einschließlich `vault_key_generations` und `vault_key_envelopes`. Fehlt ein Ursprung, gilt der
   Cursor „nichts“. Enthält eine Transaktionsgruppe mindestens eine Änderung jenseits des Cursors,
   liefert er die vollständige Gruppe mit ihren ursprünglichen HLCs; effektive Zell-Teilmengen sind
   nicht erlaubt. Er sortiert die vollständigen Gruppen nach HLC aufsteigend über alle Ursprünge und
   schickt `Page`s, die nie eine Transaktionsgruppe teilen (`paginate_changes`). Gerätelokale Tabellen
   scannt er nie. Schlüsselgenerationen und Umschläge haben damit keinen zweiten Lieferweg.
4. Der Empfänger prüft jede Seite je Transaktionsgruppe (research R5), wendet gültige Gruppen in
   einer Transaktion an und verbucht dauerhaft abgewiesene Gruppen als Skip-Range. Danach erhöht er
   je Ursprung seinen Fortschritt auf den höchsten HLC dieses Ursprungs, der in dieser Seite entweder
   angewendet oder als Skip-Range verbucht wurde. Fortschritt und Skip-Ranges werden atomar mit der
   Anwendung geschrieben. Bricht die Verbindung vorher ab, gilt der Stand der zuletzt vollständig
   angewendeten oder verbuchten Seite.

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
N → H  LinkDone    { link_id, applied: true }
       -- H veröffentlicht die neue Geräteliste --
```

- `k = HKDF-SHA256(c, info = "holzi/link/proof/v1")`,
  `mac_x = HMAC-SHA256(k, "holzi-link/v1" ‖ lp(nonce_h) ‖ lp(nonce_n) ‖ lp(endpoint_h) ‖
lp(endpoint_n) ‖ lp(device_h) ‖ lp(device_n) ‖ rolle_x)`, `rolle_x` ∈ {`H`, `N`}.
- Falscher `mac`, abgelaufener oder verbrauchter Code: Verbindung zu, nichts übertragen (FR-024).
  Der Code ist verbraucht, sobald `LinkHello` gesendet wurde.
- H erzeugt vor `LinkTransfer` eine zufällige `link_id` und legt einen dauerhaften
  `pending_link`-Datensatz mit `link_id`, dem vollständigen neuen Gerätelistensatz und dem Zustand
  `transferring` an. N speichert `link_id` und den Zustand `awaiting_publication`, bevor es
  `LinkDone` sendet. Beide Seiten behalten dafür das aus dem einmal verwendeten Code abgeleitete
  Link-Sitzungsgeheimnis; der Code selbst muss nicht erneut vorgelegt werden.
- Nach einem Abbruch oder Neustart darf H mit `LinkResume { link_id, mac }` die offene Sitzung
  wiederaufnehmen. N bestätigt dabei ein bereits angewendetes `LinkTransfer` erneut mit
  `LinkDone`; die MAC wird über `"holzi-link-resume/v1" ‖ lp(link_id) ‖ lp(state)` mit dem
  Link-Sitzungsgeheimnis gebildet. H veröffentlicht den gespeicherten Gerätelistensatz in einer
  idempotenten Einfügeoperation, markiert `pending_link` als abgeschlossen und löscht ihn erst
  danach. Ein erneutes `LinkDone` wiederholt nur diese Veröffentlichung und überträgt weder Snapshot
  noch private Daten. So ist die Veröffentlichung auch nach einem Absturz zwischen `LinkDone` und
  dem ersten Listen-Commit möglich, ohne den verbrauchten Code erneut zu verwenden.
- `snapshot` sind `Page`s wie in Abschnitt 2 mit dem Stand „nichts“ für jeden Ursprung: alle Zellen
  des Bereichs „Vault“ mit ihren ursprünglichen HLCs, samt Lösch-Log, ohne gerätelokale Tabellen.
  `progress` ist der Fortschrittsstand von H nach der letzten Seite. N prüft je Transaktionsgruppe
  (FR-013) und wendet an. `key_generations` und `envelopes_for_n` sind dabei nur der initiale
  Link-Transfer; im anschließenden gewöhnlichen Sync laufen diese Zeilen ausschließlich über
  `Pull`/`Page`.
- Bricht die Verbindung vor `LinkDone` ab, löscht N die angelegte Vault; H verwirft den offenen
  `pending_link`-Datensatz nur nach einem ausdrücklich authentifizierten Abbruch. Ein bloßer
  Verbindungsabbruch lässt ihn für `LinkResume` bestehen, falls N `LinkDone` bereits dauerhaft
  gespeichert hat. Nach einem bestätigten
  `LinkDone` bleibt N bis zur beobachteten neuen Geräteliste im Zustand `awaiting_publication` und
  wiederholt `LinkDone` bei einer Wiederaufnahme.
- `vault_secret` wird nur bei gewählter Hauptgerät-Rolle gesendet und nur auf dieser Verbindung
  (FR-038).
