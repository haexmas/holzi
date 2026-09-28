# Feature Specification: Spaces: Netzwerkordner zum Teilen von Dateien mit anderen Nutzern

**Feature Branch**: `027-spaces`
**Created**: 2026-09-28
**Status**: Draft
**Input**: Zeile 027 des Sync-Entwurfs
[`docs/plans/2026-09-28-sync-architecture.md`](../../docs/plans/2026-09-28-sync-architecture.md)
(§5, §8, §10.3, §11, §12, §13, §15): Ein Nutzer legt einen Space an, einen
Netzwerkordner für Dateien, und lädt andere Nutzer über ihre Vault-Identität
ein. Jedes Mitglied bindet den Space an einen Ordner auf seinen Geräten. Der
Ersteller ist der einzige Admin; er vergibt Lesen, Schreiben und Löschen, ändert
Rechte und entfernt Mitglieder, wobei der Inhaltsschlüssel rotiert wird. Der
Space nutzt die Maschinerie der Ordner-Synchronisierung eigener Geräte (Spec 025) für mehrere Nutzer. Vorgaben des Betreibers: Übertragung über iroh,
Identität, Anmeldung und Schlüsselumschläge über Nostr-Schlüssel, Umschläge im
Format NIP-44 v2, Einladungen als verschlüsselte Nostr-Direktnachricht. Kein
MLS. Referenz für den Umgang mit entfernten Mitgliedern: haex-vault ADR 0002,
Repository `https://github.com/haex-space/haex-vault`, Revision
`8dce379d94e18fcd42c3b73686a06f984ca3f574`, Pfad
`docs/adr/0002-shared-space-authenticity-and-confidentiality.md`.

## Begriffe

- **Vault-Identität**: der Schlüssel, der eine Vault als Ganzes ausweist. Er ist
  auf allen Geräten derselben Vault gleich. Einladungen, Rechte und
  Schlüsselumschläge gehen immer an eine Vault-Identität, nie an einzelne
  Geräte (Spec 024).
- **Geräteschlüssel**: der Schlüssel eines einzelnen Geräts. Mit ihm meldet sich
  das Gerät an und unterschreibt seine Änderungen.
- **Gerätebestätigung**: eine von der Vault-Identität unterschriebene Aussage
  „dieses Gerät handelt für diese Vault“. Jeder Empfänger prüft damit, für
  welche Vault ein Gerät schreibt.
- **Bereich**: ein Rahmen, in dem Daten synchronisiert werden, mit eigenen
  Schlüsseln und einem eigenen Postfach am Relay: der „Bereich Vault“, der
  „Bereich eines Space“ oder der „Bereich einer Datenfreigabe“.
- **Space**: ein Bereich für **Dateien**, der sich wie ein Netzwerkordner
  verhält. Rechte gelten für den ganzen Ordner. Ein Space enthält nie Daten aus
  der SQLite-Datenbank der Vault.
- **Admin**: die Vault, die den Space angelegt hat. Es gibt genau einen Admin je
  Space; die Rolle ist nicht übertragbar und nicht vergebbar.
- **Mitglied**: eine Vault, die der Admin in den Space eingeladen hat, samt all
  ihren Geräten. Der Admin ist selbst Mitglied mit allen Fähigkeiten.
- **Fähigkeit**: ein Recht eines Mitglieds im Space: **Lesen** (alle Dateien
  lesen), **Schreiben** (neue Dateien hinzufügen, bestehende ändern, eigene
  Dateien löschen), **Löschen** (auch fremde Dateien löschen).
- **Eigene Datei**: eine Datei, die ein Gerät der eigenen Vault in den Space
  gebracht hat. Wer eine Datei gebracht hat, steht unveränderlich an ihrem
  Eintrag im Dateiindex; spätere Änderungen durch andere ändern das nicht.
- **Mitgliederliste**: die vom Admin unterschriebene Liste der Mitglieder
  (Vault-Identitäten) und ihrer Fähigkeiten. Ihre Version ist ihre
  **Generation**, die der Schlüsselgeneration, zu der sie gehört. Sie liegt
  verschlüsselt im Space und zusätzlich lesbar am Relay, das damit entscheidet,
  wer das Postfach lesen und beschreiben darf (Spec 026).
- **Inhaltsschlüssel**: der Schlüssel, mit dem Änderungspakete und
  Dateiindex-Einträge eines Space verschlüsselt sind.
- **Schlüsselgeneration**: ein Inhaltsschlüssel samt Nummer, erstellt für genau
  eine Mitgliederliste. Jede Änderung der Mitgliederliste erzeugt eine neue
  Generation; ihre Mitgliederliste steht danach fest. Neue Mitglieder erhalten
  zusätzlich Umschläge für alle älteren Generationen, damit sie ältere Inhalte
  lesen können; Rechte in einer älteren Generation geben ihnen diese Umschläge
  nicht. Mehrere Generationen können nebeneinander gelten; jeder verschlüsselte
  Inhalt nennt die Generation, mit der er verschlüsselt ist.
- **Änderungspaket**: die Einheit, in der Änderungen eines Bereichs verschlüsselt
  zwischen Geräten und über das Relay reisen.
- **Dateiindex**: das Verzeichnis eines Space: welche Dateien es gibt, mit Pfad,
  Größe, Änderungszeit, Ersteller, letztem Bearbeiter und Verweis auf ihr
  Objekt (Spec 025).
- **Objekt**: der verschlüsselte, unveränderliche Inhalt einer Dateifassung,
  benannt nach dem Prüfwert des verschlüsselten Inhalts. Eine Änderung erzeugt
  ein neues Objekt.
- **Konfliktkopie**: eine zusätzliche Datei, die entsteht, wenn zwei Fassungen
  derselben Datei unabhängig voneinander entstanden sind. Keine Fassung geht
  verloren. Ihr Name folgt Spec 025, „<Name> (Konflikt <Gerätename> <Datum
  Uhrzeit>).<Endung>“, wobei im Space vor dem Gerätenamen der Anzeigename des
  Mitglieds steht (FR-029).
- **Relay**: der nicht vertrauenswürdige Server, über den Geräte synchronisieren,
  die nicht gleichzeitig online sind. Es sieht keine Inhalte (Spec 026).
- **Postfach**: der Speicherplatz eines Bereichs am Relay für Änderungspakete,
  darunter Dateiindex und Mitgliederliste. Das Postfach ist vom Speicher für
  Objekte getrennt (FR-040).
- **Speicher-Backend A**: vom Relay bereitgestellter Speicher für Objekte
  (Spec 026). **Speicher-Backend B**: ein eigener S3-Speicher des Admins
  (Spec 029).
- **Datenfreigabe**: das Teilen einzelner Einträge oder Sammlungen aus der
  SQLite-Datenbank mit anderen Nutzern (Spec 028). Nicht Teil von Spaces.

## Beziehung zu bestehenden Specs

- **Spec 024** (Vault-Identität,
  Geräteschlüssel, Gerätebestätigung, direkte Synchronisierung eigener Geräte):
  Diese Spec setzt voraus, dass jede Vault eine echte Vault-Identität hat, dass
  Geräte sich über Gerätebestätigungen ausweisen und dass Einträge, die eine
  Vault erhält, über die Synchronisierung eigener Geräte auf alle ihre Geräte
  kommen. Ein Schlüsselumschlag an ein Mitglied wird deshalb nur einmal je
  Vault erstellt. Schlüssel der Vault selbst verlassen die Vault nie über einen
  Space. Das Rotieren einer Vault-Identität (Gerät verloren) und die Übergabe
  an die neue Identität in jedem geteilten Bereich definiert Spec 024; diese
  Spec wendet sie auf Spaces an (siehe Edge Cases). Die Anwesenheits- und
  Signalisierungsserver aus Spec 024 tragen auch Einladungen (FR-041) und das
  Auffinden von Geräten anderer Mitglieder (FR-039). Ob ein Änderungspaket als
  Ganzes oder je Änderung geprüft wird, legt Spec 024 fest (FR-026).
