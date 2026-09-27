# Feature Specification: Eigener S3-Speicher für Spaces

**Feature Branch**: `029-own-s3-storage`
**Created**: 2026-09-28
**Status**: Draft
**Input**: Zeile 029 des Spec-Schnitts im Sync-Entwurf
[`docs/plans/2026-09-28-sync-architecture.md`](../../docs/plans/2026-09-28-sync-architecture.md)
(§12 Speicher-Backend B, §13 Bedrohungsmodell, Entscheidungen D11 und D12):
Der Admin eines Space verbindet seinen eigenen S3-kompatiblen Speicher. Je
Space gibt es einen Bucket und zwei eingeschränkte Zugangsschlüssel (nur Lesen,
Lesen und Schreiben), die der Admin in seiner Vault erzeugt und verschlüsselt an
die Mitglieder gibt, je nach Fähigkeit. Das Relay bekommt nie Zugangsdaten. Die
Geräte greifen direkt auf den Speicher zu. Beim Entzug von Rechten werden die
Zugangsschlüssel erneuert. Weil S3 „nur eigene Dateien löschen“ nicht
durchsetzen kann, MUSS die Versionierung des Buckets eingeschaltet sein. Kann ein
Anbieter keine eingeschränkten Zugangsschlüssel erzeugen, stellt ein Gerät des
Admins kurzlebige Zugangslinks aus, solange es online ist.

## Begriffe

- **Space**, **Admin**, **Fähigkeiten Lesen/Schreiben/Löschen**: wie in Spec 027.
  Ein Space ist ein Netzwerkordner für Dateien; Admin ist nur, wer ihn angelegt
  hat.
- **Dateiindex**, **Objekt**, **Konfliktkopie**: wie in Spec 025. Ein Objekt ist
  der verschlüsselte, unveränderliche Inhalt einer Datei, benannt nach dem Hash
  seines Chiffretexts. Wer eine Datei ändert, schreibt ein neues Objekt; alte
  Objekte werden aufgeräumt.
- **Speicher-Backend A**: Speicher, den das Relay bereitstellt (Spec 026).
- **Speicher-Backend B**: der eigene S3-kompatible Speicher eines Nutzers; Thema
  dieser Spec.
- **Anbieter**: der Dienst, bei dem der eigene Speicher liegt (zum Beispiel ein
  Cloud-Anbieter oder ein selbst betriebener MinIO-Server).
- **Speicherverbindung**: Endpunkt, Region, Anbieter und die
  **Hauptzugangsdaten** des Admins bei diesem Anbieter. Die Hauptzugangsdaten
  dürfen Buckets anlegen und Zugangsschlüssel erzeugen und widerrufen. Sie liegen
  nur in der Vault des Admins.
- **Space-Bucket**: der eine Bucket, der zu genau einem Space gehört und nur
  dessen Objekte enthält.
- **Zugangsschlüssel**: ein vom Anbieter ausgestellter Schlüssel, der nur für
  einen Space-Bucket gilt, in zwei Arten: **nur Lesen** und **Lesen und
  Schreiben**. Jede Erneuerung erzeugt eine neue **Generation** des
  Zugangsschlüssels; ältere Generationen werden widerrufen.
- **Eignungsprüfung**: die Prüfung, ob ein Anbieter mit den eingegebenen
  Hauptzugangsdaten alles kann, was diese Spec verlangt.
- **Versionierung**: die Fähigkeit eines Buckets, gelöschte oder überschriebene
  Objekte als ältere Versionen aufzubewahren. Die **Aufbewahrungsfrist** legt
  fest, wie lange ältere Versionen bleiben.
- **Zugangslink**: ein vom Anbieter signierter, kurzlebiger Link für genau ein
  Objekt und genau eine Art Zugriff (lesen, hochladen oder löschen). Ein Gerät des
  Admins stellt ihn aus, wenn der Anbieter keine eingeschränkten
  Zugangsschlüssel kann (Ersatzweg).
- **Relay**, **Postfach**, **Mitgliederliste**: wie in Spec 026. Das Relay gilt
  als nicht vertrauenswürdig.
- **Vault-Identität**, **Geräteschlüssel**, **Gerätebestätigung**: wie in
  Spec 024.
- **Inhaltsschlüssel**, **Schlüsselgeneration**: wie in Spec 024 und 027. Ein
  Zugangsschlüssel ist kein Inhaltsschlüssel: Er öffnet den Speicher, nicht die
  Dateien.

## Beziehung zu bestehenden Specs

- Sync-Entwurf
  [`docs/plans/2026-09-28-sync-architecture.md`](../../docs/plans/2026-09-28-sync-architecture.md):
  Diese Spec setzt §12 B um. Sie ersetzt die Speicher-Zugriffsschicht mit
  IAM-Beiwerk des zurückgestellten Entwurfs
  [`2026-09-07-cross-user-sharing-deferred-design.md`](../../docs/plans/2026-09-07-cross-user-sharing-deferred-design.md)
  (§5 dort), wie der Sync-Entwurf es festhält.
- [`024-own-device-sync`](../024-own-device-sync/spec.md) (Vault-Identität,
  Geräte, Sync zwischen eigenen Geräten): Die Hauptzugangsdaten des Admins und
  geöffnete Zugangsschlüssel gehören zu den Nur-direkt-Daten aus Spec 024: Sie
  reisen nur über direkte Verbindungen zwischen Geräten derselben Vault, nie über
  ein Postfach des Relays (auch nicht das der eigenen Vault) und nie in einen
  anderen Bereich. Die Umschläge für Zugangsschlüssel sind an die Vault-Identität
  der Mitglieder adressiert, wie jeder andere Schlüsselumschlag; jedes Gerät
  öffnet sie selbst.
- [`025-own-device-file-sync`](../025-own-device-file-sync/spec.md) (Dateisync
  zwischen eigenen Geräten): Die eigenen synchronisierten Ordner einer Vault
  können statt direkter Übertragung und Backend A ebenfalls Backend B nutzen
  (User Story 6). Dateiindex und Objekte bleiben unverändert; diese Spec ändert
  nur, wo die Objekte liegen.
- [`026-blind-relay`](../026-blind-relay/spec.md) (Relay und Backend A):
  Bei Backend B ist das Relay an Dateien nicht beteiligt. Das Postfach des Space
  (Dateiindex, Mitgliederliste, Schlüsselumschläge) liegt unabhängig davon auf
  dem Relay des Admins, wenn eines eingerichtet ist (Spec 026 und 027); ohne
  Relay synchronisiert der Space das Postfach nur direkt zwischen den Geräten der
  Mitglieder (Spec 027). Backend B ändert nur, wo die Objekte liegen.
- [`027-spaces`](../027-spaces/spec.md) (Spaces): Spec 027 legt Mitglieder,
  Fähigkeiten, Einladungen, Entzug und das Erneuern des Inhaltsschlüssels fest.
  Diese Spec hängt sich an diese Ereignisse: Jede Einladung, jede
  Rechteänderung und jedes Entfernen verteilt oder erneuert zusätzlich die
  Zugangsschlüssel (FR-015 bis FR-021). Für Backend B erfüllt das Erneuern der
  Zugangsschlüssel die Forderung von Spec 027, dass das Speicher-Backend
  entfernte Mitglieder abweist (FR-023 dort), mit der Frist aus SC-002. Die
  Löschregeln (Schreiben löscht nur eigene Dateien, Löschen löscht alle) und
  deren Prüfung beim Empfänger bleiben in Spec 027. Spec 027 verlangt beim
  Anlegen eine Wahl zwischen Backend A und „nur direkte Übertragung“ (FR-003
  dort); die Wahl ist Pflicht, einen Standard gibt es nicht. Diese Spec fügt
  Backend B als dritte Wahl hinzu und hebt den Ausschluss „Speicher-Backend eines
  bestehenden Space wechseln“ aus Spec 027 auf (User Story 8). Einen Space
  aufzulösen bleibt auch hier ausgeschlossen. Für den Ersatzweg (User Story 5)
  nutzt diese Spec die direkte Verbindung zwischen Mitgliedern aus Spec 027.
