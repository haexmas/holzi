# Research: Passwortmanager

**Spec**: [spec.md](./spec.md) | **Plan**: [plan.md](./plan.md) | **Stand**: 2026-10-02

Jede Entscheidung: **Entscheidung**, **Begründung**, **verworfene Alternativen**.
Belege sind Dateien im Repository (Stand `main` am 2026-10-02) und im Quellstand von
haex-vault (`8dce379d94e18fcd42c3b73686a06f984ca3f574`). Wo etwas aus allgemeinem
Wissen und nicht aus dem Code stammt, steht es dabei. **Nicht geprüft** wurde, ob die
neuen Crates (`keepass`, `csv`, `sha1`, `tauri-plugin-clipboard-manager`) mit den
gewünschten Funktionen bauen; das klärt Aufgabe T003 (R12, R9).

## R1 — Zwölf Tabellen in einer Migration `0022`, Triggerversion 14, SQL in eigener Datei

**Befund**: Die letzte Migration ist `0021_own_device_sync`; die nächste freie Nummer
ist `0022` (`src-tauri/src/identity/migrations.rs`). Die Datei hat bereits 577 Zeilen,
über der 500-Zeilen-Grenze der Constitution. Neue CRDT-Tabellen bekommen auf bestehenden
Vaults nur dann Trigger, wenn `HOLZI_TRIGGER_VERSION` (heute 13) steigt; jede Migration
mit neuen CRDT-Tabellen hat sie bisher erhöht (0011, 0019, 0021). Der Handshake schickt
Migrationszahl und Triggerversion; Geräte mit ungleichem Stand synchronisieren nicht
(`sync/handshake.rs:88-93`).

**Entscheidung**: Eine Migration `0022_passwords` legt alle zwölf Tabellen und ihre Indizes
an. Das SQL liegt in einer neuen Datei `identity/migrations_passwords.rs` (Konstante),
`migrations.rs` registriert sie mit einer Zeile. `HOLZI_TRIGGER_VERSION` wird auf 14
erhöht, mit einer Zeile im Doc-Kommentar der Konstante. Die Tabellen heißen wie in
haex-vault `haex_passwords_*`; haex-crdt behandelt das Präfix nicht besonders (nur
`haex_crdt_*` und `haex_app_migrations_*` sind von den Änderungsereignissen
ausgenommen, `db/connection_context.rs:53` im gepinnten Stand `aeb26eb`). Spalten und ihre
Bedeutung wie in haex-vault; die Abweichungen listet [data-model.md](./data-model.md).

**Begründung**: Eine Migration je Spec ist das Muster der Vorgänger. Der Trigger-Bump ist
zwingend, sonst wären die Tabellen auf bestehenden Vaults nicht synchron (der Test
`tests/vault_upgrade.rs` prüft das Verhalten).

**Folge**: Nach `0022` synchronisieren Geräte erst, wenn **alle** auf dem neuen Stand sind.
Das gilt für jede Schemaänderung und wird in den Hinweisen des Quickstarts genannt.

**Verworfen**:

- _Migration ohne Trigger-Bump_: neue Tabellen blieben auf bestehenden Vaults ungetrackt.
- _Mehrere Migrationen (je Gruppe von Tabellen)_: bringt nichts; eine Fehlerstelle weniger
  bei einer.
- _SQL in `migrations.rs`_: würde die Datei auf rund 700 Zeilen bringen.

## R2 — Eindeutigkeit ohne UNIQUE-Constraint: deterministische Kennungen

**Befund**: haex-crdt bricht beim Anwenden entfernter Änderungen den **ganzen Abgleich** ab,
wenn ein INSERT eine UNIQUE-, NOT-NULL- oder PK-Regel verletzt (`ApplyPolicy::on_insert_constraint`
liefert `Abort`; `sync/inbound.rs:270` übergibt keine eigene Policy). In haex-vault gibt es
UNIQUE auf `haex_passwords_tags.name`, `haex_passwords_passkeys.credential_id` und dem Paar
`(item_id, tag_id)`. Legen zwei Geräte unabhängig denselben Tagnamen an, hätten sie zwei
Zeilen mit verschiedenen Kennungen, und der nächste Abgleich bräche ab. Gleiche
Primärschlüssel dagegen sind kein Konflikt: Die Zeilen werden spaltenweise zusammengeführt.

**Entscheidung**:

1. Die UNIQUE-Constraints entfallen; an ihre Stelle treten gewöhnliche Indizes (Suche).
2. Tabellen mit einem natürlichen Schlüssel bekommen eine **abgeleitete Kennung**
   (UUIDv5, Namensraum je Tabelle, `uuid` hat `v5` bereits): `tags.id = v5(NS_TAG, fold(name))`,
   `item_tags.id = v5(NS_ITEM_TAG, itemId ":" tagId)`, `passkeys.id = v5(NS_PASSKEY, credentialId)`.
   Unabhängig angelegte gleiche Tags, Verknüpfungen oder Passkeys treffen so dieselbe Zeile
   und führen sich zusammen (US8, Szenario 5). Die Anwendung prüft die Eindeutigkeit vor dem
   Schreiben (`fold`: Unicode-Normalform NFC, Kleinschreibung, Leerraum trimmen, damit „Work“
   und „work“ ein Tag sind; die Schreibweise des ersten Anlegers bleibt sichtbar). **Warum
   Unicode-Normalisierung nötig ist**: Dasselbe sichtbare Zeichen hat oft zwei Schreibweisen,
   etwa „é“ als ein Zeichen (U+00E9) oder als „e“ plus Akzent (U+0065 U+0301); macOS und
   manche Exporte liefern die zerlegte Form. Die abgeleitete Kennung hängt an den Bytes des
   Namens: ohne NFC bekäme „Café“ je nach Quelle zwei Kennungen, und der Import würde Tags
   verdoppeln. `unicode-normalization` steht schon im `Cargo.lock` (transitiv) und wird direkte
   Abhängigkeit; das ändert den Bauumfang nicht.
3. Ein Tag bekommt beim Umbenennen **nicht** die Kennung des neuen Namens. Benennen zwei
   Geräte gleichzeitig verschieden um oder legt ein Gerät einen Namen an, den das andere
   gerade vergibt, können zwei Zeilen mit demselben Namen entstehen. Beim Öffnen der Vault
   führt `reconcile_tags` sie zusammen (kleinste Kennung gewinnt, Verknüpfungen werden
   umgehängt, die übrigen Zeilen gelöscht, alles in einem `write`).