- [`025-own-device-file-sync`](../025-own-device-file-sync/spec.md) (Ordner-Synchronisierung
  eigener Geräte): Diese Spec **erweitert** die Maschinerie aus Spec 025 auf
  mehrere Nutzer. Dateiindex, Objekte, Übertragung, Konfliktkopien und die
  Bindung an einen lokalen Ordner gelten unverändert; Spec 025 ist der Fall
  „Space mit genau einem Mitglied, der eigenen Vault“. Neu sind hier Mitglieder,
  Fähigkeiten, Einladungen, Entzug, Schlüsselrotation und direkte
  Verbindungen zwischen Geräten verschiedener Vaults. Die Verwaltung von Spaces
  steht in der Einstellungskategorie „Föderation“ (Spec 023) in der
  Unteransicht „Spaces“, neben der Unteransicht „Ordner“ aus Spec 025.
- **Spec 026** (Relay, Postfächer, Mitgliederlisten,
  Speicher-Backend A): Das Postfach eines Space, die Prüfung der
  Mitgliederliste am Relay, die Löschregeln für Objekte (FR-028 dort) und
  Speicher-Backend A kommen aus Spec 026. Diese Spec legt fest, wann der Admin
  eine neue Mitgliederliste hochlädt, was Mitglieder prüfen, wenn das Relay
  etwas Unberechtigtes durchlässt, und dass ein Gerät des Admins alte Objekte
  aufräumt (FR-042).
- **Spec 028** (Datenfreigaben): Daten aus der
  SQLite-Datenbank werden nie über Spaces geteilt, sondern je Nutzer über
  Datenfreigaben. Beide nutzen dieselben Bausteine (Vault-Identität,
  Schlüsselgenerationen, Mitgliederliste, Relay), sind aber getrennte Bereiche.
  Spec 028 übernimmt von hier die direkte Verbindung zwischen Mitgliedern
  (FR-039) und die Zustellung von Einladungen (FR-041).
- **Spec 029** (Speicher-Backend B): Spec 029 fügt den
  eigenen S3-Speicher als Wahl beim Anlegen eines Space hinzu und bringt den
  Wechsel des Speicher-Backends eines bestehenden Space (User Story 8 dort).
  Bis dahin bietet diese Spec Speicher-Backend A und die direkte Übertragung
  an. Das Speicher-Backend wird in der Ansicht des Space eingestellt.
- [`023-settings-app`](../023-settings-app/spec.md): Die Ansichten dieser Spec
  folgen dem Aufbau der Einstellungs-App (Werkzeugleiste, großer Titel,
  abgerundete Gruppen mit Zeilen). Eine Auswahl gilt sofort, ohne Knopf zum
  Übernehmen (FR-021 dort). Nur unumkehrbare Schritte (Mitglied entfernen, Space
  verlassen) fragen einmal nach.
- [`020-tab-navigation`](../020-tab-navigation/spec.md): Jede Ansicht dieser Spec
  ist ein Ort im Tab. Anlegen, Einladen, Rechte ändern, Entfernen, Annehmen,
  Ablehnen und Verlassen sind Aktionen im Katalog von Spec 020.

## Clarifications

### Session 2026-09-27

- Q: Was ist ein Space, und was bedeuten Lesen und Schreiben dort? → A: Ein Space
  ist ein Netzwerkordner nur für Dateien. Lesen heißt alle Dateien lesen;
  Schreiben heißt neue Dateien hinzufügen und bestehende ändern (D2).
- Q: Werden Daten aus der SQLite-Datenbank über Spaces geteilt? → A: Nein. Lesen
  und Schreiben von Daten werden je Nutzer einzeln vergeben, über
  Datenfreigaben (Spec 028), nie über Spaces (D3).
- Q: Wer verwaltet einen Space? → A: Der Ersteller ist der einzige Admin. Er
  lädt ein, vergibt, ändert und entzieht Rechte und entfernt Mitglieder (D6).
- Q: Wer darf einladen, und darf ein Mitglied weiterteilen? → A: Nur der Admin
  lädt ein. Ein Mitglied kann den Space nicht weiterteilen. Dass jemand Dateien
  herunterlädt und in einem eigenen Space erneut teilt, wird nicht verhindert
  (D7).
- Q: Wird MLS für die Schlüssel der Spaces verwendet? → A: Nein (D13).
- Q: Warum kein MLS? → A: MLS braucht eine feste Reihenfolge aller
  Mitgliedschaftsänderungen, die CRDT-Synchronisierung führt Änderungen dagegen
  ohne Reihenfolge zusammen. In haex-vault führte das zu Geräten, die dieselbe
  Änderung unterschiedlich annahmen, zu Lücken, die das Abholen ganzer
  Änderungsgruppen blockierten, und zu Geräten, die dauerhaft ausgesperrt
  blieben. Schlüsselgenerationen werden deshalb gewöhnliche synchronisierte
  Einträge, die nebeneinander gelten dürfen, und Umschläge gehen an die Vault,
  nicht an das Gerät.

### Session 2026-09-28

- Q: Was passiert, wenn zwei Nutzer dieselbe Datei gleichzeitig ändern? → A: Es
  entsteht eine Konfliktkopie; keine Fassung geht verloren (D10).
- Q: Darf das Relay Zugangsdaten zu einem eigenen Speicher halten? → A: Nein.
  Das Relay ist nicht vertrauenswürdig und erhält nie die S3-Zugangsdaten eines
  Nutzers (D11).
- Q: Welche Speicher-Backends gibt es für Spaces? → A: Beide in v1: Speicher des
  öffentlichen Relays (A, Spec 026) und eigener S3-Speicher (B, Spec 029) (D12).
  Direkte Übertragung zwischen Geräten bleibt immer möglich.
- Q: Was bedeutet der Entzug für ein entferntes Mitglied? → A: Das Relay und der
  Speicher weisen es sofort ab, Dateien, die nach dem Entzug hinzukommen oder
  geändert werden, kann es nicht lesen. Was es schon entschlüsseln konnte,
  behält es, wie in haex-vault ADR 0002. Ein vollständiges Neuverschlüsseln
  alter Dateien ist nicht Teil von v1.
- Q: Was passiert, wenn die Vault des Admins verloren geht? → A: Der Space ist
  dann eingefroren: Inhalte bleiben, Mitglieder arbeiten mit ihren bisherigen
  Rechten weiter, aber die Mitgliedschaft kann sich nicht mehr ändern. Eine
  Funktion „Admin übertragen“ ist nicht Teil dieser Spec (Entwurf §15, Punkt 3).
- Q: Wo liegen die Dateien eines Space auf dem Gerät? → A: In einem Ordner des Dateisystems, den das Mitglied je Gerät wählt (wie Spec 025).
- Q: Darf ein Space auf mehreren Relays liegen? → A: Nein, nicht in v1: ein Heimat-Relay, das Relay des Admins (Spec 026 FR-040).
- Q: Wie ist Löschen geregelt? → A: „Schreiben“ erlaubt das Löschen eigener Dateien (Ersteller = eigene Vault); fremde Dateien löschen erfordert die Stufe „Löschen“ (FR-016). Gilt ebenso für Spec 028.
- Q: Wie wirkt ein Entzug bei gleichzeitigem Schreiben? → A: Nur nach vorn. Eine Änderung gilt, wenn ihr Autor in ihrer Schlüsselgeneration das Recht hatte; jedes empfangende Gerät prüft zusätzlich gegen die neueste ihm bekannte Mitgliederliste, das Relay sperrt sofort. Das verbleibende kleine Zeitfenster wird akzeptiert (FR-024). Gilt ebenso für Spec 028.
- Q: Erzeugt auch eine Einladung eine neue Schlüsselgeneration? → A: Ja. Jede Änderung der Mitgliederliste erzeugt eine neue Generation mit fester Mitgliederliste; Datenänderungen nie (FR-019). Gilt ebenso für Spec 028.

## User Scenarios & Testing _(mandatory)_

### User Story 1 - Einen Space anlegen und an einen Ordner binden (Priority: P1)