- [`028-data-shares`](../028-data-shares/spec.md) (Datenfreigaben): nicht
  betroffen. Datenfreigaben haben keine Objekte und nutzen keinen Speicher.
- [`023-settings-app`](../023-settings-app/spec.md): Die Einstellungen dieser
  Spec liegen in der Kategorie „Föderation“. Den Speicher eines Space legt der
  Admin in der Detailansicht des Space in der Unteransicht „Spaces“ fest (Spec
  027), den Speicher der eigenen Ordner in der Unteransicht „Ordner“ (Spec 025).
  Die Einstellung gilt nach der Auswahl ohne Knopf zum Übernehmen (FR-021 dort).
  Wo die gespeicherten Speicherverbindungen verwaltet werden, klärt der Plan.
- Referenz (gelesen, nicht portiert): der Dateisync von haex-vault, Repository
  `https://github.com/haex-space/haex-vault`, Revision
  `8dce379d94e18fcd42c3b73686a06f984ca3f574`, Pfad `src-tauri/src/file_sync/`.
  Dort hält der Server volle Zugangsdaten eines Buckets je Nutzer; diese Spec
  verlangt das Gegenteil.

## Clarifications

### Session 2026-09-28

- Q: Darf das Relay die Zugangsdaten eines eigenen S3-Speichers halten? → A:
  Nein (D11). Das Relay ist nicht vertrauenswürdig. Es bekommt weder die
  Hauptzugangsdaten noch einen Zugangsschlüssel in lesbarer Form.
- Q: Kommen beide Speicher-Backends in v1? → A: Ja (D12): Speicher des Relays (A)
  und eigener S3-Speicher (B).
- Q: (Betreiber) „Relay braucht dann vollen S3-Zugriff?“ → A: Nein. Bei eigenem
  S3 gibt der Admin den Mitgliedern auf den Space-Bucket beschränkte
  Zugangsschlüssel, verschlüsselt an ihre Vaults. Die Geräte greifen damit direkt
  auf den Speicher zu, das Relay ist für Dateien nicht beteiligt. Nur bei
  Backend A ist das Relay selbst der Speicheranbieter; dort liegen ohnehin nur
  Chiffretexte.
- Q: Wie viele Buckets und Schlüssel je Space? → A: Ein Bucket je Space und zwei
  Zugangsschlüssel, nur Lesen und Lesen und Schreiben (Entwurf §12 B).
- Q: Wie schützt der Space sich vor einem Mitglied mit Schreibrecht, das fremde
  Objekte direkt im Speicher löscht? → A: Gar nicht vorab, denn S3 kennt kein
  „nur eigene Objekte löschen“. Deshalb MUSS die Einrichtung die Versionierung
  einschalten; gelöschte Objekte werden aus älteren Versionen zurückgeholt
  (Entwurf §12 B).
- Q: Gehört der Wechsel eines Space zwischen Backend A und B in diese Spec? → A:
  Ja, als P3 (User Story 8). Begründung: Objekte sind in beiden Backends
  identisch, also ist ein Wechsel ein Kopieren und Umschalten, ohne neue
  Verschlüsselung. Ohne Wechsel bliebe die Wahl beim Anlegen endgültig, denn
  Spec 027 kennt weder einen Wechsel noch das Auflösen eines Space. Weil der
  Wechsel nicht nötig ist, um Backend B überhaupt zu nutzen, steht er hinten.
- Q: Wer darf den Speicher eines Space verbinden oder wechseln? → A: Nur der
  Admin (D6). Mitglieder wählen keinen Speicher; sie nutzen, was der Admin
  festgelegt hat.

## User Scenarios & Testing _(mandatory)_

### User Story 1 - Eigenen Speicher für einen Space verbinden (Priority: P1)

Eine Nutzerin legt einen Space für ihre Familie an und möchte die Fotos nicht
auf dem Speicher des Relays ablegen, sondern bei ihrem eigenen Anbieter. Sie gibt
Endpunkt, Region und ihre Hauptzugangsdaten ein. holzi prüft den Anbieter, legt
einen Bucket für den Space an, schaltet die Versionierung ein und erzeugt die
beiden Zugangsschlüssel. Ab dann liegen die Objekte des Space in ihrem Bucket.

**Why this priority**: Das ist der Kern der Spec. Ohne Verbindung gibt es kein
Backend B.

**Independent Test**: Gegen einen S3-kompatiblen Testanbieter mit
Versionierung und eingeschränkten Schlüsseln einen Space anlegen und den
Speicher verbinden: Es gibt genau einen neuen Bucket mit eingeschalteter
Versionierung und zwei Zugangsschlüssel, die nur für diesen Bucket gelten. Eine
Datei im Space landet als Objekt in diesem Bucket, nicht auf dem Relay.

**Acceptance Scenarios**:

1. **Given** die Nutzerin ist Admin eines Space, **When** sie in der
   Detailansicht des Space „Eigenen Speicher verbinden“ wählt und Endpunkt, Region und Hauptzugangsdaten eingibt, **Then**
   führt holzi die Eignungsprüfung durch, bevor irgendetwas angelegt oder
   gespeichert wird.
2. **Given** die Eignungsprüfung ist bestanden, **When** die Nutzerin bestätigt,
   **Then** legt holzi einen Bucket an, dessen Name den Namen des Space nicht
   verrät, schaltet die Versionierung und die Aufbewahrungsfrist ein, erzeugt
   beide Zugangsschlüssel und zeigt den Space als „Speicher: eigener S3“.
3. **Given** die Hauptzugangsdaten sind falsch oder der Endpunkt ist nicht
   erreichbar, **When** die Prüfung läuft, **Then** nennt holzi den Grund
   verständlich, speichert keine Zugangsdaten und legt nichts an.
4. **Given** die Nutzerin hat schon eine Speicherverbindung gespeichert, **When**
   sie einen weiteren Space anlegt, **Then** kann sie diese Verbindung wählen,
   ohne die Zugangsdaten erneut einzugeben, und der neue Space bekommt einen
   eigenen Bucket.
5. **Given** ein Mitglied ohne Admin-Rolle öffnet den Space, **When** es die
   Speicherangaben ansieht, **Then** sieht es, dass der Space eigenen S3-Speicher
   nutzt, aber keine Möglichkeit, ihn zu ändern, und keine Hauptzugangsdaten.

---

### User Story 2 - Mitglieder greifen direkt auf den Speicher zu (Priority: P1)

Ein Familienmitglied mit Fähigkeit Lesen öffnet den Space und lädt Fotos
herunter; seine Schwester mit Schreibrecht legt neue Fotos hinein. Beide greifen
direkt auf den Bucket zu, mit dem Zugangsschlüssel, den der Admin ihnen passend
zu ihrer Fähigkeit gegeben hat. Das Relay sieht davon nichts, und der Anbieter
sieht nur unlesbare Objekte.

**Why this priority**: Ein verbundener Speicher, den die Mitglieder nicht
erreichen, bringt nichts. Und hier muss das Versprechen „Relay und Anbieter sehen
keine Inhalte“ halten.

**Independent Test**: Ein Mitglied mit Lesen und eines mit Schreiben einladen.
Das Lesen-Mitglied lädt Dateien herunter, sein Versuch hochzuladen oder zu
löschen scheitert am Speicher. Das Schreiben-Mitglied lädt eine Datei hoch. Die
Protokolle und der Speicher des Relays enthalten keinen Zugangsschlüssel und keine
Übertragung dieser Objekte. Eine Durchsicht des Buckets findet keinen Dateinamen
und keinen Klartext.

**Acceptance Scenarios**:

