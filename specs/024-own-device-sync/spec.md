# Feature Specification: Vault-Identität, Geräteschlüssel und Datensync zwischen eigenen Geräten

**Feature Branch**: `024-own-device-sync`
**Created**: 2026-09-28
**Status**: Draft
**Input**: Entwurf [`2026-09-28-sync-architecture.md`](../../docs/plans/2026-09-28-sync-architecture.md),
Zeile 024 der Aufteilung in §14, aus der Brainstorming-Sitzung des Betreibers
vom 2026-09-27/28. Eine Vault läuft gleichzeitig auf mehreren Geräten; jedes
Gerät hat einen eigenen Nostr-Schlüssel, die Vault-Identität ist auf allen
Geräten dieselbe (D1). Der private Schlüssel der Vault-Identität liegt auf
jedem Gerät in der verschlüsselten Vault; ein verlorenes Gerät wird durch einen
Wechsel der Vault-Identität ausgesperrt (D8). Kein MLS (D13). Die Geräte einer
Vault finden sich über verschlüsselte Präsenzmeldungen über Nostr, verbinden
sich direkt über iroh und gleichen die ganze Vault außer gerätelokalen Daten
ab, auch auf direkten Verbindungen in verschlüsselten, signierten
Änderungspaketen. Jede Änderung behält ihren wahren Autor, der Fortschritt wird
je Ursprungsgerät verfolgt. Heute erzeugt holzi nur einen Platzhalter anstelle
eines echten Schlüsselpaars (`src-tauri/src/identity/bootstrap.rs`); eine
kopierte Vault-Datei funktioniert schon. Die Kopplung eines frisch
installierten Geräts per Code ist P2, das Aussperren eines verlorenen Geräts
P3. Die Unteransicht „Geräte“ der Einstellungs-Kategorie „Föderation“ bekommt
„zuletzt online“ und den Live-Stand. Sync über das Relay kommt mit Spec 026.

## Begriffe

- **Vault-Identität**: das Schlüsselpaar der Vault (secp256k1, im Nostr-Format).
  Es ist auf allen Geräten der Vault dasselbe; der private Schlüssel liegt nur
  in der verschlüsselten Vault (D8). Sein öffentlicher Teil ist die Adresse der
  Vault für andere Nutzer (Specs 027, 028).
- **Geräteschlüssel**: ein eigenes Schlüsselpaar je Gerät und Vault. Es verlässt
  das Gerät nie und wird nicht synchronisiert.
- **Gerätebestätigung**: eine mit der Vault-Identität signierte Aussage „Gerät X
  gehört zu Vault V“. Sie nennt den öffentlichen Geräteschlüssel, die
  Netzwerkkennung des Geräts und den Zeitpunkt der Ausstellung.
- **Gerät der Vault**: eine Installation von holzi, die die Vault geöffnet hat
  und eine gültige Gerätebestätigung besitzt. Gleichbedeutend mit einem Eintrag
  der Geräteliste (Spec 023).
- **Präsenzmeldung**: eine verschlüsselte, nur für die Geräte der eigenen Vault
  lesbare Nachricht eines Geräts mit seiner aktuellen Erreichbarkeit.
- **Bereich**: wozu eine Menge von Änderungen gehört: die Vault selbst (Bereich
  „Vault“), ein Space (Bereich eines Space, Spec 027) oder eine Datenfreigabe
  (Bereich einer Datenfreigabe, Spec 028). In dieser Spec gibt es nur den
  Bereich „Vault“; auch das Postfach der eigenen Vault bei einem Relay gehört
  zu ihm. (Nicht zu verwechseln mit den Unteransichten einer
  Einstellungs-Kategorie in Spec 023.)
- **Änderungspaket**: eine verschlüsselte, signierte Gruppe von Änderungen eines
  Bereichs. Es wird auch auf direkten Verbindungen zwischen eigenen Geräten
  verschlüsselt. Ein Änderungspaket ist atomar (FR-013).
- **Momentaufnahme**: ein vollständiger Stand eines Bereichs mit den
  ursprünglichen Autoren und Signaturen jeder Änderung, etwa im Postfach eines
  Relays (Spec 026). Anders als ein Änderungspaket wird sie je vollständiger
  Transaktionsgruppe geprüft (FR-013).
- **Transaktionsgruppe**: die Änderungen mit gemeinsamem Zeitstempel der
  hybriden logischen Uhr, also eine Transaktion. Sie wird nie geteilt und nie
  zum Teil angewendet (FR-013).
- **Nur-direkt-Daten**: die Vault-Geheimnisse, die nur auf direkten
  Verbindungen zwischen Geräten derselben Vault reisen, nie durch ein Postfach
  eines Relays und nie in einen anderen Bereich (FR-038).
- **Inhaltsschlüssel** und **Schlüsselgeneration**: der Schlüssel, mit dem
  Änderungspakete eines Bereichs verschlüsselt sind, und seine Generation. Für
  den Bereich „Vault“ gibt es genau einen Inhaltsschlüssel je Vault-Identität;
  eine neue Generation entsteht nur mit einer neuen Vault-Identität.
- **Laufnummer**: die Nummer, die ein Gerät jeder Änderung gibt, die es selbst
  erzeugt: lückenlos aufsteigend je Ursprungsgerät, zusätzlich zum Zeitstempel
  der hybriden logischen Uhr, der weiter über Konflikte entscheidet (FR-019).
- **Fortschrittsstand** (Versionsvektor): für jedes Ursprungsgerät die höchste
  Laufnummer, bis zu der ein Gerät alle Änderungen dieses Ursprungsgeräts
  lückenlos hat („lückenloser Fortschritt“, FR-019). Zwei Geräte vergleichen
  ihre Fortschrittsstände und tauschen nur, was dem anderen fehlt.
- **Mitgliederliste**: die von einem Admin-Gerät signierte Liste der
  Mitglieds-Vaults eines gemeinsamen Bereichs mit ihren Rechten und einer
  Generation (Specs 026–028). Diese Spec legt nur die Regeln fest, die alle
  gemeinsamen Bereiche teilen: die Grenze beim Entzug (FR-042) und die Wahl
  zwischen Listen gleicher Generation (FR-043).
- **Grenze**: der Teil einer Mitgliederliste, die einer Vault Rechte entzieht:
  je Gerät dieser Vault die höchste Laufnummer, die das Admin-Gerät beim
  Ausstellen angewendet hatte („Grenze beim Entzug“, FR-042).
- **Gerätelokale Daten**: Tabellen und Spalten, die ausdrücklich vom Sync
  ausgenommen sind (Endung `_no_sync`), etwa gespeicherte Sitzungen (Spec 022).
- **Relay**: das nicht vertrauenswürdige, blinde Relay aus Spec 026, ein Server
  mit einem **Postfach** je Bereich. Diese Spec baut es noch nicht, legt aber
  fest, was es nie erhalten darf (FR-038) und wie es sich beim Wechsel der
  Vault-Identität verhält (FR-041).
- **Übergabe**: eine mit der alten Vault-Identität signierte Aussage „Vault V
  heißt jetzt neue Identität N“, die die Vault nach einem Wechsel ihrer
  Identität in jedem gemeinsamen Bereich veröffentlicht (FR-039). Eine
  Übergabe allein bindet nichts um (FR-041).
- **Annahme**: eine von einer Mitglieds-Vault signierte Aussage „wir übernehmen
  für Vault V die neue Identität N“, die sie nach dem Prüfcode-Abgleich in den
  Bereich hochlädt (FR-040). Erst eine Mehrheit von Annahmen bindet die
  Admin-Rechte um („Übernahme erst nach Mehrheitsbestätigung“, FR-041).

## Beziehung zu bestehenden Specs

- Entwurf [`2026-09-28-sync-architecture.md`](../../docs/plans/2026-09-28-sync-architecture.md):
  Diese Spec setzt §4 (Identitäten), §5 (gemeinsame Bausteine) und §6 (Ebene 1,
  Datensync zwischen eigenen Geräten) um, dazu die Grundlagen, die §14 schon
  für 024 verlangt: Bereich und Schlüssel-Kennung in jedem Änderungspaket, die
  Stelle für echte Signaturprüfung, die Positivliste, die Vault-Geheimnisse auf
  der eigenen Vault hält, und das unveränderliche Feld „Ersteller“.
- [`2026-09-04-v1-scope-design.md`](../../docs/plans/2026-09-04-v1-scope-design.md)
  §4: „Vault = SQLite = Identität“ und eigene Schlüssel je Gerät bleiben die
  Grundlage. Die Fähigkeiten `pairing-authority` und `confirmation-authority`
  zwischen eigenen Geräten und die `peer_instances`-Einträge übernimmt diese
  Spec nicht: Weil jedes Gerät den privaten Schlüssel der Vault-Identität hat
  (D8), sind alle Geräte einer Vault gleichberechtigt. Die Aussage aus §5 dort,
  iroh trage keinen Zustand, gilt nicht mehr; der Datensync läuft über iroh.
- [`001-frontend-onboarding`](../001-frontend-onboarding/spec.md): „Öffnen“
  einer kopierten Vault-Datei bleibt, wie es ist, und wird zum ersten Weg, ein
  Gerät hinzuzufügen (User Story 1). Die Kopplung eines frisch installierten
  Geräts (User Story 5) ersetzt den nicht gebauten Ablauf „Verbinden“ aus Spec
  001 (User Story 2, FR-015 bis FR-017 dort): Code oder QR bleiben, die
  gegenseitig signierten `peer_instances`-Einträge entfallen.
- [`013-vault-lifecycle-isolation`](../013-vault-lifecycle-isolation/spec.md)
  und ADR-0003: Ein App-Prozess ist eine Vault-Session. Der Sync läuft nur
  innerhalb dieser Session; Sperren oder Schließen beendet alle Verbindungen
  sofort (FR-031).
- [`014-portable-mode`](../014-portable-mode/spec.md): Ein Gerät im portablen
  Modus ist ein Gerät der Vault wie jedes andere. Alles, was der Sync speichert,
  unterliegt den Speicherregeln von Spec 014.