Anna möchte einen Ordner „Familienfotos“ mit ihrer Familie teilen. Sie legt in
holzi einen Space an, gibt ihm einen Namen, wählt einen Ordner auf ihrem Rechner
und als Speicher den des Relays. Die Dateien im Ordner werden verschlüsselt
hochgeladen. Auf ihrem Laptop wählt sie für denselben Space einen anderen
Ordner, und die Fotos erscheinen dort.

**Why this priority**: Ohne einen Space gibt es nichts zu teilen. Diese Story
zeigt auch, dass die Maschinerie aus Spec 025 im Rahmen eines Space trägt.

**Independent Test**: Einen Space mit einem Ordner mit drei Dateien anlegen,
Speicher-Backend A wählen. Auf einem zweiten Gerät derselben Vault den Space an
einen leeren Ordner binden: Die drei Dateien erscheinen dort unverändert. Eine
Untersuchung von Relay und Speicher findet weder Dateinamen noch den Namen des
Space noch Dateiinhalte.

**Acceptance Scenarios**:

1. **Given** die Übersicht der Spaces, **When** Anna „Space anlegen“ wählt,
   einen Namen eingibt, einen Ordner und ein Speicher-Backend wählt, **Then**
   gibt es den Space, Anna ist sein Admin, und der Ordner ist an ihn gebunden.
2. **Given** ein neuer Space mit einem Ordner, der schon Dateien enthält,
   **When** der Space angelegt ist, **Then** kommen diese Dateien in den Space,
   mit Anna als Erstellerin.
3. **Given** ein Space, den Anna auf einem Gerät angelegt hat, **When** sie ein
   anderes Gerät ihrer Vault öffnet, **Then** zeigt es den Space als „noch nicht
   an einen Ordner gebunden“, bis sie dort einen Ordner wählt.
4. **Given** die Wahl des Speicher-Backends beim Anlegen, **When** Anna sie
   öffnet, **Then** stehen Speicher-Backend A und „nur direkte Übertragung“ zur
   Wahl, und nach Spec 029 zusätzlich Speicher-Backend B.
5. **Given** ein angelegter Space, **When** Anna seine Mitgliederliste ansieht,
   **Then** steht dort genau ein Mitglied, ihre Vault, als Admin mit allen
   Fähigkeiten.

---

### User Story 2 - Jemanden einladen, und die Einladung annehmen oder ablehnen (Priority: P1)

Anna lädt ihren Bruder Ben ein. Sie fügt seine Vault-Identität ein, gibt ihm
einen Namen und wählt „Lesen und Schreiben“. Ben bekommt in holzi eine
Einladung: Space „Familienfotos“ von Anna, mit seinen Rechten. Er nimmt an,
wählt einen Ordner, und die Fotos erscheinen. Später lädt Anna eine Bekannte
ein, die ablehnt.

**Why this priority**: Das Teilen mit anderen Nutzern ist der Kern der Spec.

**Independent Test**: Zwei Vaults A und B. A lädt B mit „Lesen“ ein. B nimmt an
und wählt einen Ordner: Alle Dateien des Space erscheinen dort, auch die, die
vor der Einladung hinzugekommen sind. Mit einer dritten Vault C wiederholen und
ablehnen: C hat danach keinen Space und keinen Schlüssel dazu; A sieht C als
„abgelehnt“ und nicht mehr als Mitglied.

**Acceptance Scenarios**:

1. **Given** Anna ist Admin eines Space, **When** sie „Mitglied einladen“ wählt,
   eine gültige Vault-Identität, einen Namen und eine Fähigkeitsstufe angibt,
   **Then** erscheint Ben in der Mitgliederliste als „eingeladen“ mit diesen
   Rechten.
2. **Given** die Einladung ist verschickt, **When** ein Gerät von Bens Vault
   online ist, **Then** zeigt holzi die Einladung mit Name des Space, Name und
   Vault-Identität des Admins und Bens Rechten, mit „Annehmen“ und „Ablehnen“.
3. **Given** die Einladung, **When** Ben „Annehmen“ wählt und einen Ordner
   angibt, **Then** erscheinen alle Dateien des Space in diesem Ordner, und
   Anna sieht Ben als Mitglied statt als „eingeladen“.
4. **Given** die Einladung, **When** die Eingeladene „Ablehnen“ wählt, **Then**
   verschwindet die Einladung auf allen Geräten ihrer Vault, Anna sieht sie als
   „abgelehnt“, und sie wird aus der Mitgliederliste entfernt wie in
   User Story 4.
5. **Given** Ben hat mehrere Geräte, **When** er die Einladung auf einem Gerät
   annimmt, **Then** kennen seine anderen Geräte den Space ebenfalls, ohne dass
   Anna etwas tut; jedes Gerät bindet ihn an einen eigenen Ordner.
6. **Given** Anna gibt eine ungültige Vault-Identität oder ihre eigene ein,
   **When** sie einladen will, **Then** lehnt holzi das mit einem Hinweis ab.
7. **Given** Ben ist Mitglied mit „Schreiben“, **When** er die Mitgliederliste
   öffnet, **Then** gibt es für ihn keine Möglichkeit, jemanden einzuladen.

---

### User Story 3 - Gemeinsam an Dateien arbeiten (Priority: P1)

Ben legt neue Fotos in seinen Ordner und benennt eines um. Anna sieht beides in
ihrem Ordner. Beide bearbeiten dieselbe Textdatei, während Ben offline ist. Als
er wieder online ist, haben beide die Datei zweimal: die eine Fassung unter dem
alten Namen und die andere als Konfliktkopie, deren Name zeigt, von wem sie
stammt.

**Why this priority**: Ohne gemeinsames Arbeiten wäre ein Space nur eine
Veröffentlichung.

**Independent Test**: Zwei Mitglieder mit „Schreiben“. Das eine fügt eine Datei
hinzu, das andere ändert sie: Beide Ordner zeigen danach dieselbe Fassung. Beide
ändern dieselbe Datei offline und gehen online: Jeder Ordner enthält beide
Fassungen, eine davon als Konfliktkopie, und auf allen Geräten dieselben
Dateien.

**Acceptance Scenarios**:

1. **Given** Ben hat „Schreiben“, **When** er eine Datei in seinen Ordner legt,
   **Then** erscheint sie in den Ordnern aller Mitglieder, mit Ben als
   Ersteller.
2. **Given** Ben hat „Schreiben“, **When** er eine Datei ändert, umbenennt oder
   verschiebt, die Anna angelegt hat, **Then** sehen alle Mitglieder die
   Änderung; Anna bleibt Erstellerin, Ben ist letzter Bearbeiter.
3. **Given** Anna und Ben ändern dieselbe Datei, ohne die Änderung des anderen
   zu kennen, **When** beide Änderungen zusammenkommen, **Then** gibt es auf
   allen Geräten beider Vaults dieselbe Datei und genau eine Konfliktkopie mit
   der anderen Fassung, deren Name das Mitglied nennt, von dem sie stammt.
4. **Given** Ben hat „Schreiben“ ohne „Löschen“, **When** er eine Datei löscht,
   die er selbst in den Space gebracht hat, **Then** verschwindet sie bei allen
   Mitgliedern.
5. **Given** Ben hat „Schreiben“ ohne „Löschen“, **When** er eine Datei löscht,
   die Anna angelegt hat, **Then** wird sie bei niemandem gelöscht, sie kommt
   in Bens Ordner zurück, und holzi zeigt ihm, dass er fremde Dateien nicht
   löschen darf.
6. **Given** Anna und Ben sind nie gleichzeitig online, **When** der Space ein
   Postfach am Relay und ein Speicher-Backend hat, **Then** erreichen
   Änderungen des einen den anderen trotzdem über Relay und Speicher.

---

### User Story 4 - Ein Mitglied entfernen (Priority: P1)

Anna entfernt eine frühere Mitbewohnerin aus dem Space „WG“. Ab diesem Moment
weisen Relay und Speicher deren Geräte ab. Neue Dateien und neue Fassungen kann
sie nicht lesen, auch wenn sie eine Kopie davon irgendwo erwischt. Was schon in
ihrem Ordner lag, bleibt bei ihr.