1. **Given** der Admin lädt ein Mitglied mit Lesen ein, **When** die Einladung
   angenommen ist, **Then** erhält die Vault des Mitglieds den Zugangsschlüssel
   „nur Lesen“ und keinen anderen.
2. **Given** der Admin lädt ein Mitglied mit Schreiben oder Löschen ein, **When**
   die Einladung angenommen ist, **Then** erhält die Vault des Mitglieds den
   Zugangsschlüssel „Lesen und Schreiben“.
3. **Given** ein Mitglied hat einen Zugangsschlüssel, **When** eines seiner
   Geräte eine Datei des Space lädt oder hochlädt, **Then** geht die Übertragung
   direkt zwischen Gerät und Anbieter, nicht über das Relay.
4. **Given** ein Mitglied mit „nur Lesen“, **When** es versucht, ein Objekt
   hochzuladen oder zu löschen, **Then** lehnt der Anbieter ab.
5. **Given** ein Mitglied hat mehrere Geräte, **When** ein Gerät den Umschlag
   mit dem Zugangsschlüssel erhalten hat, **Then** kommt der Umschlag über den
   Sync zwischen eigenen Geräten (Spec 024) auf die anderen Geräte, und jedes
   Gerät öffnet ihn selbst, ohne dass der Admin etwas tun muss.
6. **Given** der Bucket enthält Objekte des Space, **When** jemand mit vollem
   Zugriff den Bucket durchsieht, **Then** findet er nur Objekte mit Namen aus
   Hashes, ohne Dateinamen, Pfade, Ordnerstruktur oder lesbaren Inhalt.

---

### User Story 3 - Entzug erneuert die Zugangsschlüssel (Priority: P1)

Der Admin entfernt ein Mitglied aus dem Space. holzi erneuert sofort den
Zugangsschlüssel, den das Mitglied hatte, widerruft den alten beim Anbieter und
gibt den neuen allen verbleibenden Mitgliedern mit derselben Fähigkeit. Der alte
Schlüssel in der Vault des entfernten Mitglieds öffnet den Bucket nicht mehr.

**Why this priority**: Ein entferntes Mitglied darf nicht weiter Objekte laden
oder löschen können. Ohne Erneuerung wäre Entzug bei Backend B wirkungslos.

**Independent Test**: Zwei Mitglieder mit Lesen einladen, eines entfernen: Der
alte Zugangsschlüssel „nur Lesen“ wird vom Anbieter abgelehnt, das verbleibende
Mitglied liest mit dem neuen weiter. Ein Mitglied von Schreiben auf Lesen
zurückstufen: Es kann nicht mehr hochladen, aber weiter lesen.

**Acceptance Scenarios**:

1. **Given** der Admin entfernt ein Mitglied, **When** das Entfernen wirksam
   ist, **Then** erzeugt holzi eine neue Generation des Zugangsschlüssels, den das
   Mitglied hatte, gibt sie allen verbleibenden Mitgliedern mit dieser Art und
   widerruft die alte Generation beim Anbieter.
2. **Given** dieselbe Lage, **When** das entfernte Mitglied danach mit dem alten
   Schlüssel auf den Bucket zugreift, **Then** lehnt der Anbieter ab (SC-002).
3. **Given** der Admin stuft ein Mitglied von Schreiben oder Löschen auf Lesen
   zurück, **When** die Änderung wirksam ist, **Then** wird der Zugangsschlüssel
   „Lesen und Schreiben“ erneuert und nur an die verbleibenden Schreibenden
   verteilt, und das zurückgestufte Mitglied erhält den aktuellen
   Zugangsschlüssel „nur Lesen“.
4. **Given** der Admin stuft ein Mitglied von Lesen auf Schreiben hoch, **When**
   die Änderung wirksam ist, **Then** erhält das Mitglied den aktuellen
   Zugangsschlüssel „Lesen und Schreiben“, ohne dass ein Zugangsschlüssel
   erneuert wird.
5. **Given** ein Mitglied lädt gerade ein Objekt, **When** es entfernt wird,
   **Then** darf die laufende Übertragung scheitern; was es schon geladen hat,
   behält es (Spec 027).
6. **Given** die Erneuerung ist abgeschlossen, **When** die verbleibenden
   Mitglieder auf den Bucket zugreifen, **Then** nutzen sie ohne eigenes Zutun die
   neue Generation.

---

### User Story 4 - Gelöschte Objekte zurückholen (Priority: P2)

Ein Mitglied mit Schreibrecht löscht mit seinem Zugangsschlüssel direkt im
Bucket Objekte, die andere hochgeladen haben, am Dateiindex vorbei. Die
Einträge im Dateiindex bestehen weiter. holzi bemerkt auf einem Gerät des Admins,
dass Objekte fehlen, und holt sie aus den älteren Versionen des Buckets zurück.

**Why this priority**: Das ist die Absicherung gegen die Grenze von S3. Sie ist
nötig, damit „Schreiben löscht nur eigene Dateien“ bei Backend B mehr als ein
Versprechen ist, aber erst relevant, wenn ein Mitglied sich falsch verhält.

**Independent Test**: Mit dem Zugangsschlüssel „Lesen und Schreiben“ ein Objekt
direkt im Bucket löschen, das ein anderes Mitglied hochgeladen hat: Ein Gerät des
Admins meldet das fehlende Objekt und stellt es wieder her; die Datei ist danach
für alle Mitglieder wieder lesbar.

**Acceptance Scenarios**:

1. **Given** ein gültiger Eintrag im Dateiindex verweist auf ein Objekt, das im
   Bucket fehlt, **When** ein Gerät des Admins den Bucket prüft, **Then** stellt
   es das Objekt aus der jüngsten älteren Version wieder her und zeigt dem Admin
   einen Hinweis, welche Dateien betroffen waren.
2. **Given** ein Objekt wurde mit anderem Inhalt überschrieben, **When** ein
   Gerät es lädt, **Then** erkennt es an dem nicht passenden Hash, dass es nicht
   stimmt, verwendet es nicht, und ein Gerät des Admins stellt die passende ältere
   Version wieder her.
3. **Given** ein Mitglied hat eine Datei rechtmäßig gelöscht (Spec 027), **When**
   das zugehörige Objekt aufgeräumt wird, **Then** stellt holzi es nicht wieder
   her.
4. **Given** die Aufbewahrungsfrist eines gelöschten Objekts ist abgelaufen,
   **When** der Admin die betroffene Datei wiederherstellen will, **Then** meldet
   holzi, dass keine ältere Version mehr vorhanden ist, und nennt die Datei als
   verloren, sofern kein Gerät sie noch hat.
5. **Given** der Admin öffnet die Speicherangaben des Space, **When** er die
   Aufbewahrungsfrist ändert, **Then** gilt die neue Frist für den Bucket, und
   holzi zeigt, dass Löschungen nur innerhalb dieser Frist rückgängig zu machen
   sind.

---

### User Story 5 - Anbieter ohne eingeschränkte Zugangsschlüssel (Priority: P2)

Ein Nutzer betreibt einen S3-kompatiblen Speicher, der Versionierung kann, aber
keine auf einen Bucket beschränkten Schlüssel ausstellt. holzi sagt ihm das bei
der Einrichtung und bietet den Ersatzweg an: Die Mitglieder bekommen keinen
Zugangsschlüssel, sondern fragen ein Gerät des Admins über die direkte
Verbindung zwischen Mitgliedern (Spec 027) nach kurzlebigen Zugangslinks. Das funktioniert nur, solange
ein Gerät des Admins online ist.

**Why this priority**: Ohne diesen Weg wären Anbieter ohne eingeschränkte
Schlüssel für Spaces mit Mitgliedern ganz ausgeschlossen. Der Weg hat aber eine
spürbare Grenze und ist deshalb nicht der Normalfall.

