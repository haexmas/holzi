# Research: Speicherverbindungen (S3) und ihre Weitergabe an Erweiterungen

**Feature**: [spec.md](./spec.md) | **Plan**: [plan.md](./plan.md) | **Date**: 2026-10-05

Jede Entscheidung mit Begründung und verworfenen Alternativen. Pfade sind relativ zum Repository.

## R1 — S3-Client

**Entscheidung**: `rusty-s3` 0.10 (Standard-Features `rustcrypto` und `full`, ohne `aws-lc-rs`) für das
Signieren (AWS Signature V4 als vorsignierte Adressen) und das Lesen der XML-Antworten; die Übertragung
macht das vorhandene `reqwest` 0.13 mit rustls und ring.

**Begründung**: holzi hat keinen S3-Client. `rusty-s3` ist Sans-IO: Es erzeugt nur signierte Anfragen
und liest Antworten, wählt also weder TLS noch Laufzeit. So bleibt es bei einer HTTP-Schicht in holzi
(reqwest, rustls mit ring, wie in `extensions/web.rs` und den Adaptern), und Grenzen für Laufzeit und
Größe lassen sich genauso streamend durchsetzen wie in `web.rs`. Die Kryptografie kommt aus `hmac` und
`sha2` (RustCrypto, schon im Lock), die Zeit aus `jiff`, XML aus `instant-xml`. Es läuft auf Android
und iOS, weil nichts davon an eine Plattform gebunden ist.

**Verworfen**:

- `aws-sdk-s3`: eigene HTTP- und TLS-Schicht, standardmäßig aws-lc-rs (holzi lehnt aws-lc-rs ab),
  groß, eigene Laufzeitannahmen.
- `rust-s3` (haex-vault, 0.37): bringt eine eigene HTTP-Schicht mit; zwei HTTP-Stacks im Build.
- `object_store`: viel Umfang (mehrere Clouds, Multipart, Retry-Logik), der hier nicht gebraucht wird.
- Eigenes Signieren nach SigV4 über `hmac`/`sha2`: wenige hundert Zeilen, aber ein Signaturverfahren mit
  vielen Randfällen (Kodierung, kanonische Kopfzeilen); ein gepflegtes kleines Crate ist das bessere
  Risiko (Laziness Ladder: vorhandene Abhängigkeit → keine; dann kleinste neue).

## R2 — Zugangsdaten im Passwortmanager, unerreichbar für Erweiterungen

**Entscheidung**: Ein Eintrag im Passwortmanager bekommt einen **Eigentümer** (`owner`, Text, leer =
der Nutzer). Speicherverbindungen legen ihre Zugangsdaten als Eintrag mit `owner = 'storage'` an
(Titel „S3: <Name des Anbieters>“, Benutzername = Zugangsschlüssel, Passwort = Geheimnis, eigenes Feld
`sessionToken` wenn vorhanden). Neue Regel **Z14** in 034 (`contracts/access.md`): Ein Eintrag mit
Eigentümer ist für jeden Aufrufer außer `User` und `Internal { feature }` mit `feature == owner` nicht
vorhanden (`NotFound`, nicht in `list_headers`, nicht in den Agent-Kopfdaten, nicht als Quelle eines
Verweises), unabhängig von Freigaben. Die Speicherverbindungen lesen ihn als
`Caller::Internal { feature: "storage" }` mit einer im Code festen Freigabe.

**Begründung**: Die Recherche zeigt, dass 034 heute nichts vor einer Freigabe für `*` verbirgt außer dem
Papierkorb (`access::visible`, `items::item_state`). Spec 038 FR-012 und der Grundsatz des Betreibers
verlangen, dass keine Erweiterung je Zugangsdaten erreicht. Der Eigentümer sitzt am Eintrag selbst und
läuft durch denselben Pfad wie der Papierkorb (`ItemState`, `visible`, `headers_in_scope`,
`agent_headers`, Verweise über `references_db::visible`), also eine Regel an einer Stelle. Der Nutzer
sieht die Einträge weiter im Passwortmanager (Z1); ändert er dort die Zugangsdaten, gilt das für die
Verbindung. `Caller::Internal` gibt es schon für genau solche holzi-Funktionen (029 „s3-storage“), es
wird hier zum ersten Mal benutzt.

**Verworfen**:

- Ein reserviertes Tag: 034 schließt reservierte Tags ausdrücklich aus (`contracts/access.md`), und ein
  Tag lässt sich vom Nutzer entfernen; dann wären die Zugangsdaten offen.
- Eine eigene Tabelle für Zugangsdaten außerhalb von 034: widerspricht Spec 038 FR-005 und 029
  (Zugangsdaten im Passwortmanager).