**Why this priority**: Ohne verlässliches Entfernen kann niemand einem Space
vertrauen, dessen Mitglieder sich ändern.

**Independent Test**: Drei Vaults A (Admin), B und C in einem Space. A entfernt
C. Danach fügt B eine Datei hinzu und ändert eine alte. Die Geräte von C
bekommen vom Relay und Speicher nichts mehr; selbst mit einer Kopie des
Postfachs und des Speichers kann C weder die neue Datei noch die neue Fassung
entschlüsseln. B und A arbeiten ohne Unterbrechung weiter.

**Acceptance Scenarios**:

1. **Given** Anna ist Admin, **When** sie ein Mitglied entfernt und die
   Rückfrage bestätigt, **Then** verschwindet es aus der Mitgliederliste, eine
   neue Schlüsselgeneration nur für die verbleibenden Mitglieder entsteht, und
   eine neue Mitgliederliste geht an das Relay.
2. **Given** die neue Mitgliederliste ist am Relay, **When** ein Gerät des
   entfernten Mitglieds Änderungen abholen, hochladen oder Objekte laden will,
   **Then** weisen Relay und Speicher es ab.
3. **Given** ein entferntes Mitglied, **When** nach dem Entzug eine Datei
   hinzukommt oder geändert wird, **Then** ist sie mit einem Schlüssel
   verschlüsselt, den das entfernte Mitglied nie erhalten hat.
4. **Given** ein entferntes Mitglied, **When** eines seiner Geräte danach mit
   einem Gerät eines Mitglieds direkt Verbindung aufnimmt, **Then** gibt das
   Mitglied ihm keine Daten des Space und nimmt keine an.
5. **Given** ein entferntes Mitglied, **When** eines seiner Geräte vom Entzug
   erfährt, **Then** zeigt holzi den Space als „entfernt“, synchronisiert ihn
   nicht mehr und lässt die Dateien im Ordner liegen.
6. **Given** Annas Gerät ist beim Entfernen offline, **When** es wieder online
   ist, **Then** lädt es die neue Mitgliederliste hoch, bevor es andere
   Änderungen des Space verschickt.

---

### User Story 5 - Rechte eines Mitglieds ändern (Priority: P2)

Anna gibt Ben zusätzlich „Löschen“, damit er den Ordner aufräumen kann. Einer
Tante, die versehentlich Dateien überschrieben hat, nimmt sie „Schreiben“ weg.
Jede Wahl gilt sofort.

**Why this priority**: Rechte ändern sich im Leben eines geteilten Ordners;
ohne diese Story bliebe nur Entfernen und neu Einladen.

**Independent Test**: Ein Mitglied von „Lesen“ auf „Lesen und Schreiben“ setzen:
Seine neuen Dateien erscheinen bei allen. Auf „Lesen“ zurücksetzen: Danach
geschriebene Änderungen kommen bei keinem anderen Mitglied an. Auf „Lesen,
Schreiben und Löschen“ setzen: Es kann fremde Dateien löschen.

**Acceptance Scenarios**:

1. **Given** die Mitgliederliste, **When** Anna die Fähigkeitsstufe eines
   Mitglieds wählt, **Then** gilt die Wahl sofort, ohne Knopf zum Übernehmen,
   mit neuer Mitgliederliste und neuer Schlüsselgeneration.
2. **Given** Ben hat nun „Löschen“, **When** er eine Datei löscht, die Anna
   angelegt hat, **Then** verschwindet sie bei allen Mitgliedern.
3. **Given** Anna nimmt der Tante „Schreiben“, **When** die Tante danach eine
   Datei ändert, **Then** übernimmt kein anderes Mitglied diese Änderung (siehe
   FR-024 zu einer gleichzeitig geschriebenen letzten Änderung).
4. **Given** Anna ändert Rechte, **When** das betroffene Mitglied das nächste
   Mal online ist, **Then** zeigt holzi ihm seine neuen Rechte.
5. **Given** Anna selbst, **When** sie die Mitgliederliste ansieht, **Then**
   kann sie ihre eigenen Rechte nicht ändern und sich nicht entfernen.

---

### User Story 6 - Sehen, wer im Space ist und was man darf (Priority: P2)

Ben öffnet den Space und sieht, wer dabei ist, wer der Admin ist und welche
Rechte er selbst hat. Als Mitglied mit nur „Lesen“ legt seine Frau eine Datei in
ihren Ordner; holzi zeigt ihr, dass diese Datei nur bei ihr liegt.

**Why this priority**: Wer nicht sieht, was er darf, wundert sich über Dateien,
die nicht ankommen.

**Independent Test**: Als Mitglied mit „Lesen“ den Space öffnen: Die Liste zeigt
alle Mitglieder mit Namen und Rechten und den Admin. Eine Datei in den Ordner
legen: Sie erreicht niemanden und ist als „nur lokal“ markiert.

**Acceptance Scenarios**:

1. **Given** ein Mitglied, **When** es den Space öffnet, **Then** sieht es Name
   des Space, gebundenen Ordner, Speicher-Backend, Admin, alle Mitglieder mit
   Namen, Vault-Identität und Fähigkeiten sowie seine eigenen Rechte
   hervorgehoben.
2. **Given** ein Mitglied mit nur „Lesen“, **When** es eine Datei in seinen
   Ordner legt, **Then** wird sie nicht übertragen, und holzi markiert sie als
   „nur lokal, keine Schreibrechte“.
3. **Given** ein Mitglied mit nur „Lesen“, **When** es eine Datei des Space
   ändert oder löscht, **Then** erreicht die Änderung niemanden; der Ordner
   bekommt die Fassung des Space zurück, eine geänderte Fassung bleibt als
   lokale Konfliktkopie erhalten.
4. **Given** der Admin ist länger offline, **When** ein Mitglied den Space
   öffnet, **Then** arbeitet es mit seinen bisherigen Rechten ungehindert
   weiter.

---

### User Story 7 - Einen Space selbst verlassen (Priority: P3)

Die Bekannte, die Annas Einladung einmal angenommen hat, braucht den Space nicht
mehr. Sie verlässt ihn selbst. Ihre Geräte synchronisieren ihn nicht mehr, die
Dateien in ihrem Ordner bleiben als gewöhnliche Dateien liegen. Anna sieht, dass
sie gegangen ist.

**Why this priority**: Ohne diese Story müsste jedes Mitglied den Admin bitten,
es zu entfernen.

**Independent Test**: Als Mitglied „Space verlassen“ wählen und bestätigen: Auf
keinem Gerät dieser Vault wird der Space weiter synchronisiert, der Ordner
behält seine Dateien. Sobald ein Gerät des Admins online ist, fehlt das Mitglied
in der Mitgliederliste, und eine neue Schlüsselgeneration ist entstanden.

**Acceptance Scenarios**:

1. **Given** ein Mitglied, das nicht Admin ist, **When** es „Space verlassen“
   wählt und bestätigt, **Then** hören alle Geräte seiner Vault auf, den Space
   zu synchronisieren, und der Admin erhält eine Nachricht darüber.
2. **Given** die Nachricht ist beim Admin angekommen, **When** ein Gerät des
   Admins online ist, **Then** entfernt es das Mitglied wie in User Story 4,
   ohne Zutun des Admins.
3. **Given** der Admin, **When** er seinen eigenen Space ansieht, **Then** gibt
   es für ihn kein „Space verlassen“.

---

### Edge Cases

- **Eingeladene Vault offline**: Die Einladung wartet, bis ein Gerät der
  eingeladenen Vault online ist. Sie bleibt offen, bis sie angenommen,
  abgelehnt oder vom Admin durch Entfernen zurückgezogen wird. Bis dahin steht
  die Vault als „eingeladen“ in der Mitgliederliste.
- **Einladung wird zurückgezogen, bevor sie ankommt**: Kommt die Einladung
  danach an, zeigt holzi sie nicht mehr an oder als „zurückgezogen“; es entsteht
  kein Space auf den Geräten der Eingeladenen.