- [`022-session-restore`](../022-session-restore/spec.md): Gespeicherte
  Sitzungen gehen nie an andere Geräte (FR-010 dort). Diese Spec nimmt
  gerätelokale Daten deshalb vom Sync aus (FR-016).
- [`023-settings-app`](../023-settings-app/spec.md): Die Kategorie „Föderation“
  zeigt die Geräte der Vault (FR-022 dort). Ihre Clarifications verschieben
  „zuletzt online“ und die Live-Aktualisierung auf die Sync-Spec; das ist diese
  Spec (User Story 4). Die Geräteliste wird zur Unteransicht „Geräte“ der
  Kategorie; die Specs 025–028 fügen die Unteransichten „Ordner“, „Relays“,
  „Spaces“ und „Datenfreigaben“ hinzu. Die neuen Knöpfe „Gerät koppeln“ und
  „Gerät aussperren“ sind Handlungen im Sinne von FR-021 dort.
- ADR-0001 (gerätebezogene Daten): Gerätebezogene Zeilen mit
  `vault_device_uuid` werden synchronisiert; nur gerätelokale Daten nicht.
- Die geplanten Specs zur **Steuerung durch Agenten** (017–021): Agenten dürfen
  die Geräteliste lesen, aber weder koppeln noch aussperren und nie an
  Schlüssel kommen (FR-036).
- Folgende Specs bauen hierauf auf: **025** (Dateisync zwischen eigenen
  Geräten), **026** (Relay mit Postfächern, auch für den Bereich „Vault“, damit
  Geräte sich angleichen, die nie gleichzeitig online sind), **027** (Spaces),
  **028** (Datenfreigaben), **029** (eigener S3-Speicher). Sie verweisen für
  die Nur-direkt-Daten (FR-038), die Atomarität von Änderungspaketen und die
  Prüfung von Momentaufnahmen je Transaktionsgruppe (FR-013), den lückenlosen
  Fortschritt (FR-019), die Grenze beim Entzug (FR-042), die Mitgliederlisten
  gleicher Generation (FR-043) sowie für den Wechsel der Vault-Identität in
  gemeinsamen Bereichen samt der Übernahme erst nach Mehrheitsbestätigung
  (FR-039 bis FR-041) auf diese Spec.

## Clarifications

### Session 2026-09-27

- Q: Wen identifiziert ein Nostr-Schlüssel, die Vault oder das Gerät? → A: Das
  Gerät. Dieselbe Vault läuft gleichzeitig auf mehreren Geräten, jedes mit
  eigener Nostr-Identität; die Vault-Identität ist auf allen dieselbe (D1).
- Q: Werden Schlüssel wie in haex-vault über MLS verteilt? → A: Nein. Die feste
  Reihenfolge der MLS-Epochen passt nicht zu einem CRDT ohne Reihenfolge (D13).
  Schlüsselgenerationen sind gewöhnliche CRDT-Zeilen, die nebeneinander bestehen
  dürfen.

### Session 2026-09-28

- Q: Wo liegt der private Schlüssel der Vault-Identität? → A: Auf jedem Gerät
  der Vault, in der verschlüsselten Datenbank (D8). Damit kann jedes Gerät
  weitere Geräte bestätigen.
- Q: Wie wird ein verlorenes oder gestohlenes Gerät ausgesperrt? → A: Durch
  einen Wechsel der Vault-Identität: neues Schlüsselpaar, neue Bestätigungen
  für die verbleibenden Geräte (D8). Die Datenbank des verlorenen Geräts
  schützt weiter die Passphrase.
- Q: Darf ein Server je Klartext der Vault sehen? → A: Nein. Das Relay ist nicht
  vertrauenswürdig (D11) und sieht keinen Inhalt (D9). Diese Spec baut das
  Relay noch nicht, legt aber das Format fest: Änderungen reisen immer
  verschlüsselt, auch direkt zwischen eigenen Geräten, und Vault-Geheimnisse
  verlassen nie die eigenen Geräte.
- Q: Was gehört in diese Spec, was in die folgenden? → A: Die Aufteilung aus §14
  des Entwurfs: 024 bringt Identitäten, direkte Verbindungen, Präsenz und den
  Datensync zwischen eigenen Geräten samt der Grundlagen für Bereiche,
  Autorenschaft und Positivliste. Dateien kommen mit 025, das Relay mit 026.
- Q: Wie kommt ein weiteres Gerät zur Vault? → A: Eine kopierte Vault-Datei
  funktioniert schon heute und bleibt der erste Weg (P1). Die Kopplung eines
  frisch installierten Geräts per QR oder Code ist P2, das Aussperren eines
  Geräts P3.
- Q: Welche Vault-Geheimnisse dürfen nie über ein Relay reisen, auch nicht
  über das Postfach der eigenen Vault? → A: Eine feste Liste, die
  Nur-direkt-Daten: der private Schlüssel der Vault-Identität, die
  Hauptzugangsdaten des Admins für S3 (Spec 029) und entpackte Inhalts- und
  Zugangsschlüssel. Alles Übrige der Vault darf verschlüsselt durch das
  eigene Postfach, das zum Bereich „Vault“ gehört (FR-018, FR-038).
- Q: Wer regelt den Wechsel der Vault-Identität gegenüber Spaces und
  Datenfreigaben? → A: Diese Spec allein. Die Vault veröffentlicht in jedem
  gemeinsamen Bereich eine mit der alten Identität signierte Übergabe;
  Mitglieder übernehmen die neue Identität erst nach Abgleich eines
  Prüfcodes, und zwei konkurrierende Übergaben frieren den Bereich beim Relay
  ein (FR-039 bis FR-041).
- Q: Wird ein Änderungspaket ganz oder je Änderung geprüft? → A: Ein
  Änderungspaket ganz: Ist eine Änderung ungültig, fällt das ganze Paket.
  Eine Momentaufnahme je Änderung (FR-013). (Nach dem Review präzisiert: je
  vollständiger Transaktionsgruppe, nie teilweise.)
- Q: Leitet ein Gerät Änderungen immer mit ihrem ursprünglichen Autor weiter?
  → A: Ja, mit einer Ausnahme: Die Vault des Eigentümers gibt eine Änderung
  zwischen überlappenden Datenfreigaben als neue, eigene Änderung aus
  (FR-021, Spec 028).
- Q: Bekommen Kopien einer Vault, die schon vor dieser Spec auf mehrere Geräte kopiert wurden, dieselbe Vault-Identität? → A: Ja. Die Identität wird aus dem gemeinsamen Platzhalter abgeleitet; die Kopien bleiben eine Vault (FR-004).
- Q: Welche Server nutzt holzi für Präsenz, NAT-Durchgang und Einladungen, solange es kein eigenes Relay gibt? → A: Voreingestellte öffentliche Nostr- und iroh-Relays, änderbar in den Einstellungen (FR-008).

## User Scenarios & Testing _(mandatory)_

### User Story 1 - Zwei eigene Geräte halten die Vault gleich (Priority: P1)

Eine Nutzerin hat ihre Vault auf dem Laptop. Sie kopiert die Vault-Datei auf
ihren Desktop-Rechner und öffnet sie dort mit ihrer Passphrase. Sobald beide
Geräte laufen, finden sie sich und verbinden sich. Ein Chat, den sie am Laptop
beginnt, steht wenige Sekunden später auch am Desktop; ändert sie am Desktop
eine Einstellung der Vault, gilt sie gleich darauf auch am Laptop. Was sie
tut, während eines der Geräte aus ist, holt das andere nach, sobald beide
wieder laufen.

**Why this priority**: Das ist der Kern der Spec und das erste Ziel des
Entwurfs. Ohne Sync ist jede Kopie einer Vault eine eigene Abzweigung.

**Independent Test**: Vault-Datei auf ein zweites Gerät kopieren, auf beiden
öffnen, auf jedem Gerät etwas ändern: Jede Änderung erscheint auf dem anderen.
Ein Gerät beenden, auf dem anderen weiterarbeiten, das erste wieder öffnen: Es
holt alles nach.

**Acceptance Scenarios**:

1. **Given** eine Vault-Datei wurde auf ein zweites Gerät kopiert und dort
   geöffnet, **When** das zweite Gerät die Vault zum ersten Mal öffnet, **Then**
   erzeugt es seinen eigenen Geräteschlüssel und seine eigene
   Gerätebestätigung, ohne dass die Nutzerin etwas tun muss.
2. **Given** beide Geräte haben die Vault offen und sind im selben Netz oder im
   Internet erreichbar, **When** sie laufen, **Then** verbinden sie sich ohne
   Zutun der Nutzerin.
3. **Given** die Geräte sind verbunden, **When** die Nutzerin auf einem Gerät
   Daten der Vault anlegt, ändert oder löscht, **Then** zeigt das andere Gerät
   dieselbe Änderung, ohne dass sie neu laden muss.
4. **Given** ein Gerät war aus, während auf dem anderen Änderungen entstanden,
   **When** beide wieder verbunden sind, **Then** hat das zurückgekehrte Gerät
   alle diese Änderungen, und beide haben denselben Stand.
5. **Given** beide Geräte ändern offline dasselbe Feld desselben Eintrags,
   **When** sie sich verbinden, **Then** haben beide danach denselben Wert (die
   jüngere Änderung gilt), und alle übrigen Felder beider Änderungen bleiben
   erhalten.
6. **Given** gerätelokale Daten wie die gespeicherte Sitzung eines Geräts
   (Spec 022), **When** die Geräte synchronisieren, **Then** kommen diese Daten
   nie auf dem anderen Gerät an.
7. **Given** der Laptop war bisher das einzige Gerät der Vault und
   veröffentlicht deshalb keine Präsenz, **When** die kopierte Vault-Datei auf
   dem Desktop zum ersten Mal geöffnet wird, **Then** findet der Laptop die
   Kopie über deren Präsenzmeldung, die beiden verbinden sich, und von da an
   veröffentlichen beide Präsenz (FR-007).

---

### User Story 2 - Nur eigene Geräte kommen an die Daten (Priority: P1)