**Independent Test**: Gegen einen Testanbieter ohne eingeschränkte Schlüssel
einrichten: Die Prüfung meldet das und bietet den Ersatzweg an. Ein Mitglied lädt
eine Datei über einen Zugangslink, solange ein Gerät des Admins online ist; ist
keines online, sieht das Mitglied, warum der Speicher gerade nicht erreichbar
ist.

**Acceptance Scenarios**:

1. **Given** die Eignungsprüfung stellt fest, dass der Anbieter keine
   eingeschränkten Zugangsschlüssel erzeugen kann, **When** sie das Ergebnis
   zeigt, **Then** erklärt holzi den Unterschied: Zugriff für Mitglieder nur,
   solange ein Gerät des Admins online ist.
2. **Given** der Admin wählt den Ersatzweg, **When** ein Mitglied ein Objekt
   braucht, **Then** fragt sein Gerät ein online erreichbares Gerät der
   Admin-Vault über die direkte Verbindung nach einem Zugangslink und lädt das
   Objekt damit direkt beim Anbieter.
3. **Given** ein Gerät des Admins erhält eine Anfrage, **When** es sie prüft,
   **Then** stellt es nur Links aus, die zur aktuellen Fähigkeit des anfragenden
   Mitglieds laut Mitgliederliste passen, und keine Links zum Löschen älterer
   Versionen oder zum Ändern des Buckets.
4. **Given** kein Gerät der Admin-Vault ist online, **When** ein Mitglied ein
   Objekt braucht, **Then** zeigt holzi, dass der Speicher nur erreichbar ist,
   wenn ein Gerät des Admins online ist, und bezieht das Objekt, wenn möglich, von
   einem anderen Gerät, das es hat (Spec 025 und 027).
5. **Given** ein Mitglied wird entfernt, **When** es danach um einen Zugangslink
   bittet, **Then** lehnt das Gerät des Admins ab; bereits ausgestellte Links
   laufen spätestens nach ihrer Gültigkeitsdauer ab.

---

### User Story 6 - Eigene synchronisierte Ordner auf eigenem Speicher (Priority: P2)

Ein Nutzer synchronisiert Ordner zwischen seinen eigenen Geräten (Spec 025) und
möchte, dass die Dateien auch dann ankommen, wenn seine Geräte nicht
gleichzeitig online sind, ohne den Speicher des Relays zu nutzen. Er verbindet
dafür seinen eigenen S3-Speicher. Weil nur seine eigenen Geräte zugreifen,
braucht es keine eingeschränkten Zugangsschlüssel.

**Why this priority**: Das nutzt dieselbe Einrichtung ohne Mitglieder und ohne
Schlüsselverteilung. Es ist wertvoll, aber nicht nötig, um Spaces mit eigenem
Speicher zu betreiben.

**Independent Test**: Auf Gerät 1 den eigenen Speicher für die eigenen Ordner
verbinden, Gerät 2 einmal direkt mit Gerät 1 verbinden (dabei erhält es die
Speicherverbindung), Gerät 2 beenden. Dann auf Gerät 1 eine Datei ablegen,
Gerät 1 beenden, danach Gerät 2 starten: Die Datei kommt aus dem Bucket an,
obwohl beide seit dem Ablegen nie gleichzeitig online waren.

**Acceptance Scenarios**:

1. **Given** der Nutzer hat eigene synchronisierte Ordner, **When** er in den
   Einstellungen unter „Föderation“ → „Ordner“ den eigenen Speicher dafür
   verbindet, **Then** legt holzi einen Bucket für die eigenen Ordner an und
   nutzt ihn von allen eigenen Geräten aus.
2. **Given** dieselbe Einrichtung, **When** ein weiteres eigenes Gerät sich
   direkt mit einem Gerät verbindet, das die Speicherverbindung hat, **Then**
   erhält es sie über diese direkte Verbindung (Spec 024, Nur-direkt-Daten) und
   greift ohne weitere Eingabe zu.
3. **Given** der Anbieter kann keine Versionierung, **When** der Nutzer ihn für
   die eigenen Ordner verbindet, **Then** warnt holzi, dass gelöschte Objekte
   nicht wiederherstellbar sind, erlaubt die Einrichtung aber.
4. **Given** der Anbieter kann keine eingeschränkten Zugangsschlüssel, **When**
   der Nutzer ihn für die eigenen Ordner verbindet, **Then** ist das kein
   Hindernis, und es gibt keinen Hinweis auf den Ersatzweg.

---

### User Story 7 - Bucket aufgeben und Zugangsdaten aufräumen (Priority: P2)

Die Admin zieht einen Space von ihrem eigenen Speicher weg (User Story 8) oder
nutzt ihn nicht mehr für ihre eigenen Ordner. holzi widerruft alle
Zugangsschlüssel dieses Buckets beim Anbieter und fragt, ob der Bucket samt aller
älteren Versionen gelöscht werden soll. Braucht keiner ihrer Spaces und nicht ihre
eigenen Ordner die Speicherverbindung mehr, kann sie sie entfernen; danach liegen
die Hauptzugangsdaten auf keinem ihrer Geräte mehr.

**Why this priority**: Zugangsschlüssel, die weiter gelten, obwohl niemand den
Bucket mehr nutzt, sind ein offenes Tor, und Hauptzugangsdaten, die ohne Zweck in
der Vault liegen, ein unnötiges Risiko. Das Aufräumen gehört zur Sicherheit,
auch wenn es selten vorkommt.

**Independent Test**: Die eigenen Ordner vom eigenen Speicher auf direkte
Übertragung zurückstellen: holzi fragt nach dem Bucket; mit „Bucket löschen“ ist
er weg, ohne bleibt er bestehen. Danach die Speicherverbindung entfernen: Keine
Vault-Datei der eigenen Geräte enthält noch die Hauptzugangsdaten. Für einen
Space dasselbe nach einem Umzug nach User Story 8: Beide Zugangsschlüssel werden
vom Anbieter abgelehnt.

**Acceptance Scenarios**:

1. **Given** ein Space oder die eigenen Ordner hören auf, einen Bucket zu nutzen,
   **When** das wirksam ist, **Then** widerruft holzi alle Generationen aller
   Zugangsschlüssel dieses Buckets beim Anbieter.
2. **Given** dieselbe Lage, **When** holzi nach dem Bucket fragt, **Then** kann
   die Admin wählen, ihn samt aller älteren Versionen zu löschen oder ihn zu
   behalten; Standard ist Behalten.
3. **Given** die Admin will eine Speicherverbindung entfernen, **When** noch ein
   Space oder die eigenen Ordner sie nutzen, **Then** verhindert holzi das und
   nennt, wer sie noch nutzt.
4. **Given** eine Speicherverbindung wird entfernt, **When** das Entfernen
   wirksam ist, **Then** enthält keine Vault-Datei eines eigenen Geräts mehr die
   Hauptzugangsdaten; ein Löschvermerk des Syncs trägt höchstens die Kennung des
   Eintrags.
5. **Given** der Widerruf beim Anbieter scheitert (etwa ohne Netz), **When** die
   Admin den Bucket aufgibt, **Then** merkt holzi sich den ausstehenden Widerruf,
   versucht ihn erneut, sobald der Anbieter erreichbar ist, und zeigt ihn als
   offen an, bis er gelungen ist.

---

### User Story 8 - Speicher eines Space wechseln (Priority: P3)

Ein Admin hat einen Space auf dem Speicher des Relays angelegt und möchte ihn in
seinen eigenen S3-Speicher umziehen, oder umgekehrt. holzi kopiert alle Objekte,
auf die der Dateiindex verweist, in den neuen Speicher, schaltet den Space erst
danach um und räumt den alten Speicher auf.

**Why this priority**: Nützlich, weil sonst die erste Wahl endgültig wäre. Aber
nicht nötig, um Backend B zu nutzen (siehe Clarifications).