- **Zwei Nutzer ändern dieselbe Datei gleichzeitig**: Es entsteht genau eine
  Konfliktkopie (FR-029). Ändern mehr als zwei Mitglieder gleichzeitig, entsteht
  je unterlegener Fassung eine Konfliktkopie.
- **Einer ändert eine Datei, ein anderer löscht sie gleichzeitig**: Die Änderung
  geht nicht verloren; die geänderte Fassung bleibt als Datei im Space (Spec 025
  für denselben Fall bei eigenen Geräten gilt entsprechend).
- **Entferntes Mitglied ist noch online und schreibt weiter**: Seine Geräte
  wissen vielleicht noch nichts vom Entzug. Das Relay nimmt seine Pakete nicht
  mehr an, Mitglieder geben ihm direkt keine Daten und nehmen keine an. Eine
  Änderung, die es gleichzeitig mit dem Entzug geschrieben hat, kann noch
  angenommen werden (FR-024); alles Spätere wird verworfen.
- **Die Geräte des Admins ändern gleichzeitig Rechte**: Anna entfernt Ben auf
  dem Laptop, während sie auf dem Rechner Carlas Rechte ändert. Beide Geräte
  erzeugen eine neue Schlüsselgeneration; beide gelten. Sobald die Änderungen
  zusammenkommen, gilt eine gemeinsame Mitgliederliste ohne Ben, und keine
  Generation, die noch Ben einen Schlüssel gibt, wird mehr zum Verschlüsseln
  benutzt (FR-021, FR-022).
- **Mitglied mit „Lesen“ lädt hoch**: Die Datei bleibt lokal und ist als „nur
  lokal“ markiert (US6). Versucht ein veränderter Client trotzdem, ein Paket
  oder Objekt hochzuladen, weist das Relay es ab; kommt es auf einem anderen Weg
  an, verwirft jedes Mitglied es (FR-026).
- **Relay lässt eine unberechtigte Änderung durch**: Jedes empfangende Gerät
  prüft Unterschrift, Gerätebestätigung, Fähigkeit und Ersteller selbst und
  verwirft die Änderung. Sie wird nicht angewendet, und holzi vermerkt sie im
  Protokoll, ohne den Nutzer mit Fehlermeldungen zu überhäufen.
- **Relay hält Daten zurück oder liefert Altes**: Das Relay kann Daten nicht
  fälschen, aber zurückhalten. Geräte erkennen Lücken und holen fehlende Daten
  direkt von anderen Mitgliedern, wenn diese online sind.
- **Speicher liefert ein falsches Objekt**: Das Objekt passt nicht zu seinem
  Namen oder lässt sich nicht entschlüsseln; das Gerät verwirft es und holt es
  anderswo.
- **Die Vault des Admins geht verloren**: Der Space ist eingefroren. Mitglieder
  lesen und schreiben mit ihren bisherigen Rechten weiter, aber niemand kann
  einladen, Rechte ändern oder entfernen. Eine Mitgliederliste mit Ablaufzeit
  am Relay läuft dann irgendwann ab; wie lange sie gilt, regelt Spec 026. Danach
  bleibt nur die direkte Übertragung.
- **Ein Mitglied rotiert seine Vault-Identität** (Gerät verloren): Es gilt die
  Übergabe aus Spec 024. Die Vault veröffentlicht für den Space eine von der
  alten Identität unterschriebene Übergabe an die neue. Der Admin und die
  anderen Mitglieder übernehmen die neue Identität erst, nachdem sie sie per
  Prüfcode bestätigt haben, weil auch der Dieb den alten Schlüssel hat. Danach
  ersetzt ein Gerät des Admins die alte Identität in der Mitgliederliste; das
  ist eine Änderung der Mitgliederliste nach FR-019.
- **Der Admin rotiert seine Vault-Identität**: Es gilt ebenfalls Spec 024. Das
  Relay nimmt die Übergabe für den an die Admin-Identität gebundenen Bereich an
  (Spec 026 FR-021). Kommen zwei konkurrierende Übergaben für dieselbe alte
  Identität an, friert das Relay den Bereich ein und nimmt keine neue
  Mitgliederliste an, bis die Clients der Mitglieder den Konflikt lösen; holzi
  zeigt den Konflikt an.
- **Admin lädt eine Vault ein, die schon Mitglied ist**: holzi lehnt das mit
  dem Hinweis ab, die Rechte stattdessen zu ändern.
- **Ein Gerät eines Mitglieds wurde nie an einen Ordner gebunden**: Es zeigt den
  Space als „noch nicht gebunden“ und lädt nichts herunter.
- **Der gebundene Ordner wird gelöscht oder ist nicht erreichbar**: Es gilt
  Spec 025; der Space wird auf diesem Gerät angehalten, nicht geleert, und
  Löschungen werden nicht an andere Mitglieder weitergegeben.
- **Kein Relay eingerichtet**: Der Space synchronisiert nur direkt (FR-040);
  Änderungen erreichen ein Mitglied erst, wenn eines seiner Geräte gleichzeitig
  mit einem Gerät online ist, das sie hat.
- **Speicher-Backend voll oder Kontingent erschöpft**: Neue Dateien bleiben
  lokal mit Hinweis beim Mitglied, das sie hinzufügen wollte; die direkte
  Übertragung zu gleichzeitig online befindlichen Mitgliedern bleibt möglich.

## Requirements _(mandatory)_

### Functional Requirements

**Space anlegen**

- **FR-001**: Nutzer MÜSSEN einen Space mit einem Namen anlegen können. Die Vault,
  auf der er angelegt wird, ist sein Admin; das ist für die Lebensdauer des
  Space unveränderlich.
- **FR-002**: Beim Anlegen MUSS der Nutzer einen lokalen Ordner wählen, an den
  der Space auf diesem Gerät gebunden wird. Jedes weitere Gerät eines Mitglieds
  MUSS den Space an einen eigenen, frei gewählten Ordner binden können; bis
  dahin lädt es nichts herunter. Die Bindung ist gerätebezogen und geht nicht
  an andere Geräte.
- **FR-003**: Beim Anlegen MUSS der Nutzer das Speicher-Backend des Space
  wählen; es gibt keine Vorauswahl. Zur Wahl stehen Speicher-Backend A (nur
  wenn ein Relay eingerichtet ist) oder „nur direkte Übertragung“, nach
  Spec 029 zusätzlich Speicher-Backend B. „Nur direkte Übertragung“ heißt:
  Objekte reisen nur zwischen Geräten; das Postfach darf das Relay trotzdem
  nutzen (FR-040). Unabhängig von der Wahl MÜSSEN Geräte von Mitgliedern
  Objekte und Änderungspakete direkt austauschen können, wenn sie gleichzeitig
  online sind (FR-039).
- **FR-004**: Beim Anlegen MUSS holzi die erste Schlüsselgeneration und die
  erste Mitgliederliste (nur der Admin, mit allen Fähigkeiten) erstellen und die
  Mitgliederliste an das Relay des Space hochladen, sofern die Vault des Admins
  eines eingerichtet hat (FR-040).
- **FR-005**: Name des Space, Ordner- und Dateinamen, Pfade, Größen,
  Änderungszeiten und Inhalte DÜRFEN das Gerät nur verschlüsselt verlassen.
  Relay und Speicher-Backend sehen davon nichts außer der Länge des
  Verschlüsselten.
- **FR-006**: Ein Space DARF NUR Dateien enthalten. Daten aus der
  SQLite-Datenbank der Vault und Schlüssel der Vault DÜRFEN NIE in einen Space
  gelangen.

**Übertragung**

- **FR-039** (direkte Verbindung zwischen Mitgliedern): Geräte verschiedener
  Vaults, die Mitglieder desselben Space sind, MÜSSEN sich für diesen einen
  Bereich direkt verbinden können. Beim Verbindungsaufbau MUSS jedes Gerät
  seinen Geräteschlüssel und eine Gerätebestätigung einer Vault nachweisen,
  die auf der aktuellen Mitgliederliste dieses Bereichs steht; die Gegenseite
  prüft beides gegen die neueste ihr bekannte Mitgliederliste und bricht die
  Verbindung sonst ab. Eine solche Verbindung DARF NUR Daten dieses einen
  Bereichs tragen, nie Daten des Bereichs Vault oder eines anderen Bereichs.
  Geräte finden einander über eine verschlüsselte Anwesenheitsmeldung, die an
  die Mitglieds-Vaults des Bereichs adressiert ist (über die Anwesenheits- und
  Signalisierungsserver aus Spec 024), oder über die Signalisierung des Relays
  (Spec 026 FR-009). Datenfreigaben (Spec 028) und Spaces mit
  Speicher-Backend B (Spec 029) nutzen dieselbe Regel für ihre Bereiche.