Die Nutzerin hat eine zweite, private Vault auf demselben Laptop und ihr
Kollege hat seine eigene Vault. Keines dieser fremden Geräte bekommt Daten ihrer
Vault, auch wenn es sich als eines ihrer Geräte ausgibt. Wer den Netzverkehr
mitliest, sieht keinen Inhalt. Der private Schlüssel der Vault-Identität reist
nur zwischen ihren eigenen Geräten.

**Why this priority**: Die Sicherheit ist die Voraussetzung dafür, den Sync
überhaupt einzuschalten. Sie muss vom ersten Tag an stimmen, nicht erst mit dem
Relay.

**Independent Test**: Ein Gerät mit einer anderen Vault und ein Gerät mit
gefälschter Gerätebestätigung versuchen, sich mit einem Gerät der Vault zu
verbinden: Beide werden abgewiesen, bevor irgendein Inhalt fließt. Den Verkehr
zwischen zwei eigenen Geräten mitschneiden: Er enthält keinen Klartext der
Vault.

**Acceptance Scenarios**:

1. **Given** ein Gerät mit einer anderen Vault, **When** es sich mit einem Gerät
   dieser Vault verbinden will, **Then** wird die Verbindung abgewiesen, und es
   erhält keine Daten und keine Änderungspakete.
2. **Given** ein Gerät legt eine Gerätebestätigung vor, die nicht von dieser
   Vault-Identität signiert ist oder deren Geräteschlüssel es nicht besitzt,
   **When** es sich verbinden will, **Then** wird es abgewiesen.
3. **Given** zwei eigene Geräte synchronisieren, **When** jemand den Verkehr
   mitliest, **Then** sieht er weder Inhalte noch Tabellen-, Spalten- oder
   Schlüsselnamen der Vault.
4. **Given** ein Änderungspaket wurde unterwegs verändert, **When** es ankommt,
   **Then** wird es verworfen und nicht angewendet.
5. **Given** die Präsenzmeldung eines Geräts, **When** jemand sie liest, der
   kein Gerät dieser Vault ist, **Then** erfährt er daraus nicht, wo das Gerät
   erreichbar ist.

---

### User Story 3 - Keine Änderung geht verloren oder doppelt, der Autor stimmt (Priority: P1)

Die Nutzerin hat drei Geräte: Laptop, Desktop und ein Arbeitsgerät. Laptop und
Arbeitsgerät sind nie gleichzeitig an. Der Desktop läuft meistens. Was sie am
Laptop ändert, kommt über den Desktop auf das Arbeitsgerät und umgekehrt. Jede
Änderung kommt genau einmal an, und bei jeder Änderung steht weiter, auf
welchem Gerät sie entstand, nicht, über welches Gerät sie kam.

**Why this priority**: Über indirekte Wege gehen mit einem einfachen „zuletzt
gesendet“ Änderungen verloren, und haex-crdt trägt heute das weiterleitende
Gerät als Autor ein (Entwurf §3.2). Spaces und Datenfreigaben (027, 028)
brauchen den richtigen Autor für ihre Rechte.

**Independent Test**: Drei Geräte; A und C sind nie gleichzeitig mit B
verbunden. Auf A und C abwechselnd Änderungen erzeugen, jeweils nur mit B
synchronisieren: Am Ende haben alle drei denselben Stand, keine Änderung fehlt
oder ist doppelt, und jede nennt ihr Ursprungsgerät.

**Acceptance Scenarios**:

1. **Given** Gerät A hat Änderungen nur mit B ausgetauscht, **When** B sich
   danach mit C verbindet, **Then** erhält C alle Änderungen von A.
2. **Given** C hat Änderungen von A schon über B erhalten, **When** A und C sich
   später direkt verbinden, **Then** tauschen sie diese Änderungen nicht noch
   einmal aus, und keine wird doppelt angewendet.
3. **Given** eine Änderung von A kam über B bei C an, **When** C ihren Autor
   anzeigt oder prüft, **Then** ist der Autor Gerät A der Vault, nicht B.
4. **Given** eine Verbindung bricht mitten im Austausch ab, **When** die Geräte
   sich wieder verbinden, **Then** fehlt keine Änderung, keine ist doppelt, und
   keine zusammengehörige Gruppe von Änderungen wurde nur zum Teil angewendet.
5. **Given** eine neuere Änderung von A kommt über B bei C an, bevor eine
   ältere Änderung von A dort ist, **When** C seinen Fortschrittsstand
   berechnet, **Then** zählt C die neuere Änderung nicht als lückenlosen
   Fortschritt, erkennt die Lücke und fordert die fehlende Änderung
   ausdrücklich an.
6. **Given** unter einer schon vergebenen Laufnummer von A trifft eine zweite,
   andere Änderung ein, **When** C sie prüft, **Then** verwirft C sie als
   Fälschung und behält die zuerst angenommene.

---

### User Story 4 - Die Geräteliste zeigt, wer online ist (Priority: P2)

In den Einstellungen, Kategorie „Föderation“, Unteransicht „Geräte“, sieht die
Nutzerin ihre Geräte.
Bei jedem steht, ob es gerade verbunden ist oder wann es zuletzt online war.
Schaltet sie ein Gerät ein, springt es in der Liste auf „online“; benennt sie
ein Gerät auf diesem um, steht der neue Name gleich darauf auch auf den
anderen. Ein neu hinzugekommenes Gerät erscheint in der Liste, sobald es sich
zum ersten Mal synchronisiert hat.

**Why this priority**: Spec 023 hat „zuletzt online“ und die Live-Liste
ausdrücklich auf diese Spec verschoben. Die Nutzerin sieht hier, ob der Sync
arbeitet.

**Independent Test**: Zwei Geräte; auf A die Unteransicht „Geräte“ offen
lassen. B einschalten, umbenennen, beenden: A zeigt B nacheinander als online,
mit neuem Namen und dann mit „zuletzt online“ und passender Zeit, ohne neu zu
laden.

**Acceptance Scenarios**:

1. **Given** die Unteransicht „Geräte“, **When** ein anderes Gerät gerade
   verbunden ist, **Then** steht es als „online“ da.
2. **Given** ein Gerät ist nicht verbunden, **When** die Liste es zeigt,
   **Then** steht dort, wann es zuletzt online war, oder „noch nie online
   gesehen“, wenn dieses Gerät davon nichts weiß.
3. **Given** die Unteransicht ist offen, **When** ein Gerät online geht, offline
   geht, umbenannt wird oder neu dazukommt, **Then** aktualisiert sich die Liste
   ohne Zutun.
4. **Given** ein Gerät, mit dem kein Sync möglich ist (andere, inkompatible
   Version von holzi), **When** die Liste es zeigt, **Then** steht dort, dass
   eines der Geräte aktualisiert werden muss.

---

### User Story 5 - Neues Gerät per Code koppeln (Priority: P2)

Die Nutzerin installiert holzi auf einem neuen Rechner, auf dem noch keine
Vault liegt. Auf dem Laptop wählt sie in „Föderation“, Unteransicht „Geräte“,
den Knopf „Gerät koppeln“; der Laptop zeigt einen QR-Code und denselben Code
als Text. Auf dem neuen Rechner wählt sie auf der Startseite die Kopplung,
scannt den Code oder gibt ihn ein, vergibt einen Gerätenamen und eine
Passphrase für diese Installation. Der Laptop zeigt den Namen des neuen Geräts;
sie bestätigt dort. Danach überträgt der Laptop die Vault, und der neue Rechner
ist ein Gerät der Vault wie jedes andere.

**Why this priority**: Die kopierte Vault-Datei deckt den Bedarf schon ab (User
Story 1). Die Kopplung ist bequemer und nötig, wo sich keine Datei kopieren
lässt, aber nicht die Voraussetzung für den Sync.

**Independent Test**: Auf einem Gerät der Vault einen Code erzeugen, auf einer
frischen Installation eingeben, auf dem ersten Gerät bestätigen: Die frische
Installation hat danach die Vault mit allen Daten, erscheint in der Geräteliste
beider Geräte und synchronisiert weiter.

**Acceptance Scenarios**:

1. **Given** ein Gerät der Vault, **When** die Nutzerin „Gerät koppeln“ wählt,
   **Then** zeigt es einen einmal verwendbaren, kurzlebigen Code als QR-Code und
   als Text.
2. **Given** eine frische Installation und ein gültiger Code, **When** die
   Nutzerin ihn dort eingibt, **Then** zeigt das erste Gerät den Namen des neuen
   Geräts und wartet auf ihre Bestätigung, bevor es irgendetwas überträgt.
3. **Given** die Nutzerin bestätigt, **When** die Übertragung läuft, **Then**
   zeigt die neue Installation den Fortschritt und öffnet danach die Vault mit
   allen Daten außer den gerätelokalen des ersten Geräts.
4. **Given** ein abgelaufener, schon benutzter oder falsch eingegebener Code,
   **When** er eingegeben wird, **Then** schlägt die Kopplung mit einer klaren
   Meldung fehl, und es wird nichts übertragen.
5. **Given** die Nutzerin lehnt auf dem ersten Gerät ab oder bricht ab, **When**
   das passiert, **Then** erhält die neue Installation nichts, und der Code ist
   verbraucht.

---

### User Story 6 - Verlorenes Gerät aussperren (Priority: P3)

Das Arbeitsgerät der Nutzerin wurde gestohlen. Auf dem Laptop wählt sie in
„Föderation“, Unteransicht „Geräte“, beim gestohlenen Gerät „Gerät
aussperren“. holzi erklärt, was
passiert: Die Vault bekommt eine neue Identität, das gestohlene Gerät bekommt
keine neuen Daten mehr, die Daten, die schon darauf sind, schützt weiter nur
die Passphrase, und jedes verbleibende Gerät muss die neue Identität einmal
bestätigen. Sie bestätigt. Auf dem Desktop fragt holzi beim nächsten Start
nach, ob er die neue Identität übernehmen soll; sie gleicht dazu einen Code mit
dem Laptop ab. Die Nutzerin ist außerdem Mitglied eines Space ihres Kollegen:
holzi teilt dem Space mit, dass ihre Vault jetzt die neue Identität hat, und der
Kollege übernimmt sie erst, nachdem er mit ihr einen Prüfcode abgeglichen hat.

