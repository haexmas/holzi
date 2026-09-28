# Research: Vault-Identität, Geräteliste und Datensync zwischen eigenen Geräten

**Spec**: [spec.md](./spec.md) | **Plan**: [plan.md](./plan.md) | **Datum**: 2026-09-28

Jede Entscheidung nennt, was gewählt wurde, warum und was verworfen wurde. Fakten zu fremden
Bibliotheken stammen aus deren veröffentlichten Quellen (crates.io, docs.rs) oder aus haex-vault
(Repository `https://github.com/haex-space/haex-vault`, Revision
`8dce379d94e18fcd42c3b73686a06f984ca3f574`) und sind mit „geprüft“ markiert; alles andere ist
eine Entscheidung dieses Plans.

## R1 Abhängigkeiten

**Entscheidung**:

| Crate              | Version                        | Features                                          | Wozu                                                                        |
| ------------------ | ------------------------------ | ------------------------------------------------- | --------------------------------------------------------------------------- |
| `iroh`             | 1.2                            | `default-features = false`, `tls-ring`            | QUIC-Endpunkt, direkte Verbindungen, iroh-Relay für NAT                     |
| `nostr`            | 0.45                           | `nip44`, `nip59`                                  | Schlüssel, Schnorr-Signaturen, NIP-44-Umschläge, Präsenz-Umschläge          |
| `nostr-sdk`        | 0.45                           | Standard; `local-relay` nur in `dev-dependencies` | Client für Präsenz und Verknüpfungs-Treffpunkt; In-Prozess-Relay für Tests  |
| `secp256k1`        | 0.30 (dieselbe wie in `nostr`) | –                                                 | Prüfen von Schnorr-Signaturen über eigene Nutzdaten                         |
| `hkdf`             | 0.13                           | –                                                 | Ableitungen (Vault-Identität aus Platzhalter, Präsenz-Postfach, Treffpunkt) |
| `chacha20poly1305` | 0.10                           | `std`                                             | XChaCha20-Poly1305 für die Namen in der Geräteliste                         |
| `postcard`         | 1                              | `use-std`                                         | kompakte, deterministische Kodierung der Sync-Nachrichten                   |
| `qrcode`           | 0.14                           | `svg`                                             | Verknüpfungscode als SVG für die Oberfläche                                 |

`sha2`, `zeroize`, `base64`, `uuid`, `tokio`, `tokio-util`, `serde` sind schon da.

**Begründung**: iroh ist Betreibervorgabe; geprüft: iroh 1.2.0 ist die aktuelle stabile Version
(MSRV 1.91), haex-vault nutzt dieselbe Linie mit `tls-ring`. rust-nostr 0.45 hat die NIP-Features
nur noch im Crate `nostr`, nicht mehr in `nostr-sdk` (geprüft). `nostr::PublicKey` hat keine
Prüfmethode für fremde Nutzdaten; dafür dient `secp256k1` 0.30 direkt (geprüft, dieselbe Version
vermeidet doppelte Typen). postcard statt JSON für die Rahmen, weil Handshake, Geräteliste und
Momentaufnahme Binärdaten tragen und JSON sie als Base64 um ein Drittel aufbläht; die Änderungen
selbst bleiben `ColumnChange` von haex-crdt mit ihren JSON-Werten. Die Version steckt im ALPN (R6).

**Verworfen**: `irpc`/`irpc-iroh` (zusätzliche Abstraktion ohne Bedarf; der Protokollumfang ist
klein), `aws-lc-rs` als TLS-Backend (braucht cmake/nasm), ein eingebettetes Nostr-Relay (024
braucht nur Clients), `spake2` für das Verknüpfen (R11: der Code hat 128 Bit Zufall).

## R2 Schlüssel und wo sie liegen

**Entscheidung**:

- **Vault-Identität**: öffentlicher Schlüssel in der synchronisierten Tabelle `vault_identity`
  (nur noch `pubkey`); der private Schlüssel in der neuen Tabelle `vault_identity_secret_no_sync`,
  die nur auf Hauptgeräten eine Zeile hat. Migration: die heutige Spalte `privkey` in
  `vault_identity` entfällt (Tabelle wird neu aufgebaut; die Platzhalter-Zeilen hatten nie einen
  HLC-Zeitstempel und sind nie gesynct worden).
- **Geräteschlüssel** (secp256k1) und **iroh-Schlüssel** (ed25519): in
  `device_keys_no_sync(installation_uuid PK, device_secret, device_pubkey, endpoint_secret,
endpoint_id, created_at)`. Beim Öffnen sucht holzi die Zeile zur eigenen Installations-UUID; gibt
  es keine, erzeugt es neue Schlüssel (FR-003, FR-006). Zeilen anderer Installationen bleiben
  unberührt: Eine Kopie ist ein vollständiges Backup (FR-006), und der Schlüssel des Quellgeräts
  wird nie verwendet, weil holzi nur die Zeile der eigenen Installation liest.