**Independent Test**: Einen Space mit Dateien auf Backend A auf Backend B
umziehen (von „nur direkte Übertragung“ ebenso): Danach sind alle Dateien für
alle Mitglieder lesbar, neue Objekte landen im Bucket, und der Speicher des
Relays enthält keine Objekte des Space mehr. Dasselbe in die andere Richtung.

**Acceptance Scenarios**:

1. **Given** ein Space auf Backend A, **When** der Admin den Wechsel auf eigenen
   Speicher startet, **Then** laufen Eignungsprüfung und Einrichtung wie in User
   Story 1, danach kopiert ein Gerät des Admins alle Objekte, auf die gültige
   Einträge verweisen.
2. **Given** das Kopieren läuft, **When** Mitglieder den Space nutzen, **Then**
   lesen und schreiben sie weiter im bisherigen Speicher; neue Objekte kopiert
   holzi ebenfalls mit.
3. **Given** alle Objekte sind kopiert und geprüft, **When** holzi umschaltet,
   **Then** nutzen alle Mitglieder ab da den neuen Speicher, und der alte wird
   aufgeräumt (Backend A: Objekte gelöscht; Backend B: Zugangsschlüssel
   widerrufen, Bucket nach Wahl des Admins).
4. **Given** das Kopieren bricht ab, **When** der Admin es erneut startet,
   **Then** setzt es fort, ohne schon kopierte Objekte erneut zu übertragen, und
   der Space bleibt bis zum Umschalten im alten Speicher.

---

### Edge Cases

- **Zugangsschlüssel gelangt nach außen**: Der Admin kann jeden
  Zugangsschlüssel jederzeit von Hand erneuern (FR-020). Wer einen Schlüssel
  „nur Lesen“ hat, bekommt nur Chiffretexte, die er ohne Inhaltsschlüssel nicht
  öffnen kann. Wer „Lesen und Schreiben“ hat, kann Objekte löschen (zurückholbar
  nach User Story 4) oder unbrauchbare Objekte hochladen (am Hash erkannt, beim
  Aufräumen entfernt), aber keine Datei fälschen.
- **Mitglied wird während eines Downloads entfernt**: Die laufende Übertragung
  darf mit dem Widerruf scheitern. Über den Ersatzweg ausgestellte Links bleiben
  bis zu ihrem Ablauf gültig (höchstens 15 Minuten, FR-024).
- **Anbieter ohne Versionierung**: Für einen Space mit Mitgliedern lehnt holzi
  die Einrichtung ab und nennt den Grund (FR-006). Für die eigenen Ordner warnt
  holzi nur (User Story 6).
- **Versionierung wird später beim Anbieter ausgeschaltet** (etwa von Hand in
  dessen Oberfläche): Ein Gerät des Admins bemerkt das bei der nächsten Prüfung,
  schaltet sie wieder ein, wenn es kann, und warnt den Admin sonst.
- **Falsche Zugangsdaten**: Die Eignungsprüfung scheitert mit verständlichem
  Grund (falscher Schlüssel, fehlende Rechte, Endpunkt nicht erreichbar, Region
  passt nicht). Nichts wird gespeichert oder angelegt.
- **Hauptzugangsdaten werden beim Anbieter widerrufen oder laufen ab**: Die
  Zugangsschlüssel der Mitglieder gelten weiter, solange der Anbieter sie nicht
  mitwiderruft. Erneuerung, Wiederherstellung und Ersatzweg scheitern jedoch;
  holzi zeigt dem Admin, dass neue Hauptzugangsdaten nötig sind.
- **Bucket wird außerhalb von holzi gelöscht**: Die Geräte erkennen, dass der
  Bucket fehlt, und zeigen den Speicher des Space als nicht verfügbar. Der Admin
  kann einen neuen Bucket einrichten; Geräte mit Schreibrecht laden die Objekte
  hoch, die sie noch haben. Dateien, deren Objekt kein Gerät mehr hat, meldet
  holzi als verloren.
- **Admin offline**: Mit Zugangsschlüsseln arbeiten die Mitglieder ohne den
  Admin weiter. Rechteänderungen und Erneuerungen gibt es ohnehin nur von einem
  Gerät des Admins aus. Auf dem Ersatzweg ist der Speicher nicht erreichbar,
  solange kein Gerät des Admins online ist (User Story 5).
- **Zwei Geräte des Admins erneuern gleichzeitig**: Es entstehen zwei neue
  Generationen desselben Zugangsschlüssels. Mitglieder nutzen die höchste
  Generation; das Gerät des Admins, das die niedrigere sieht, widerruft sie
  (FR-019). Kein Zugangsschlüssel bleibt gültig, ohne dass er in der Vault des
  Admins verzeichnet ist.
- **Ein Mitglied lädt Datenmüll hoch** und treibt die Kosten: holzi kann das
  nicht verhindern. Nicht referenzierte Objekte räumt ein Gerät des Admins auf
  (FR-040); der Admin kann
  das Mitglied zurückstufen oder entfernen. Wenn der Anbieter den Verbrauch
  meldet, zeigt holzi ihn an (FR-036).
- **Anbieter-Kontingent erschöpft**: Hochladen scheitert mit einer Meldung, die
  auf den Speicher des Space verweist; die Datei bleibt lokal und wird später
  erneut versucht.
- **Gerät des Admins wird gestohlen**: Das Gerät hat die Hauptzugangsdaten.
  Nach dem Erneuern der Vault-Identität (Spec 024) weist holzi den Admin darauf
  hin, dass er auch die Hauptzugangsdaten beim Anbieter erneuern und in holzi neu
  eingeben soll, und erneuert danach alle Zugangsschlüssel aller Spaces dieser
  Verbindung.
- **Vault-Identität eines Mitglieds ändert sich** (Spec 024, gestohlenes
  Gerät): Der Admin verteilt den Inhaltsschlüssel neu (Spec 027) und erneuert
  dabei den Zugangsschlüssel dieses Mitglieds wie beim Entfernen.
- **Neues Mitglied, während eine Erneuerung aussteht**: Es erhält die höchste
  Generation, die das Gerät des Admins kennt.
- **Admin-Vault geht ganz verloren**: Der Space ist eingefroren (Entwurf §15,
  offene Frage 3). Die Zugangsschlüssel gelten weiter, bis der Nutzer sie beim
  Anbieter selbst widerruft; holzi kann sie ohne die Admin-Vault nicht mehr
  verwalten.

## Requirements _(mandatory)_

### Functional Requirements

**Speicher verbinden und prüfen**

- **FR-001**: Der Admin eines Space MUSS für den Space eigenen S3-kompatiblen
  Speicher verbinden können, beim Anlegen des Space oder später (dann mit
  Wechsel nach FR-037). Mitglieder ohne Admin-Rolle DÜRFEN den Speicher NICHT
  festlegen oder ändern.
- **FR-002**: Zum Verbinden MUSS holzi Endpunkt, Region und die
  Hauptzugangsdaten abfragen, dazu, wenn der Anbieter es braucht, die Angabe, wie
  Buckets adressiert werden. Für die Anbieter aus FR-008 SOLL holzi Endpunkt und
  Adressierung vorbelegen.
- **FR-003**: Eine gespeicherte Speicherverbindung MUSS für weitere Spaces und
  die eigenen Ordner wiederverwendbar sein, ohne die Zugangsdaten erneut
  einzugeben.
- **FR-004**: Vor dem Speichern der Zugangsdaten und vor dem Anlegen von
  irgendetwas beim Anbieter MUSS holzi die Eignungsprüfung durchführen. Sie MUSS
  mindestens prüfen, ob (a) die Zugangsdaten gültig sind und der Endpunkt
  erreichbar ist, (b) Buckets angelegt werden können, (c) die Versionierung
  eingeschaltet werden kann, (d) eine Aufbewahrungsfrist für ältere Versionen
  gesetzt werden kann, (e) auf einen Bucket beschränkte Zugangsschlüssel „nur
  Lesen“ und „Lesen und Schreiben“ erzeugt und widerrufen werden können und
  (f) der Schlüssel „Lesen und Schreiben“ weder ältere Versionen löschen noch
  Versionierung oder Aufbewahrungsfrist ändern kann.