**Why this priority**: Selten, aber ohne diesen Weg bleibt ein gestohlenes
Gerät für immer Teil der Vault. Der Wechsel der Vault-Identität ist die einzige
Möglichkeit (D8), weil jedes Gerät den privaten Schlüssel hat.

**Independent Test**: Drei Geräte; eines aussperren und das dritte die neue
Identität bestätigen lassen: Das ausgesperrte Gerät kann sich mit keinem der
beiden mehr verbinden und erhält keine neue Änderung; die beiden verbleibenden
synchronisieren weiter. Ist die Vault Mitglied eines Space (Spec 027), übernimmt
die andere Mitglieds-Vault die neue Identität erst nach Abgleich des Prüfcodes;
das Relay nimmt ab der ersten Übergabe nichts mehr von der alten Identität an
und bindet die Admin-Rechte erst um, wenn eine Mehrheit der übrigen Mitglieder
dieselbe neue Identität angenommen hat.

**Acceptance Scenarios**:

1. **Given** die Geräteliste, **When** die Nutzerin ein anderes Gerät aussperren
   will, **Then** erklärt holzi vor der Bestätigung die Folgen und dass sie
   nicht rückgängig zu machen sind.
2. **Given** das Aussperren ist bestätigt, **When** es abgeschlossen ist,
   **Then** hat die Vault auf diesem Gerät eine neue Vault-Identität, und das
   ausgesperrte Gerät ist in der Liste als ausgesperrt markiert.
3. **Given** ein verbleibendes Gerät kennt noch die alte Vault-Identität,
   **When** es die neue angeboten bekommt, **Then** übernimmt es sie erst, wenn
   die Nutzerin das auf diesem Gerät bestätigt; bis dahin synchronisiert es mit
   den Geräten der neuen Identität nicht.
4. **Given** das Aussperren ist abgeschlossen, **When** das ausgesperrte Gerät
   sich verbinden will oder Änderungspakete anbietet, **Then** wird es von jedem
   Gerät abgewiesen, das die neue Identität übernommen hat.
5. **Given** Änderungen, die das ausgesperrte Gerät vor dem Aussperren
   geschrieben hat und die schon angekommen waren, **When** das Aussperren
   abgeschlossen ist, **Then** bleiben sie erhalten.
6. **Given** die Vault ist Mitglied oder Admin eines Space oder einer
   Datenfreigabe (Specs 027, 028), **When** das Aussperren abgeschlossen ist,
   **Then** veröffentlicht sie in jedem dieser Bereiche eine mit der alten
   Identität signierte Übergabe, die die neue Identität nennt.
7. **Given** ein Gerät einer anderen Vault, die Mitglied eines solchen Bereichs
   ist, erhält die Übergabe, **When** es sie anzeigt, **Then** übernimmt es die
   neue Identität erst, nachdem sein Nutzer einen Prüfcode mit der Nutzerin
   abgeglichen und bestätigt hat; bis dahin nimmt es von der neuen Identität
   nichts an.
8. **Given** die Vault der Nutzerin ist Admin eines Space, **When** die erste
   Übergabe für diesen Bereich beim Relay eintrifft, **Then** nimmt das Relay
   dort nichts mehr an, was mit der alten Identität signiert ist, bedient die
   Lesezugriffe der übrigen Mitglieder weiter und bindet die Admin-Rechte erst
   an die neue Identität, wenn eine Mehrheit der übrigen Mitglieder der
   letzten gültigen Mitgliederliste eine Annahme genau dieser neuen Identität
   hochgeladen hat.
9. **Given** für dieselbe alte Identität treffen zwei verschiedene Übergaben
   ein (etwa eine der Nutzerin und eine des Diebs), **When** ein Mitglied sie
   sieht, **Then** zeigt es den Konflikt und übernimmt keine von sich aus; die
   neue Identität, die zuerst die Mehrheit der Annahmen erreicht, gilt beim
   Relay und bei allen Mitgliedern, die andere wird abgewiesen.

---

### Edge Cases

- Eine Vault hat nur ein einziges Gerät: Es veröffentlicht im Bereich „Vault“
  keine Präsenz und baut keine direkten Verbindungen auf, lauscht aber auf
  Präsenzmeldungen an seine Vault-Identität (FR-007). Uploads in das Postfach
  eines Relays (Spec 026) und der Verkehr mit Mitgliedern von Spaces und
  Datenfreigaben (Specs 027, 028) bleiben erlaubt.
- Die Vault-Datei eines solchen Geräts wird kopiert: Die Kopie kennt das
  Quellgerät aus der Datei und veröffentlicht deshalb Präsenz; das Quellgerät
  erfährt so von ihr, und ab dann veröffentlichen beide (FR-007). Voneinander
  unabhängige Vaults mit je einem Gerät bleiben still.
- Eine Vault-Datei wird kopiert, samt der gerätelokalen Daten, die in der Datei
  liegen (ADR-0001): Das Zielgerät verwendet keinen Geräteschlüssel und keine
  Gerätebestätigung des Quellgeräts, sondern erzeugt eigene (FR-006).
- Zwei Geräte treten mit demselben Geräteschlüssel oder derselben Gerätekennung
  auf, etwa nach dem Klonen einer ganzen virtuellen Maschine: holzi erkennt
  das, synchronisiert mit keinem der beiden weiter und sagt der Nutzerin, dass
  ein Gerät doppelt vorhanden ist (FR-030).
- Zwei eigene Geräte sind nie gleichzeitig online und es gibt kein drittes: Sie
  gleichen sich in dieser Spec nicht an; das leistet erst das Postfach des
  Relays (Spec 026). Die Geräteliste zeigt für beide „zuletzt online“.
- Ein Gerät läuft mit einer älteren holzi-Version, deren Vault-Schema die
  Änderungen des anderen nicht kennt: Die Geräte synchronisieren nicht
  miteinander, bis beide passend aktualisiert sind; keine Änderung geht
  verloren, beide zeigen den Grund (FR-029).
- Die Uhr eines Geräts geht stark falsch: Die Reihenfolge der Änderungen
  richtet sich nach der hybriden logischen Uhr von haex-crdt, nicht allein nach
  der Wanduhr; „zuletzt online“ kann auf diesem Gerät ungenau sein.
- Die Nutzerin sperrt oder schließt die Vault mitten in einem Austausch: Alle
  Verbindungen enden sofort (Spec 013); halb empfangene Änderungspakete werden
  nicht angewendet und beim nächsten Mal neu geholt.
- Ein Gerät im portablen Modus (Spec 014) auf einem fremden Rechner: Es ist ein
  gewöhnliches Gerät der Vault. Wer es danach nicht mehr in der Vault haben
  will, sperrt es aus (User Story 6).
- Während eines Aussperrens hat ein verbleibendes Gerät Änderungen offline
  geschrieben: Sie gehen nicht verloren; sobald es die neue Identität
  übernommen hat, synchronisiert es sie wie sonst.
- Ein Dieb sperrt mit dem gestohlenen Gerät seinerseits die Nutzerin aus: Ihre
  Geräte übernehmen seine neue Identität nur mit ihrer Bestätigung auf jedem
  Gerät (FR-027), also nie. Veröffentlicht er in gemeinsamen Bereichen eine
  eigene Übergabe, übernehmen die Mitglieder sie nur nach einem
  Prüfcode-Abgleich (FR-040). Seine Übergabe allein bindet beim Relay nichts
  um; trifft sie neben der Übergabe der Nutzerin ein, sehen die Mitglieder den
  Konflikt, und es gilt nur die neue Identität, die zuerst die Mehrheit der
  Annahmen erreicht (FR-041).
- Eine Vault aus einer Zeit vor dieser Spec hat nur einen Platzhalter als
  Vault-Identität und wurde schon auf mehrere Geräte kopiert (FR-004).
- Während der Kopplung verliert eine Seite die Verbindung: Die Kopplung bricht
  ab, die neue Installation behält nichts Halbes, und die Nutzerin beginnt mit
  einem neuen Code.

## Requirements _(mandatory)_

### Functional Requirements

**Identitäten**

- **FR-001**: Beim Anlegen einer Vault MUSS holzi eine echte Vault-Identität
  (secp256k1-Schlüsselpaar aus einer kryptographisch sicheren Zufallsquelle)
  erzeugen und in der verschlüsselten Vault speichern. Der Platzhalter von
  heute DARF für neue Vaults nicht mehr entstehen.
- **FR-002**: Der private Schlüssel der Vault-Identität DARF NIE in Protokollen,
  Diagnosen, Fehlermeldungen, der Oberfläche, für Erweiterungen oder für Agenten
  erscheinen. Er gehört zu den Nur-direkt-Daten (FR-038): Er DARF die Vault
  nur als Inhalt eines Änderungspakets des Bereichs „Vault“ auf einer direkten
  Verbindung an ein Gerät derselben Vault verlassen oder bei der Kopplung an
  das neu bestätigte Gerät (FR-024).
- **FR-003**: Jedes Gerät MUSS für jede Vault, die es öffnet, einen eigenen
  Geräteschlüssel haben, der beim ersten Öffnen der Vault auf diesem Gerät
  entsteht. Ein Geräteschlüssel DARF NIE synchronisiert, über das Netz
  übertragen oder von einem anderen Gerät verwendet werden.
- **FR-004**: Eine Vault, die vor dieser Spec angelegt wurde und nur den
  Platzhalter trägt, MUSS beim ersten Öffnen mit dieser Version eine echte
  Vault-Identität bekommen. Kopien derselben Vault, die schon vor dieser Spec auf mehrere Geräte kopiert wurden, MÜSSEN dabei dieselbe Vault-Identität bekommen, abgeleitet aus dem gemeinsamen Platzhalter; sie bleiben eine Vault und synchronisieren miteinander. Wie abgeleitet wird, klärt der Plan.