- **Inhaltsschlüssel des Bereichs „Vault“**: nur als Umschläge in der synchronisierten Tabelle
  `vault_key_envelopes`; jedes Gerät entpackt seinen Umschlag selbst und hält den entpackten
  Schlüssel in `vault_content_keys_no_sync`. Eine neue Generation erreicht so nie ein entferntes
  Gerät.
- Alle geheimen Bytes in `Zeroizing`, `Debug` geschwärzt; `nostr::SecretKey` wird nur kurz für eine
  Operation gebaut (geprüft: sein `Drop` löscht nur nach Möglichkeit, `ConversationKey` ist
  `Copy` und wird nicht gehalten).

**Begründung**: `_no_sync` hält Geheimnisse vom Sync fern, liegt aber in der Datei. Eine Kopie
eines Hauptgeräts nimmt den privaten Schlüssel der Vault-Identität mit; das verlangt FR-044
ausdrücklich. Für den Geräteschlüssel schreibt FR-006 vor, dass die Kopie ihn nicht verwendet und
nicht löscht; der Schlüssel je Installations-UUID erfüllt das und hält alles in der verschlüsselten
Datei (auch im portablen Modus, Spec 014). Wer Datei und Passphrase hat, hat ohnehin alle Daten.

**Verworfen**: Geräteschlüssel in einer Datei neben der Vault (braucht eine eigene Verschlüsselung
und bricht den portablen Modus), im Schlüsselbund des Betriebssystems (plattformabhängig, auf Linux
nicht überall vorhanden), fremde Zeilen beim Öffnen einer Kopie löschen (beschneidet das Backup,
Betreiber-Entscheidung), Inhaltsschlüssel entpackt in einer synchronisierten Tabelle (würde eine
neue Generation mit dem Sync an Geräte tragen, die sie nicht bekommen sollen).

## R3 Vault-Identität aus dem Platzhalter (FR-004)

**Entscheidung**: Trägt eine Vault nur den Platzhalter, berechnet holzi beim ersten Öffnen
`sk = HKDF-SHA256(ikm = platzhalter_privkey, salt = "holzi", info = "holzi/vault-identity/v1" ||
zähler)` und erhöht `zähler`, bis `sk` ein gültiger secp256k1-Skalar ist. Jede Kopie rechnet
dasselbe und kommt zur selben Vault-Identität. Jede Kopie ist danach ein Hauptgerät und trägt sich
in eine eigene Geräteliste der Generation 1 ein; treffen sich zwei Kopien, gilt FR-043 und ein
Hauptgerät veröffentlicht die zusammengeführte Generation 2.

**Begründung**: deterministisch, ohne Abstimmung zwischen den Kopien; der Platzhalter ist echter
Zufall aus `bootstrap.rs` und damit als Eingabe geeignet.

**Verworfen**: jede Kopie eine eigene Identität (trennt Kopien, die der Betreiber als eine Vault
behalten will, Clarification FR-004).

## R4 Abgleich je Ursprungsgerät über den HLC (FR-012, FR-013, FR-019, FR-020)

**Entscheidung**: Der Bereich „Vault“ gleicht den aktuellen Stand ab, wie haex-crdt es vorsieht.
Es gibt keine gespeicherten Pakete und keine Laufnummern.

- **Ursprungsgerät**: Jede Zelle trägt in ihrem Spalten-HLC die Knoten-ID des schreibenden Geräts;
  die Knoten-ID ist die `vault_device_uuid` der Installation (geprüft: `HolziBootstrap` liefert
  sie, `Database::open` initialisiert damit den HLC; `hlc_node_id_suffix`, `hlc_is_from_node` in
  `src/crdt/hlc.rs`). `apply` übernimmt den HLC der fremden Zelle unverändert und schaltet die
  Trigger dabei ab (geprüft, `engine.rs`), der Ursprung bleibt also über jede Weiterleitung
  erhalten. Das Feld `ColumnChange.device_id` füllt der Scanner mit dem scannenden Gerät; holzi
  verwendet es nicht und liest den Ursprung aus `hlc_timestamp`.
- **Fortschrittsstand**: `sync_progress_no_sync(origin, max_hlc)`, je Ursprungsgerät der höchste HLC,
  bis zu dem dieses Gerät alle Änderungen des Ursprungs hat. Für sich selbst ist es der eigene
  jüngste HLC.