- **FR-005**: Das Ergebnis der Eignungsprüfung MUSS eine von drei Stufen sein
  und dem Admin mit Grund gezeigt werden: **geeignet** (alles aus FR-004),
  **nur über den Ersatzweg** ((a) bis (c) erfüllt, (e) oder (f) nicht) oder
  **ungeeignet**. Jede nicht erfüllte Eigenschaft MUSS einzeln und verständlich
  benannt werden.
- **FR-006**: Für einen Space DARF holzi einen Anbieter ohne Versionierung NICHT
  einrichten. Für die eigenen Ordner (FR-031) DARF holzi es mit Warnung.
- **FR-007**: Scheitert die Prüfung oder die Einrichtung, DÜRFEN weder
  Zugangsdaten in der Vault noch halb angelegte Buckets oder Zugangsschlüssel
  beim Anbieter zurückbleiben; was holzi schon angelegt hat, MUSS es wieder
  entfernen.
- **FR-008**: holzi MUSS in v1 mindestens diese Anbieter nachweislich
  unterstützen: [NEEDS CLARIFICATION: Welche Anbieter muss v1 geprüft und
  getestet unterstützen? Entwurf §12 (Kandidaten aus der Entwurfssitzung,
  ungeprüft) nennt Cloudflare R2, Backblaze B2, MinIO und AWS S3, dazu Hetzner
  Object Storage als ungeprüft. Ob diese Anbieter auf einen Bucket beschränkte
  Schlüssel, Versionierung, eine Aufbewahrungsfrist und Schlüssel ohne Recht zum
  Löschen älterer Versionen bieten (FR-004 c bis f), ist ungeprüft und
  entscheidet, ob sie „geeignet“ oder „nur über den Ersatzweg“ sind.] Andere S3-kompatible Anbieter DÜRFEN verbunden werden, wenn sie die
  Eignungsprüfung bestehen.

**Bucket und Versionierung**

- **FR-009**: Jeder Space mit eigenem Speicher MUSS genau einen eigenen Bucket
  haben, der nur Objekte dieses Space enthält. Zwei Spaces DÜRFEN sich KEINEN
  Bucket teilen, auch nicht bei derselben Speicherverbindung.
- **FR-010**: Der Name des Buckets DARF weder Namen noch Kennung des Space noch
  Hinweise auf seine Mitglieder enthalten.
- **FR-011**: Bei der Einrichtung MUSS holzi die Versionierung des Buckets
  einschalten und eine Aufbewahrungsfrist für ältere Versionen setzen. Die
  Standardfrist ist 30 Tage; der Admin DARF sie ändern.
- **FR-012**: Der Bucket DARF NICHT öffentlich lesbar oder beschreibbar sein;
  holzi MUSS öffentlichen Zugriff bei der Einrichtung ausschließen, wenn der
  Anbieter das erlaubt.
- **FR-013**: Ein Gerät des Admins MUSS, wenn es online ist, mindestens einmal
  am Tag und bei jedem Öffnen des Space prüfen, ob die Versionierung noch
  eingeschaltet ist, sie wieder einschalten, wenn sie aus ist, und den Admin
  warnen, wenn das nicht gelingt.

**Zugangsschlüssel und Verteilung**

- **FR-014**: holzi MUSS bei der Einrichtung auf der Stufe „geeignet“ je Space
  zwei Zugangsschlüssel erzeugen, die nur für dessen Bucket gelten: „nur Lesen“
  (Objekte lesen und auflisten) und „Lesen und Schreiben“ (zusätzlich Objekte
  hochladen und löschen, nicht aber ältere Versionen löschen und keine
  Bucket-Einstellungen ändern).
- **FR-015**: holzi MUSS jedem Mitglied genau den Zugangsschlüssel geben, den
  seine Fähigkeit braucht: „nur Lesen“ für Lesen, „Lesen und Schreiben“ für
  Schreiben oder Löschen. Ein Mitglied mit Lesen DARF den Schlüssel „Lesen und
  Schreiben“ NICHT entschlüsseln können.
- **FR-016**: Zugangsschlüssel MÜSSEN zusammen mit den Angaben, die ein Gerät
  zum Zugriff braucht (Endpunkt, Region, Bucket), in Umschlägen an die
  Vault-Identität des Mitglieds verteilt werden, auf demselben Weg wie die übrigen
  verschlüsselten Daten des Space (Spec 027). Das Relay DARF sie nur verschlüsselt
  weiterreichen.
- **FR-017**: Ein empfangener Umschlag für einen Zugangsschlüssel MUSS in der
  Vault des Mitglieds liegen und über den Sync zwischen eigenen Geräten zu dessen
  anderen Geräten gelangen; jedes Gerät MUSS den Umschlag selbst mit der
  Vault-Identität öffnen. Geöffnete Zugangsschlüssel und die Hauptzugangsdaten
  des Admins (FR-033) gehören zu den Nur-direkt-Daten (Spec 024): Sie DÜRFEN nur
  über direkte Verbindungen zwischen Geräten derselben Vault reisen und NICHT in
  ein Postfach des Relays (auch nicht das der eigenen Vault), in andere Spaces
  oder in Datenfreigaben gelangen. Nur die Umschläge nach FR-016 reisen über das
  Relay.

**Erneuern bei Entzug**

- **FR-018**: Entfernt der Admin ein Mitglied oder nimmt ihm eine Fähigkeit,
  MUSS holzi den betroffenen Zugangsschlüssel erneuern: neue Generation beim
  Anbieter erzeugen, an alle verbleibenden Mitglieder mit dieser Art verteilen,
  danach die vorherige Generation beim Anbieter widerrufen. Welcher Schlüssel
  betroffen ist, folgt aus FR-015. Das Erneuern des Inhaltsschlüssels (Spec 027)
  geschieht unabhängig davon.
- **FR-019**: Beim Anbieter DARF je Art höchstens die höchste Generation
  gültig bleiben, sobald die Verteilung abgeschlossen ist. Sieht ein Gerät des
  Admins eine niedrigere, noch gültige Generation, MUSS es sie widerrufen.
- **FR-020**: Der Admin MUSS jeden Zugangsschlüssel eines Space jederzeit von
  Hand erneuern können, mit derselben Wirkung wie FR-018.
- **FR-021**: Kommt ein Mitglied hinzu oder erhält es eine höhere Fähigkeit,
  MUSS es die aktuelle Generation des passenden Schlüssels erhalten, ohne dass ein
  Zugangsschlüssel erneuert wird. Die neue Schlüsselgeneration des
  Inhaltsschlüssels nach Spec 027 bleibt davon unberührt.
- **FR-022**: Scheitert ein Widerruf beim Anbieter, MUSS holzi ihn als offen
  festhalten, erneut versuchen, sobald der Anbieter erreichbar ist, und ihn dem
  Admin bis zum Erfolg anzeigen.

**Ersatzweg über ein Gerät des Admins**

- **FR-023**: Auf der Stufe „nur über den Ersatzweg“ MUSS holzi den Space ohne
  Zugangsschlüssel einrichten. Die Geräte der Mitglieder MÜSSEN dann für jedes
  Objekt einen Zugangslink bei einem online erreichbaren Gerät der Admin-Vault
  anfragen, über die direkte Geräteverbindung (Spec 027, direkte Verbindung
  zwischen Mitgliedern), nie über das Relay. Eigene Geräte des Admins brauchen
  keine Zugangslinks; sie nutzen die Hauptzugangsdaten, die sie über die direkte
  Verbindung zwischen eigenen Geräten (Spec 024) erhalten.