4. `generator_presets`: genau ein Standard ist keine Datenbankregel. Bei mehreren
   Standards gilt der mit dem jüngsten `updated_at`; das nächste Speichern bereinigt.

**Begründung**: Ein angehaltener Sync wegen eines Tagnamens ist schlimmer als eine seltene
doppelte Zeile, die sich selbst heilt. Abgeleitete Kennungen machen den häufigen Fall
(gleiche Tags beim Import auf zwei Geräten) zum Nicht-Ereignis.

**Verworfen**:

- _UNIQUE behalten_: stoppt den Sync (Befund oben).
- _Eigene `ApplyPolicy` in holzi, die Konflikte auflöst_: Änderung am Sync-Kern (Spec 024),
  nicht Ziel dieser Spec; wäre die richtigere Dauerlösung (siehe Bewusste Grenzen).
- _Zufällige Kennungen plus nachträgliches Zusammenführen für alles_: macht den
  Normalfall zum Sonderfall.

**ponytail**: Das Zusammenführen läuft nur beim Öffnen. Bis zum nächsten Öffnen kann ein
doppeltes Tag sichtbar sein (die Oberfläche fasst gleiche Namen in der Anzeige zusammen).
Aufrüstweg: eigene `ApplyPolicy` im Sync.

## R3 — Papierkorb: besonderer Ordner, Herkunft wird gemerkt, Kinder zuerst löschen

**Befund**: haex-vault führt den Papierkorb als Ordner mit der festen Kennung `trash`,
legt ihn lazy an (Fehler beim Einfügen werden verschluckt, der Name ist fest deutsch) und
**vergisst beim Wiederherstellen den früheren Ort** (`stores/passwords/groups.ts`:
Wiederherstellen setzt die Gruppe auf `NULL`). Die Spec verlangt den früheren Ort (FR-016).
haex-crdt wendet entfernte Löschungen mit ausgeschalteten Fremdschlüsseln an; ein
Kaskadieren findet auf dem Peer **nicht** statt. Ein lokales Kaskadieren löst die
Löschtrigger der Kinder wahrscheinlich aus, ist aber in keinem Test belegt (Bericht der
Backend-Recherche, Abschnitt 4).

**Entscheidung**:

- Die Papierkorb-Zeile `id = 'trash'` legt Rust bei Bedarf in demselben `write` an, das etwas
  in den Papierkorb verschiebt (`INSERT` nur wenn sie fehlt; Fehler werden weitergegeben).
  `name` bleibt `NULL`; die Oberfläche zeigt für die Kennung `trash` den übersetzten Namen.
  Zwei Geräte, die sie unabhängig anlegen, treffen denselben Primärschlüssel und führen
  zusammen.
- Zwei **zusätzliche, nullbare Spalten** merken den früheren Ort des direkt gelöschten
  Elements: `group_items.trashed_from_group_id` und `groups.trashed_from_parent_id`
  (kein Fremdschlüssel, weil der Ordner inzwischen fehlen kann). Wiederherstellen setzt den
  Ort zurück, falls der Ordner noch existiert und nicht selbst im Papierkorb liegt, sonst
  an die Wurzel (US4, Szenario 2). Kinder eines gelöschten Ordners behalten ihre Struktur
  im Papierkorb und tragen keine Herkunft.
- **Endgültiges Löschen** löscht Kinder ausdrücklich und zuerst (Passkeys, Schlüssel-Wert-Paare,
  Tag-Verknüpfungen, Anhang-Verknüpfungen, Verlaufsstände samt `snapshot_binaries`,
  `group_items`, dann `item_details`; bei Ordnern rekursiv von unten nach oben), in **einem**
  `write`. Der Fremdschlüssel `ON DELETE CASCADE` bleibt als Sicherheitsnetz, die
  Anwendung verlässt sich nicht darauf. Ein Zwei-Geräte-Test belegt, dass jede Zeile ihren
  eigenen Löschmarker hat und beim Peer ankommt.
- Ein Ordner, der im Papierkorb liegt, ist nicht als Ziel für Verschieben erlaubt; das
  Zyklus-Verbot (FR-010) prüft Rust bei jedem Verschieben (haex-vault prüft nur beim
  Mehrfachverschieben).
- Ein Eintrag oder Ordner, dessen Ordner bei einem Peer gelöscht wurde, hat einen hängenden
  Verweis. Abfragen behandeln unbekannte Ordner wie die Wurzel (`LEFT JOIN`).

**Begründung**: Die zwei Spalten sind die kleinste Abweichung, die FR-016 erfüllt, und sie
sind additiv (haex-vault-Abfragen laufen weiter). Explizites Löschen macht das Verhalten
unabhängig von einer ungeprüften Annahme über die Kaskade.

**Verworfen**:

- _Herkunft im Verlaufsstand ablegen_: vermischt Verlauf und Papierkorb, Gruppen haben keinen.
- _Spec ändern: Wiederherstellen immer an der Wurzel_: bessere Nutzererfahrung wiegt hier
  schwerer als eine Spalte.
- _Auf die FK-Kaskade verlassen_: unbelegt (siehe Befund).

## R4 — Anhänge: BLOB, 25 MiB, Pfad statt Bytes, Aufräumen mit Karenzzeit

**Befund**:

- Die Obergrenze für eine Schreibtransaktion ist `MAX_CRDT_TRANSACTION_BYTES = 100 MiB`
  (Summe der Parametergrößen); holzi übergibt sie unverändert (`instances/vault_config.rs:36`).
- Eine Zelle von 25 MiB ist synchronisierbar: haex-crdt kodiert BLOBs als
  `{"$blob_hex": …}` (zweifache Größe, rund 50 MiB), `Change::split` zerlegt sie in
  Teile je 4 MiB (`sync/change.rs`, `PAGE_BUDGET`), der Empfänger puffert bis
  `2 × Limit + 1 MiB` und prüft die roh gerechnete Gruppengröße gegen 100 MiB
  (`sync/inbound.rs`). Es gibt kein Zeitlimit je Seite.
- `serve_pull` materialisiert alle passenden Zellen im Speicher (zweifach); `Database::read/write`
  laufen auf einer gesperrten Verbindung und große Abfragen werden vollständig geladen.
- Beim Aufräumen entsteht ein **Wettlauf**: Gerät A sieht eine Binärzeile ohne Verweis,
  weil der Verweis von Gerät B noch nicht eingetroffen ist, löscht sie, und der Löschmarker
  wandert zu B und löscht dort die Datei, die B braucht. haex-vault räumt beim Öffnen
  ohne Rücksicht darauf auf.