- **Liefern**: Die Gegenseite schickt ihren Fortschrittsstand. Der Sender scannt jede CRDT-Tabelle
  und das Lösch-Log (`haex_deleted_rows`) einmal ab dem kleinsten Cursor der Gegenseite
  (`scan_table_for_local_changes` mit `after_hlc`; geprüft: Zeilen werden nach dem Zeilen-HLC
  vorgefiltert, dann jede Spalte nach ihrem eigenen HLC), behält je Zelle nur, was jenseits des
  Cursors ihres Ursprungs liegt, sortiert alles nach HLC aufsteigend über alle Ursprünge hinweg und
  packt es mit `paginate_changes` in Seiten, die nie eine Transaktionsgruppe teilen (geprüft). Ein
  Ursprung, den die Gegenseite nicht nennt, hat den Cursor „nichts“.
- **Anwenden**: Der Empfänger prüft jede Seite je Transaktionsgruppe (R5), wendet sie über
  `Database::apply_remote_changes` in einer Transaktion an und erhöht danach je Ursprung seinen
  Fortschritt auf den höchsten HLC dieses Ursprungs in der Seite. Der Fortschritt wird erst nach
  dem Commit geschrieben: Bricht es dazwischen ab, holt das Gerät die Seite noch einmal, und
  `apply` überspringt, was es schon hat (gleicher HLC, LWW). Ein Fortschritt über dem wirklichen
  Stand kann so nicht entstehen.
- **Lückenlos**: Weil jede Seite je Ursprung aufsteigend ist und der Fortschritt nur mit
  angewendeten Seiten wächst, hat jedes Gerät von jedem Ursprung immer einen Anfang ohne Lücke. Ein
  Gerät, das über B nur einen Teil von A bekam, fordert beim nächsten Mal ab genau diesem Stand an,
  bei wem auch immer.
- **Überschriebene Zellen**: Hat P eine Zelle einer Transaktion von A schon überschrieben, liefert
  der Sender von dieser Transaktion nur den Rest und P's Zelle im Ursprung P. Weil die Seiten über
  alle Ursprünge nach HLC sortiert sind, folgt P's Zelle kurz danach, meist in derselben Seite; der
  Endstand stimmt immer (Edge Case in der Spec).
- **Anstoßen**: Nach jedem lokalen Commit (R19) und nach jedem angewendeten Empfang sendet ein
  Gerät seinen neuen Fortschrittsstand an jedes verbundene Gerät; wer daraus erkennt, dass ihm
  etwas fehlt, fordert es an. Daten werden nie ungefragt geschoben, so entsteht auch beim Anstoßen
  keine Lücke.

**Begründung**: haex-crdt bringt alles mit: HLC je Transaktion, Ursprung im HLC, Filter nach
Cursor, Seiten ohne geteilte Transaktion, idempotentes `apply`. Ein eigenes Paketprotokoll würde
jede lokale Transaktion ein zweites Mal speichern, verschlüsseln und signieren, ohne dass eigene
Geräte das brauchen (Betreiber-Entscheidung beim Plan).

**Verworfen**: gespeicherte, signierte Änderungspakete mit Laufnummern je Ursprungsgerät (doppelte
Datenhaltung, vier Änderungen an haex-crdt, Nutzen erst bei Fremden; Laufnummern kommen dort mit
027/028), ein einziger Zeitpunkt „zuletzt synchronisiert“ je Gegenseite (verliert Änderungen über
Zwischengeräte), Daten ungefragt nach jedem Commit schieben (Lücken beim Empfänger).

## R5 Prüfen beim Empfang (FR-013, FR-014, FR-021, FR-028)

**Entscheidung**: holzi prüft vor `apply`, gruppiert nach Transaktions-HLC
(`group_by_hlc_key`, geprüft). Eine Gruppe hat genau einen Ursprung. Verworfen wird eine ganze
Gruppe, wenn ihr Ursprung in einer bekannten gültigen Geräteliste als entfernt steht und ihr HLC
jenseits seiner Grenze liegt, oder wenn sie eine Tabelle berührt, die dieses Gerät nicht als
CRDT-Tabelle kennt. Die übrigen Gruppen gehen an `Database::apply_remote_changes` (heute mit
`SignatureApplyPolicy` und `NoopSignatureProvider`, geprüft `database/mod.rs`). Ein Ursprung, der
auf keiner Geräteliste steht, wird angenommen, wenn ein geprüftes eigenes Gerät ihn liefert: Das
betrifft Daten aus der Zeit vor dieser Spec und Kopien in der Wartezeit (FR-044).

Momentaufnahmen beim Verknüpfen prüft holzi auf dieselbe Weise.

**Begründung**: Im Bereich „Vault“ bürgt das liefernde Gerät (FR-021); die Prüfung richtet sich nur
gegen entfernte Geräte. Weil sie vor `apply` stattfindet und der Fortschritt nach dem Commit
geschrieben wird, braucht holzi keine eigene `ApplyPolicy` und keinen neuen Einstieg in haex-crdt.