- **FR-024**: Ein Gerät des Admins MUSS vor dem Ausstellen die Gerätebestätigung
  des anfragenden Geräts und die Fähigkeit seiner Vault in der aktuellen
  Mitgliederliste prüfen. Es DARF nur Links für Lesen, Hochladen oder Löschen
  eines einzelnen Objekts ausstellen, passend zur Fähigkeit, mit einer
  Gültigkeit von höchstens 15 Minuten, und keine Links zum Löschen älterer
  Versionen oder zum Ändern des Buckets.
- **FR-025**: Solange kein Gerät der Admin-Vault erreichbar ist, MUSS holzi den
  Mitgliedern zeigen, dass der Speicher des Space nur mit einem Gerät des Admins
  erreichbar ist, und Objekte, wenn möglich, direkt von anderen Geräten beziehen
  (Spec 025 und 027).

**Direkter Zugriff und was der Speicher sieht**

- **FR-026**: Mit Zugangsschlüssel MÜSSEN die Geräte Objekte direkt beim
  Anbieter lesen und hochladen. Das Relay DARF an diesen Übertragungen NICHT
  beteiligt sein.
- **FR-027**: Im Bucket DÜRFEN nur Objekte liegen, benannt nach dem Hash ihres
  Chiffretexts. Dateinamen, Pfade, Ordnerstruktur, Klartextgrößen und
  Klartext-Hashes DÜRFEN weder in Objektnamen noch in Metadaten der Objekte
  stehen.
- **FR-028**: Ein Gerät MUSS jedes geladene Objekt gegen den Hash im Dateiindex
  prüfen und ein nicht passendes Objekt verwerfen.

**Wiederherstellen und Aufräumen**

- **FR-029**: Ein Gerät des Admins MUSS, wenn es online ist, mindestens einmal
  am Tag und auf Anforderung des Admins prüfen, ob jedes Objekt, auf das ein
  gültiger Eintrag im Dateiindex verweist, im Bucket vorhanden ist und zum Hash
  passt. Fehlt es oder passt es nicht, MUSS das Gerät die jüngste passende ältere
  Version wiederherstellen und dem Admin die betroffenen Dateien nennen.
- **FR-030**: Objekte, deren Einträge rechtmäßig gelöscht oder ersetzt wurden
  (Spec 025 und 027), DÜRFEN NICHT wiederhergestellt werden. Ist keine passende
  ältere Version mehr vorhanden, MUSS holzi die Datei als verloren melden, sofern
  kein Gerät das Objekt noch hat.
- **FR-040**: Alte Objekte (ersetzte Versionen einer Datei, gelöschte Dateien)
  und nicht referenzierte Objekte MUSS bei Backend B ein Gerät des Admins mit den
  Hauptzugangsdaten aus dem Bucket entfernen, sobald kein gültiger Eintrag im
  Dateiindex mehr auf sie verweist (Spec 027). Bei den eigenen Ordnern (FR-031)
  räumt ein eigenes Gerät auf. Ältere Versionen dieser Objekte bleiben bis zum
  Ende der Aufbewahrungsfrist erhalten.

**Eigene synchronisierte Ordner**

- **FR-031**: Der Nutzer MUSS für die eigenen synchronisierten Ordner seiner
  Vault (Spec 025) eigenen Speicher verbinden können, mit einem eigenen Bucket für
  diese Ordner.
- **FR-032**: Für die eigenen Ordner MÜSSEN die eigenen Geräte die
  Speicherverbindung selbst nutzen; Zugangsschlüssel und Ersatzweg entfallen. Die
  Eignungsprüfung MUSS fehlende eingeschränkte Schlüssel dort ignorieren und
  fehlende Versionierung nur als Warnung melden.

**Ende eines Space und Hauptzugangsdaten**

- **FR-033**: Die Hauptzugangsdaten DÜRFEN die Vault des Admins NICHT verlassen,
  außer über die direkte Verbindung zu seinen eigenen Geräten (Spec 024,
  Nur-direkt-Daten). Sie DÜRFEN weder an Mitglieder noch in Spaces,
  Datenfreigaben, Postfächer oder Momentaufnahmen des Relays gelangen, auch nicht
  verschlüsselt, und DÜRFEN in keinem Protokoll stehen.
- **FR-034**: Hört ein Space oder hören die eigenen Ordner auf, einen Bucket zu
  nutzen (Wechsel nach FR-037, Rückkehr der eigenen Ordner zur direkten
  Übertragung), MUSS holzi alle Generationen aller Zugangsschlüssel dieses
  Buckets widerrufen und den Admin fragen, ob der Bucket samt älterer Versionen
  gelöscht werden soll; Standard ist Behalten. Dasselbe MUSS gelten, wenn eine
  spätere Spec das Auflösen eines Space einführt (Spec 027 schließt es aus).
- **FR-035**: Der Admin MUSS eine Speicherverbindung entfernen können, sobald
  kein Space und nicht die eigenen Ordner sie mehr nutzen; vorher MUSS holzi das
  verhindern und die Nutzer der Verbindung nennen. Nach dem Entfernen DÜRFEN die
  Hauptzugangsdaten in keiner Vault-Datei eines eigenen Geräts bleiben; ein
  Löschvermerk des Syncs DARF nur die Kennung des Eintrags tragen.

**Verbrauch**

- **FR-036**: holzi KANN dem Admin den belegten Speicher eines Space anzeigen,
  getrennt nach aktuellen und älteren Versionen, wenn der Anbieter diese Angaben
  liefert.

**Wechsel des Speicher-Backends**

- **FR-037**: Der Admin MUSS einen Space von Backend A oder „nur direkte
  Übertragung“ (Spec 027) auf Backend B und von B auf A umziehen können. Ein
  Gerät des Admins MUSS dazu alle Objekte, auf die gültige Einträge verweisen,
  in den neuen Speicher kopieren und gegen ihren Hash prüfen, bevor der Space
  umschaltet.
- **FR-038**: Bis zum Umschalten MÜSSEN die Mitglieder den bisherigen Speicher
  weiter nutzen können; ein abgebrochener Umzug MUSS sich fortsetzen lassen, ohne
  schon kopierte Objekte erneut zu übertragen.
- **FR-039**: Nach dem Umschalten MUSS holzi den alten Speicher aufräumen:
  Objekte auf Backend A löschen, bei Backend B die Zugangsschlüssel widerrufen und
  den Bucket nach Wahl des Admins löschen oder behalten (wie FR-034).

### Key Entities

- **Speicherverbindung**: Anbieter, Endpunkt, Region, Adressierung und
  Hauptzugangsdaten. Gehört einer Vault, liegt nur auf deren Geräten (FR-033) und
  kann von mehreren Spaces und den eigenen Ordnern genutzt werden.
- **Speicher des Space**: welches Backend ein Space nutzt und bei Backend B
  welcher Bucket, welche Stufe der Eignung (geeignet oder Ersatzweg) und welche
  Aufbewahrungsfrist. Wird vom Admin geschrieben und mit den Daten des Space
  verteilt, ohne Hauptzugangsdaten.
- **Zugangsschlüssel**: Art (nur Lesen, Lesen und Schreiben), Generation,
  Kennung beim Anbieter (für den Widerruf) und die geheimen Schlüsseldaten. Die
  Vault des Admins verzeichnet alle Generationen und ob sie widerrufen sind.
- **Schlüsselumschlag für Zugangsschlüssel**: ein Zugangsschlüssel samt
  Endpunkt, Region und Bucket, verschlüsselt an die Vault-Identität genau eines
  Mitglieds.
- **Eignungsergebnis**: Stufe (geeignet, nur über den Ersatzweg, ungeeignet)
  und die Liste der erfüllten und nicht erfüllten Eigenschaften aus FR-004.
- **Zugangslink**: Objekt, Art des Zugriffs, Ablaufzeit. Wird nur auf dem
  Ersatzweg ausgestellt und nicht gespeichert.
- **Offener Widerruf**: ein Zugangsschlüssel, dessen Widerruf beim Anbieter
  noch aussteht (FR-022).

