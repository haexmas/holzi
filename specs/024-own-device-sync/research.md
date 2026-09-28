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
| `chacha20poly1305` | 0.10                           | `std`                                             | XChaCha20-Poly1305 für Änderungspakete                                      |
| `postcard`         | 1                              | `use-std`                                         | kompakte, deterministische Kodierung der Sync-Nachrichten                   |
| `qrcode`           | 0.14                           | `svg`                                             | Verknüpfungscode als SVG für die Oberfläche                                 |

`sha2`, `zeroize`, `base64`, `uuid`, `tokio`, `tokio-util`, `serde` sind schon da.

**Begründung**: iroh ist Betreibervorgabe; geprüft: iroh 1.2.0 ist die aktuelle stabile Version
(MSRV 1.91), haex-vault nutzt dieselbe Linie mit `tls-ring`. rust-nostr 0.45 hat die NIP-Features
nur noch im Crate `nostr`, nicht mehr in `nostr-sdk` (geprüft). `nostr::PublicKey` hat keine
Prüfmethode für fremde Nutzdaten; dafür dient `secp256k1` 0.30 direkt (geprüft, dieselbe Version
vermeidet doppelte Typen). postcard statt JSON auf dem Draht, weil Pakete Binärdaten tragen und JSON
sie als Base64 um ein Drittel aufbläht; die Version steckt im ALPN (R6).

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
  es keine, erzeugt es neue Schlüssel (FR-003, FR-006). Zeilen anderer Installationen löscht es beim
  ersten Öffnen einer Kopie sofort (FR-006: der Schlüssel des Quellgeräts wird nie verwendet).
- **Inhaltsschlüssel des Bereichs „Vault“**: nur als Umschläge in der synchronisierten Tabelle
  `vault_key_envelopes`; jedes Gerät entpackt seinen Umschlag selbst und hält den entpackten
  Schlüssel in `vault_content_keys_no_sync`. Eine neue Generation erreicht so nie ein entferntes
  Gerät, auch nicht als Inhalt eines Pakets.
- Alle geheimen Bytes in `Zeroizing`, `Debug` geschwärzt; `nostr::SecretKey` wird nur kurz für eine
  Operation gebaut (geprüft: sein `Drop` löscht nur nach Möglichkeit, `ConversationKey` ist
  `Copy` und wird nicht gehalten).

**Begründung**: `_no_sync` hält Geheimnisse vom Sync fern, liegt aber in der Datei. Eine Kopie
eines Hauptgeräts nimmt den privaten Schlüssel der Vault-Identität mit; das verlangt FR-044
ausdrücklich. Für den Geräteschlüssel schreibt FR-006 nur vor, dass die Kopie ihn nicht verwendet;
das Löschen beim ersten Öffnen erfüllt das und hält alles in der verschlüsselten Datei (auch im
portablen Modus, Spec 014).

**Verworfen**: Geräteschlüssel in einer Datei neben der Vault (braucht eine eigene Verschlüsselung
und bricht den portablen Modus), im Schlüsselbund des Betriebssystems (plattformabhängig, auf Linux
nicht überall vorhanden), Inhaltsschlüssel entpackt in einer synchronisierten Tabelle (würde eine
neue Generation über alte Pakete an entfernte Geräte tragen).

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

## R4 Laufnummern und Paketprotokoll (FR-019 bis FR-021)

**Entscheidung**: haex-crdt synchronisiert Zustände, nicht Vorgänge (geprüft: der Scanner liefert
den aktuellen Stand je Zelle, `src/crdt/scanner/mod.rs`). Lückenlose Laufnummern verlangen deshalb
ein **Paketprotokoll beim Ursprungsgerät**:

1. Die Trigger von haex-crdt tragen jede lokal begonnene Transaktion mit ihrem HLC-Zeitstempel in
   eine neue Tabelle `haex_crdt_local_tx_no_sync(hlc PK, node, seq, UNIQUE(node, seq))` ein, `seq`
   lückenlos je Gerät (Upstream-Änderung U2, [contracts/haex-crdt-upstream.md](./contracts/haex-crdt-upstream.md)).
   Die Trigger laufen nur bei lokalen Schreibvorgängen, weil `apply` sie abschaltet (geprüft,
   `engine.rs:105`).
2. Nach jedem Commit weckt ein Commit-Beobachter (U4) den **Versiegler**. Er liest die eigenen
   Änderungen nach seinem Cursor, gruppiert sie nach HLC, ordnet jeder Gruppe über
   `haex_crdt_local_tx_no_sync` ihre Laufnummer zu, signiert jede Zelle, verschlüsselt die Gruppe
   zu einem Änderungspaket und legt es unveränderlich in `sync_packages_no_sync` ab.