**Verworfen**: eine eigene `ApplyPolicy` über einen neuen Einstieg `apply_remote_changes_with_policy`
(nur nötig, um den Fortschritt im selben Commit zu schreiben; das ist wegen des idempotenten
`apply` unnötig), eine ganze Seite verwerfen, wenn eine Gruppe ungültig ist (der Sender liefert die
Gruppe immer wieder, der Abgleich stünde still).

## R6 Transport: iroh-Endpunkt und Protokoll

**Entscheidung**:

- Ein Endpunkt je Vault-Session, erst nach dem Entsperren gebaut, mit dem gespeicherten
  iroh-Schlüssel: `Endpoint::builder(presets::Minimal)` (keine n0-DNS/pkarr-Adresssuche),
  `.secret_key(..)`, `.alpns([b"holzi-sync/1", b"holzi-link/1"])`,
  `.relay_mode(RelayMode::custom(urls))` aus den Einstellungen (Standard: die öffentlichen
  iroh-Relays von n0), eigene Adresssuche über `MemoryLookup`, gefüllt aus Präsenzmeldungen (R7). `bind()`
  mit 15 s Zeitgrenze (geprüft: haex-vault macht das, weil `bind` an der DNS-Auflösung der iroh-Relays hängen kann).
- Annehmen über `Router` mit je einem `ProtocolHandler` pro ALPN.
- **Gegenseitiger Handshake** auf dem ersten Bi-Stream, den der Annehmende öffnet: Herausforderung
  mit 32 Byte Zufall in beide Richtungen; jede Seite signiert mit ihrem Geräteschlüssel (Schnorr)
  `SHA-256("holzi-device-auth/v1" || len‖nonce_a || len‖nonce_b || len‖eigene endpoint_id ||
len‖fremde endpoint_id || len‖vault_pubkey)`. Geprüft wird: `endpoint_id` gleich
  `Connection::remote_id()`, Signatur passt zum Geräteschlüssel, dieser steht mit genau dieser
  `endpoint_id` auf der aktuellen Geräteliste und gilt nicht als entfernt. Muster aus haex-vault
  `quic_did_auth` (geprüft), dort nur einseitig; hier gegenseitig, weil die Bindung
  `endpoint_id ↔ Geräteschlüssel` sonst nur an der Präsenzmeldung hinge.
- **Nachrichten**: ein Bi-Stream je Anfrage, Rahmen `u32 BE Länge ‖ postcard`, Obergrenze 4 MiB je
  Rahmen, geprüft vor dem Anlegen des Puffers; Änderungen werden seitenweise übertragen. Höchstens 8
  gleichzeitige Streams je Verbindung. Einzelheiten:
  [contracts/sync-protocol.md](./contracts/sync-protocol.md).
- **Ende der Session** (FR-031): der Sync-Dienst hängt am Abbruch-Token des Vault-Gates;
  `router.shutdown()` mit 2 s Zeitgrenze, danach alle Endpunkt-Klone freigeben (geprüft: UDP-Sockets
  schließen erst, wenn der letzte Klon weg ist).

**Verworfen**: `presets::N0` (veröffentlicht Adressen bei n0 über pkarr, widerspricht der
Präsenz über Nostr), ein einseitiger Handshake, ein großer Rahmen je Sync (bis 200 MB in
haex-vault; Seiten halten den Speicher klein).

## R7 Präsenz über Nostr (FR-007, FR-008, FR-010)

**Entscheidung**:

- **Postfach-Schlüssel**: `mb_sk = HKDF(inhaltsschlüssel der aktuellen Generation, info =
"holzi/presence/v1" || tagesnummer)`, daraus `mb_pk`. Nur Geräte mit dem Inhaltsschlüssel können
  ihn bilden; nach einem Entfernen wechselt er mit der Generation (FR-026).
- **Ereignis**: ein flüchtiges Ereignis der Art 21059 im Aufbau von NIP-59 (Siegel der Art 13 mit
  dem Geräteschlüssel signiert, außen ein zufälliger Einmalschlüssel, `p`-Tag `mb_pk`, Inhalt mit
  NIP-44 verschlüsselt), von Hand gebaut statt über `GiftWrapBuilder`. Inhalt: Geräteschlüssel,
  `endpoint_id`, URL des iroh-Relays und direkte Adressen, Generation der Geräteliste, Zeitstempel, Nonce.
  Flüchtige Arten speichert ein Nostr-Relay nicht (geprüft, NIP-01).
- **Takt**: beim Öffnen der Vault, bei jeder Adressänderung (`watch_addr`) und alle 60 s. Wer
  gerade online ist, abonniert `kind 21059, #p = mb_pk` (auch für den Vortag, um den Tageswechsel
  abzudecken) und verbindet sich mit jedem neuen Gerät; so reicht es, dass eine Seite die andere
  hört. „Online“ gilt bis 150 s nach der letzten Meldung oder solange eine Verbindung besteht
  (SC-008: Wechsel binnen 60 s sichtbar).