- **FR-005**: Jedes Gerät MUSS eine Gerätebestätigung besitzen, die die
  Vault-Identität signiert hat und die Vault, den öffentlichen Geräteschlüssel,
  die Netzwerkkennung des Geräts und den Zeitpunkt der Ausstellung nennt. Weil
  jedes Gerät die Vault-Identität hat, MUSS es sich diese Bestätigung selbst
  ausstellen können; die Nutzerin muss dafür nichts tun.
- **FR-006**: Öffnet eine Installation eine Vault-Datei, die von einem anderen
  Gerät kopiert wurde, MUSS sie einen neuen Geräteschlüssel und eine neue
  Gerätebestätigung für sich erzeugen und DARF Geräteschlüssel und
  Gerätebestätigung des Quellgeräts NICHT verwenden, auch wenn sie in der Datei
  liegen.

**Finden und Verbinden**

- **FR-007**: Jedes Gerät, dessen Vault mindestens ein weiteres Gerät kennt
  (einen weiteren Eintrag der Geräteliste mit gültiger Gerätebestätigung), MUSS,
  solange die Vault offen ist, eine Präsenzmeldung veröffentlichen, die nur
  Geräte derselben Vault entschlüsseln können und die angibt, wie das Gerät
  gerade erreichbar ist. Eine Präsenzmeldung MUSS von ihrem Gerät signiert sein
  und nach kurzer Zeit ablaufen, wenn sie nicht erneuert wird. Eine Vault mit
  nur einem bekannten Gerät DARF im Bereich „Vault“ keine Präsenzmeldung
  veröffentlichen und keine direkte Verbindung aufbauen, außer während einer
  Kopplung (FR-023) oder als Antwort auf eine gültige Präsenzmeldung eines
  anderen Geräts derselben Vault. Ihr Gerät MUSS trotzdem, solange die Vault
  offen ist, auf Präsenzmeldungen an seine Vault-Identität lauschen. So findet
  ein Quellgerät eine frische Kopie seiner Vault-Datei: Die Kopie kennt das
  Quellgerät aus der kopierten Datei und veröffentlicht deshalb Präsenz; das
  Quellgerät erfährt von ihr aus deren Präsenzmeldung, nimmt sie nach FR-009
  in seine Geräteliste auf, und von da an veröffentlichen beide. Voneinander
  unabhängige Vaults mit je einem Gerät bleiben still. Die Einschränkung gilt
  nur für Präsenz und direkten Sync im Bereich „Vault“: Uploads in das Postfach
  eines Relays (Spec 026) und der Verkehr mit Mitgliedern von Spaces und
  Datenfreigaben (Specs 027, 028) bleiben auch mit einem einzigen Gerät
  erlaubt.
- **FR-008**: Geräte derselben Vault MÜSSEN sich über die Präsenzmeldungen
  finden und selbständig eine direkte Verbindung aufbauen, im selben Netz wie
  über das Internet, auch hinter üblichen Heimroutern. Die Nutzerin MUSS dafür
  keine Adresse eingeben. holzi MUSS dafür voreingestellte öffentliche Nostr-Relays und iroh-Relays nutzen, die der Nutzer in den Einstellungen ändern oder durch eigene Server ersetzen kann. Dieselben Server tragen auch die Einladungen zu Spaces und Datenfreigaben (Spec 027, Einladungen).
- **FR-009**: Beim Verbindungsaufbau MÜSSEN beide Seiten ihre Gerätebestätigung
  vorlegen und beweisen, dass sie den zugehörigen Geräteschlüssel besitzen. Ein
  Gerät DARF eine Verbindung nur annehmen, wenn die Bestätigung von seiner
  eigenen, aktuellen Vault-Identität signiert ist und nicht zu einem
  ausgesperrten Gerät gehört. Vor dieser Prüfung DARF kein Inhalt und kein
  Änderungspaket fließen.
- **FR-010**: Geräte MÜSSEN die Verbindung selbständig wieder aufbauen, wenn sie
  abbricht oder sich die Erreichbarkeit eines Geräts ändert (anderes Netz,
  Aufwachen aus dem Ruhezustand).

**Änderungspakete und Bereiche**

- **FR-011**: Jede Änderung MUSS genau einem Bereich angehören. In dieser Spec
  ist das immer der Bereich „Vault“; das Format MUSS weitere Bereiche (Spaces,
  Datenfreigaben) ohne Änderung aufnehmen können.
- **FR-012**: Änderungen MÜSSEN in Änderungspaketen reisen, auch auf direkten
  Verbindungen zwischen eigenen Geräten. Ein Änderungspaket MUSS seinen Bereich
  und die Kennung seines Inhaltsschlüssels offen tragen und alles andere
  (Tabellen, Spalten, Schlüssel der Zeilen, Zeitstempel, Werte, Autoren)
  verschlüsselt. Ein Gerät DARF ein Änderungspaket unverändert weitergeben
  können, ohne es zu öffnen.
- **FR-013**: Ein Änderungspaket DARF eine zusammengehörige Gruppe von
  Änderungen (eine Transaktion, also die Änderungen mit gemeinsamem Zeitstempel
  der hybriden logischen Uhr) nie auf mehrere Pakete aufteilen. Ein
  Änderungspaket ist atomar: Ist auch nur eine Änderung darin ungültig (etwa
  Signatur, Autor, Recht, Bereich oder Feld „Ersteller“ nach FR-022), MUSS der
  Empfänger das ganze Paket verwerfen; angewendet wird es ganz oder gar nicht,
  sodass keine Transaktion zum Teil ankommt. Eine Momentaufnahme dagegen MUSS
  der Empfänger je vollständiger Transaktionsgruppe prüfen: Ist eine Änderung
  ungültig, verwirft er ihre ganze Transaktionsgruppe; die übrigen gültigen
  Gruppen übernimmt er. Eine Transaktionsgruppe DARF NIE zum Teil angewendet
  werden. Das verfeinert die Clarification „Eine Momentaufnahme je Änderung“:
  geprüft wird jede Änderung, verworfen oder übernommen wird je Gruppe. Specs
  026 und 027 wenden diese Regeln an.
- **FR-014**: Ein Empfänger MUSS ein Änderungspaket verwerfen, das sich nicht
  entschlüsseln lässt, verändert wurde oder zu einem anderen Bereich gehört als
  angegeben, und DARF davon nichts anwenden.
- **FR-015**: Der Bereich „Vault“ MUSS einen Inhaltsschlüssel haben, der nur in
  der Vault liegt. Eine neue Schlüsselgeneration entsteht nur mit einer neuen
  Vault-Identität (FR-026). Ein Empfänger MUSS jedes Paket mit dem Schlüssel
  entschlüsseln, den dessen Kennung nennt, ohne dass eine „aktuelle“ Generation
  vereinbart sein muss.

**Was synchronisiert wird**

- **FR-016**: Der Sync MUSS alle Daten der Vault umfassen außer den
  gerätelokalen Daten. Gerätelokale Daten DÜRFEN NIE ein Gerät über den Sync
  verlassen.
- **FR-017**: Anlegen, Ändern und Löschen MÜSSEN ankommen; eine Löschung MUSS
  auf allen Geräten gelten, auch auf solchen, die den Eintrag zwischendurch noch
  geändert hatten. Gleichzeitige Änderungen desselben Felds MÜSSEN auf allen
  Geräten zum selben Ergebnis führen.
- **FR-018**: Welche Daten die eigene Vault in einen anderen Bereich als „Vault“
  verlassen dürfen (den Bereich eines Space oder einer Datenfreigabe), MUSS
  eine ausdrückliche Positivliste festlegen, keine Ausschlussliste. Das
  Postfach der eigenen Vault bei einem Relay (Spec 026) ist kein anderer
  Bereich, sondern gehört zum Bereich „Vault“; was dort nicht hin darf, regelt
  FR-038. Der Inhaltsschlüssel des Bereichs „Vault“, alle Nur-direkt-Daten und
  alle anderen Vault-Geheimnisse DÜRFEN auf dieser Liste nie stehen; eine neue
  Tabelle ist nicht darauf, solange sie nicht ausdrücklich eingetragen wird.
- **FR-038**: Folgende **Nur-direkt-Daten** DÜRFEN nur auf direkten, nach
  FR-009 geprüften Verbindungen zwischen Geräten derselben Vault reisen, nie
  durch ein Postfach eines Relays (auch nicht das der eigenen Vault) und nie in
  einen anderen Bereich als „Vault“:
  (a) der private Schlüssel der Vault-Identität;
  (b) die Hauptzugangsdaten des Admins für einen eigenen S3-Speicher (Spec 029);
  (c) entpackte Inhaltsschlüssel von Spaces und Datenfreigaben und entpackte
  Zugangsschlüssel (Spec 029). Die Umschläge, in denen diese Schlüssel an die
  Vault-Identität gerichtet sind, entpackt jedes Gerät selbst, weil jedes Gerät
  deren privaten Schlüssel hat (D8).
  Alle übrigen Daten der Vault außer den gerätelokalen, auch der Dateiindex
  samt den Schlüsseln je Datei der eigenen synchronisierten Ordner (Spec 025)
  und die Umschläge selbst, DÜRFEN durch das Postfach der eigenen Vault reisen,
  verschlüsselt mit dem Inhaltsschlüssel des Bereichs „Vault“. Specs 025, 026,
  028 und 029 verweisen auf diese Liste; ein neues Vault-Geheimnis, das nie
  ein Relay erreichen darf, MUSS hier eingetragen werden.

**Fortschritt und Autorenschaft**