- **FR-040** (Postfach getrennt vom Objektspeicher): Das Postfach eines Space (Änderungspakete mit Dateiindex und
  Mitgliederliste) MUSS auf dem Relay liegen, das die Vault des Admins
  eingerichtet hat, unabhängig davon, wo die Objekte liegen. Hat die Vault des
  Admins kein Relay eingerichtet, MUSS der Space nur direkt synchronisieren
  (FR-039); Mitglieder müssen dann gleichzeitig online sein, und die Ansicht
  des Space MUSS das anzeigen.

**Einladen**

- **FR-007**: Nur der Admin DARF einladen. holzi DARF Mitgliedern, die nicht
  Admin sind, keine Möglichkeit zum Einladen oder Weiterteilen anbieten, und
  Mitglieder MÜSSEN jede Mitgliederliste verwerfen, die nicht der Admin
  unterschrieben hat.
- **FR-008**: Der Admin MUSS ein Mitglied über dessen Vault-Identität einladen
  können, mit einem Namen, unter dem es im Space erscheint, und einer
  Fähigkeitsstufe (FR-015). holzi MUSS ungültige Vault-Identitäten, die eigene
  und die eines bestehenden Mitglieds abweisen.
- **FR-009**: Eine Einladung MUSS die eingeladene Vault in die Mitgliederliste
  aufnehmen, eine neue Schlüsselgeneration für die neue Mitgliederliste
  erzeugen, deren Schlüssel und die Schlüssel aller älteren Generationen an
  die eingeladene Vault verpacken und der eingeladenen Vault eine
  verschlüsselte Direktnachricht mit Kennung und Name des Space, Admin,
  Fähigkeiten und Hinweisen auf das Relay senden (FR-041).
- **FR-010**: Die Einladung MUSS auf allen Geräten der eingeladenen Vault
  angezeigt werden, sobald eines davon online ist, mit Name des Space, Name und
  Vault-Identität des Admins und den eigenen Fähigkeiten, und mit „Annehmen“
  und „Ablehnen“. Sie MUSS offen bleiben, bis sie angenommen, abgelehnt oder
  zurückgezogen ist.
- **FR-011**: Nimmt die eingeladene Vault an, MUSS das annehmende Gerät nach
  einem Ordner fragen und dann alle aktuellen Dateien des Space erhalten
  können, auch die, die vor der Einladung hinzugekommen sind. Der Admin MUSS
  die Annahme angezeigt bekommen, sobald sie ihn erreicht.
- **FR-012**: Lehnt die eingeladene Vault ab, MUSS die Einladung auf allen
  ihren Geräten verschwinden, und der Admin MUSS darüber eine Nachricht
  erhalten. Das nächste online befindliche Gerät des Admins MUSS die Vault dann
  entfernen wie in FR-018 und sie als „abgelehnt“ anzeigen.
- **FR-013**: Der Admin MUSS eine offene Einladung zurückziehen können; das ist
  ein Entfernen nach FR-018.
- **FR-014**: Nimmt eine Vault an, MÜSSEN alle Geräte dieser Vault den Space
  erhalten, ohne dass der Admin etwas tut. Der Admin DARF NICHT erfahren, wie
  viele Geräte die Vault hat.
- **FR-041** (Zustellung von Einladungen): Einladungen und die Antworten darauf
  („angenommen“, „abgelehnt“, „verlassen“) MÜSSEN als verschlüsselte
  Nostr-Nachricht an die Vault-Identität des Empfängers gehen. Zugestellt
  werden sie über die Anwesenheits- und Signalisierungsserver aus Spec 024 und
  zusätzlich über die Signalisierung des Relays des Admins, wenn dieses sie
  anbietet (Spec 026 FR-009). Die Server DÜRFEN vom Inhalt nichts sehen außer
  Empfänger, Zeit und Länge. Solange keine Antwort eingetroffen ist, MÜSSEN
  Geräte des Admins die Zustellung einer offenen Einladung wiederholen, wenn
  sie online sind. Datenfreigaben (Spec 028) nutzen denselben Weg.

**Fähigkeiten**

- **FR-015**: Jedes Mitglied MUSS genau eine Fähigkeitsstufe haben: „Lesen“,
  „Lesen und Schreiben“ oder „Lesen, Schreiben und Löschen“. Der Admin hat
  immer alle Fähigkeiten; die Rolle Admin ist nicht vergebbar.
- **FR-016**: Die Fähigkeiten MÜSSEN so wirken:
  - **Lesen**: alle Dateien des Space lesen.
  - **Schreiben**: neue Dateien hinzufügen, bestehende Dateien ändern,
    umbenennen und verschieben, auch fremde; eigene Dateien löschen.
  - **Löschen**: jede Datei des Space löschen, auch fremde.

  Löschen entfernt eine Datei aus dem Dateiindex. Das Objekt dazu entfernt
  später ein Gerät des Admins (FR-042); andere Mitglieder löschen keine
  Objekte selbst. Die Löschregel des Relays für Objekte (hochladende Vault,
  Vaults mit „Löschen“, Admin; Spec 026 FR-028) widerspricht dem deshalb
  nicht.

- **FR-017**: Der Admin MUSS die Fähigkeitsstufe jedes Mitglieds außer seiner
  eigenen ändern können. Die Wahl MUSS sofort gelten, ohne Knopf zum
  Übernehmen, und eine neue Mitgliederliste und eine neue Schlüsselgeneration
  erzeugen (FR-019).

**Entzug und Schlüssel**

- **FR-018**: Der Admin MUSS jedes Mitglied außer sich selbst nach einer
  Rückfrage entfernen können. Entfernen MUSS eine neue Mitgliederliste ohne
  das Mitglied und eine neue Schlüsselgeneration nur für die verbleibenden
  Mitglieder erzeugen.
- **FR-019**: Jede Änderung der Mitgliederliste (Einladen, Rechte ändern,
  Entfernen, Verlassen, Ablehnen) MUSS eine neue Schlüsselgeneration erzeugen,
  deren Schlüssel an jede Vault der neuen Mitgliederliste verpackt wird, einmal
  je Vault, nie je Gerät. Die Mitgliederliste einer Generation steht danach
  fest. Kommt ein Mitglied neu hinzu, MUSS es zusätzlich Umschläge für alle
  älteren Generationen erhalten, damit es ältere Inhalte lesen kann. Nur
  Geräte des Admins DÜRFEN Mitgliederlisten und Schlüsselgenerationen eines
  Space erstellen.
- **FR-020**: Hat der Space ein Postfach am Relay (FR-040), MUSS das Gerät des
  Admins eine geänderte Mitgliederliste an das Relay hochladen, bevor es neue Inhalte mit der neuen Generation verschickt,
  und sofort, sobald es online ist, wenn die Änderung offline geschah.
- **FR-021**: Erzeugen Geräte des Admins unabhängig voneinander neue
  Mitgliederlisten, MÜSSEN nach dem Zusammenführen alle Geräte dieselbe
  Mitgliederliste ergeben, und ein Gerät des Admins MUSS sie mit einer höheren
  Generation als jede der zusammengeführten neu unterschreiben und hochladen. Alle
  gleichzeitig entstandenen Schlüsselgenerationen MÜSSEN gültig bleiben, so
  dass Inhalte, die mit einer davon verschlüsselt sind, lesbar bleiben.