- **Ein-Gerät-Vault** (FR-007): abonniert, veröffentlicht aber nicht; antwortet nur, indem es sich
  mit einem sich meldenden Gerät verbindet.
- **Server** (FR-008): voreingestellte öffentliche Nostr-Relays, die der Nutzer ändern kann; vor
  der Auslieferung prüft ein Task per NIP-11, dass sie flüchtige Ereignisse annehmen. Kandidaten
  (zu prüfen): `wss://relay.damus.io`, `wss://nos.lol`, `wss://relay.primal.net`. iroh-Relays:
  die n0-Standardrelays, ersetzbar.

**Begründung**: Präsenz nützt nur, wenn beide Geräte online sind, denn der Sync ist direkt; ein
gespeichertes Ereignis (1059) würde Nostr-Relays mit Meldungen füllen und wegen der Zeitverschiebung von
NIP-59 (bis zu 2 Tage, geprüft) eine lange Ablaufzeit brauchen. Der tägliche Postfach-Schlüssel
verhindert, dass ein Nostr-Relay dieselbe Vault über Tage verknüpft (D9 erlaubt Metadaten, verlangt es
aber nicht).

**Verworfen**: Art 1059 mit NIP-40-Ablauf (gespeichert, Ablauf vor 2 Tagen unzuverlässig),
Präsenz an jedes Gerät einzeln (mehr Ereignisse, Geräteanzahl sichtbar), mDNS im lokalen Netz
(nicht verlangt; die iroh-Relays decken auch das lokale Netz ab).

## R8 Geräteliste (FR-005, FR-043)

**Entscheidung**: Die Liste ist ein kanonisch kodierter Datensatz (postcard, feste Feldfolge) mit
Vault-Identität, Generation, Einträgen (Geräteschlüssel, `endpoint_id`, Rolle, `vault_device_uuid`,
verschlüsselter Name) und entfernten Einträgen (Geräteschlüssel, `vault_device_uuid`, Grenze als HLC), signiert mit der
Vault-Identität über `SHA-256("holzi-device-list/v1" || bytes)`. Der Name ist mit einem aus dem
Inhaltsschlüssel abgeleiteten Schlüssel verschlüsselt (FR-005: Sync-Server und Mitglieder
sehen ihn nicht). Gespeichert in der synchronisierten Tabelle `device_lists(list_hash PK, generation,
payload, signature)`; die geltende Liste ist die höchste Generation, bei Gleichstand der kleinste
Hash. Ein Gerät gilt als entfernt, sobald irgendeine gültige Liste es als entfernt führt.

Ein Hauptgerät, das zwei gültige Listen gleicher Generation sieht, veröffentlicht die Vereinigung
(aktuelle Geräte beider, abzüglich aller entfernten) als nächste Generation. Das deckt auch das
gegenseitige Entfernen zweier Hauptgeräte ab (Edge Case): beide bleiben entfernt.

**Begründung**: Die Liste reist als gewöhnliche Vault-Information (FR-005) und wird zusätzlich im
Handshake verglichen (FR-009), damit ein Gerät eine neuere Liste auch ohne vorherigen Sync kennt.
Gespeichert als unveränderliche Zeilen statt als eine überschriebene Zeile, damit „entfernt bleibt
entfernt“ aus allen je gesehenen Listen folgt.

## R9 Inhaltsschlüssel und Umschläge (FR-015)

**Entscheidung**: Ein Inhaltsschlüssel sind 32 Zufallsbytes mit `key_id = SHA-256("holzi-key-id/v1"
|| schlüssel)[0..16]`, einer Generation und dem Bereich. Umschlag: NIP-44 v2 vom Geräteschlüssel
des ausstellenden Hauptgeräts an den Geräteschlüssel des Empfängers, Inhalt `{bereich, generation,
key_id, schlüssel}` (Format aus dem Entwurf, §5.2). In 024 verwendet ihn holzi für zweierlei: den
Postfach-Schlüssel der Präsenz (R7) und die Namen in der Geräteliste (XChaCha20-Poly1305 mit 24 Byte
Nonce und einem abgeleiteten Schlüssel `HKDF(inhaltsschlüssel, "holzi/device-name/v1")`, zusätzliche
Daten `generation ‖ device_pubkey`). Beides nutzt die höchste Generation, die an kein entferntes
Gerät verpackt ist (Entwurf §5.2). Änderungspakete mit diesem Schlüssel kommen mit Spec 026.

**Verworfen**: Änderungen auch zwischen eigenen Geräten mit dem Inhaltsschlüssel verschlüsseln (die
iroh-Verbindung ist schon mit TLS 1.3 verschlüsselt und beidseitig geprüft; eine zweite Schicht
schützt dort nichts zusätzlich).