- Eine eigene Tabelle `item_owners` neben `haex_passwords_item_details`: zwei Zeilen, die zusammen
  angelegt und gelöscht werden müssen; ein Sync-Zwischenstand ohne Eigentümer-Zeile würde den Eintrag
  kurz offenlegen. Die Spalte im Eintrag hat diesen Zwischenstand nicht.

## R3 — Datenmodell und Sync

**Entscheidung**: Migration `0027_storage_connections` mit zwei synchronisierten Tabellen
(`haex_storage_connections`, `haex_storages`), einer geräteeigenen (`storage_tests_no_sync` für das
letzte Testergebnis je Speicher) und der Spalte `owner` in `haex_passwords_item_details`;
`HOLZI_TRIGGER_VERSION` wird erhöht. Kennungen als UUID, keine UNIQUE-Constraints in synchronisierten
Tabellen (wie `migrations_extensions.rs`). SQL in einer eigenen Datei
`src-tauri/src/identity/migrations_storage.rs`, registriert in `migrations.rs`.

**Begründung**: Spec FR-006 (vault-weit, 029 FR-033). Das letzte Testergebnis ist eine Beobachtung eines
Geräts (ein Gerät ohne Netz soll nicht allen anderen „Fehler“ melden), daher `_no_sync`.

**Verworfen**: Verbindung und Speicher in einer Tabelle (SDK-„Backend“ = beides): 029 legt Buckets auf
einer Verbindung an, die Verbindung muss ohne Bucket bestehen können (Spec Annahme, Begriffe).

## R4 — Bereich einer Erweiterung im Bucket

**Entscheidung**: Präfix `holzi-ext/<extension_id>/`, mit `extension_id = UUIDv5(NS_EXT,
"<publicKey>:<name>")` (`extensions/ids.rs::extension_id`), auch für Entwicklerversionen (nicht deren
geräteeigene `dev_extension_id`). Schlüssel der Erweiterung sind relativ dazu.

**Begründung**: Die Kennung hängt nur an Herausgeberschlüssel und Name, ist also auf allen Geräten und
über Neuinstallationen gleich (FR-010), und mit 36 Zeichen kurz genug für die 1024-Byte-Grenze von S3.
Eine Entwicklerversion soll dieselben Objekte sehen wie die installierte Fassung derselben Erweiterung.

**Verworfen**: `TablePrefix` (`<publicKey>__<name>__`): bis über 100 Zeichen und mit dem Namen im
Schlüssel; der öffentliche Schlüssel im Bucket verrät beim Anbieter mehr als nötig.

## R5 — Schlüsselprüfung

**Entscheidung**: Ein Schlüssel einer Erweiterung ist gültig, wenn er nicht leer ist, nicht mit `/`
beginnt, keinen leeren Teil (`//`), keinen Teil `.` oder `..`, kein Steuerzeichen (U+0000–U+001F,
U+007F) und keinen Rückstrich enthält, gültiges UTF-8 ist und zusammen mit dem Präfix höchstens 1024
Bytes lang ist. Ein Präfix fürs Auflisten folgt denselben Regeln, darf aber leer sein und mit `/`
enden. Antworten des Anbieters, deren Schlüssel nicht mit dem Präfix der Erweiterung beginnen, werden
verworfen (zweite Absicherung).

**Begründung**: FR-011, SC-003. S3 kennt keine Pfade, aber Anbieter und Werkzeuge deuten `/` und `..`
unterschiedlich; ein strenger Satz Regeln vor dem Aufruf hält die Bereiche sicher getrennt.

## R6 — Dialog von holzi für Erweiterungen

**Entscheidung**: Wie `extension_dialog_confirm` (`bridge/methods.rs`, `host.open_dialog`): Der
Bridge-Aufruf meldet `extension-storage-request {requestId, frame, kind, proposal}` und wartet
(`recv_timeout`, 300 s wie `DIALOG_WAIT`); ein Vue-Dialog (`StorageDialog.vue`, im Rahmen wie
`FrameDialog.vue`) zeigt Vorschlag und Felder für Zugangsdaten und antwortet über den Command
`storage_dialog_resolve(requestId, answer)`. Die Antwort mit Zugangsdaten geht nur vom Fenster von holzi
an Rust, nie an die Erweiterung; Rust legt an, testet und gibt der Erweiterung nur Kennung oder Fehler.
Schließen des Rahmens oder Ablauf zählt als Abbruch (`drop_dialogs_of`).

**Begründung**: Ein vorhandenes, getestetes Muster für „Erweiterung wartet auf den Nutzer“; FR-013,
FR-013a. Eine native Dialogbox (wie der Speichern-Dialog) kann keine Formularfelder.

**Verworfen**: Rückfrage über den Weg der Berechtigungen (1004 und Wiederholung): Sie trägt keine
Eingaben und keine Antwort an die Erweiterung (Kennung des neuen Speichers).