3. Weitergegeben werden immer diese gespeicherten Pakete, nie ein neuer Scan. Empfangene Pakete
   landen ebenfalls dort, so kann jedes Gerät Lücken anderer schließen.

Wurde eine Zelle einer älteren Transaktion vor dem Versiegeln schon überschrieben, trägt deren
Paket sie nicht mehr; der neuere Wert kommt mit der späteren Laufnummer, und der lückenlose
Fortschritt garantiert, dass der Empfänger auch diese bekommt. LWW führt damit zum selben Stand.

**Autor**: Ursprungsgerät (Geräteschlüssel), Vault und Laufnummer stehen im Signatur-Eintrag der
Zelle (`sig`-JSON, das haex-crdt unverändert speichert und weitergibt, geprüft
`apply/signature_policy.rs`, `scanner/emit.rs`). Upstream U1 korrigiert zusätzlich
`ColumnChange.device_id` auf das Ursprungsgerät.

**Aufräumen**: Ein Paket darf aus `sync_packages_no_sync` entfernt werden, wenn jedes Gerät der
aktuellen Geräteliste laut zuletzt bekanntem Fortschrittsstand es hat. Ein später verknüpftes Gerät
bekommt den Stand beim Verknüpfen als Momentaufnahme (R11), nicht die alten Pakete.

**Verworfen**: Laufnummer beim Scannen vergeben (nicht lückenlos über Weiterleitungen), Hook über
`execute_with_crdt` (holzi schreibt an 24 Stellen über `with_connection`), ein Schreib-Wrapper in
holzi um jede Stelle (fehleranfällig, jede neue Stelle müsste daran denken).

## R5 Prüfen beim Empfang (FR-013, FR-014, FR-021)

**Entscheidung**: eine eigene `ApplyPolicy` und ein eigener `SignatureProvider` in
`src-tauri/src/sync/policy.rs`, übergeben über das neue `apply_remote_changes_with_policy` (U3).

- `preflight`: jede Signatur prüfen (in Paketen über direkte, nach FR-009 geprüfte Verbindungen
  darf sie entfallen, FR-021), Ursprungsgerät auf der Geräteliste oder innerhalb seiner Grenze,
  Laufnummer noch frei oder gleich, Bereich passt.
- `before_commit`: Hat `apply` eine Zelle aus einem anderen Grund als „veraltet“, „im Paket
  überholt“ oder „durch Löschung verdeckt“ übersprungen, schlägt der Commit fehl und nichts wird
  geschrieben (Paket atomar, FR-013). Im selben Commit schreibt die Policy den neuen
  Fortschrittsstand und legt das Paket ab.
- Momentaufnahmen (für Verknüpfen und Spec 026): holzi prüft vorab je Transaktionsgruppe und gibt
  nur die gültigen Gruppen an `apply`.

**Begründung**: `apply_remote_changes` ist heute fest an `SignatureApplyPolicy` gebunden (geprüft,
`database/mod.rs:226-232`), und `with_connection` gibt nur `&Connection`, `apply` braucht
`&mut Connection`. Ein Einstieg mit eigener Policy ist die kleinste allgemeine Änderung.

## R6 Transport: iroh-Endpunkt und Protokoll

**Entscheidung**:

- Ein Endpunkt je Vault-Session, erst nach dem Entsperren gebaut, mit dem gespeicherten
  iroh-Schlüssel: `Endpoint::builder(presets::Minimal)` (keine n0-DNS/pkarr-Adresssuche),
  `.secret_key(..)`, `.alpns([b"holzi-sync/1", b"holzi-link/1"])`,
  `.relay_mode(RelayMode::custom(urls))` aus den Einstellungen (Standard: die öffentlichen
  n0-Relays), eigene Adresssuche über `MemoryLookup`, gefüllt aus Präsenzmeldungen (R7). `bind()`
  mit 15 s Zeitgrenze (geprüft: haex-vault macht das, weil `bind` an Relay-DNS hängen kann).
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
  Rahmen, geprüft vor dem Anlegen des Puffers; Pakete werden seitenweise übertragen. Höchstens 8
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
  `endpoint_id`, Relay-URL und direkte Adressen, Generation der Geräteliste, Zeitstempel, Nonce.
  Flüchtige Arten speichert ein Relay nicht (geprüft, NIP-01).
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
gespeichertes Ereignis (1059) würde Relays mit Meldungen füllen und wegen der Zeitverschiebung von
NIP-59 (bis zu 2 Tage, geprüft) eine lange Ablaufzeit brauchen. Der tägliche Postfach-Schlüssel
verhindert, dass ein Relay dieselbe Vault über Tage verknüpft (D9 erlaubt Metadaten, verlangt es
aber nicht).