## R10 Kodierung signierter Daten

**Entscheidung**: Signiert wird immer `SHA-256(domänen-tag ‖ kanonische bytes)` mit BIP-340;
kanonische Bytes sind postcard eines festen Structs. Domänen-Tags: `holzi-device-auth/v1`,
`holzi-device-list/v1`, `holzi-admission/v1`, `holzi-link/v1`; die Präsenzmeldung ist ein
Nostr-Ereignis und trägt dessen Signatur (R7). Signaturen
je Änderung gibt es in 024 nicht (FR-021); die Zellsignatur mit Laufnummer für gemeinsame Bereiche
legen Specs 027 und 028 fest und bauen dafür auf `column_sig_preimage*` von haex-crdt auf.

**Verworfen**: JSON für signierte Daten (Feldreihenfolge und Zahlformat sind nicht eindeutig).

## R11 Verknüpfen (FR-023 bis FR-025)

**Entscheidung**:

- **Code**: 16 Zufallsbytes, als Base32 in Vierergruppen (26 Zeichen) und als QR-Code (SVG aus
  `qrcode`), einmal verwendbar, 10 Minuten gültig. Auf dem Desktop gibt die Nutzerin den Code ein
  oder fügt ihn ein; einen Kamera-Scanner gibt es erst mit mobilen Geräten (FR-024 verlangt
  „scannen oder eingeben“).
- **Treffpunkt**: Das Hauptgerät abonniert, solange der Code gilt, flüchtige Ereignisse an
  `rv_pk = HKDF(code, "holzi/link/rendezvous/v1")`. Die neue Installation veröffentlicht dort ihre
  `endpoint_id` und Adressen (NIP-44 an `rv_pk`); das Hauptgerät verbindet sich über ALPN
  `holzi-link/1`.
- **Nachweis**: beide Seiten senden `HMAC-SHA256(HKDF(code, "holzi/link/proof/v1"), transkript)`
  über beide `endpoint_id`s und Geräteschlüssel; erst danach zeigt das Hauptgerät den Namen und die
  Rollenfrage (FR-024). 128 Bit Zufall machen einen Offline-Angriff auf das Transkript aussichtslos.
- **Übertragung**: nach der Bestätigung eine Momentaufnahme des Bereichs „Vault“ (alle Zellen mit
  ihren ursprünglichen HLCs, samt Lösch-Log, ohne gerätelokale Daten), der Fortschrittsstand, alle
  Inhaltsschlüssel-Generationen als Umschläge an das neue Gerät, die neue Geräteliste und bei
  gewählter Hauptgerät-Rolle der private Schlüssel der Vault-Identität, alles auf dieser
  Verbindung. Die neue Installation legt ihre Vault mit ihrer Passphrase an, wendet die
  Momentaufnahme an und behält bei Abbruch nichts (FR-025). Das Hauptgerät veröffentlicht die neue
  Geräteliste erst danach.
- Die neue Installation muss dieselbe Version des Sync-Protokolls und des Vault-Schemas haben
  (FR-029); sonst bricht das Verknüpfen mit einer Meldung ab.

**Verworfen**: eine kopierte SQLite-Datei übertragen (die neue Passphrase kennt der Absender
nicht), SPAKE2 mit kürzerem Code (zusätzliche Kryptographie für einen Komfortgewinn, den der
QR-Code schon bringt).

## R12 Kopie der Vault-Datei und Aufnahmeanfrage (FR-006, FR-044, FR-045)

**Entscheidung**: Beim Öffnen erkennt holzi eine Kopie daran, dass `device_keys_no_sync` keine
Zeile für die eigene Installations-UUID hat, aber Zeilen anderer. Es erzeugt eigene Schlüssel in
einer neuen Zeile und legt über den vorhandenen `known_devices`-Ablauf eine neue
`vault_device_uuid` an; die neue Installation schreibt damit unter einer eigenen HLC-Knoten-ID. Die
Zeilen des Quellgeräts bleiben unverändert in der Datei (FR-006).

- Mit `vault_identity_secret_no_sync`: neue Geräteliste mit sich selbst als Hauptgerät, Hinweis an
  die Nutzerin.
- Ohne: signierte Aufnahmeanfrage (Name, Geräteschlüssel, `endpoint_id`), per Präsenzmeldung an
  die Vault; jedes Gerät, das sie empfängt, legt sie in der synchronisierten Tabelle
  `admission_requests` ab. Die Kopie synchronisiert nichts, bis sie auf der Geräteliste steht;
  ihre lokalen Änderungen tragen ihre neue Knoten-ID und gehen nach der Aufnahme mit dem
  gewöhnlichen Abgleich mit (R4).