- Es gibt keinen Zugriff aus dem Webview auf das Dateisystem: `plugin-fs` ist deklariert,
  aber nicht registriert; `capabilities/default.json` erlaubt nur `dialog:allow-open`.

**Entscheidung**:

- Spalte `haex_passwords_binaries.data` ist `BLOB`, Größe und Hash wie in haex-vault; der Hash
  ist der SHA-256 der Rohdaten (nicht der Base64-Zeichen).
- Das Limit von **25 MiB** prüft Rust über `metadata().len()` **bevor** es liest, und noch
  einmal an der gelesenen Länge. Fehler: `PasswordsAttachmentTooLarge { bytes, limit }`.
- Dateien kommen und gehen **über Pfade**, nicht über den Webview: `passwords_attachment_add`
  bekommt einen Pfad aus dem Dateidialog, `passwords_attachment_save` schreibt an einen
  Pfad aus dem Speichern-Dialog (neue Capability `dialog:allow-save`). So laufen keine
  25 MiB als Base64 durch IPC. Nur Vorschaubilder gehen als Rohbytes zurück
  (`tauri::ipc::Response`).
- Jeder Anhang wird in einem **eigenen** `write` geschrieben (Summe je Transaktion unter
  100 MiB); BLOB-Spalten werden nur in eigenen Abfragen gelesen, nie in Listen.
- Die Fremdschlüssel von `item_binaries.binary_hash` und `snapshot_binaries.binary_hash`
  auf `binaries.hash` sind `ON DELETE RESTRICT` (haex-vault: `CASCADE`). Lokal kann so keine
  Binärzeile gelöscht werden, die noch einer braucht.
- **Aufräumen** (`prune_binaries`) löscht nur Binärzeilen ohne Verweis, die älter als **sieben
  Tage** sind (`created_at`); es läuft einmal beim Öffnen der Vault (Hook neben
  `vault_events::start_for_active_instance`, `instances/open.rs:143`), nicht nach jedem
  Löschen. Endgültig gelöschte Anhänge belegen also noch bis zu sieben Tage Platz.
- Importierte Anhänge über dem Limit werden übersprungen und im Bericht genannt.

**Begründung**: Pfad-basierte Übergabe vermeidet den teuersten Fehler (Speicher und IPC);
die Karenzzeit schließt den Wettlauf, ohne einen Sync-Eingriff zu verlangen.

**Verworfen**:

- _Binärdaten über IPC als Base64_: 33 % mehr, vollständig im Webview, 25 MiB je Aufruf.
- _Sofortiges Aufräumen_: Datenverlust im Wettlauf (Befund).
- _`plugin-fs` im Frontend_: zusätzliche Plugin-Registrierung und breite Rechte.
- _Zeilenweise Aufteilung großer Dateien in Blöcke (Tabelle `…_chunks`)_: sauberer für den
  Sync, aber ein anderes Datenmodell als haex-vault.

**ponytail**: Es gibt kein Gesamtbudget für Anhänge; ein erster Vollabruf lädt alle
Binärzellen auf einmal in den Speicher (zwei Kopien je Zelle). Aufrüstweg: gestaffeltes
Scannen in haex-crdt (Spec 024). Die Pfad-Übergabe setzt einen Dateisystempfad voraus;
auf Mobilgeräten (Content-URIs) braucht es später `plugin-fs` oder einen Stream.

## R5 — Verlauf: Stand nach der Änderung, ein Format, Wiederherstellen wird neu gebaut

**Befund**: haex-vault speichert nach **jedem** Speichern einen Stand des **neuen**
Zustands (auch beim Anlegen), ohne Obergrenze und ohne „unverändert überspringen“. Es gibt
zwei Formate des Standes (die Bridge schreibt `tags: null` und die OTP-Felder; die
Oberfläche liest `tagNames`). Die Oberfläche kann den Verlauf nur ansehen; ein
Wiederherstellen gibt es nicht.

**Entscheidung**:

- Ein Stand entsteht **nach** jeder Änderung des Eintrags (Speichern, Anhang hinzufügen,
  umbenennen, entfernen, Wiederherstellen) und enthält den neuen Zustand. Der „vorherige
  Stand“ der Spec ist der Stand davor. Die Spec wird entsprechend präzisiert (R20).
- **Ein** Format, `SnapshotData` mit `version: 1`, ein Obermengen-Format: Titel, Benutzername,
  Passwort, Adresse, Notiz, Symbol, Farbe, Ablaufdatum, `otpSecret`, `otpDigits`, `otpPeriod`,
  `otpAlgorithm`, `autofillAliases`, `tagNames`, `keyValues`, `attachments {fileName, binaryHash}`.
  Der Leser toleriert fehlende Felder (Stände aus haex-vault).
- Ein Stand entsteht nicht, wenn er dem jüngsten gleicht.
- **Wiederherstellen** ist ein Speichern mit den Werten des Standes (Tags werden per Name
  gesucht oder angelegt, Anhänge per Hash und Dateiname verknüpft, fehlt die Binärzeile,
  wird der Anhang ausgelassen und gemeldet); danach entsteht, wie bei jeder Änderung, ein neuer
  Stand. Der Ordner ändert sich nicht.
- Die Liste zeigt je Stand nur, **welche Felder** sich gegenüber dem Vorgänger geändert haben;
  Werte holt `passwords_history_reveal` einzeln (FR-005).

**Verworfen**: _Stand vor der Änderung speichern_ (haex-vault-Daten wären nicht lesbar
gleich), _Obergrenze der Stände_ (ohne Messung unnötig, siehe Annahmen der Spec).

## R6 — Zugriff: Aufrufer, Freigabe und Bereich als Rust-Modul; der Aufrufer ergibt sich aus dem Eingang

**Befund**: Es gibt in Rust weder ein Aufrufer- noch ein Freigabe-Konzept; ein Tauri-Command
sieht nur `State` und `AppHandle` (Bericht Abschnitt 9). Der Aktionsläufer im Frontend kennt
`ActionCaller` (`user | builtinAgent | externalAgent`), aber keine Freigaben (Bericht
Frontend, Abschnitt 3). haex-vault prüft Tag-Bereiche in `check/passwords.rs` mit den Regeln:
Vereinigung der Tags, `*` deckt alles, `ReadWrite` deckt `Read`, Außerhalb des Bereichs ist
nicht von „nicht vorhanden“ zu unterscheiden, beim Anlegen und Ändern muss ein Tag im
Bereich bleiben.