- **FR-019** („lückenloser Fortschritt“): Jedes Gerät MUSS die Änderungen, die
  es selbst erzeugt, lückenlos aufsteigend nummerieren (Laufnummer je
  Ursprungsgerät), zusätzlich zum Zeitstempel der hybriden logischen Uhr, der
  weiter über Konflikte entscheidet (FR-017). Die Laufnummer gehört zu der
  Änderung, die das Ursprungsgerät signiert (FR-021). Jedes Gerät MUSS seinen
  Fortschritt je Ursprungsgerät führen, nicht als einen einzigen Zeitpunkt und
  nicht als jüngsten Zeitstempel: Der Fortschrittsstand ist je Ursprungsgerät
  die höchste Laufnummer, bis zu der alle Änderungen dieses Geräts vorliegen.
  Eine Änderung jenseits einer Lücke DARF angewendet werden, zählt aber nicht
  zum Fortschrittsstand; die Lücke MUSS das Gerät erkennen und die fehlenden
  Änderungen ausdrücklich anfordern. Trifft unter einem schon belegten Paar aus
  Ursprungsgerät und Laufnummer eine zweite, andere Änderung ein, MUSS der
  Empfänger sie als Fälschung verwerfen. Zwei Geräte MÜSSEN beim Verbinden ihre
  Fortschrittsstände vergleichen und nur übertragen, was dem anderen fehlt, in
  beide Richtungen, die angeforderten Lücken eingeschlossen. Specs 026–028
  verwenden dieselben Laufnummern, auch für die Grenze beim Entzug (FR-042).
- **FR-020**: Über jeden Weg zwischen den Geräten, auch über Zwischengeräte,
  MUSS jede Änderung jedes Gerät genau einmal erreichen, sobald ein
  Verbindungsweg besteht. Eine abgebrochene Übertragung MUSS sich fortsetzen
  lassen, ohne Änderungen zu verlieren oder doppelt anzuwenden.
- **FR-021**: Jede Änderung MUSS ihr Ursprungsgerät und ihre Vault als Autor
  tragen und mit dem Geräteschlüssel des Ursprungsgeräts signiert sein. Ein
  weiterleitendes Gerät DARF Autor und Signatur NICHT verändern; sie MÜSSEN auch
  in gespeicherten und weitergegebenen Ständen erhalten bleiben. Einzige
  Ausnahme ist die Neuausgabe durch die Vault des Eigentümers (Admin der
  Datenfreigabe) zwischen überlappenden Datenfreigaben (Spec 028): Sie gibt
  eine weitergeleitete Änderung als neue Änderung im Bereich der zweiten
  Datenfreigabe aus, signiert von einem Gerät der Eigentümer-Vault, die damit
  ihr Autor ist; der ursprüngliche Autor bleibt nur als Anzeige erhalten, nicht
  als signierender Autor. Innerhalb des
  Bereichs „Vault“ DARF ein Empfänger auf einer nach FR-009 geprüften direkten
  Verbindung auf die Prüfung jeder einzelnen Signatur verzichten; auf allen
  anderen Wegen (Specs 026–028) ist die Prüfung Pflicht.

**Grundlagen für spätere Bereiche**

- **FR-022**: holzi MUSS für Tabellen, die es verlangen, ein unveränderliches
  Feld „Ersteller“ unterstützen: Es wird beim Anlegen einer Zeile mit der Vault
  des Autors gefüllt und DARF danach von niemandem geändert werden; ein
  Empfänger MUSS jede Änderung dieses Felds ablehnen. In dieser Spec nutzt es
  noch keine Tabelle; Specs 025, 027 und 028 bauen darauf auf.
- **FR-042** („Grenze beim Entzug“): Eine neue Mitgliederliste, die eine Vault
  entfernt oder ihre Rechte senkt, MUSS eine Grenze tragen: für jedes Gerät der
  betroffenen Vault die höchste Laufnummer (FR-019), die das ausstellende
  Admin-Gerät in diesem Moment von ihm angewendet hatte. Jedes Gerät, das diese
  Liste kennt, MUSS jede Änderung der betroffenen Vault mit einer Laufnummer
  jenseits der Grenze ihres Geräts ablehnen, die das entzogene Recht braucht,
  egal welchen Zeitstempel sie trägt. Rückdatieren ist nicht möglich, weil die
  Laufnummern bis zur Grenze schon vergeben sind (FR-019). Ein Gerät der
  betroffenen Vault, das in der Grenze fehlt, gilt mit Grenze null. Änderungen,
  die ein Gerät angewendet hatte, bevor es die Liste kannte, bleiben (das
  hingenommene Fenster aus D19); die nächste berechtigte Änderung derselben
  Zelle behebt die Abweichung. Keine Regel DARF sich auf den Zeitstempel einer
  Änderung im Verhältnis zur Liste stützen. Das verfeinert D19: Die Prüfung
  beim Empfang richtet sich nach der Grenze, nicht nach der Zeit. Specs 027 und
  028 wenden diese Regel an; im Bereich „Vault“ gibt es keine Rechte und damit
  keine Grenze.
- **FR-043** („Mitgliederlisten gleicher Generation“): Mitgliederlisten
  veröffentlichen nur Admin-Geräte, aber zwei Admin-Geräte können verschiedene
  gültige Listen mit derselben Generation veröffentlichen. Unter gültigen
  Listen derselben Generation MUSS die Liste mit dem lexikographisch kleinsten
  Hash gelten, beim Relay (Spec 026) wie bei jedem Empfänger. Das Relay MUSS
  eine gespeicherte Liste derselben Generation durch eine mit kleinerem Hash
  ersetzen, statt sie abzuweisen, und DARF sie durch eine mit größerem Hash
  NICHT ersetzen. Ein Admin-Gerät, das zwei verschiedene Listen derselben
  Generation sieht, MUSS eine Liste mit der nächsthöheren Generation
  veröffentlichen, die die Änderungen beider Listen zusammenführt. Specs 026,
  027 und 028 wenden diese Regel an.

**Gerät koppeln (P2)**

- **FR-023**: Ein Gerät der Vault MUSS auf Wunsch der Nutzerin einen
  Kopplungscode erzeugen, als QR-Code und als Text. Der Code MUSS einmal
  verwendbar sein, nach spätestens 10 Minuten ablaufen und genug Zufall
  enthalten, dass er sich nicht erraten lässt. Er enthält, wie die neue
  Installation das Gerät erreicht.
- **FR-024**: Eine frische Installation MUSS auf ihrer Startseite die Kopplung
  anbieten: Code scannen oder eingeben, Gerätename und Passphrase für diese
  Installation vergeben. Beide Seiten MÜSSEN beweisen, dass sie den Code
  kennen, bevor irgendetwas übertragen wird; das erste Gerät MUSS danach den
  Namen des neuen Geräts zeigen und die Übertragung erst nach Bestätigung der
  Nutzerin beginnen. Erst dann erhält die neue Installation die Vault
  einschließlich der Vault-Identität und stellt ihre eigene Gerätebestätigung
  aus.
- **FR-025**: Die Passphrase DARF bei der Kopplung NIE übertragen werden. Bricht
  die Kopplung ab, MUSS die neue Installation alles verwerfen, was sie bis dahin
  erhalten hat.

**Gerät aussperren (P3)**

- **FR-026**: Die Nutzerin MUSS ein anderes Gerät der Vault aussperren können.
  Das Aussperren MUSS eine neue Vault-Identität und damit eine neue
  Schlüsselgeneration des Bereichs „Vault“ erzeugen, die verbleibenden Geräte
  neu bestätigen, das ausgesperrte Gerät als ausgesperrt vermerken und die
  neue Identität in allen gemeinsamen Bereichen übergeben (FR-039). Vor der
  Bestätigung MUSS holzi die Folgen erklären (neue Identität, keine neuen Daten
  mehr für das Gerät, vorhandene Daten dort schützt nur die Passphrase,
  Bestätigung auf jedem verbleibenden Gerät nötig, Mitglieder gemeinsamer
  Spaces und Datenfreigaben müssen die neue Identität per Prüfcode
  bestätigen, nicht umkehrbar). Diese Spec ist die einzige, die den Wechsel
  der Vault-Identität regelt; Specs 026, 027 und 028 verweisen hierauf.
- **FR-027**: Ein Gerät, das noch die alte Vault-Identität hat, DARF die neue
  nur übernehmen, wenn die Nutzerin das auf diesem Gerät bestätigt, nachdem sie
  einen Prüfcode mit einem Gerät der neuen Identität abgeglichen hat. Eine mit
  der alten Identität signierte Übergabe allein DARF NICHT genügen, weil auch
  das ausgesperrte Gerät die alte Identität besitzt. Den privaten Schlüssel der
  neuen Identität erhält es nur auf einer direkten Verbindung (FR-038).
- **FR-028**: Nach dem Aussperren MUSS jedes Gerät der neuen Identität
  Verbindungen und Änderungspakete des ausgesperrten Geräts abweisen, egal
  welchen Zeitstempel diese tragen. Änderungen, die vorher angekommen waren,
  bleiben erhalten.
- **FR-039**: Nach einem Wechsel der Vault-Identität MUSS die Vault für jeden
  Bereich, dessen Mitglied oder Admin sie ist (Bereich eines Space oder einer
  Datenfreigabe, Specs 027, 028), eine Übergabe veröffentlichen: mit der alten
  Identität signiert, die neue Identität nennend. Sie geht auf denselben Wegen
  wie die übrigen Nachrichten des Bereichs (Postfach beim Relay, direkte
  Verbindungen zwischen Mitgliedern).
- **FR-040**: Ein Gerät einer anderen Vault, die Mitglied eines solchen
  Bereichs ist, DARF die neue Identität erst übernehmen, wenn sein Nutzer einen
  Prüfcode mit der Nutzerin der wechselnden Vault abgeglichen und das bestätigt
  hat, wie bei eigenen Geräten (FR-027); die Übergabe allein DARF NICHT
  genügen, weil auch der Dieb die alte Identität besitzt. Bis dahin MUSS es den
  offenen Wechsel anzeigen und DARF von der neuen Identität keine
  Änderungen, Mitgliederlisten oder Rechte annehmen. Nach der Bestätigung MUSS
  es eine mit seiner eigenen Vault-Identität signierte Annahme genau dieser
  neuen Identität in den Bereich hochladen. Eine Mitglieds-Vault DARF je
  Bereich und alter Identität nur eine Annahme ausstellen. Mitgliederlisten und
  Admin-Rechte der neuen Identität nimmt es erst nach FR-041 an.