## R13 Entfernen (FR-026 bis FR-028)

**Entscheidung**: Ein Hauptgerät stellt in einer Transaktion aus: Geräteliste Generation+1 ohne
das Gerät, mit dessen Grenze (sein Fortschrittsstand für das Gerät, also der höchste HLC, bis zu
dem es alle Änderungen von ihm hat, R4), und eine neue
Inhaltsschlüssel-Generation mit Umschlägen nur an die verbleibenden Geräte. Laufende Verbindungen
zum entfernten Gerät werden sofort geschlossen, sein Eintrag aus dem `MemoryLookup` genommen. Das
Neu-Unterschreiben von Mitgliederlisten (FR-026) ist in 024 leer, weil es noch keine Spaces gibt;
die Stelle ist als Erweiterungspunkt für 027/028 angelegt. Geheimnisse werden nicht verändert.

## R14 Doppelte Geräte und Versionen (FR-029, FR-030)

**Entscheidung**: Die Protokollversion steckt im ALPN; die Schema-Version (Migrationsstand von
holzi und haex-crdt-Triggerversion) wird im Handshake verglichen. Bei Abweichung: kein Sync, beide
zeigen „ein Gerät muss aktualisiert werden“. Doppelt: zwei gleichzeitige Präsenzmeldungen oder
Verbindungen mit demselben Geräteschlüssel, aber verschiedenen `endpoint_id`s, oder dieselbe
`vault_device_uuid` auf zwei Geräteschlüsseln → Sync mit beiden anhalten, Meldung.

## R15 Oberfläche

**Entscheidung**: `FederationView.vue` wird zur Unteransicht „Geräte“ mit Kopf (öffentliche
Vault-Identität als `npub`, kopierbar), Liste (Name, Rolle, dieses Gerät, online/zuletzt online,
Grund bei Problemen), auf Hauptgeräten „Gerät verknüpfen“, „Gerät entfernen“ und offene
Aufnahmeanfragen, darunter die Gruppe „Verbindungsserver“ (Nostr- und iroh-Relays). Aufbau nach
COSMIC/GNOME wie in Spec 023 (Gruppen und Zeilen aus haex-ui). Die Startseite bekommt „Mit einer
Vault verknüpfen“ (`LinkSheet.vue`). Live-Aktualisierung über ein Tauri-Ereignis
`sync-devices-changed`. Jede Handlung ist eine Aktion im Katalog von Spec 020; Verknüpfen,
Aufnehmen, Ablehnen und Entfernen sind nicht für Agenten freigegeben (FR-036).

## R16 Tests

**Entscheidung**:

- **Rust-Unit-Tests** in eigenen `_tests.rs`-Dateien: NIP-44-Testvektoren, Schnorr, HKDF-Ableitung
  aus dem Platzhalter, Regeln der Geräteliste (Generation, kleinster Hash, entfernt bleibt
  entfernt, gegenseitiges Entfernen), Liefern je Ursprung (Cursor, Sortierung über alle
  Ursprünge, Seiten ohne geteilte Gruppe), Fortschritt erst nach dem Commit, Abbruch zwischen
  Seiten, Grenze eines entfernten Geräts, Prüfung je Gruppe.
- **Rust-Integrationstests** unter `src-tauri/tests/`: mehrere Geräte im selben Prozess, jedes mit
  eigener Vault-Datei und eigenem iroh-Endpunkt (`presets::Minimal`, `RelayMode::Disabled`,
  `MemoryLookup` mit den Loopback-Adressen, geprüft aus haex-vault
  `owner_sync_integration_tests/helpers.rs`), Präsenz über `nostr_sdk::local_relay::MockRelay`.
  SC-003 (drei Geräte, 1.000 Änderungen, wechselnde Wege) läuft hier.
- **E2E** (Spec 016): zwei App-Prozesse, Nostr über ein kleines Test-Relay aus
  `nostr-sdk`/`local-relay`, das das E2E-Werkzeug startet, iroh direkt über Loopback; die Server
  setzt das Szenario über die Einstellungen. Szenarien für User Stories 1, 2, 3 und 5 (SC-011).
- Keine willkürlichen Wartezeiten in Tests (Constitution): Warten auf Ereignisse mit Zeitgrenze.

## R17 Leistung

Ein Abgleich scannt jede CRDT-Tabelle einmal ab dem kleinsten Cursor der Gegenseite und filtert je
Ursprung im Speicher; bei 1–10 Geräten und den heutigen Tabellen sind das wenige Dutzend Abfragen.
Im laufenden Betrieb liegen die Cursor dicht am aktuellen Stand, der Scan liefert nur die jüngsten
Zeilen. SC-001 (5 s) wird von Präsenz und Verbindungsaufbau bestimmt; bei bestehender Verbindung
stößt der Commit sofort den Abgleich an (R4, R19). Sollte der Scan je Tabelle messbar werden, kann
er sich auf die Tabellen beschränken, die haex-crdt als geändert markiert
(`scan_dirty_tables`).