**Entscheidung**: Neues Modul `passwords/access.rs` mit den reinen Typen `Caller`
(`User`, `BuiltinAgent`, `Extension{id}`, `ExternalAgent{id}`, `Internal{feature}`),
`Grant {action, scope}`, `Scope {All | Tags(…)}` und den Funktionen `authorize_list`,
`authorize_read`, `authorize_write`, `authorize_delete`. Ein `PasswordsService` bündelt
Zugriffsprüfung und Speicherfunktionen und ist die **einzige** Schnittstelle für alles außer der
Oberfläche (FR-024). Wichtig:

- Der Aufrufer ergibt sich aus dem **Eingang**, nie aus einem Parameter des Frontends: Die
  Commands der Oberfläche rufen den Dienst mit `Caller::User`; der Command für den
  eingebauten Agenten (`passwords_agent_search`) ruft ihn fest mit `Caller::BuiltinAgent`;
  holzi-Funktionen rufen ihn aus Rust mit `Caller::Internal{…}` und fest einkompilierter
  Freigabe. Erweiterungen und externe Agenten bekommen mit 017–019 und 021 eigene Eingänge.
- Rechte: `User` darf alles ohne Freigabe (FR-031). `BuiltinAgent` bekommt nie Geheimnisse
  und nie eine Freigabe (FR-027): nur `list_headers_for_agent` ist erlaubt. Alle anderen
  Aufrufer folgen den Regeln aus FR-025 bis FR-029.
- Fehler: eine fehlende **Art** (Lesen gegeben, Schreiben verlangt) ist `Forbidden`; ein
  Eintrag **außerhalb des Bereichs** ist `NotFound` wie ein nicht vorhandener (FR-029).
- Beim Schreiben im Bereich `Tags` darf die gesendete Tagliste nur Tags des Bereichs enthalten
  und muss mindestens eines enthalten (Anlegen); beim Ändern muss der Zieleintrag bereits im
  Bereich liegen und danach mindestens ein Tag des Bereichs tragen. **Tags außerhalb des Bereichs
  bleiben unverändert** (Klärung): der Aufrufer sieht sie, kann sie aber weder entfernen noch
  hinzufügen noch umbenennen oder löschen; ein neues Tag außerhalb des Bereichs in der
  gesendeten Liste ist `Forbidden`, damit er einen Eintrag nicht in den Bereich eines anderen
  Aufrufers hebt. haex-vault ersetzt dagegen die ganze Tagliste und entfernt unsichtbare Tags
  still. Es gibt **keine** automatische Standard-Tag-Ergänzung wie in haex-vault.
- **Löschen** durch andere Aufrufer als `User` verschiebt in den Papierkorb (FR-015); endgültiges
  Löschen, Wiederherstellen und Papierkorb leeren sind `User` vorbehalten (Z8, Z11).
- **Nutzungsmeldung statt reservierter Tags** (Klärung): eine holzi-Funktion meldet über das Trait
  `EntryUsage`, welche Einträge sie nutzt; der Passwortmanager erkennt das nicht an Tags oder Namen.
- Passkeys ohne Eintrag (`item_id` leer) liegen außerhalb jedes Tag-Bereichs.
- **Alles läuft über den Dienst**, auch die Oberfläche (Betreiber-Entscheidung zur Analyse):
  `PasswordsService` hat für jede Funktion der Oberfläche eine Methode (Ordner, Tags, Papierkorb,
  Verlauf, Anhänge, Passkeys, Voreinstellungen, Import). Jede Methode prüft den Aufrufer zuerst;
  für alles außer Eintrag lesen, anlegen, ändern und löschen gilt **Z11**: andere Aufrufer als
  `User` sind `Forbidden`, bis eine spätere Spec für sie eine Regel schreibt. So gibt es einen
  einzigen Eingang und keine zweite Schicht, an der die Prüfung vorbeiliefe.

**Der eingebaute Agent**: Kopfdaten für ihn sind enger als FR-026: nur Kennung, Titel, Tags,
Ordnername, `hasTotp`; ohne Benutzername und Adresse, damit auch ohne Geheimnisse nicht
die Liste der Konten an einen Cloud-Anbieter geht (R14, R20).

**Verworfen**:

- _Aufrufer als Parameter des Commands_: ein Webview-Fehler oder ein untergeschobener
  Aufruf könnte `User` behaupten.
- _Freigaben jetzt schon speichern_: gehört in 017–019 und 021 (FR-030); eine Tabelle
  ohne Verwalter wäre totes Datenmodell.
- _Alles im Frontend prüfen_: Rust ist die Instanz, an der Tabellen liegen; FR-024 verlangt
  die Prüfung dort.

## R7 — Geheimnisse bleiben im Backend, bis der Nutzer sie verlangt

**Entscheidung**: Der Standardzugriff der Oberfläche liefert **keine** Geheimnisse:

- `passwords_get_item` liefert alle Felder außer Passwort, TOTP-Secret, Passkey-Schlüsseln und
  den **Werten** eigener Felder (statt ihrer `hasPassword`, `hasOtpSecret`, `keyValues[].hasValue`).
  Die Notiz ist kein Geheimnis im Sinne von FR-005 und kommt mit.
- `passwords_reveal` holt ein einzelnes Geheimnis auf bewusste Handlung (Auge, Halten).
- `passwords_copy_field` kopiert in Rust: der Wert läuft nie durch den Webview (R9).
- `passwords_totp_code` berechnet in Rust (R8); der Webview sieht nur Code und Restzeit.
- Speichern ist ein **Teil-Update**: ein nicht übermitteltes Geheimnis bleibt unverändert, ein
  übermitteltes ersetzt, ein ausdrücklich geleertes löscht. Der Editor zeigt ein Passwort
  als „••••“ mit Knöpfen „Anzeigen“, „Ersetzen“ und „Generieren“.
- Typen mit Geheimnissen implementieren `Debug` mit Schwärzung (`<redacted>`), Fehler tragen
  nie einen Wert (FR-040); die Zeichenketten aus der Datenbank werden nach Gebrauch mit
  `zeroize` überschrieben, wo sie lokal gehalten werden.

**Begründung**: Das Fenster des Passwortmanagers ist ein Webview; was dort im Speicher
liegt, liegt auch in Heap-Dumps, in der Verlaufsliste der Entwicklerwerkzeuge und in
fehlerhaft geloggten Zuständen. Die Kosten (Teil-Update, Reveal-Command) sind klein.