- **FR-041** („Übernahme erst nach Mehrheitsbestätigung“): Eine Übergabe, die
  nur mit der alten Identität signiert ist, DARF NIE etwas umbinden. Mit der
  ersten Übergabe für einen Bereich MUSS das Relay (Spec 026) dort nichts mehr
  annehmen, was mit der alten Identität signiert ist, weil deren Schlüssel als
  kompromittiert gilt, und MUSS die Lesezugriffe der übrigen Mitglieder der
  letzten gültigen Mitgliederliste weiter bedienen. Ist die wechselnde Vault
  Admin eines Bereichs, dessen Kennung an ihre Identität gebunden ist (Spec 026
  FR-021), DARF das Relay die Admin-Rechte erst dann an eine neue Identität
  binden, wenn mehr als die Hälfte der übrigen Mitglieds-Vaults der letzten
  gültigen Mitgliederliste eine Annahme (FR-040) genau dieser neuen Identität
  hochgeladen hat. Konkurrieren mehrere Übergaben, gilt die neue Identität, die
  zuerst diese Mehrheit erreicht; weil jede Mitglieds-Vault nur eine Annahme
  ausstellt, kann das höchstens eine sein, und alle anderen MUSS das Relay
  abweisen. Dieselbe Regel MÜSSEN die Geräte der Mitglieder selbst anwenden:
  Mitgliederlisten und Admin-Rechte der neuen Identität nehmen sie erst an,
  wenn sie die Mehrheit der Annahmen selbst geprüft haben. Jeder Client, der
  zwei verschiedene Übergaben derselben alten Identität sieht, MUSS den
  Konflikt anzeigen und DARF keine der beiden von sich aus übernehmen. Ist die
  wechselnde Vault nur Mitglied, bindet das Relay nichts um; die neue Identität
  bekommt Zugang erst über eine neue Mitgliederliste eines Admins, der sie
  nach FR-040 übernommen hat. Ein Bereich ohne andere Mitglieder, etwa das
  Postfach der eigenen Vault im Bereich „Vault“, wird nie umgebunden; die
  Vault legt mit der neuen Identität ein neues Postfach und damit einen neuen
  Bereich an (Spec 026). Das verfeinert die Clarification, nach der zwei
  konkurrierende Übergaben den Bereich beim Relay einfrieren: Eingefroren ist
  ab der ersten Übergabe alles, was die alte Identität signiert, aufgelöst wird
  durch die Mehrheit der Annahmen.

**Verträglichkeit und Betrieb**

- **FR-029**: Geräte mit unverträglichen Versionen des Sync oder des
  Vault-Schemas DÜRFEN nicht miteinander synchronisieren. Beide MÜSSEN den
  Grund zeigen (FR-034), und keine Änderung DARF dadurch verloren gehen; nach
  dem Aktualisieren synchronisieren sie ohne weiteres Zutun.
- **FR-030**: Treten zwei Geräte mit demselben Geräteschlüssel oder derselben
  Gerätekennung auf, MUSS holzi den Sync mit beiden anhalten und der Nutzerin
  melden, dass ein Gerät doppelt vorhanden ist.
- **FR-031**: Sperren oder Schließen der Vault MUSS alle Sync-Verbindungen und
  laufenden Übertragungen sofort beenden (innerhalb der Fristen von Spec 013);
  danach DARF das Gerät für diese Vault keine Präsenz mehr erneuern und nichts
  mehr senden oder annehmen.
- **FR-032**: Der Sync MUSS im Hintergrund laufen, ohne die Bedienung zu
  blockieren; empfangene Änderungen MÜSSEN in offenen Fenstern und Tabs ohne
  Neuladen erscheinen.

**Föderation in den Einstellungen**

- **FR-033**: Die Geräteliste in der Unteransicht „Geräte“ der Kategorie
  „Föderation“ (Spec 023 FR-022) MUSS
  je anderem Gerät zeigen, ob es gerade verbunden ist („online“), sonst wann es
  zuletzt online war, oder „noch nie online gesehen“. „Zuletzt online“ ist der
  jüngste Zeitpunkt, den dieses Gerät von dem anderen kennt, aus eigener
  Verbindung, aus Präsenzmeldungen oder über ein drittes Gerät.
- **FR-034**: Die Liste MUSS sich live aktualisieren, wenn ein Gerät online oder
  offline geht, umbenannt wird, neu dazukommt oder ausgesperrt wird. Ein Gerät,
  mit dem nach FR-029 oder FR-030 kein Sync möglich ist, MUSS den Grund zeigen;
  ein ausgesperrtes Gerät ist als solches markiert.
- **FR-035**: Die Unteransicht „Geräte“ MUSS die Knöpfe „Gerät koppeln“
  (FR-023) und an jedem anderen Gerät „Gerät aussperren“ (FR-026) anbieten.
  Das Aussperren dieses Geräts selbst DARF NICHT angeboten werden.
- **FR-036**: Agenten mit Leserecht auf die Einstellungen DÜRFEN die Geräteliste
  samt Online-Stand abrufen. Kopplung und Aussperren DÜRFEN nur von der Nutzerin
  selbst ausgelöst werden, nie von Agenten oder Erweiterungen; Schlüssel,
  Gerätebestätigungen und Kopplungscodes DÜRFEN Agenten und Erweiterungen nie
  erreichen.
- **FR-037**: Alle neuen Texte (Stände, Meldungen, Erklärungen) MÜSSEN auf
  Deutsch und Englisch vorliegen (Spec 023 FR-020).

### Key Entities

- **Vault-Identität**: Schlüsselpaar der Vault; öffentlicher Teil, privater
  Teil (nur in der Vault), Generation. Mit dem Aussperren entsteht eine neue.
- **Geräteschlüssel**: Schlüsselpaar eines Geräts für eine Vault; bleibt auf
  diesem Gerät.
- **Gerätebestätigung**: von der Vault-Identität signiert; Vault, öffentlicher
  Geräteschlüssel, Netzwerkkennung, Ausstellungszeit.
- **Gerät der Vault** (erweitert aus Spec 023): Name, dieses Gerät ja/nein,
  Gerätebestätigung, online jetzt, zuletzt online, ausgesperrt ja/nein, Grund,
  falls kein Sync möglich ist.
- **Präsenzmeldung**: vom Gerät signiert, nur für die eigene Vault lesbar;
  Erreichbarkeit, Ablaufzeit.
- **Bereich**: Kennung und Art (in dieser Spec nur „Vault“).
- **Änderungspaket**: Bereich, Kennung des Inhaltsschlüssels, verschlüsselter
  Inhalt aus einer oder mehreren vollständigen Transaktionen mit Autor und
  Signatur je Änderung.
- **Inhaltsschlüssel**: Kennung, Schlüsselgeneration, Bereich.
- **Änderung**: Bereich, Ursprungsgerät, Vault des Autors, Laufnummer,
  Zeitstempel der hybriden logischen Uhr, Inhalt, Signatur des
  Ursprungsgeräts.
- **Fortschrittsstand**: je Ursprungsgerät die höchste Laufnummer, bis zu der
  alle Änderungen vorliegen; dazu die erkannten Lücken.
- **Mitgliederliste** (Grundlage für Specs 026–028): Bereich, Generation,
  Mitglieds-Vaults mit Rechten, bei einem Entzug die Grenze je Gerät der
  betroffenen Vault; von einem Admin-Gerät signiert.
- **Kopplungscode**: einmalig, kurzlebig; Erreichbarkeit des anbietenden Geräts
  und ein Geheimnis für den gegenseitigen Nachweis.
- **Übergabe**: alte und neue Vault-Identität, Bereich, Zeitpunkt; mit der alten
  Identität signiert. Je Mitglied: offen, angenommen oder im Konflikt; beim
  Relay: alte Identität gesperrt, umgebunden (sobald die Mehrheit erreicht ist)
  oder abgewiesen.
- **Annahme**: Mitglieds-Vault, Bereich, alte und neue Identität; mit der
  Vault-Identität des Mitglieds signiert, höchstens eine je Bereich und alter
  Identität.
- **Nur-direkt-Daten**: feste Liste von Vault-Geheimnissen (FR-038), die nie
  ein Relay und nie einen anderen Bereich erreichen.

## Success Criteria _(mandatory)_

### Measurable Outcomes

- **SC-001**: Sind zwei Geräte verbunden, erscheint eine Änderung in 95 % der
  Fälle innerhalb von 5 Sekunden auf dem anderen Gerät, im selben Netz wie über
  das Internet.
- **SC-002**: Zwei Geräte, die beide online sind, sind nach dem Öffnen der Vault
  innerhalb von 30 Sekunden verbunden, ohne Zutun der Nutzerin. Das gilt auch
  für eine frische Kopie der Vault-Datei und ihr Quellgerät, das bis dahin das
  einzige Gerät der Vault war.
- **SC-003**: In einer automatischen Prüfung mit drei Geräten und wechselnden,
  indirekten Wegen (mindestens 1.000 Änderungen) fehlt am Ende auf keinem Gerät
  eine Änderung, keine ist doppelt angewendet, alle Geräte haben denselben
  Stand, und 100 % der Änderungen nennen ihr wahres Ursprungsgerät.
- **SC-004**: Nach einer Trennung, in der ein Gerät N Änderungen erzeugt hat,
  überträgt das Wiederverbinden genau diese N Änderungen und keine, die das
  andere Gerät schon hat.
- **SC-005**: In 100 % der geprüften Fälle (fremde Vault, gefälschte oder fremd
  signierte Gerätebestätigung, fehlender Besitznachweis, ausgesperrtes Gerät)
  wird die Verbindung abgewiesen, bevor ein einziges Änderungspaket fließt.
- **SC-006**: Ein Mitschnitt des Verkehrs zwischen zwei Geräten enthält keinen
  Klartext von Vault-Inhalten, Tabellen- oder Spaltennamen; eine automatische
  Prüfung zeigt, dass gerätelokale Daten in keinem Paket und der private
  Schlüssel der Vault-Identität in keinem Protokoll vorkommen. Wohin die
  übrigen Vault-Geheimnisse dürfen, prüft SC-012.