- **FR-022**: Ein Gerät DARF zum Verschlüsseln nur eine Schlüsselgeneration
  verwenden, die an keine Vault außerhalb der aktuellen Mitgliederliste
  verpackt ist; unter diesen die höchste nach einer festen, auf allen Geräten
  gleichen Regel. Gibt es keine solche Generation, MUSS ein Gerät des Admins
  eine neue erzeugen.
- **FR-023**: Nach einem Entzug MÜSSEN Relay und Speicher-Backend die Geräte
  des entfernten Mitglieds abweisen, sobald die neue Mitgliederliste
  hochgeladen ist. Geräte von Mitgliedern DÜRFEN Daten des Space direkt nur mit
  Geräten von Vaults austauschen, die in ihrer aktuellen Mitgliederliste
  stehen (FR-039).
- **FR-024**: Eine Änderung MUSS angenommen werden, wenn ihr Autor in der
  Schlüsselgeneration, mit der sie verschlüsselt ist, die nötige Fähigkeit
  hatte. Ein Entzug wirkt nur nach vorn; eine gleichzeitig mit dem Entzug
  geschriebene letzte Änderung kann noch angenommen werden. Änderungen, die ein
  Mitglied erreichen, nachdem es selbst eine neuere Mitgliederliste ohne das
  Recht des Autors kennt, und deren Zeitstempel nach dieser Änderung der
  Mitgliederliste liegt, MÜSSEN verworfen werden.

- **FR-025**: Ein entferntes Mitglied DARF keine Datei und keine Fassung lesen
  können, die nach dem Entzug hinzukommt oder entsteht. Was es vor dem Entzug
  entschlüsseln konnte, behält es; holzi DARF NICHT versprechen, das zu
  verhindern. Sobald eines seiner Geräte vom Entzug erfährt, MUSS es den Space
  als „entfernt“ anzeigen, die Synchronisierung beenden und die Dateien im
  Ordner liegen lassen.
- **FR-026**: Jedes Gerät eines Mitglieds MUSS jede eingehende Änderung selbst
  prüfen, unabhängig davon, was Relay oder Speicher durchgelassen haben:
  Unterschrift des Geräts, Gerätebestätigung durch eine Vault der
  Mitgliederliste, die nötige Fähigkeit dieser Vault nach FR-024 und dass der
  Ersteller einer Datei unverändert bleibt. Eine Änderung, die eine Prüfung
  nicht besteht, MUSS verworfen und protokolliert werden. Dabei gilt Spec 024:
  Ein Änderungspaket ist unteilbar und wird als Ganzes verworfen, wenn eine
  seiner Änderungen ungültig ist; eine Momentaufnahme wird je Änderung geprüft,
  ungültige Änderungen fallen weg, der Rest bleibt.
- **FR-027**: Ist die Vault des Admins nicht erreichbar oder verloren, MÜSSEN
  Mitglieder mit ihren bisherigen Rechten weiterarbeiten können. Die
  Mitgliedschaft kann sich dann nicht ändern; holzi MUSS das nicht verhindern
  oder umgehen.

**Dateien**

- **FR-028**: Hinzufügen, Ändern, Umbenennen, Verschieben und Löschen von
  Dateien MÜSSEN wie in Spec 025 über den Dateiindex und unveränderliche
  Objekte laufen, für alle Mitglieder gleich. Jeder Eintrag MUSS die Vault
  nennen, die die Datei in den Space gebracht hat, und die, die sie zuletzt
  geändert hat.
- **FR-029**: Ändern zwei oder mehr Mitglieder dieselbe Datei, ohne die Änderung
  des anderen zu kennen, MUSS je unterlegener Fassung genau eine Konfliktkopie
  entstehen, auf allen Geräten aller Mitglieder dieselbe. Ihr Name MUSS dem
  Muster aus Spec 025 folgen, „<Name> (Konflikt <Gerätename> <Datum
  Uhrzeit>).<Endung>“, mit dem Anzeigenamen des Mitglieds, von dem die Fassung
  stammt, vor dem Gerätenamen; das genaue Format legt der Plan fest. Ihr
  Ersteller ist die Vault dieses Mitglieds. Keine Fassung DARF verloren gehen.
- **FR-030**: Änderungen im Ordner, die das Mitglied nicht darf, DÜRFEN NICHT
  übertragen werden:
  - Neue Dateien eines Mitglieds ohne „Schreiben“ bleiben lokal und MÜSSEN als
    „nur lokal, keine Schreibrechte“ markiert werden.
  - Ändert ein Mitglied ohne „Schreiben“ eine Datei des Space, MUSS der Ordner
    die Fassung des Space zurückbekommen; die lokale Fassung MUSS als lokale
    Konfliktkopie erhalten bleiben.
  - Löscht ein Mitglied eine Datei, die es nicht löschen darf, MUSS sie in
    seinen Ordner zurückkommen, mit Hinweis.
- **FR-031**: Hat der Space ein Postfach am Relay und ein Speicher-Backend,
  MÜSSEN Mitglieder, die nie gleichzeitig online sind, trotzdem alle
  Änderungen erhalten. Bei „nur direkter Übertragung“ mit Postfach erreichen
  Änderungen des Dateiindex alle Mitglieder über das Postfach, Objekte aber
  nur direkt; sie MÜSSEN übertragen werden, sobald zwei Geräte gleichzeitig
  online sind, die sie haben beziehungsweise brauchen. Ohne Relay gilt das für
  alle Daten des Space (FR-040).
- **FR-032**: Geräte MÜSSEN jedes erhaltene Objekt gegen seinen Namen prüfen und
  ein nicht passendes oder nicht entschlüsselbares Objekt verwerfen und anderswo
  holen.
- **FR-042** (Aufräumen alter Objekte): Objekte, auf die der Dateiindex nicht
  mehr verweist (ersetzte Fassungen, gelöschte Dateien), MUSS ein Gerät des
  Admins entfernen. Bei Speicher-Backend A löscht es sie am Relay, das dem
  Admin das Löschen erlaubt (Spec 026 FR-028); bei Speicher-Backend B löscht es
  sie mit den Zugangsdaten des Admins (Spec 029). Ein Objekt, auf das noch
  eine Konfliktkopie oder eine noch nicht zusammengeführte Fassung des
  Dateiindex verweisen kann, DARF NICHT entfernt werden; wann das sicher
  ausgeschlossen ist, legt der Plan fest.

**Verlassen**

- **FR-033**: Ein Mitglied, das nicht Admin ist, MUSS den Space nach einer
  Rückfrage verlassen können. Danach MÜSSEN alle Geräte seiner Vault die
  Synchronisierung beenden und die Dateien im Ordner liegen lassen, und der
  Admin MUSS eine Nachricht erhalten, auf die hin ein Gerät des Admins das
  Mitglied nach FR-018 entfernt.
- **FR-034**: Der Admin DARF den eigenen Space nicht verlassen.

**Anzeige**

- **FR-035**: holzi MUSS eine Übersicht aller Spaces der Vault zeigen, mit Name,
  Rolle (Admin oder Mitglied), eigener Fähigkeitsstufe und Zustand auf diesem
  Gerät („synchronisiert“, „noch nicht gebunden“, „angehalten“, „entfernt“).
  Offene Einladungen stehen darüber.
- **FR-036**: Die Ansicht eines Space MUSS Name, gebundenen Ordner dieses Geräts,
  Speicher-Backend, den Admin und alle Mitglieder mit Namen, Vault-Identität,
  Fähigkeitsstufe und Zustand („eingeladen“, „Mitglied“) zeigen, dem Admin
  zusätzlich abgelehnte Einladungen als „abgelehnt“; die eigene Zeile MUSS
  hervorgehoben sein. Nur der Admin sieht die Bedienelemente
  zum Einladen, Ändern und Entfernen.
- **FR-037**: Die Ansichten MÜSSEN dem Aufbau der Einstellungs-App folgen
  (Spec 023: Werkzeugleiste, großer Titel, abgerundete Gruppen mit Zeilen) und
  Orte im Tab sein (Spec 020). Sie liegen in der Einstellungskategorie
  „Föderation“ (Spec 023), Unteransicht „Spaces“; das Speicher-Backend eines
  Space wird in dessen Ansicht eingestellt. Anlegen, Einladen, Rechte ändern, Entfernen,
  Annehmen, Ablehnen und Verlassen MÜSSEN Aktionen im Katalog von Spec 020
  sein.