## Success Criteria _(mandatory)_

### Measurable Outcomes

- **SC-001**: Speicher, Datenbank und Protokolle des Relays enthalten nach einer
  vollständigen Testreihe (Einrichten, Einladen, Hoch- und Herunterladen, Entzug,
  Löschen) in 0 % der Fälle Hauptzugangsdaten, weder lesbar noch verschlüsselt,
  und in 0 % der Fälle einen Zugangsschlüssel in lesbarer Form.
- **SC-002**: Nach dem Entfernen eines Mitglieds lehnt der Anbieter dessen alten
  Zugangsschlüssel in 100 % der Testläufe innerhalb von 5 Minuten ab, sofern ein
  Gerät des Admins online ist und der Anbieter die Stufe „geeignet“ hat.
- **SC-003**: Eine Durchsicht des Buckets mit vollem Zugriff findet in 0 % der
  Objekte einen Dateinamen, Pfad oder lesbaren Inhalt, und der Name des Buckets
  enthält weder Namen noch Kennung des Space.
- **SC-004**: Ein Admin mit bereitliegenden Hauptzugangsdaten verbindet einen
  Space auf einem geeigneten Anbieter in weniger als 3 Minuten mit seinem eigenen
  Speicher.
- **SC-005**: Für jeden ungeeigneten Testanbieter nennt die Eignungsprüfung in
  100 % der Fälle jede fehlende Eigenschaft einzeln, und danach bleiben beim
  Anbieter 0 Buckets und 0 Zugangsschlüssel aus dem Versuch zurück.
- **SC-006**: Ein Objekt, das ein Mitglied mit Schreibrecht direkt im Bucket
  gelöscht oder überschrieben hat, ist innerhalb der Aufbewahrungsfrist in 100 %
  der Testläufe wiederherstellbar und nach der nächsten Prüfung durch ein Gerät
  des Admins wieder für alle Mitglieder lesbar.
- **SC-007**: Ein Mitglied mit Lesen scheitert in 100 % der Versuche am Anbieter,
  wenn es hochladen oder löschen will; ein Mitglied mit Schreiben scheitert in
  100 % der Versuche, ältere Versionen zu löschen oder die Versionierung
  auszuschalten.
- **SC-008**: Auf dem Ersatzweg ist jeder Zugangslink nach höchstens 15 Minuten
  ungültig, und ein entferntes Mitglied erhält in 0 % der Anfragen einen neuen
  Link.
- **SC-009**: Nach einem Umzug zwischen Backend A und B sind 100 % der Dateien
  für alle Mitglieder lesbar, und der alte Speicher enthält nach dem Aufräumen
  keine Objekte des Space mehr (Backend B: sofern der Admin den Bucket löschen
  lässt).
- **SC-010**: Nachdem ein Space oder die eigenen Ordner einen Bucket aufgegeben
  haben, gelten beim Anbieter 0 Zugangsschlüssel dieses Buckets mehr, sobald der
  Anbieter erreichbar war.

## Assumptions

- Spec 027 liefert Mitgliederliste, Fähigkeiten, Einladung, Entzug und das
  Erneuern des Inhaltsschlüssels; diese Spec hängt nur die Zugangsschlüssel daran.
  Die dort noch offenen Löschregeln (Entwurf §11) ändern hier nur, wer
  rechtmäßig löschen darf, nicht, wie der Speicher geschützt wird.
- Spec 024 liefert die direkte Verbindung zwischen eigenen Geräten, die
  Gerätebestätigung und die Liste der Nur-direkt-Daten; Spec 026 lässt diese
  Daten auch verschlüsselt in keinem Postfach des Relays zu. Spec 027 liefert die
  direkte Verbindung zwischen Geräten verschiedener Mitglieder eines Space, die
  der Ersatzweg nutzt. Die Hauptzugangsdaten und geöffnete Zugangsschlüssel sind
  Nur-direkt-Daten; FR-017 und FR-033 wiederholen das nur für sie. Ein zweites
  Gerät des Admins, das nie gleichzeitig mit einem anderen eigenen Gerät online
  ist, bekommt die Hauptzugangsdaten deshalb erst bei der nächsten direkten
  Verbindung; dasselbe gilt für die Speicherverbindung der eigenen Ordner (User
  Story 6).
- Das Aufräumen alter Objekte (FR-040) übernimmt bei Backend B ein Gerät des
  Admins, weil nur es die Hauptzugangsdaten hat; bei Backend A regelt Spec 026,
  wer löschen darf.
- Eingeschränkte Zugangsschlüssel werden über die Verwaltungsschnittstelle des
  jeweiligen Anbieters erzeugt, die nicht bei allen Anbietern Teil der
  S3-Schnittstelle ist. Deshalb braucht jeder unterstützte Anbieter eine eigene
  Anbindung dafür; der Plan legt fest, wie.
- Ein Zugangsschlüssel gilt je Art für alle Mitglieder dieser Art, nicht je
  Mitglied. Entfernen eines Mitglieds erneuert deshalb den Schlüssel aller
  verbleibenden Mitglieder dieser Art. Schlüssel je Mitglied wären feiner, aber
  der Entwurf sieht zwei Schlüssel je Space vor, und die Rotationskosten bleiben
  klein.
- Mitglieder mit Löschen erhalten denselben Schlüssel wie Mitglieder mit
  Schreiben, weil S3 Schreiben und Löschen nicht trennt. Die Unterscheidung prüfen
  die Empfänger im Dateiindex (Spec 027); User Story 4 fängt die Grenze ab.
- Die Aufbewahrungsfrist von 30 Tagen ist eine Voreinstellung, keine Garantie
  des Anbieters. Setzt der Anbieter sie nicht um, gilt sie als nicht erfüllt
  (FR-004 d), und der Admin sieht das im Eignungsergebnis.
- Die Wiederherstellung (FR-029) setzt voraus, dass mindestens ein Gerät des
  Admins regelmäßig online ist. Zwischen zwei Prüfungen kann eine gelöschte Datei
  für Mitglieder unerreichbar sein.
- Der Anbieter sieht IP-Adressen, Zeitpunkte, Anzahl und Größe der Objekte und
  wer mit welchem Zugangsschlüssel zugreift. Das ist wie beim Relay (Entwurf
  §10.4) hingenommen.
- Kosten beim Anbieter trägt der Nutzer, dem die Speicherverbindung gehört.
- Verlust der Admin-Vault friert den Space ein (Entwurf §15, offene Frage 3);
  eine Übergabe der Admin-Rolle ist nicht Teil dieser Spec.

## Nicht im Umfang

- Zugangsschlüssel je Mitglied statt je Art.
- Neues Verschlüsseln aller Objekte nach einem Entzug; ein entferntes Mitglied
  behält, was es schon entschlüsseln konnte (Entwurf §13, Spec 027).
- Mehrere Speicher für einen Space gleichzeitig (Spiegelung, Redundanz über
  Anbieter hinweg).
- Speicher, der kein S3 spricht (WebDAV, SFTP, Dateifreigaben im Netz).
- Verwaltung von Konten, Abrechnung oder Kontingenten beim Anbieter über die
  Anzeige nach FR-036 hinaus.
- Eine Übergabe der Admin-Rolle oder der Speicherverbindung an ein anderes
  Mitglied.
- Einen Space auflösen oder löschen (wie Spec 027); FR-034 gilt, sobald eine
  spätere Spec das einführt.
- Ein Umzug von Backend B auf „nur direkte Übertragung“; dabei gingen Dateien
  verloren, die kein Gerät hat.
- Datenfreigaben (Spec 028); sie nutzen keinen Objektspeicher.
- Eigener Speicher für das Postfach eines Space; Dateiindex, Mitgliederliste und
  Schlüsselumschläge laufen weiter über das Relay des Admins, wenn eines
  eingerichtet ist, sonst nur direkt (Spec 026 und 027).