**Verworfen**: Art 1059 mit NIP-40-Ablauf (gespeichert, Ablauf vor 2 Tagen unzuverlässig),
Präsenz an jedes Gerät einzeln (mehr Ereignisse, Geräteanzahl sichtbar), mDNS im lokalen Netz
(nicht verlangt; die iroh-Relays decken auch das lokale Netz ab).

## R8 Geräteliste (FR-005, FR-043)

**Entscheidung**: Die Liste ist ein kanonisch kodierter Datensatz (postcard, feste Feldfolge) mit
Vault-Identität, Generation, Einträgen (Geräteschlüssel, `endpoint_id`, Rolle, `vault_device_uuid`,
verschlüsselter Name) und entfernten Einträgen (Geräteschlüssel, Grenze), signiert mit der
Vault-Identität über `SHA-256("holzi-device-list/v1" || bytes)`. Der Name ist mit einem aus dem
Inhaltsschlüssel abgeleiteten Schlüssel verschlüsselt (FR-005: Relay und Mitglieder sehen ihn
nicht). Gespeichert in der synchronisierten Tabelle `device_lists(list_hash PK, generation,
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
key_id, schlüssel}` (Format aus dem Entwurf, §5.2). Pakete: XChaCha20-Poly1305 mit 24 Byte Nonce,
zusätzliche Daten `bereich ‖ key_id ‖ ursprungsgerät ‖ laufnummer`. Neue Pakete nutzen die höchste
Generation, die an kein entferntes Gerät verpackt ist (Entwurf §5.2).

## R10 Kodierung signierter Daten

**Entscheidung**: Signiert wird immer `SHA-256(domänen-tag ‖ kanonische bytes)` mit BIP-340;
kanonische Bytes sind postcard eines festen Structs. Domänen-Tags: `holzi-device-auth/v1`,
`holzi-device-list/v1`, `holzi-cell/v1`, `holzi-admission/v1`, `holzi-link/v1`. Zellsignatur über
`bereich, tabelle, pks, spalte, hlc, wert, ursprungsgerät, vault, laufnummer`; sie ergänzt die
Vorbild-Funktionen `column_sig_preimage*` von haex-crdt um Laufnummer und Vault.

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
  ihren Originalsignaturen, ohne gerätelokale Daten), der Fortschrittsstand, alle
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
Zeile für die eigene Installations-UUID hat, aber Zeilen anderer. Es löscht diese, erzeugt eigene
Schlüssel und legt über den vorhandenen `known_devices`-Ablauf eine neue `vault_device_uuid` an.

- Mit `vault_identity_secret_no_sync`: neue Geräteliste mit sich selbst als Hauptgerät, Hinweis an
  die Nutzerin.
- Ohne: signierte Aufnahmeanfrage (Name, Geräteschlüssel, `endpoint_id`), per Präsenzmeldung an
  die Vault; jedes Gerät, das sie empfängt, legt sie in der synchronisierten Tabelle
  `admission_requests` ab. Die Kopie synchronisiert nichts, bis sie auf der Geräteliste steht;
  ihre lokalen Änderungen versiegelt sie trotzdem, sie gehen nach der Aufnahme mit.

## R13 Entfernen (FR-026 bis FR-028)

**Entscheidung**: Ein Hauptgerät stellt in einer Transaktion aus: Geräteliste Generation+1 ohne
das Gerät, mit dessen Grenze (höchste lückenlose Laufnummer, die es von ihm hat), und eine neue
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
  entfernt, gegenseitiges Entfernen), Fortschritt und Lücken, Fälschungserkennung, Paket atomar,
  Momentaufnahme je Gruppe.
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

Versiegeln nach jedem Commit ist ein Scan ab Cursor über die als geändert markierten Tabellen
(`haex_crdt_dirty_tables_no_sync`); eine Zellsignatur kostet unter 0,1 ms. SC-001 (5 s) wird von
Präsenz und Verbindungsaufbau bestimmt, nicht vom Versiegeln; bei bestehender Verbindung schiebt
das Ursprungsgerät neue Pakete sofort an verbundene Geräte.

## R18 Graphify-Abfrage

Abfragen: „where are vault identity, known_devices and extension table ownership defined?“ und
„which modules handle vault open/unlock, lifecycle close, onboarding start page and tauri command
registration?“ (Budget 1.000 bzw. 1.200, abgeschnitten). Kandidaten und Ergebnis:
`identity/bootstrap.rs`, `identity/migrations.rs`, `storage/known_devices.rs`, `device/commands.rs`
werden erweitert; `instances/presence.rs` meint Prozesse, nicht Geräte, und passt nicht;
`vault_gate` liefert Abbruch-Token und Task-Tracker für den Sync-Dienst. Ein Sync-Modul gibt es
nicht; `src-tauri/src/sync/` ist neu. Der Graph ist vom 2026-09-21 und älter als 022/023; die
Kandidaten wurden im Code geprüft (zur manuellen Nachprüfung vermerkt).