## R7 — Grenzen, Auflisten, Fehler

**Entscheidung**:

- Hochladen: Base64-Daten höchstens `max_response_bytes` (wie der Anfragekörper in `web.rs`), sonst 7000.
- Herunterladen: streamend gelesen, abgebrochen bei `max_response_bytes / 4 * 3` (Base64-Aufschlag, wie
  `web.rs::max_body`), sonst 7000.
- Eine Frist (`timeout_ms`) für den ganzen Aufruf, auch beim Auflisten über mehrere Seiten.
- Auflisten folgt den Seiten des Anbieters bis `max_rows` Objekte; gibt es mehr, antwortet holzi 7000 mit
  dem Hinweis, ein engeres Präfix zu nutzen (das SDK hat keine Fortsetzung).
- Fehler beim Anbieter: fehlender Schlüssel oder Bucket 1001; Zugangsdaten abgelehnt (403, 401,
  `InvalidAccessKeyId`, `SignatureDoesNotMatch`, `ExpiredToken`) 2002 mit `details.kind =
"accessDenied"`; Netz, Zeitüberschreitung, 5xx 2002 mit `kind = "network"`; nie Endpunkt, Kopfzeilen
  oder Antworttext des Anbieters in der Meldung.

**Begründung**: FR-014, FR-015 und die Grenzwerte von Spec 017 (`sql::exec::Limits`), kein neuer
Grenzwert.

## R8 — Unverschlüsselte Endpunkte

**Entscheidung**: `https` immer; `http` nur, wenn der Host eine Loopback-Adresse, `localhost` oder eine
private Adresse (RFC 1918, RFC 4193, Link-Local) ist; der Dialog und die Einstellungen kennzeichnen ihn.
Prüfung beim Speichern und vor jedem Aufruf (die Adresse eines Namens kann sich ändern; geprüft wird die
aufgelöste Adresse wie in `web.rs`). Keine Weiterleitungen folgen (S3 antwortet mit Fehler statt
Umleitung; eine Umleitung wäre ein Fehler 2002).

**Begründung**: FR-017. Selbst betriebenes RustFS im Heimnetz ohne Zertifikat ist ein Kernfall.

## R9 — Der Verbindungstest

**Entscheidung**: Schreiben, Lesen, Auflisten und Löschen eines Testobjekts
`holzi-test/<UUID>` im Bucket; Löschen auch, wenn ein Zwischenschritt scheitert. Ergebnis
„bestanden“ oder ein Grund aus R7 (Zugangsdaten falsch, Endpunkt nicht erreichbar, Bucket fehlt, Recht
fehlt). In den Einstellungen nur nach Bestätigung gespeichert; das Ergebnis landet in
`storage_tests_no_sync`.

**Begründung**: FR-003, FR-004, SC-004. Nur Lesen würde einen Schlüssel „nur Lesen“ als geeignet melden,
mit dem eine Erweiterung dann nicht hochladen kann.

## R10 — Tests ohne Netz

**Entscheidung**: Ein Trait `RemoteStore` (R21 von Spec 017) mit der S3-Umsetzung und einer Fälschung im
Speicher für die Tests der Bridge (Rechte, Präfix, Schlüssel, Grenzen, Dialog). Die S3-Umsetzung wird
gegen `wiremock` (schon Dev-Abhängigkeit, lokaler Listener) getestet: signierte Anfrage vorhanden,
Antworten lesen, Fehlerabbildung, Grenzen. Ein echter Anbieter (RustFS im Container) nur in der
Quickstart-Anleitung und im e2e-Rig, nicht in `cargo test`.

**Begründung**: Constitution: keine Netzdienste in Tests; `wiremock` ist ein lokaler Listener wie in
`web_tests.rs` und den Adapter-Tests.

## R11 — Anpassung des vault-sdk

**Entscheidung**: Eigener PR im vault-sdk (haex-space/vault-sdk, über den haexmas-Fork):

- `AddBackendRequest`: `{ name, type: "s3", config: { endpoint?, region, bucket, pathStyle? },
sameProviderAs?: backendId }`, ohne Zugangsdaten.
- `UpdateBackendRequest`: `{ backendId, name?, config?: { bucket? } }`, ohne Zugangsdaten.
- `StorageBackendInfo`: `{ id, type, name, providerName, bucket }`.
- `S3Config` mit Zugangsdaten als veraltet markiert und entfernt im nächsten Major.

holzi lehnt einen Aufruf mit `accessKeyId`, `secretAccessKey` oder `sessionToken` ab (3001), bevor ein
Dialog erscheint (FR-013a), egal mit welcher SDK-Fassung.

**Begründung**: Clarifications 2026-10-05 (Zugangsdaten nur in holzi; die Liste nur mit Namen).