**Verworfen**: _Alle Felder im Klartext laden wie haex-vault_ (der Store dort lädt sogar
alle Passwörter aller Einträge in den Speicher).

## R8 — TOTP in Rust, ohne neue Bibliothek außer SHA-1

**Befund**: `otpauth` im Frontend (haex-vault) rechnet im Webview. In holzi gibt es weder
`totp-rs` noch `base32`; `hmac` 0.13 und `sha2` 0.11 sind direkte Abhängigkeiten, `sha1` nur
transitiv.

**Entscheidung**: `passwords/totp.rs`: RFC 6238 über `hmac` mit SHA-1, SHA-256, SHA-512; die
Base32-Dekodierung (RFC 4648, ohne Auffüllung, Groß-/Kleinschreibung egal, Leerraum
ignoriert) wird selbst geschrieben (rund 30 Zeilen, mit den Testvektoren der RFC). `sha1`
kommt als direkte Abhängigkeit hinzu (RustCrypto, gleiche Familie wie `sha2`). Eingabe wird
geprüft: Ziffern 6–10, Periode 1–300 s, Algorithmus aus der Liste, Secret dekodierbar;
sonst `InvalidInput` mit Feldnamen. `parse_otp_input` nimmt `otpauth://totp/…` oder ein
nacktes Secret (Verhalten von `parseOtpData` aus haex-vault, plus die Prüfung, die dort fehlt).

**Defaults und ungültige Daten**: Beim Anlegen und Ändern wird ein ungültiger Wert abgelehnt
(`InvalidInput` mit Feldname) und ein fehlender bekommt den Standard (6, 30, `SHA1`). Gelesen
werden `NULL`-Werte ebenfalls als Standard. Per Sync oder Import kann trotzdem ein ungültiger
Wert in die Tabelle kommen (anderes Gerät, andere Programmversion); darum meldet `get_item`
einen Zustand `otpState` (`none`, `valid`, `invalid`), `passwords_totp_code` liefert für
`invalid` einen `InvalidInput`, und die Oberfläche zeigt am Eintrag eine Meldung mit „Ersetzen“
und „Entfernen“, ohne etwas still zu ändern. Beim Import wird ein ungültiges Secret nicht
übernommen: der Eintrag entsteht ohne TOTP und steht im Bericht als „mit Verlust“.

**Verworfen**: _`totp-rs`_ (zusätzliche Abhängigkeiten, die QR- und Serde-Funktionen
nicht gebraucht werden), _`otpauth` im Frontend_ (Secret im Webview, R7).

## R9 — Zwischenablage in Rust, mit eigenem Löschen

**Befund**: Es gibt kein Zwischenablage-Plugin; das Frontend nutzt `navigator.clipboard.writeText`
ohne Löschen. Ob `navigator.clipboard.readText` im WebKitGTK ohne Rückfrage geht, ist offen.

**Entscheidung**: `tauri-plugin-clipboard-manager` (offiziell, Rust-API über
`ClipboardExt`) wird **nur in Rust** benutzt: `passwords_copy_field` liest den Wert in Rust,
schreibt ihn in die Zwischenablage und startet eine abbrechbare Aufgabe, die nach der
Einstellung `passwords.clipboard_clear_seconds` (Vault-Einstellung; Auswahl Aus, 15, 30
(Standard), 60, 120) den Inhalt löscht, **wenn er noch dem kopierten Wert gleicht**. Eine neue
Kopie bricht die laufende Aufgabe ab; das Schließen der Vault löscht sofort, falls etwas
aussteht (der Prozess endet mit der Vault, Spec 013). Es braucht keine JS-Berechtigung für die
Zwischenablage.

**Verworfen**: _`navigator.clipboard` im Webview_ (Wert im Webview, Lesen ungeklärt),
_`arboard` direkt_ (Plugin bringt Mobil-Unterstützung und Capability-Modell).

**ponytail**: Beendet sich holzi ohne Vault-Schließen (Absturz), bleibt der Wert liegen.

## R10 — Generator im Frontend, mit gleichmäßiger Auswahl und Klassengarantie

**Befund**: Die Logik in haex-vault (74 Zeilen) hat Modulo-Verzerrung (`Uint32 % n`), keine
Garantie je Zeichenklasse, keine Längenobergrenze und eine Musterschreibweise ohne
Maskierung. Tests (323 Zeilen) sind auf Vitest und eine Mock-Zufallsquelle gebaut.

**Entscheidung**: `src/lib/passwords/generator.ts` (reines TS, `crypto.getRandomValues`),
neu geschrieben statt kopiert:

- Auswahl per **Rejection Sampling** (keine Verzerrung).
- Jede gewählte Klasse kommt mindestens einmal vor (FR-013); Länge kleiner als die Zahl der
  Klassen oder leere Auswahl ergibt `{ error: 'length_below_classes' | 'no_class' | … }`
  statt eines leeren Passworts (FR-014). Länge 1–256.