- **SC-007**: Eine Vault mit nur einem Gerät veröffentlicht während einer
  ganzen Vault-Session keine Präsenzmeldung und baut keine direkte Verbindung
  im Bereich „Vault“ auf, solange sich kein anderes Gerät derselben Vault
  meldet; ihr einziger Präsenzverkehr ist das Lauschen auf Präsenzmeldungen an
  ihre Vault-Identität. Uploads in ein Relay-Postfach (Spec 026) und Verkehr
  mit Mitgliedern von Spaces und Datenfreigaben (Specs 027, 028) zählen nicht
  dazu.
- **SC-008**: Die Geräteliste zeigt einen Wechsel von online zu offline oder
  umgekehrt innerhalb von 60 Sekunden, eine Umbenennung innerhalb der Zeit aus
  SC-001.
- **SC-009**: Eine Kopplung dauert vom Anzeigen des Codes bis zum Beginn der
  Übertragung weniger als 2 Minuten; eine Nutzerin schafft sie beim ersten
  Versuch ohne Anleitung.
- **SC-010**: Nach dem Aussperren nimmt kein Gerät der neuen Identität eine
  einzige Verbindung oder Änderung des ausgesperrten Geräts mehr an, und die
  verbleibenden Geräte synchronisieren weiter.
- **SC-011**: Die Szenarien der User Stories 1 bis 3 laufen als automatische
  Tests mit mehreren App-Prozessen gegen die gebaute App (Spec 016) und
  bestehen.
- **SC-012**: Eine automatische Prüfung zeigt für jeden Eintrag der
  Nur-direkt-Daten (FR-038), dass er nur in Paketen auf direkten Verbindungen
  zwischen Geräten derselben Vault vorkommt: in 100 % der geprüften Fälle in
  keinem Paket und keiner Momentaufnahme für ein Postfach eines Relays (auch
  nicht das der eigenen Vault), in keinem Paket eines anderen Bereichs als
  „Vault“ und in keinem Protokoll. Die Prüfung schlägt fehl, sobald ein
  Eintrag der Liste auf einem dieser Wege auftaucht.
- **SC-013**: Nach einem Wechsel der Vault-Identität übernimmt in 100 % der
  geprüften Fälle kein Mitglied eines gemeinsamen Bereichs die neue Identität
  ohne bestätigten Prüfcode-Abgleich; ab der ersten Übergabe nimmt das Relay im
  Bereich nichts mehr an, was mit der alten Identität signiert ist; die
  Admin-Rechte gehen nie ohne Annahmen einer Mehrheit der übrigen Mitglieder
  auf eine neue Identität über; und bei zwei konkurrierenden Übergaben gilt bei
  Relay und Mitgliedern genau die Identität, die zuerst die Mehrheit erreicht,
  während jedes Mitglied bis dahin den Konflikt zeigt. Prüfbar, sobald Spaces
  bestehen (Spec 027).
- **SC-014**: In einer automatischen Prüfung, die Änderungen über mehrere Wege
  in vertauschter Reihenfolge zustellt, erkennt jedes Gerät in 100 % der Fälle
  jede Lücke, fordert die fehlenden Änderungen an und zählt keine Änderung
  jenseits einer Lücke zu seinem Fortschrittsstand; am Ende sind alle Lücken
  geschlossen. Jede zweite, andere Änderung unter einem schon belegten Paar aus
  Ursprungsgerät und Laufnummer wird verworfen.
- **SC-015**: Nachdem eine Mitgliederliste einer Vault ein Recht entzogen hat,
  lehnt jedes Gerät, das diese Liste kennt, in 100 % der geprüften Fälle alle
  Änderungen dieser Vault jenseits der Grenze ab, die das entzogene Recht
  brauchen, auch rückdatierte. Änderungen, die ein Gerät angewendet hatte,
  bevor es die Liste kannte, dürfen bleiben, bis eine berechtigte Änderung
  derselben Zelle sie überschreibt (hingenommenes Risiko, D19). Prüfbar, sobald
  Spaces bestehen (Spec 027).
- **SC-016**: Veröffentlichen zwei Admin-Geräte verschiedene Listen derselben
  Generation, wählen das Relay und alle Empfänger in 100 % der geprüften Fälle
  dieselbe Liste, unabhängig von der Reihenfolge des Eintreffens, und ein
  Admin-Gerät veröffentlicht danach eine zusammengeführte Liste der nächsten
  Generation. Prüfbar, sobald Spaces bestehen (Spec 027).
- **SC-017**: Eine Momentaufnahme mit einer ungültigen Änderung verliert beim
  Empfänger in 100 % der geprüften Fälle genau die Transaktionsgruppe dieser
  Änderung; alle anderen Gruppen kommen an, keine Gruppe zum Teil. Prüfbar,
  sobald es Momentaufnahmen gibt (Spec 026).

## Assumptions

- Vom Betreiber vorgegeben: Transport über iroh (direkte QUIC-Verbindungen, mit
  einem iroh-Relay nur für den Verbindungsaufbau durch NAT), Schlüssel und
  Präsenz im Nostr-Format (secp256k1). Die Präsenzmeldung ersetzt die übliche
  Adresssuche von iroh über pkarr/DNS (Entwurf §4). Wie der Sync-Kanal heißt
  (Arbeitsname `holzi-sync/1`) und wie Pakete und Meldungen kodiert sind,
  klärt der Plan (Entwurf §15 Punkt 7).
- Die Netzwerkkennung eines Geräts ist sein iroh-Schlüssel; die
  Gerätebestätigung bindet ihn an den Geräteschlüssel (Entwurf §4).
- Zusammenführen, Löschvermerke und die Ausnahme gerätelokaler Daten liefert
  haex-crdt, wie holzi es einbindet: Repository
  `https://github.com/haexmas/haex-crdt`, Revision
  `ed230d2c3f58c1b10710b6025ea0ce6c20b8d009`. Dass dort heute das
  weiterleitende statt des ursprünglichen Geräts als Autor eingetragen wird
  (Entwurf §3.2), wird dort behoben, nicht in holzi umgangen. Felder werden
  einzeln zusammengeführt, die jüngere Änderung gilt; Konflikte bei Dateien
  (Konfliktkopien, D10) gehören zu Spec 025.
- Innerhalb der eigenen Vault gibt es keine Rechte: Jedes bestätigte Gerät darf
  alles (Entwurf §6). Lesen, Schreiben, Löschen und Admin gibt es erst für
  Spaces und Datenfreigaben (027, 028).
- Die Passphrase gehört zur Datei auf einem Gerät. Eine kopierte Vault-Datei
  öffnet sich mit der bisherigen Passphrase; bei der Kopplung vergibt die
  Nutzerin auf dem neuen Gerät eine Passphrase, die gleich oder anders sein
  darf. Ein Wechsel der Passphrase ist nicht Teil dieser Spec.
- Die Kopplung überträgt den aktuellen Stand der Vault samt Löschvermerken,
  nicht jede frühere Fassung; danach läuft der gewöhnliche Sync.
- Beim Aussperren bestätigt die Nutzerin die neue Identität auf jedem
  verbleibenden Gerät durch Abgleich eines Prüfcodes. Das ist die sichere
  Antwort auf Entwurf §15 Punkt 4 für die eigenen Geräte: Der Dieb hat die alte
  Identität und könnte jede mit ihr signierte Übergabe selbst ausstellen. Für
  andere Nutzer, mit denen Spaces oder Datenfreigaben bestehen, gilt dasselbe:
  Sie erfahren die neue Identität durch eine Übergabe und übernehmen sie erst
  nach Abgleich eines Prüfcodes (FR-039 bis FR-041). Diese Spec regelt das für
  alle Bereiche; Specs 026, 027 und 028 bauen die Wege dafür.
- Mobile Geräte sind nur im Vordergrund erreichbar (v1-scope §7); das ist für
  diese Spec kein Fehlerfall, sondern ein Gerät, das gerade offline ist.
  holzi läuft heute nur auf dem Desktop.
- Die gerätelokalen Daten eines Quellgeräts liegen nach ADR-0001 physisch in
  einer kopierten Datei. FR-006 verlangt nur, dass das Zielgerät sie nicht als
  eigene Identität verwendet; ob der Plan den Geräteschlüssel deshalb außerhalb
  der Vault-Datei ablegt, entscheidet er.

## Nicht im Umfang

- Sync über ein Relay und Postfächer, auch für die eigene Vault; kommt mit
  Spec 026. Diese Spec legt nur fest, was nie in ein Postfach darf (FR-038)
  und wie das Relay Übergaben behandelt (FR-041). Bis dahin gleichen sich nur
  Geräte an, zwischen denen irgendwann ein Weg über gleichzeitig laufende
  Geräte besteht.
- Dateisync zwischen eigenen Geräten (Spec 025), Spaces (027),
  Datenfreigaben (028), eigener S3-Speicher (029).
- Rechte zwischen eigenen Geräten; die Fähigkeiten `pairing-authority` und
  `confirmation-authority` aus v1-scope §4.
- Ein Schalter, der den Sync auf einem Gerät pausiert, und die Wahl einzelner
  Daten, die nicht synchronisiert werden sollen.
- Geräte ohne Aussperren aus der Liste entfernen oder umbenennen (außer diesem
  Gerät, Spec 023); ein Gerät, das die Nutzerin nicht mehr braucht, löscht die
  Vault dort selbst.
- Das Aussperren rückgängig machen; ein ausgesperrtes Gerät kommt nur durch eine
  neue Kopplung zurück.
- Wiederherstellung, wenn alle Geräte verloren sind; dafür bleibt eine Kopie der
  Vault-Datei.
- Verstecken, welche Geräte zu einer Vault gehören, vor den genutzten Servern
  (Pseudonyme, D9) und Schutz vor Verkehrsanalyse.
- Eine Übersicht des Sync-Fortschritts je Datenart oder ein Protokoll
  einzelner übertragener Änderungen.