- **FR-038**: holzi MUSS dem betroffenen Mitglied zeigen, wenn sich seine Rechte
  geändert haben oder es entfernt wurde, sobald eines seiner Geräte davon
  erfährt.

### Key Entities

- **Space**: Kennung, Name (nur verschlüsselt außerhalb der Geräte), Admin
  (Vault-Identität), Speicher-Backend, Dateiindex, Mitgliederlisten und
  Schlüsselgenerationen. Der Bereich eines Space, mit eigenem Postfach am
  Relay, sofern der Admin eines eingerichtet hat.
- **Mitgliederliste**: Kennung des Space, Generation, Einträge
  „Vault-Identität → Fähigkeitsstufe“, Ausstellungs- und Ablaufzeit,
  Unterschrift des Admins. Verschlüsselt im Space; eine lesbare Fassung am
  Relay (Spec 026).
- **Mitglied**: Vault-Identität, Name im Space, Fähigkeitsstufe, Zustand
  („eingeladen“, „Mitglied“, „abgelehnt“). Namen stehen nur im verschlüsselten
  Teil.
- **Schlüsselgeneration**: Nummer, zugehörige Mitgliederliste, erstellendes
  Gerät des Admins, Inhaltsschlüssel verpackt je Vault der Mitgliederliste und
  je später hinzugekommenem Mitglied. Mehrere Generationen gelten
  nebeneinander.
- **Einladung**: verschlüsselte Direktnachricht (FR-041) an eine Vault-Identität mit
  Kennung und Name des Space, Admin, Fähigkeitsstufe und Hinweisen auf das
  Relay; Antwort „angenommen“, „abgelehnt“ oder „verlassen“ zurück an den Admin.
- **Ordnerbindung**: je Gerät und Space der gewählte lokale Ordner. Verlässt
  das Gerät nie (Spec 025).
- **Dateiindex-Eintrag, Objekt, Konfliktkopie**: wie in Spec 025, ergänzt um
  Ersteller-Vault und letzte Bearbeiter-Vault.

## Success Criteria _(mandatory)_

### Measurable Outcomes

- **SC-001**: Ein entferntes Mitglied kann in 100 % der Fälle keine Datei und
  keine Fassung lesen, die nach dem Entzug hinzukommt oder entsteht, auch mit
  einer vollständigen Kopie von Postfach und Speicher-Backend.
- **SC-002**: Eine Untersuchung von Relay und Speicher-Backend findet in 0 Fällen
  Namen von Spaces, Ordnern oder Dateien, Pfade, Klartextgrößen oder Inhalte.
- **SC-003**: 100 % der Änderungen, die ein Autor ohne die nötige Fähigkeit
  verschickt (nur „Lesen“, fremde Datei ohne „Löschen“, nach dem Entzug, fremde
  Vault), werden von jedem Gerät jedes Mitglieds verworfen, auch wenn das Relay
  sie weitergibt.
- **SC-004**: Ist ein Gerät des Admins online, weist das Relay ein entferntes
  Mitglied spätestens 10 Sekunden nach dem Bestätigen des Entfernens ab.
- **SC-005**: Ein Admin lädt jemanden in unter 1 Minute ein, ausgehend von der
  Übersicht der Spaces; sind beide online, sieht die eingeladene Vault die
  Einladung in unter 30 Sekunden.
- **SC-006**: Bei gleichzeitigen Änderungen derselben Datei gehen in 0 % der
  Fälle Fassungen verloren, und jede Konfliktkopie existiert auf allen Geräten
  aller Mitglieder genau einmal.
- **SC-007**: Ein Mitglied, das ein weiteres Gerät zu seiner Vault hinzufügt,
  erhält dort in 100 % der Fälle Zugang zum Space, ohne dass der Admin etwas
  tut.
- **SC-008**: Nach einer Änderung der Mitgliederliste können in 100 % der Fälle
  alle verbleibenden Mitglieder weiterlesen und -schreiben, auch Inhalte, die
  mit gleichzeitig entstandenen Schlüsselgenerationen verschlüsselt sind.
- **SC-009**: Entfernen eines Mitglieds aus einem Space mit 20 Mitgliedern
  dauert auf dem Gerät des Admins höchstens 5 Sekunden bis zum Hochladen der
  neuen Mitgliederliste.

## Assumptions

- Spec 024 liefert Vault-Identität, Geräteschlüssel, Gerätebestätigungen und die
  Synchronisierung eigener Geräte; Spec 025 Dateiindex, Objekte, Konfliktkopien
  und die Bindung an einen lokalen Ordner; Spec 026 Relay, Postfächer, die
  Prüfung der Mitgliederliste und Speicher-Backend A.
- Wie für eigene Geräte (Spec 025) wählt jedes Gerät den lokalen Ordner selbst.
- Die Vault-Identität eines anderen Nutzers kommt auf einem Weg außerhalb von
  holzi zum Admin (Kopieren, QR-Code). Spec 024 sorgt dafür, dass ein Nutzer
  seine eigene Vault-Identität anzeigen und kopieren kann. Ein Adressbuch ist
  nicht Teil dieser Spec.
- Ein Space nutzt das Relay, das die Vault des Admins eingerichtet hat; die
  Einladung nennt es. Ob ein Bereich zugleich auf mehreren Relays liegen darf,
  ist die offene Frage aus Spec 026 FR-040; sie gilt für Spaces ebenso und wird
  hier nicht getrennt gestellt.
- Das Speicher-Backend wird beim Anlegen gewählt (FR-003). Den Wechsel des
  Speicher-Backends eines bestehenden Space bringt Spec 029 (User Story 8).
- Neue Mitglieder lesen alle Dateien, auch ältere, weil sie Umschläge für alle
  älteren Schlüsselgenerationen erhalten (FR-009, FR-019).
- Die Regel nach FR-024 lässt eine Lücke: Ein entferntes Mitglied, das noch
  ältere Schlüssel hat, könnte einer Änderung einen frühen Zeitstempel geben.
  Das Relay nimmt seine Pakete nicht mehr an, und Mitglieder tauschen mit ihm
  direkt nichts aus (FR-023), so dass solche Änderungen nur über ein Mitglied
  ankommen, das sie vor dem Entzug erhalten hat. Die Betreiberentscheidung zu
  FR-024 muss diese Lücke mit bewerten.
- Die Verschlüsselung versteckt nicht, wer Mitglied ist: Das Relay sieht die
  Mitgliederliste (Vault-Identitäten und Fähigkeiten), Zeiten und Größen (D9).
- Das Relay kann Daten zurückhalten, aber weder lesen noch fälschen (D11).
  Verfügbarkeit sichern direkte Übertragung und mehrere Geräte.
- Die Kosten einer Rotation wachsen mit der Zahl der Mitglieds-Vaults, nicht
  der Geräte. Für v1 wird mit Spaces bis etwa 20 Mitgliedern gerechnet.

## Nicht im Umfang

- Weiterteilen durch Mitglieder oder Einladen durch andere als den Admin (D7).
  Dass jemand Dateien herunterlädt und anderswo erneut teilt, wird nicht
  verhindert.
- Admin übertragen, mehrere Admins oder vergebbare Admin-Rechte (D6, Entwurf
  §15 Punkt 3). Geht die Vault des Admins verloren, ist der Space eingefroren.
- Vollständiges Neuverschlüsseln alter Dateien nach einem Entzug.
- Daten aus der SQLite-Datenbank in Spaces (D3; dafür Spec 028).
- Rechte je Unterordner oder je Datei; Rechte gelten für den ganzen Space.
- Einen Space auflösen oder löschen. Das Speicher-Backend eines bestehenden
  Space zu wechseln kommt mit Spec 029 (User Story 8).
- Papierkorb oder Wiederherstellen gelöschter Dateien (für Speicher-Backend B
  siehe Spec 029).
- Verbergen der Mitglieder vor dem Relay (Pseudonyme je Bereich, D9).