- Mustermodus mit der Schreibweise aus haex-vault (`c C v V d a A s`, andere Zeichen
  buchstäblich) plus `\` zur Maskierung; `excludeChars` gilt wie dort nicht im Mustermodus.
- Mischung der garantierten Zeichen per Fisher-Yates mit derselben Zufallsquelle.
- Die Prüfungen laufen mit `node --test` und einer deterministischen Zufallsquelle, die der
  Generator als Parameter nimmt (Muster der Vorgänger-Skripte).

**Begründung**: Das Passwort liegt nur im Webview, wenn der Nutzer es erzeugt; Rust braucht
es nicht. Reines TS passt zum Prüfmuster (`scripts/check-*.ts`). Rejection Sampling ist eine
bekannte, kleine Korrektur.

**Verworfen**: _Generator in Rust_ (zusätzliche Command-Fläche, Tests nicht im vorhandenen
Muster), _haex-vault-Code 1:1_ (Verzerrung, keine Garantien).

## R11 — Suche im Frontend über die Kopfdaten

**Entscheidung**: Der Store hält die Kopfdaten (ohne Geheimnisse) und filtert im Speicher
(`src/lib/passwords/search.ts`: `fold` wie `lib/settings/search.ts`, Wort-Teiltreffer über
Titel, Benutzername, Adresse, Tag-Namen; Notizen und Geheimnisse nie). Der Ort ist ein
Abfrageparameter des Tab-Ortes (`?q=`), sodass Vor und Zurück die Suche wiederherstellen; der
Suchtext ist kein Geheimnis. 5.000 Einträge je rund 300 Byte sind 1,5 MB.

**Verworfen**: _Suche in SQLite_ (Geheimnis-Spalten müssten ausgeschlossen werden, der
Vorteil fehlt bei dieser Größe), _`fuse.js`_ (vorhanden, aber Unschärfe passt nicht zu
einem Namensfilter; `fold` genügt und ist testbar).

## R12 — Import in Rust: ein Einlesen, eine Transaktion, Vorschau vorab

**Befund**: haex-vault importiert in der Oberfläche, schreibt Zeile für Zeile ohne
Transaktion (ein Abbruch hinterlässt Halbes), erkennt bei Bitwarden und LastPass keine
Doppelten und dupliziert beim KeePass-Wiederholen die Kindzeilen. Die CSV-Zerlegung ist
zeilenweise und bricht bei Zeilenumbrüchen in Notizen. KeePass nutzt `kdbxweb` mit einem
Argon2-Shim aus `hash-wasm`; Schlüsseldateien werden nicht unterstützt. Im Frontend fehlen
`kdbxweb`, Argon2 und ein CSV-Parser; in Rust fehlen `keepass`, `argon2` und (ohne `llm-cpu`)
`csv`.

**Entscheidung**: Der Import liegt in Rust (`passwords/import/`). Reine Funktionen
`bytes → ImportModel` (je Format), danach `apply_import` in **einem** `write`:

1. `passwords_import_preview` liest die Datei, gibt Zahl der Einträge, Ordner, Anhänge,
   Doppelten und Hinweise zurück und schreibt nichts.
2. `passwords_import_run` liest erneut und schreibt alles oder nichts. Gesamtgröße über
   100 MiB (Obergrenze der Transaktion) wird **vor** dem Schreiben mit
   `PasswordsImportTooLarge` abgelehnt; einzelne Anhänge über 25 MiB werden übersprungen und
   gemeldet.
3. **Formate**: KeePass-`kdbx` (Passwort, optional Schlüsseldatei) über das Crate `keepass`;
   Bitwarden JSON (unverschlüsselt) und CSV; LastPass CSV, beide mit dem Crate `csv`
   (RFC 4180, Zeilenumbrüche in Feldern). Zuordnungen wie in haex-vault, soweit sie dort
   funktionieren: Ordner (LastPass: `grouping` an `/` teilen), Tags (Bitwarden-Typen werden
   Tags `secure-note`, `credit-card`, `identity`), eigene Felder, TOTP (vorher geprüft, R8),
   Anhänge (KeePass) und **Passkeys** (Bitwarden `fido2Credentials`, KeePassXC-Attribute). Die
   Quellen liefern nur den privaten Schlüssel (PKCS8); die Tabelle verlangt auch den öffentlichen
   (SPKI, `NOT NULL`). Er wird für **ES256 (P-256)** mit dem Crate `p256` (Features `pkcs8`,
   `pem`; `pkcs8`, `spki` und `pem-rfc7468` stehen schon im `Cargo.lock`) abgeleitet. Passkeys
   mit anderem Algorithmus oder unlesbarem Schlüssel stehen als „nicht übernommen“ im Bericht.
   Schlüsselpaare für Tests entstehen zur Laufzeit im Test, nie als Datei im Repository
   (Constitution I). Die Kodierungen der Felder (Base64 oder Base64url bei Credential-ID und
   Benutzerkennung, UUID-Form der Bitwarden-Credential-ID) und die Kodierung in haex-vault prüft
   Aufgabe T003 anhand des Codes dort und eigener Beispieldaten, damit gespeicherte Passkeys
   zwischen beiden Produkten lesbar bleiben. **Warum nicht wie in haex-vault**: haex-vault
   importiert keine Passkeys; es erzeugt sie selbst (`generatePasskeyPairAsync`, ES256) und hat
   deshalb beide Schlüssel zur Hand. Der Fall „nur der private Schlüssel ist bekannt“ kommt dort
   nicht vor. Der KeePass-Papierkorb wird nicht übernommen (Annahme der
   Spec). Symbole und Verlauf der Quelle werden im ersten Wurf nicht importiert.
4. Kennungen sind neu (v4); Doppelte erkennt `(title, username, url)` gegen vorhandene
   Einträge **außerhalb** des Papierkorbs; `onDuplicate: skip | create` entscheidet die
   Oberfläche nach der Vorschau (FR-023).
5. Passwort und Schlüsseldatei-Pfad laufen als `Passphrase`-artiger Typ (zeroizing, `Debug`
   geschwärzt) durch den Command.

**Ungeprüft und deshalb Aufgabe T003**: ob `keepass` KDBX 3 und 4, Argon2-Varianten,
Schlüsseldateien, Anhänge und Eigenschaften der Einträge in der benötigten Form liefert und in
beiden Feature-Konfigurationen baut. Fällt die Prüfung negativ aus, ist die zweite Wahl der
Weg von haex-vault (`kdbxweb` plus `hash-wasm` im Frontend, Parser liefert `ImportModel` als
JSON an einen Rust-Command `apply_import`); die übrigen Teile des Plans ändern sich dann
nicht.

**Verworfen**: _Import im Frontend wie haex-vault_ (kein Alles-oder-nichts, Passwörter und
Anhänge durchlaufen den Webview, doppelte Parser für Test und Betrieb), _Import je Format als
eigene App_ (ein Assistent mit drei Quellen genügt).

## R13 — Oberfläche: eine App, Orte im Tab, kein Geheimnis in Verlauf oder Titel

**Befund** (Frontend-Bericht): `AppDefinition` in `lib/wm/apps.ts` (id, titleKey, icon,
Größen, `multiInstance`, `tabTitle`); die Komponente hängt in `components/wm/appRoutes.ts`;
Orte sind `RoutePattern` mit `:param`. Die Sitzungssicherung speichert je Tab den **ganzen
Verlauf** (Pfad, Abfrage, Titel); der Titel eines verlassenen Ortes kommt aus
`useWmTab().setTitle(...)` (wie im Chat) und steht in der Verlaufsliste. Symbole müssen als
Literal in `.vue`/`.ts` stehen, sonst bündelt `@nuxt/icon` sie nicht (`nuxt.config.ts:64-73`).

**Entscheidung**:

- App `system.passwords` mit `icon: 'lucide:key-round'`, 960×640, Mindestgröße 360 breit,
  **`multiInstance: true`** (mehrere Fenster zum Vergleichen; die Spec fordert dann das
  Verhalten nach Spec 030), `tabTitle: 'app'` (fester Name).
- Orte: `/` (Wurzel), `/folder/:id`, `/entry/:id`, `/entry/:id/history`, `/trash`,
  `/generator`, `/import`. Nur Kennungen als Parameter, Abfrage `?q=` und `?tag=` (Suchtext
  und Tag-Kennung). **Nie** ein Titel, Name oder Wert in Pfad, Abfrage oder
  `setTitle`; Titel der Orte kommen aus festen `titleKey`s. Ein Prüfskript sichert das gegen
  `snapshotSession` und die Routentabelle.
- Rahmen (Werkzeugleiste, ausblendbare Seitenleiste, Überlagerung unter 672 px): vorhandene
  Bausteine. Die Seitenleiste von Einstellungen ist an `SETTINGS_CATEGORIES` gebunden;
  `components/chat/SidebarLayout.vue` ist der kleinste allgemeine Rahmen. Aufgabe T002
  befragt den Graph und entscheidet zwischen **dort anknüpfen** und **gemeinsamen Rahmen
  herausziehen**; ein Umbau des Chats ist nicht Teil dieser Spec und wird bei Zweifel mit dem
  Betreiber geklärt (Constitution: graphify-first).
- Wiederverwendet werden `SettingsGroup`, `SettingsRow`, `SettingsOptionRow`, `SettingsSelect`,
  `WmRouterView`, `WmLink`, `useTabRouter`, `useWmTab`, `onVaultTablesChanged(['haex_passwords_*'])`.
  Neu sind: Baum und Liste, Editor mit Abschnitten, Generator-Fläche, Import-Assistent,
  Papierkorb-Ansicht, Dialoge. Farben nur aus Theme-Token (`check:templates`).
- Aufgedeckte Werte und Timer leben in lokalen Refs der Komponente, enden beim Verlassen des
  Eintrags und gehen nie in den Store.

**Verworfen**: _Singleton wie Einstellungen_ (Vergleich zweier Einträge ist ein Hauptfall;
Singleton verdeckt die Konfliktfälle der Spec nicht, sondern vermeidet sie nur), _eigene
Seitenleiste kopieren_ (zweite Kopie neben zwei vorhandenen).

## R14 — Aktionen: nur eine lesende Suche für den eingebauten Agenten

**Befund**: Der Aktionsläufer kennt Aufruferart und die Felder `agentCallable`,
`builtinAgentCallable`, aber keine Freigaben. `check-agent-actions.ts:215-231` lässt für
agentenfähige Aktionen **keine** Eingabe- oder Ergebnisfelder zu, deren Namen auf
`/secret|private|password|passphrase|token|apiKey|credential/i` passen; es prüft außerdem den
Schnappschuss `chat/eval/tools.json` und führt jede lesende, für den eingebauten Agenten
aufrufbare Aktion mit Beispieleingaben aus.

**Entscheidung**: Der Katalog bekommt eine Aktion `passwords.items.search` (Bereich
`passwords.read`, Wirkung `read`, `builtinAgentCallable` Standard) mit Eingabe `{ query?, tag?, limit? }`
und Ergebnis `{ items: [{ id, title, tags, folder, hasTotp }] }`. Der Handler ruft
`passwords_agent_search` (R6). Es gibt **keine** Aktion, die ein Geheimnis liest, kopiert oder
ändert; solche kommen mit den Eingängen von 017–019 und 021. Der Agent öffnet einen Eintrag
über die vorhandene Aktion `wm.tab.navigate` (Pfad `/entry/<id>`). `scopes.ts` bekommt
`passwords.read`; `export:eval-tools` erzeugt den Schnappschuss neu (CI vergleicht).

**Begründung**: Das minimale Angebot erfüllt FR-027 ohne Ausnahme und lässt die
Freigabe-Verwaltung bei den Specs, die sie bauen.

**Verworfen**: _Aktionen für Anlegen/Ändern für den eingebauten Agenten_ (Passwörter in
Modell-Eingaben, Spec 032 FR-010), _Ausnahme in der Namensprüfung_ (das Skript ist die
Absicherung, nicht die Hürde).

## R15 — Live-Aktualisierung und Konflikte beim Speichern

**Entscheidung**: Der Store lädt Übersicht und Kopfdaten über `onVaultTablesChanged(['haex_passwords_*'], reload)`
(leises Neuladen, FR-038). Der Editor arbeitet auf einem Entwurf; das Neuladen überschreibt
ihn nie. `passwords_update_item` bekommt `expectedUpdatedAt` (Zeichenkette mit
Millisekunden, von Rust beim Schreiben gesetzt, nicht `CURRENT_TIMESTAMP`); weicht der
Stand ab oder fehlt der Eintrag, kommt `PasswordsConflict { reason: 'changed' | 'deleted' }`
und der Editor bietet „Meine Änderungen behalten (neu anlegen)“ oder „Fremde Änderung
übernehmen“ an. Zwei Fenster teilen den Store und damit Daten, nicht Entwürfe.

**Verworfen**: _Spaltenweise Zusammenführung im Editor_ (ein Dialog mit Feldvergleich ist
Umfang für später), _blindes Überschreiben_ (Spec Edge Case).

**ponytail**: Der Vergleich ist auf Millisekunden genau; zwei Schreibvorgänge in derselben
Millisekunde würden nicht erkannt. Aufrüstweg: HLC der Zeile als Token.

## R16 — Tests und CI

**Entscheidung**:

- Rust-Einheiten in `*_tests.rs` neben den Modulen: `access_tests.rs` (Matrix aus FR-025 bis
  FR-029), `totp_tests.rs` (Testvektoren RFC 6238, Sonderfälle), `trash_tests.rs`,
  `snapshots_tests.rs`, `import/*_tests.rs` (Beispieldateien in `tests/fixtures/passwords/`,
  ohne echte Geheimnisse), `binaries_tests.rs`, `tags_tests.rs` (Kennungen, Zusammenführen).
- Integration unter `src-tauri/tests/`: `passwords_roundtrip.rs` (CRDT-Metadaten, Löschmarker
  je Zeile, Vorlage `preferences_roundtrip.rs`), `passwords_sync.rs` (zwei und drei Geräte:
  Tags, Löschen mit Kindern, gleichzeitige Feldänderungen, 25-MiB-Anhang; Vorlage
  `sync_three_devices.rs`), `passwords_access.rs` (Dienst über die öffentliche API),
  `passwords_import.rs`.
- Migration: `migrations_tests.rs` (frische und aktualisierte Vault, Tabellen vorhanden),
  `vault_upgrade.rs` (Trigger auf den neuen Tabellen nach Wiederöffnen).
- Frontend: `check:passwords` (`scripts/check-passwords-generator.ts`, `-search.ts`,
  `-routes.ts` mit Sitzungs-Stichprobe, `-actions.ts`), Regression `check:agent-actions`,
  `check:wm-navigation`, `check:templates`, `typecheck`, `lint`, `format:check`.
- Eine End-to-End-Szene `scripts/e2e/scenarios/passwords-basic.test.ts` (anlegen, suchen,
  TOTP-Code, Papierkorb) und eine Sync-Szene mit zwei Geräten (Tag und Löschen) nach
  `sync-two-devices.test.ts`.
- CI: `check:passwords` neben den anderen `check:*`; die Rust-Konfigurationen
  (Standard und `--no-default-features`) bauen alle neuen Crates in beiden.

## R17 — Texte

**Entscheidung**: neuer Namensraum `passwords` in `de.json` und `en.json` (Schlüsselbäume
gleich), `wm.apps.passwords`, `actions.passwords.*`, `actions.scopes.passwords.read`,
Fehler `errors.passwords.*`. Backend gibt keine lokalisierten Texte aus; `errString`
(`composables/useErrorString.ts`) bekommt die neuen Fehlerarten. Die Texte stehen **mit**
der Komponente in den zentralen Dateien (haex-vault hat Beschriftungen beim Aufteilen einer
Komponente verloren).

## R18 — Dateidialoge und Berechtigungen

**Entscheidung**: `dialog:allow-save` kommt in `capabilities/default.json`; Öffnen gibt es
schon. Rust liest und schreibt die gewählten Pfade selbst (`spawn_blocking`). Die Pfade
stammen immer aus einem Dialog oder einer Ablage-Geste des Nutzers und werden **nicht**
dauerhaft gespeichert. Importdateien werden nur gelesen. Vorschau: Bilder (`png`, `jpg`,
`gif`, `webp`) als Blob-URL (`img-src … blob:` ist erlaubt); **PDFs und andere Dateien nur
Herunterladen** (R20): Die CSP hat `object-src 'none'` und kein `frame-src`, WebKitGTK
bringt keinen PDF-Betrachter mit, und `pdfjs-dist` wäre eine schwere neue Abhängigkeit.

## R19 — ADR

**Entscheidung**: `docs/adr/0007-secrets-in-vault-db-protected-by-grants.md` hält fest:
Geheimnisse liegen unverschlüsselt in der Vault-Datenbank; Schutz sind die Verschlüsselung
der Vault und Freigaben je Aufrufer; der eingebaute Agent bekommt nie Geheimnisse; der
Aufrufer ergibt sich aus dem Eingang (R6, R7). Die Constitution verlangt einen ADR für
Entscheidungen, die ein Kernprinzip berühren (Nr. 0005 bleibt für Spec 021 reserviert,
Nr. 0006 ist vergeben). Aufgabe in `tasks.md`.

## R20 — Änderungen an der Spec aus der Planung

Die folgenden Punkte stehen jetzt in der Spec (eigener Commit, mit Begründung hier):

1. **FR-036** nennt die Abweichungen vom Datenmodell von haex-vault vollständig: BLOB, zwei
   Spalten für die Herkunft (R3), gewöhnliche Indizes statt UNIQUE (R2),
   `ON DELETE RESTRICT` bei den Binärverweisen (R4).
2. **FR-037**: Eindeutigkeit durch abgeleitete Kennungen und Prüfung in der Anwendung, nicht durch
   UNIQUE, weil ein UNIQUE-Konflikt den Abgleich anhält (R2).
3. **FR-017** und **US4, Szenario 4**: Ein Verlaufsstand enthält den **neuen** Zustand; der
   vorherige Stand bleibt als früherer Eintrag (R5).
4. **FR-021** und **US5, Szenario 4**: Vorschau nur für Bilder; PDFs werden heruntergeladen
   (R18).
5. **FR-022** und **US5, Szenario 6**: Aufräumen mit Karenzzeit von sieben Tagen (R4).
6. **FR-023**: Gesamtgröße eines Imports über 100 MiB wird vor dem Schreiben abgelehnt;
   Anhänge über 25 MiB werden übersprungen und gemeldet (R12).
7. **FR-027**: Kopfdaten für den eingebauten Agenten ohne Benutzername und Adresse (R6).
8. **FR-023** und **US7**: Passkeys (ES256) werden importiert, der öffentliche Schlüssel wird
   abgeleitet; ungültiges TOTP führt zu einem Eintrag ohne TOTP (R12).
9. **FR-003** und der Edge Case „TOTP-Secret ungültig“: Ablehnen beim Anlegen und Ändern,
   Standardwerte, Erkennen und Beheben bei Sync und Import (R8).
10. **FR-009**: Reihenfolge der Ordner ist änderbar, auch ohne Maus.
11. **FR-024**: Auch die Oberfläche läuft über den Dienst (R6).
12. **SC-002**, **SC-007**: grobe Zielgrenzen statt harter Zeiten.
13. **FR-017**: Wortwahl „Verlaufsstand“.
14. **FR-015** und **FR-028** (Klärung 2026-10-02): Löschen durch jeden Aufrufer verschiebt in den
    Papierkorb, endgültig nur der Nutzer im Papierkorb.
15. **FR-011**: Tagnamen sind ohne Rücksicht auf Groß-/Kleinschreibung und Umlautform eindeutig.
16. **FR-028**: Ein Aufrufer sieht alle Tags, ändert aber keine außerhalb seines Bereichs und
    fügt keine hinzu.
17. **FR-033/034**: keine reservierten Tags; holzi-Funktionen melden ihre genutzten Einträge
    (`EntryUsage`).
18. **FR-001**: Der Titel darf leer sein; die Oberfläche zeigt „(ohne Titel)“.

**Entscheidungen, die der Betreiber bestätigen sollte** (im Bericht an ihn genannt):
PDF-Vorschau entfällt (R18); der eingebaute Agent sieht nur Titel und Tags (R6, R14);
Aufräumen mit Karenzzeit (R4); Abweichung von haex-vault bei UNIQUE und Fremdschlüsseln
(R2, R4).