## R18 Graphify-Abfrage

Abfragen: „where are vault identity, known_devices and extension table ownership defined?“ und
„which modules handle vault open/unlock, lifecycle close, onboarding start page and tauri command
registration?“ (Budget 1.000 bzw. 1.200, abgeschnitten). Kandidaten und Ergebnis:
`identity/bootstrap.rs`, `identity/migrations.rs`, `storage/known_devices.rs`, `device/commands.rs`
werden erweitert; `instances/presence.rs` meint Prozesse, nicht Geräte, und passt nicht;
`vault_gate` liefert Abbruch-Token und Task-Tracker für den Sync-Dienst. Ein Sync-Modul gibt es
nicht; `src-tauri/src/sync/` ist neu. Der Graph ist vom 2026-09-21 und älter als 022/023; die
Kandidaten wurden im Code geprüft (zur manuellen Nachprüfung vermerkt).

## R19 Zentrale Schreib- und Lesewege in holzi

**Befund** (geprüft): holzi hat keinen allgemeinen Befehl wie `sql_execute`/`sql_execute_with_crdt`
in haex-vault. Chat, Einstellungen, Sitzung, Provider, Modelle, Wartung und Freigaben schreiben an
24 Stellen in 15 Dateien selbst über `haex_crdt::Database::with_connection`, den Notausgang hinter
dem Feature `raw-connection`, jede mit eigenem `spawn_blocking` und eigener Fehlerumwandlung. Die
CRDT-Erfassung stimmt trotzdem, weil sie über Trigger und `current_hlc()` an der Verbindung hängt:
Jede Schreibstelle auf eine CRDT-Tabelle wird erfasst, `_no_sync`-Tabellen nicht. Was nicht stimmt:
In 10 der 15 Dateien öffnet keine Stelle selbst eine Transaktion. Eine Stelle mit mehreren
Anweisungen (etwa `rename_thread`: lesen, dann schreiben) läuft so in mehreren Transaktionen mit je
eigenem HLC und ist nicht atomar, auch ohne Sync.

**Entscheidung**: Vor dem Sync bekommt holzi ein Modul `src-tauri/src/storage/vault_db.rs` mit
genau diesen Wegen, alle asynchron mit `spawn_blocking` und einheitlicher Umwandlung in
`HolziError`:

- `read(|conn| …)`: nur lesen.
- `write(|tx| …)`: öffnet immer eine Transaktion (`unchecked_transaction`, da `with_connection` nur
  `&Connection` gibt), führt alles darin aus und committet; eine Transaktion ist ein HLC und damit
  eine Transaktionsgruppe. Ob erfasst wird, folgt der Tabelle, wie in haex-crdt: CRDT-Tabellen ja,
  `_no_sync`-Tabellen nein. Nach dem Commit stößt `write` den Sync-Dienst an
  (`tokio::sync::Notify`); der fasst dicht folgende Anstöße zusammen und schickt verbundenen
  Geräten seinen Fortschrittsstand (R4). Hat sich nichts Synchronisiertes geändert, fordert niemand
  etwas an.
- `write_untracked(|tx| …)`: nur für Wartung (`storage/maintenance.rs`), schaltet die Trigger für
  die Dauer der Transaktion ab wie `haex_crdt::execute`; heute nutzt ihn keine Stelle, er entsteht
  erst, wenn eine ihn braucht.

Alle 24 Stellen ziehen auf diese Wege um. `Database::with_connection` steht danach in
`clippy.toml` unter `disallowed-methods`; nur `vault_db.rs` und der Sync-Dienst dürfen es mit einer
begründeten Ausnahme aufrufen. So kann keine neue Stelle den Weg umgehen.

**Begründung**: ein Ort für Transaktion, Fehler und Anstoß des Sync statt 24; atomare
Schreibvorgänge unabhängig vom Sync; der Sync muss nicht in haex-crdt einen Commit-Beobachter
bekommen.

**Verworfen**: ein Commit-Beobachter in haex-crdt (behandelt das Symptom, die 24 Stellen blieben),
`execute_with_crdt` von haex-crdt als einziger Weg (eine Anweisung je Aufruf und Transaktion; holzi
braucht mehrere Anweisungen in einer Transaktion; ob `write` es intern für einzelne Anweisungen
nutzt, prüft der erste Umsetzungsschritt), ein allgemeiner SQL-Befehl für die Oberfläche wie in
haex-vault (holzi schreibt nur aus dem Backend; ein SQL-Befehl wäre eine neue Angriffsfläche für
Erweiterungen und Agenten).
