# Feature Specification: Datenfreigaben: Daten von Erweiterungen mit einzelnen Nutzern teilen

**Feature Branch**: `028-data-shares`
**Created**: 2026-09-28
**Status**: Draft
**Input**: Umsetzung von Zeile 028 des Sync-Entwurfs
([`2026-09-28-sync-architecture.md`](../../docs/plans/2026-09-28-sync-architecture.md),
§9 mit §5.4, §10.3, §11 und §13). Erweiterungen erklären in ihrem Manifest mit
einem einheitlichen Schema, was sich teilen lässt: einzelne Einträge (ein
Kalendereintrag) oder ganze Sammlungen (ein Kalender). holzi prüft diese
Erklärung bei Installation und Update, bietet einen eigenen Teilen-Dialog an,
vergibt Rechte je Person, berechnet selbst, welche Einträge zu einer Freigabe
gehören, und schützt beim Senden und beim Empfangen, dass nur diese Einträge
ausgetauscht werden. Die Erweiterung selbst kommt nie mit Schlüsseln,
Synchronisierung oder Rechten in Berührung.

## Begriffe

- **Vault-Identität**: der Schlüssel, der eine Vault nach außen vertritt. Er ist
  auf allen Geräten derselben Vault gleich (Spec 024). Rechte und verschlüsselte
  Schlüssel gehen immer an eine Vault-Identität, nie an ein Gerät.
- **Geräteschlüssel**: der eigene Schlüssel eines Geräts; er unterschreibt die
  Änderungen, die auf diesem Gerät entstehen (Spec 024).
- **Gerätebestätigung**: die von der Vault-Identität unterschriebene Aussage,
  dass ein Geräteschlüssel zu dieser Vault gehört (Spec 024).
- **Bereich**: eine Einheit, die gemeinsam synchronisiert und verschlüsselt
  wird: der Bereich Vault, der Bereich eines Space oder der Bereich einer
  Datenfreigabe.
- **Änderungspaket**: eine verschlüsselte Menge zusammengehöriger Änderungen
  eines Bereichs, so wie sie zwischen Geräten und über das Relay reist. Ein
  Paket trennt nie Änderungen, die zusammen geschrieben wurden.
- **Inhaltsschlüssel**: der Schlüssel, mit dem die Änderungspakete eines
  Bereichs verschlüsselt sind. **Schlüsselgeneration**: eine Fassung davon;
  jede Änderung der Mitglieder (Einladen, Ändern von Fähigkeiten, Entfernen,
  Austreten) erzeugt eine neue, und ihre Mitgliederliste steht damit fest.
- **Relay**: der nicht vertrauenswürdige Server aus Spec 026. Es speichert und
  verteilt nur Verschlüsseltes. **Postfach**: der Speicherplatz eines Bereichs
  auf dem Relay. **Mitgliederliste**: die vom Eigentümer unterschriebene Liste
  der Vault-Identitäten und ihrer Fähigkeiten je Bereich, die auf das Relay
  hochgeladen wird. Ihre **Generation** ist die Schlüsselgeneration, zu der sie
  gehört.
- **Erweiterung**: eine haextension im Sinne von ADR-0004. **Eigene Tabellen**
  einer Erweiterung sind die Tabellen mit ihrem Präfix aus öffentlichem
  Schlüssel und Name der Erweiterung (dieselbe Konvention wie haex-vault,
  Repository `https://github.com/haex-space/haex-vault`, Revision
  `8dce379d94e18fcd42c3b73686a06f984ca3f574`, Pfad
  `src-tauri/src/extension/utils.rs`, `get_extension_table_prefix`).
- **Datenfreigabe**: das Teilen von SQLite-Daten einer Erweiterung mit
  einzelnen Nutzern. Sie hat genau einen **Eigentümer (Admin der
  Datenfreigabe)**, die Vault, die sie angelegt hat, und null oder mehr
  **Empfänger**. Diese Spec sagt im Folgenden kurz „Eigentümer“.
- **Freigabetyp**: eine Art teilbarer Daten, die eine Erweiterung erklärt,
  entweder ein einzelner Eintrag (ein Kalendereintrag) oder eine Sammlung (ein
  ganzer Kalender, eine Einkaufsliste).
- **Wurzel**: der Eintrag, den der Eigentümer teilt (der Kalender, der
  Kalendereintrag). **Zugehörige Einträge**: alle Einträge, die über die im
  Freigabetyp erklärten Fremdschlüssel von der Wurzel aus erreichbar sind (die
  Termine eines Kalenders, deren Teilnehmer und Erinnerungen).
- **Ersteller**: die Vault-Identität, die einen Eintrag angelegt hat. holzi
  setzt sie beim Anlegen, und sie ändert sich danach nie.
- **Fähigkeiten**: drei aufeinander aufbauende Stufen **Lesen**, **Schreiben**
  und **Löschen**, je Empfänger genau eine: Schreiben umfasst Lesen, Löschen
  umfasst Schreiben (wie bei Spaces, Spec 027). **Admin** ist allein der
  Eigentümer; Admin lässt sich nicht vergeben.
- **Space**: ein Netzwerkordner nur für Dateien (Spec 027). Spaces tragen nie
  SQLite-Daten.

## Beziehung zu bestehenden Specs

- **Sync-Entwurf** ([`2026-09-28-sync-architecture.md`](../../docs/plans/2026-09-28-sync-architecture.md)):
  Quelle dieser Spec. §9 beschreibt Erklärung, Prüfung, abgeleitete
  Zugehörigkeit, Schutz beim Senden und Empfangen und überlappende Freigaben,
  §11 die Fähigkeiten, §13 das Bedrohungsmodell. Die Entwurfsregeln zu Löschen
  und Entzug sind dort nur vorgeschlagen; diese Spec übernimmt sie als offene
  Fragen (FR-028, FR-034).
- **Früherer Entwurf zum Teilen zwischen Nutzern**
  ([`2026-09-07-cross-user-sharing-deferred-design.md`](../../docs/plans/2026-09-07-cross-user-sharing-deferred-design.md)):
  Die dort vorgesehene Liste, welche Zeile zu welchem Bereich gehört, entfällt.
  Die Zugehörigkeit eines Eintrags zu einer Datenfreigabe wird aus den
  erklärten Fremdschlüsseln berechnet (FR-012). Die Tabellenliste als Schutz vor
  Datenabfluss bleibt als Schutz beim Senden (FR-013).
- **Spec 024** (Vault-Identität, Geräteschlüssel und Sync zwischen eigenen
  Geräten): liefert Vault-Identität, Gerätebestätigung, unterschriebene
  Änderungen mit echtem Autor, das Führen des Erstellers durch holzi und die
  feste Liste, welche Daten die Vault überhaupt verlassen dürfen, einschließlich
  der Nur-direkt-Daten. Empfangene Datenfreigaben erreichen die übrigen Geräte
  des Empfängers über diesen Sync. Spec 024 legt außerdem fest, dass ein
  Änderungspaket nur als Ganzes angewendet wird, und wie eine Vault-Identität
  nach Verlust eines Geräts erneuert und in geteilten Bereichen übergeben wird.
- **Spec 026** (Relay): liefert Postfach und Mitgliederliste. Jede
  Datenfreigabe ist dort ein eigener Bereich. Das Relay prüft nur grob Lesen
  gegen Schreiben; alle feineren Regeln dieser Spec prüft jeder Empfänger
  selbst.
- **Spec 027** (Spaces): Spaces teilen Dateien, Datenfreigaben teilen Daten.
  Beide verwenden dieselben Fähigkeitsstufen, dieselbe Regel für eigene
  Einträge, dieselben Regeln für die Mitgliederliste und dasselbe Muster einer
  neuen Schlüsselgeneration bei jeder Änderung der Mitglieder. Diese Spec
  übernimmt aus Spec 027 den Weg für Einladungen und die direkte Verbindung
  zwischen Mitgliedern. Dateien, die ein Eintrag einer Erweiterung
  referenziert, teilt diese Spec nicht.
- **Spec 023** (Einstellungen): Die Verwaltung der Datenfreigaben liegt in der
  Einstellungskategorie „Föderation“ in der Unteransicht „Datenfreigaben“
  (FR-015).
- **ADR-0004** und die geplante Spec 017 (Erweiterungs-Host): Das signierte
  Manifest, die Bestätigung bei der Installation und die eine Prüfstelle für
  alle Anfragen einer Erweiterung kommen von dort. Diese Spec ergänzt das
  Manifest um Freigabetypen und die Brücke um zwei Anfragen: den Teilen-Dialog
  öffnen und den Freigabestatus eigener Einträge lesen. Die Tabellen einer
  Erweiterung mit Präfix, die synchronisiert werden, gibt es in holzi noch
  nicht (Entwurf §15 Punkt 10); diese Spec setzt sie voraus.
- **Spec 021** (Rechte für Agenten): Diese Spec gibt Agenten keinen Weg, eine
  Datenfreigabe anzulegen oder Rechte zu ändern (siehe Assumptions).

## Clarifications

### Session 2026-09-27

- Q: Werden SQLite-Daten über Spaces geteilt? → A: Nein (D3). Spaces sind
  Netzwerkordner nur für Dateien. Lesen und Schreiben von Daten wird einzeln je
  Nutzer vergeben, über Datenfreigaben.
- Q: Was lässt sich als Datenfreigabe teilen, und wer legt das fest? → A:
  Einzelne Einträge und ganze Sammlungen (D4). Erweiterungen erklären das mit
  einem einheitlichen Schema, das holzi für jede Erweiterung gleich prüfen kann.
- Q: Wer verwaltet eine Datenfreigabe? → A: Der Eigentümer, also wer sie
  angelegt hat, ist ihr einziger Admin (D6). Nur er lädt ein, vergibt, ändert
  und entzieht Fähigkeiten und entfernt Empfänger.
- Q: Darf ein Empfänger weiterteilen? → A: Nein (D7). Nur der Eigentümer lädt
  ein. Dass ein Empfänger Inhalte kopiert und selbst neu teilt, verhindert holzi
  nicht; das ist nicht im Umfang.
- Q: Werden die Schlüssel einer Datenfreigabe wie in haex-vault mit MLS
  verwaltet? → A: Nein (D13). MLS braucht eine feste Reihenfolge, die zu einer
  reihenfolgefreien Synchronisierung nicht passt. Schlüsselgenerationen sind
  gewöhnliche synchronisierte Einträge; zwei gleichzeitig entstandene
  Generationen sind beide gültig.

### Session 2026-09-28

- Q: Über welche Tabellen darf eine Erweiterung Freigabetypen erklären? → A: Nur
  über ihre eigenen Tabellen mit Präfix aus öffentlichem Schlüssel und Name
  (D5). Fremdschlüssel dürfen nur zwischen diesen Tabellen verlaufen, und nur
  synchronisierte Tabellen sind erlaubt. Sonst wird die ganze Erweiterung
  abgelehnt, nicht nur die Erklärung.

## User Scenarios & Testing _(mandatory)_

### User Story 1 - Erweiterung erklärt, was sich teilen lässt (Priority: P1)

Die Autorin einer Kalender-Erweiterung möchte, dass Nutzer einzelne Termine und
ganze Kalender teilen können. Sie trägt in das Manifest zwei Freigabetypen ein,
„Kalender“ und „Kalendereintrag“, jeweils mit Wurzeltabelle, Beschriftungsfeld
und zugehörigen Tabellen. Code für Schlüssel, Synchronisierung oder Rechte
schreibt sie nicht. holzi prüft die Erklärung bei der Installation und lehnt
Erweiterungen ab, die über fremde Tabellen teilen wollen.

**Why this priority**: Ohne geprüfte Erklärung gibt es nichts zu teilen. Die
Prüfung ist zugleich der erste Schutz davor, dass eine Erweiterung Daten des
Kerns oder anderer Erweiterungen hinausträgt.

**Independent Test**: Drei Testerweiterungen installieren: eine mit gültiger
Erklärung, eine mit einer Kerntabelle als zugehöriger Tabelle, eine mit einer
nicht synchronisierten Tabelle. Die erste wird installiert und ihre
Freigabetypen stehen im Bestätigungsdialog; die beiden anderen werden mit einer
Meldung abgelehnt, die Tabelle und Grund nennt, und nichts von ihnen bleibt
installiert.

**Acceptance Scenarios**:

1. **Given** eine Erweiterung, deren Freigabetypen nur eigene, synchronisierte
   Tabellen und Fremdschlüssel zwischen ihnen nennen, **When** der Nutzer sie
   installiert, **Then** zeigt der Bestätigungsdialog die Freigabetypen mit
   Namen, und nach der Bestätigung ist die Erweiterung installiert.
2. **Given** eine Erweiterung, deren Freigabetyp eine Tabelle außerhalb ihres
   Präfixes nennt (eine Kerntabelle oder eine Tabelle einer anderen
   Erweiterung), **When** der Nutzer sie installieren will, **Then** lehnt holzi
   die ganze Erweiterung ab und nennt die Tabelle.
3. **Given** eine Erweiterung, deren Fremdschlüssel aus einer eigenen Tabelle in
   eine fremde zeigt, **When** der Nutzer sie installieren will, **Then** lehnt
   holzi sie ab und nennt die Verbindung.
4. **Given** eine Erweiterung, die eine nicht synchronisierte Tabelle in einen
   Freigabetyp aufnimmt, **When** der Nutzer sie installieren will, **Then**
   lehnt holzi sie ab.
5. **Given** eine installierte Erweiterung mit bestehenden Datenfreigaben,
   **When** ein Update eine unzulässige Erklärung mitbringt, **Then** wird das
   Update abgelehnt, und die installierte Version bleibt mit ihren Daten und
   Datenfreigaben unverändert.

---

### User Story 2 - Einen Termin oder einen ganzen Kalender teilen (Priority: P1)

Anna möchte ihrem Kollegen Ben einen einzelnen Termin zeigen und ihrer Familie
den ganzen Kalender „Familie“. In der Kalender-Erweiterung wählt sie beim Termin
„Teilen“. Es öffnet sich der Teilen-Dialog von holzi, nicht einer der
Erweiterung. Er zeigt den Titel des Termins, Anna fügt Ben über seine
Vault-Identität hinzu und lässt es bei „Lesen“. Den Kalender „Familie“ teilt sie
mit ihrem Mann Carl mit „Schreiben“. Trägt sie danach einen neuen
Termin in „Familie“ ein, sieht Carl ihn, ohne dass Anna ihn eigens teilt.

**Why this priority**: Das ist der Kern der Funktion: Daten an einzelne Personen
mit einzelnen Rechten weitergeben, ohne dass die Erweiterung Schlüssel oder
Rechte selbst verwalten muss.

**Independent Test**: In einer Vault mit Testkalender einen Termin mit einer
zweiten Vault teilen und einen Kalender mit einer dritten. Die zweite Vault
erhält genau diesen Termin mit Teilnehmern und Erinnerungen, die dritte den
Kalender mit allen Terminen. Ein danach angelegter Termin im geteilten Kalender
erscheint bei der dritten Vault, bei der zweiten nicht. Kein anderer Eintrag der
Vault erreicht eine der beiden.

**Acceptance Scenarios**:

1. **Given** ein Termin in einer Erweiterung mit dem Freigabetyp
   „Kalendereintrag“, **When** die Erweiterung das Teilen anfragt, **Then**
   öffnet holzi seinen Teilen-Dialog mit Titel des Termins, Name des
   Freigabetyps, Name der Erweiterung und den bisherigen Empfängern.
2. **Given** der Teilen-Dialog, **When** Anna eine Vault-Identität hinzufügt,
   eine Fähigkeitsstufe wählt und bestätigt, **Then** besteht die
   Datenfreigabe mit diesem Empfänger, eine neue Schlüsselgeneration ist
   angelegt, und der Empfänger erhält eine Einladung.
3. **Given** der Kalender „Familie“ ist mit Carl geteilt, **When** Anna darin
   einen Termin anlegt, **Then** gehört der Termin samt Teilnehmern und
   Erinnerungen zur Datenfreigabe und erreicht Carl.
4. **Given** Anna verschiebt einen Termin aus „Familie“ in einen nicht geteilten
   Kalender, **When** die Änderung Carl erreicht, **Then** verschwindet der
   Termin aus Carls Kopie, und bei Anna bleibt er erhalten.
5. **Given** eine Datenfreigabe, **When** holzi Änderungspakete für sie
   verschickt, **Then** enthalten sie nur Wurzel und zugehörige Einträge aus
   den erklärten Tabellen und nie Geheimnisse der Vault.
6. **Given** die Erweiterung fragt nach dem Stand eines eigenen Eintrags,
   **When** holzi antwortet, **Then** erfährt sie, ob er geteilt ist, und sonst
   nichts: keine Schlüssel, keine Mitgliederliste, keine Rechte anderer.

---

### User Story 3 - Eine Freigabe empfangen und in der eigenen Erweiterung sehen (Priority: P1)

Carl erhält Annas Einladung zum Kalender „Familie“. Er sieht, von wem sie
kommt, welche Erweiterung sie braucht und welche Fähigkeiten er bekommt, und
nimmt an. Der Kalender erscheint in seiner eigenen Kalender-Erweiterung als
„geteilt von Anna“, auf seinem Laptop und seinem Telefon. Hätte er die
Erweiterung nicht installiert, böte ihm die Einladung die Installation an.

**Why this priority**: Ohne Empfangen ist Teilen wertlos. Dass die Daten in der
gewohnten Erweiterung auftauchen und nicht in einer eigenen Ansicht, macht sie
benutzbar.

**Independent Test**: Eine Einladung auf einer Empfänger-Vault mit zwei Geräten
annehmen: Die Daten erscheinen in der Erweiterung auf beiden Geräten mit dem
Hinweis auf die Herkunft. Eine zweite Einladung ablehnen: Die Vault enthält
danach nichts aus dieser Freigabe. Eine Einladung auf einer Vault ohne die
Erweiterung öffnen: Sie nennt die Erweiterung und bietet die Installation an.

**Acceptance Scenarios**:

1. **Given** eine Einladung, **When** Carl sie öffnet, **Then** zeigt holzi den
   Namen und die Vault-Identität des Eigentümers, Erweiterung, Freigabetyp,
   Beschriftung der Wurzel und die angebotenen Fähigkeiten, und bis zur Annahme
   wird nichts aus der Freigabe gespeichert.
2. **Given** Carl nimmt an, **When** die Daten eintreffen, **Then** liegen sie in
   seiner Kopie derselben Erweiterung, die sie als „geteilt von Anna“
   kennzeichnen kann.
3. **Given** Carl hat mehrere eigene Geräte, **When** eines von ihnen die
   Freigabe angenommen hat, **Then** erscheinen die Daten auch auf den anderen,
   ohne dass Anna etwas tun muss.
4. **Given** Carl hat die Erweiterung nicht installiert, **When** er die
   Einladung öffnet, **Then** nennt sie die benötigte Erweiterung und bietet die
   Installation mit der üblichen Bestätigung an; die Einladung bleibt offen, bis
   er annimmt oder ablehnt.
5. **Given** Carl lehnt ab, **When** die Ablehnung wirksam ist, **Then** enthält
   seine Vault nichts aus der Freigabe, und Anna sieht, dass Carl abgelehnt hat.
6. **Given** ein Änderungspaket enthält einen Eintrag, der nicht von der Wurzel
   aus erreichbar ist, eine fremde Tabelle betrifft oder von einer Vault ohne
   passende Fähigkeit stammt, **When** es bei Carl eintrifft, **Then** wird es
   nicht angewendet.

---

### User Story 4 - Mit Schreibrecht mitarbeiten (Priority: P2)

Anna teilt die Einkaufsliste mit Carl (Schreiben) und mit ihrer
Tochter Dana (nur Lesen). Carl trägt „Milch“ ein und hakt „Brot“ ab. Anna und
Dana sehen beides, auch wenn Anna gerade offline ist. Dana kann nichts
eintragen; ihre Erweiterung erhält eine Fehlermeldung, wenn sie es versucht.

**Why this priority**: Gemeinsames Bearbeiten ist der häufigste Grund zu teilen,
setzt aber das Lesen aus US2 und US3 voraus.

**Independent Test**: Eine Liste mit zwei Empfängern teilen, einer mit
Schreiben, einer ohne. Der mit Schreiben legt einen Eintrag an und ändert einen
fremden; beide Änderungen erreichen Eigentümer und anderen Empfänger. Der ohne
Schreiben versucht dasselbe; holzi lehnt es in seiner Vault ab, und nichts davon
verlässt sie.

**Acceptance Scenarios**:

1. **Given** Carl hat Schreiben, **When** er einen Eintrag zur geteilten Liste
   hinzufügt, **Then** gehört der Eintrag zur Freigabe, sein Ersteller ist
   Carls Vault-Identität, und er erreicht Anna und Dana.
2. **Given** Carl hat Schreiben, **When** er einen Eintrag ändert, den Anna
   angelegt hat, **Then** wird die Änderung bei allen angewendet, und der
   Ersteller bleibt Anna.
3. **Given** Anna ist offline, **When** Carl etwas ändert, **Then** erreicht die
   Änderung Dana trotzdem, und Anna erhält sie, sobald eines ihrer Geräte online
   ist.
4. **Given** Dana hat nur Lesen, **When** ihre Erweiterung einen Eintrag der
   geteilten Liste ändern oder einen neuen darunter anlegen will, **Then** lehnt
   holzi das in Danas Vault ab, und die Erweiterung erhält einen Fehler.
5. **Given** Carl hat Schreiben, aber nicht Löschen, **When** er einen eigenen
   Eintrag löscht, **Then** wird er bei allen gelöscht; **When** er einen von
   Anna angelegten Eintrag löschen will, **Then** lehnt holzi das ab.
6. **Given** jemand verändert den Ersteller eines Eintrags, **When** die
   Änderung eintrifft, **Then** lehnen alle Beteiligten sie ab.

---

### User Story 5 - Rechte ändern und entziehen (Priority: P2)

Anna und Ben arbeiten nicht mehr zusammen. Anna öffnet in den Einstellungen
unter „Föderation“ die Unteransicht „Datenfreigaben“ und entfernt Ben aus der
Freigabe des Termins. Carl setzt sie in der Einkaufsliste von Schreiben auf
Lesen zurück. Ben erhält danach nichts mehr; was er schon hatte, bleibt als
beendete Freigabe bei ihm.

**Why this priority**: Wer teilt, muss das Teilen zurücknehmen können. Ohne
Entzug ist jede Freigabe eine Entscheidung für immer.

**Independent Test**: Einen Empfänger entfernen, danach beim Eigentümer Einträge
ändern. Die Änderungen erreichen die übrigen Empfänger, der entfernte kann sie
weder abrufen noch entschlüsseln. Bei ihm steht die Freigabe als beendet. Einem
anderen Empfänger Schreiben entziehen: Seine eigene Vault lehnt danach jede
Änderung ab, und das Relay nimmt keine Pakete mehr von ihm an.

**Acceptance Scenarios**:

1. **Given** die Unteransicht „Datenfreigaben“, **When** Anna sie öffnet,
   **Then** sieht sie alle eigenen Freigaben mit Empfängern und Fähigkeiten und
   alle empfangenen mit Eigentümer.
2. **Given** Anna entfernt Ben, **When** die Änderung wirksam ist, **Then** gilt
   eine neue Mitgliederliste auf dem Relay, und eine neue Schlüsselgeneration
   geht nur an die verbliebenen Empfänger.
3. **Given** Ben ist entfernt, **When** Anna danach etwas ändert, **Then** kann
   Ben diese Änderung weder abrufen noch lesen.
4. **Given** Ben ist entfernt, **When** er seine Kalender-Erweiterung öffnet,
   **Then** sind die schon empfangenen Daten noch da, als „Freigabe beendet“
   gekennzeichnet, ohne weitere Aktualisierung, und er kann sie entfernen.
5. **Given** Anna setzt Carl auf Lesen zurück, **When** Carl danach etwas
   ändern will, **Then** lehnt seine eigene Vault das ab (FR-029); für Änderungen, die
   gleichzeitig mit der Rückstufung entstanden sind, gilt FR-034.
6. **Given** ein Empfänger will die Rechte eines anderen ändern oder jemanden
   einladen, **When** er es versucht, **Then** bietet holzi das nicht an, weil
   nur der Eigentümer Rechte vergibt.

---

### User Story 6 - Austreten und Erweiterungen entfernen (Priority: P3)

Dana braucht die Einkaufsliste nicht mehr und tritt aus der Freigabe aus. Die
Liste verschwindet aus ihrer Vault auf allen ihren Geräten, bei Anna bleibt sie
unberührt. Später deinstalliert Anna die Kalender-Erweiterung; holzi zeigt
vorher, welche Datenfreigaben dadurch enden.

**Why this priority**: Seltener als Teilen und Entziehen, aber ohne sauberes
Ende blieben Empfänger an Freigaben hängen, die sie nicht mehr wollen.

**Independent Test**: Als Empfänger austreten: Die Daten sind auf allen eigenen
Geräten weg, beim Eigentümer ist kein Eintrag gelöscht, und der Empfänger steht
nicht mehr in der Mitgliederliste, sobald ein Gerät des Eigentümers online war.
Beim Eigentümer die Erweiterung deinstallieren: Der Dialog listet die
Freigaben, danach sind sie für alle Empfänger beendet.

**Acceptance Scenarios**:

1. **Given** Dana tritt aus, **When** der Austritt wirksam ist, **Then** sind
   die empfangenen Einträge auf allen ihren Geräten entfernt, und keine dieser
   Entfernungen erreicht Anna oder Carl als Löschung.
2. **Given** Dana ist ausgetreten, **When** ein Gerät von Anna online ist,
   **Then** entfernt Annas Vault Dana aus der Freigabe mit neuer
   Schlüsselgeneration.
3. **Given** Anna deinstalliert eine Erweiterung mit eigenen Datenfreigaben,
   **When** sie die Deinstallation bestätigt, **Then** hat der Dialog vorher die
   betroffenen Freigaben genannt, und sie enden für alle Empfänger wie ein
   Entzug.
4. **Given** Carl deinstalliert die Erweiterung, zu der er Freigaben empfangen
   hat, **When** er bestätigt, **Then** hat der Dialog sie vorher genannt, und
   Carl tritt aus ihnen aus.

---

### User Story 7 - Ein Eintrag in zwei Freigaben (Priority: P3)

Anna teilt den Kalender „Familie“ mit Carl und den Termin „Elternabend“ daraus
zusätzlich einzeln mit der Nachbarin Eva, mit Schreiben. Eva ändert die Uhrzeit.
Carl sieht die neue Uhrzeit, sobald eines von Annas Geräten online war.

**Why this priority**: Die Überlappung folgt aus D4 und muss richtig
funktionieren, betrifft aber nur einen Teil der Fälle; die Verzögerung ist für
v1 hingenommen.

**Independent Test**: Einen Eintrag über seine Sammlung mit einer Vault und
einzeln mit einer zweiten teilen. Die zweite ändert ihn, während der Eigentümer
offline ist: Die erste sieht die Änderung noch nicht. Der Eigentümer kommt
online: Die erste sieht sie als Änderung des Eigentümers, mit der zweiten als
angezeigter ursprünglicher Autorin.

**Acceptance Scenarios**:

1. **Given** der Termin gehört zu beiden Freigaben, **When** Eva ihn über ihre
   Freigabe ändert, **Then** wird die Änderung gegen Evas Freigabe geprüft.
2. **Given** Anna ist offline, **When** Evas Änderung eintrifft, **Then**
   erreicht sie Carl noch nicht.
3. **Given** ein Gerät von Anna ist online und hat Evas Änderung, **When** es
   sie als neue, von diesem Gerät unterschriebene Änderung in die Freigabe
   „Familie“ weitergibt, **Then** nimmt Carl sie an, obwohl Eva dort kein
   Mitglied ist, und Eva wird als ursprüngliche Autorin angezeigt.
4. **Given** ein Empfänger ist in beiden Freigaben, **When** er aus einer
   austritt, **Then** bleibt der Termin bei ihm, solange er über die andere
   dazugehört.

---

### Edge Cases

- Ein Empfänger mit Schreiben legt unter einem geteilten Termin eine Erinnerung
  an. Sie gehört damit zur Freigabe und erreicht alle Beteiligten. Private
  Einträge unter einer geteilten Wurzel gibt es nicht; die Erweiterung sollte
  das anzeigen können (FR-018).
- Ein Empfänger mit Schreiben hängt einen Eintrag an eine Wurzel außerhalb der
  Freigabe um (verschiebt ihn in einen eigenen Kalender). Die Änderung wäre
  danach nicht mehr von der Wurzel aus erreichbar; holzi lehnt sie schon in
  seiner Vault ab (FR-029). Kopieren bleibt möglich.
- Der Eigentümer löscht die Wurzel, oder ein Empfänger mit dem Recht dazu löscht
  sie. Die Löschung erreicht alle Beteiligten, und die Datenfreigabe endet für
  alle.
- Eine Tabelle wird durch ein Update erst teilbar und enthält schon Einträge.
  Ihr Ersteller ist die eigene Vault-Identität, denn empfangen kann die Vault
  vorher nichts aus ihr haben.
- Ein Update der Erweiterung ändert, welche Tabellen zu einem Freigabetyp
  gehören. Die Zugehörigkeit wird mit der neuen Erklärung berechnet; Einträge,
  die nicht mehr dazugehören, verschwinden bei den Empfängern wie verschobene
  Einträge. Ein Update, das einen Freigabetyp mit bestehenden Freigaben
  entfernt, nennt diese in der Bestätigung, und sie enden danach (FR-006).
- Der Empfänger hat eine ältere Version der Erweiterung, der eine Tabelle oder
  Spalte fehlt, die im Paket vorkommt. Das Paket wird zurückgehalten, nicht
  verworfen, und die Freigabe zeigt den Hinweis, die Erweiterung zu
  aktualisieren (FR-025).
- Eine gleichnamige Erweiterung eines anderen Herausgebers (anderer
  öffentlicher Schlüssel) ist installiert. Sie gilt nicht als dieselbe
  Erweiterung und empfängt nichts.
- Zwei Geräte des Eigentümers ändern gleichzeitig die Rechte desselben
  Empfängers. Es gilt für alle dieselbe zuletzt geschriebene Änderung, und ein
  Gerät des Eigentümers unterschreibt die zusammengeführte Mitgliederliste mit
  einer höheren Generation neu (FR-016); da nur der Eigentümer Rechte schreibt,
  gibt es keinen Streit zwischen Admins.
- Die Vault des Eigentümers geht vollständig verloren. Die Datenfreigabe ist
  eingefroren: Empfänger behalten ihre Daten und können mit ihren Rechten
  weiter untereinander arbeiten, aber niemand kann Rechte ändern oder
  Überlappungen weitergeben. Eine Übergabe der Eigentümerschaft ist nicht im
  Umfang.
- Die Vault-Identität eines Empfängers oder des Eigentümers wird nach Verlust
  eines Geräts erneuert. Spec 024 regelt das für alle Bereiche: Die Vault
  veröffentlicht für jede Datenfreigabe, in der sie Mitglied oder Eigentümer
  ist, eine mit der alten Identität unterschriebene Übergabe an die neue; die
  anderen Beteiligten übernehmen die neue Identität erst nach Abgleich eines
  Prüfcodes, weil auch ein Dieb den alten Schlüssel hat. Treffen zwei
  konkurrierende Übergaben ein, friert das Relay die Datenfreigabe ein, bis die
  Beteiligten den Konflikt gelöst haben.
- Ein entfernter Empfänger schreibt weiter mit der älteren Schlüsselgeneration,
  die er noch hat. Das Relay lehnt das ab (Spec 026). Was direkte oder
  anderweitig eintreffende Pakete angeht, siehe FR-034.
- Eine Einladung trifft ein, während die Erweiterung gerade deinstalliert wird,
  oder für einen Freigabetyp, den die installierte Version nicht kennt. Sie
  bleibt offen wie bei fehlender Erweiterung (FR-020).
- Der Eigentümer teilt einen Eintrag mit sich selbst. holzi bietet das nicht an;
  die eigenen Geräte teilen die ganze Vault ohnehin (Spec 024).

## Requirements _(mandatory)_

### Functional Requirements

**Deklaration und Prüfung**

- **FR-001**: Eine Erweiterung MUSS in ihrem signierten Manifest Freigabetypen
  erklären können, für alle Erweiterungen im selben Schema. Jeder Freigabetyp
  nennt: einen Namen, eindeutig innerhalb der Erweiterung; die Wurzeltabelle mit
  ihrem Schlüssel; das Feld, das im Teilen-Dialog als Beschriftung dient; die
  zugehörigen Tabellen, jede mit der Fremdschlüsselspalte, die auf die
  übergeordnete Tabelle zeigt; optional je zugehöriger Tabelle den Freigabetyp
  derselben Erweiterung, dessen zugehörige Einträge mitkommen (die Termine eines
  Kalenders bringen ihre Teilnehmer und Erinnerungen mit). Tabellennamen sind
  logisch; holzi ordnet sie den Tabellen mit dem Präfix der Erweiterung zu. Das
  konkrete Format im Manifest legt der Plan fest.
- **FR-002**: holzi MUSS die Erklärung bei jeder Installation und jedem Update
  prüfen, bevor die Erweiterung verwendet werden kann, gegen das Schema, das die
  Erweiterung danach hat. holzi MUSS die ganze Erweiterung ablehnen, wenn:
  (a) eine Wurzel- oder zugehörige Tabelle außerhalb des eigenen Präfixes
  liegt; (b) ein Fremdschlüssel auf eine Tabelle außerhalb des eigenen Präfixes
  zeigt; (c) eine genannte Tabelle nicht synchronisiert wird oder als nur
  gerätelokal gekennzeichnet ist, oder eine Schlüssel-, Fremdschlüssel- oder
  Beschriftungsspalte nicht synchronisiert wird; (d) eine genannte Tabelle oder
  Spalte nicht existiert oder ein Fremdschlüssel nicht auf die übergeordnete
  Tabelle zeigt; (e) ein Name doppelt vorkommt, ein Verweis auf einen
  unbekannten Freigabetyp zeigt oder die Verweise zwischen Freigabetypen einen
  Kreis bilden.
- **FR-003**: Eine Ablehnung MUSS Erweiterung, betroffene Tabelle oder
  Verbindung und Grund nennen. Von einer abgelehnten Installation DARF NICHTS
  installiert bleiben. Bei einem abgelehnten Update MUSS die installierte
  Version mit ihren Daten und Datenfreigaben unverändert bleiben.
- **FR-004**: Der Bestätigungsdialog bei Installation und Update (ADR-0004) MUSS
  die erklärten Freigabetypen mit Namen zeigen. Ein Update, das Freigabetypen
  hinzufügt, braucht eine neue Bestätigung wie jede neue Berechtigung.
- **FR-005**: Für jede Tabelle, die in einem Freigabetyp vorkommt, MUSS holzi
  beim Anlegen eines Eintrags den Ersteller auf die Vault-Identität der
  schreibenden Vault setzen. Die Erweiterung DARF den Ersteller weder setzen
  noch ändern. Einträge, die schon bestehen, wenn eine Tabelle teilbar wird,
  erhalten die eigene Vault-Identität.
- **FR-006**: Entfernt ein Update einen Freigabetyp, zu dem Datenfreigaben
  bestehen, MUSS die Bestätigung des Updates diese nennen. Nach der Bestätigung
  enden sie wie in FR-036 (eigene) beziehungsweise FR-026 (empfangene).

**Teilen**

- **FR-007**: Den Teilen-Dialog MUSS holzi selbst anzeigen. Eine Erweiterung
  DARF ihn nur anfragen, für einen Eintrag der Wurzeltabelle eines ihrer
  Freigabetypen. Eine Datenfreigabe entsteht oder ändert sich nur durch eine
  Bestätigung des Nutzers in diesem Dialog oder in der Unteransicht
  „Datenfreigaben“ (FR-015).
- **FR-008**: Der Teilen-Dialog MUSS zeigen: die Beschriftung der Wurzel, den
  Freigabetyp, die Erweiterung, was mitgeteilt wird (bei einer Sammlung: „mit
  allen Einträgen“) und die bisherigen Empfänger mit ihren Fähigkeiten. Er MUSS
  erlauben, eine Person über ihre Vault-Identität hinzuzufügen und für jeden
  Empfänger genau eine Fähigkeitsstufe zu wählen: Lesen, Schreiben (umfasst
  Lesen) oder Löschen (umfasst Schreiben). Freie Kombinationen gibt es nicht.
  Admin DARF NICHT wählbar sein.
- **FR-009**: Rechte MÜSSEN an Vault-Identitäten gehen, nie an Geräte. Der
  Eigentümer erfährt nicht, wie viele Geräte ein Empfänger hat, und der
  Empfänger kann Geräte hinzufügen, ohne dass der Eigentümer etwas tut.
- **FR-010**: Bestätigt der Eigentümer einen neuen Empfänger, MUSS holzi: die
  Datenfreigabe anlegen, falls es sie noch nicht gibt; das Recht speichern;
  eine neue Schlüsselgeneration anlegen, deren Inhaltsschlüssel verschlüsselt
  an die Vault-Identität des Eigentümers und jedes Empfängers der neuen
  Mitgliederliste geht, einmal je Vault; eine neue Mitgliederliste mit dieser
  Generation auf das Relay laden (Spec 026); und dem Empfänger eine Einladung
  schicken, auf dem Weg für Einladungen aus Spec 027. Wie bei Spaces erzeugt
  jede Änderung der Mitglieder (Einladen, Ändern von Fähigkeiten, Entfernen,
  Austreten, Ablehnen) eine neue Schlüsselgeneration mit fester
  Mitgliederliste.
- **FR-011**: Ein neu hinzugefügter Empfänger MUSS nach der Annahme den ganzen
  aktuellen Stand der Datenfreigabe erhalten, nicht nur die Änderungen nach
  seiner Einladung. Dafür MUSS holzi ihm zusätzlich die Inhaltsschlüssel aller
  älteren Schlüsselgenerationen verschlüsselt an seine Vault-Identität geben,
  sodass er auch Änderungen von vor seiner Einladung lesen kann.
- **FR-012**: Zu einer Datenfreigabe MÜSSEN genau die Wurzel und alle
  zugehörigen Einträge gehören, berechnet aus den erklärten Fremdschlüsseln.
  Neue Einträge unter der Wurzel gehören sofort dazu. Ein Eintrag, der durch
  eine Änderung des Eigentümers nicht mehr erreichbar ist, MUSS bei den
  Empfängern aus deren Kopie verschwinden, ohne beim Eigentümer gelöscht zu
  werden; ein Eintrag, der erreichbar wird, gehört ab dann dazu.
- **FR-013**: Beim Senden für eine Datenfreigabe DÜRFEN nur Einträge aus FR-012,
  nur aus den erklärten Tabellen und nur synchronisierte Spalten die Vault
  verlassen. Die Vault-Identität, Geräteschlüssel, Inhaltsschlüssel anderer
  Bereiche und alle Tabellen des Kerns DÜRFEN eine Datenfreigabe NIE
  erreichen. Diese Grenze MUSS als feste Liste des Erlaubten umgesetzt sein,
  nicht als Liste des Verbotenen.
- **FR-014**: Nur der Eigentümer DARF Empfänger hinzufügen, Fähigkeiten ändern
  und Empfänger entfernen (D6). Für Einträge, die zu einer empfangenen
  Datenfreigabe gehören, DARF holzi keinen Teilen-Dialog öffnen; eine solche
  Anfrage der Erweiterung wird mit einem Hinweis abgelehnt (D7). Das gilt auch
  für Einträge, die der Empfänger selbst in der Freigabe angelegt hat.
- **FR-015**: holzi MUSS in der Einstellungskategorie „Föderation“ (Spec 023)
  eine Unteransicht „Datenfreigaben“ anbieten, die alle Datenfreigaben zeigt:
  die eigenen mit Empfängern, Fähigkeiten und dem Stand jeder Einladung
  (offen, angenommen, abgelehnt), die empfangenen mit Eigentümer und eigenen
  Fähigkeiten. Aus ihr MÜSSEN sich Rechte ändern, Empfänger entfernen und eigene
  Freigaben beenden lassen. Der Teilen-Dialog (FR-007, FR-008) bleibt ein
  allgemeiner Dialog des Kerns, den die Erweiterung öffnet; er gehört nicht zu
  dieser Unteransicht.
- **FR-016**: Der Eigentümer MUSS Rechte von jedem seiner Geräte aus ändern
  können. Gleichzeitige Änderungen seiner Geräte MÜSSEN sich auf allen
  Beteiligten gleich auflösen, zugunsten der zuletzt geschriebenen. Für die
  Mitgliederliste MÜSSEN dieselben Regeln gelten wie bei Spaces (Spec 027):
  (a) die Vault des Eigentümers ist am Relay für den Bereich der Datenfreigabe
  berechtigt, mit allen ihren Geräten, ohne selbst als Empfänger in der Liste
  zu stehen; (b) haben Geräte des Eigentümers unabhängig voneinander neue
  Mitgliederlisten erzeugt, MUSS ein Gerät des Eigentümers die
  zusammengeführte Liste mit einer höheren Generation als jede der
  zusammengeführten neu unterschreiben und hochladen, und alle gleichzeitig
  entstandenen Schlüsselgenerationen bleiben gültig; (c) zum Verschlüsseln
  DARF ein Gerät nur eine Schlüsselgeneration verwenden, deren
  Inhaltsschlüssel an keine inzwischen entfernte Vault verschlüsselt ist; gibt
  es keine, MUSS ein Gerät des Eigentümers eine neue anlegen; (d) zwei
  verschiedene Mitgliederlisten mit derselben Generation MÜSSEN abgelehnt
  werden.
- **FR-017**: Die Erweiterung DARF nie Inhaltsschlüssel, Mitgliederlisten,
  Rechte anderer Empfänger oder Zugriff auf die Synchronisierung erhalten.
- **FR-018**: Die Erweiterung MUSS für Einträge ihrer Wurzeltabellen und deren
  zugehörige Einträge abfragen können, ob sie geteilt sind, ob sie aus einer
  empfangenen Freigabe stammen, von wem (Name und Vault-Identität des
  Eigentümers), mit welchen eigenen Fähigkeiten und ob die Freigabe beendet
  ist, damit sie das anzeigen kann. Mehr gibt die Abfrage nicht preis.

**Empfangen**

- **FR-019**: Eine Einladung MUSS zeigen: Name und Vault-Identität des
  Eigentümers, Erweiterung, Freigabetyp, Beschriftung der Wurzel und angebotene
  Fähigkeiten. Der Empfänger MUSS annehmen oder ablehnen können. Vor der
  Annahme DARF nichts aus der Freigabe in seiner Vault gespeichert werden außer
  der Einladung selbst.
- **FR-020**: Ist die benötigte Erweiterung nicht installiert, oder kennt die
  installierte Version den Freigabetyp nicht, MUSS die Einladung das nennen und
  die Installation beziehungsweise das Update über die übliche Bestätigung
  anbieten. Die Einladung MUSS offen bleiben, bis der Empfänger annimmt oder
  ablehnt. Als dieselbe Erweiterung gilt nur eine mit gleichem öffentlichem
  Schlüssel und Namen.
- **FR-021**: Nach der Annahme MÜSSEN die Daten in den Tabellen derselben
  Erweiterung in der Vault des Empfängers landen. holzi MUSS festhalten, welche
  Einträge zu welcher empfangenen Datenfreigabe gehören.
- **FR-022**: Empfangene Daten und der an die Vault-Identität verschlüsselte
  Inhaltsschlüssel MÜSSEN auf alle Geräte des Empfängers gelangen, über den
  Sync zwischen eigenen Geräten (Spec 024). Entpackte Inhaltsschlüssel sind
  Nur-direkt-Daten (Spec 024, Nur-direkt-Daten): Jedes Gerät entpackt den
  verschlüsselten Inhaltsschlüssel selbst, und ein entpackter
  Inhaltsschlüssel DARF kein Gerät verlassen.
- **FR-023**: Jede Vault, die Änderungen einer Datenfreigabe empfängt
  (Eigentümer wie Empfänger), MUSS eine Änderung nur zulassen, wenn: (a) ihre
  Unterschrift gültig ist und die Gerätebestätigung das Gerät der
  Vault-Identität des Autors zuordnet; (b) ihre Tabelle zu den erklärten
  Tabellen des Freigabetyps derselben Erweiterung gehört; (c) der Eintrag nach
  der Änderung von der Wurzel aus erreichbar ist, ein neuer Eintrag also einen
  Fremdschlüssel in die Freigabe trägt; (d) der Autor die nötige Fähigkeit hat
  (FR-027, FR-028, FR-034) oder der Eigentümer ist; (e) der Ersteller eines
  bestehenden Eintrags unverändert bleibt und der eines neuen Eintrags der
  Autor ist. Ausgenommen von (e) sind Änderungen, die der Eigentümer nach
  FR-041 neu ausstellt: Dort bleibt der Ersteller der ursprüngliche, und
  unterschreibender Autor ist der Eigentümer.
- **FR-024**: Ein Änderungspaket ist atomar (Spec 024): Enthält es eine
  Änderung, die FR-023 verletzt, DARF KEINE Änderung aus diesem Paket
  angewendet werden. Eine Momentaufnahme wird dagegen je Änderung geprüft:
  unzulässige Änderungen werden verworfen, die übrigen angewendet. holzi MUSS
  jede Ablehnung protokollieren und mit den folgenden Paketen weitermachen.
- **FR-025**: Enthält ein Paket Tabellen oder Spalten des Freigabetyps, die die
  installierte Version der Erweiterung noch nicht kennt, MUSS holzi es
  zurückhalten statt es zu verwerfen, und die Freigabe MUSS auf ein Update der
  Erweiterung hinweisen. Nach dem Update wird es angewendet.
- **FR-026**: Der Empfänger MUSS eine Einladung ablehnen und aus einer
  angenommenen Freigabe jederzeit austreten können. Beim Austritt MÜSSEN die
  Einträge, die nur über diese Freigabe in seiner Vault sind, auf allen seinen
  Geräten entfernt werden. Diese Entfernung DARF die Datenfreigabe NICHT als
  Löschung erreichen. Ablehnung und Austritt MÜSSEN dem Eigentümer mitgeteilt
  werden; seine Vault entfernt das Recht, sobald eines seiner Geräte davon
  erfährt, wie bei einem Entzug (FR-032).

**Bearbeiten**

- **FR-027**: Schreiben MUSS erlauben, neue Einträge innerhalb der Freigabe
  anzulegen und jeden Eintrag der Freigabe zu ändern (einen Artikel zur
  geteilten Einkaufsliste hinzufügen, einen abhaken).
- **FR-028**: Löschen: Schreiben MUSS das Löschen eigener Einträge (Ersteller
  ist die eigene Vault-Identität) erlauben; fremde Einträge zu löschen MUSS die
  Fähigkeit Löschen erfordern. Der Eigentümer darf alles löschen.
  [NEEDS CLARIFICATION: Löschen – Vorschlag: „Schreiben“ erlaubt das Löschen eigener Einträge bzw. Dateien (Ersteller = eigene Vault), fremde löschen erfordert „Löschen“. Alternativen: Löschen ganz in „Schreiben“ enthalten, oder nur eigene löschbar ohne eigene Stufe „Löschen“. (Gemeinsame Frage für Spec 027 und 028.)]
- **FR-029**: holzi MUSS in der Vault des Empfängers jede Änderung an Einträgen
  einer empfangenen Freigabe ablehnen, die die eigenen Fähigkeiten nicht
  erlauben, bevor sie gespeichert wird: Ändern und Anlegen ohne Schreiben,
  Löschen nach FR-028, Umhängen eines Eintrags aus der Freigabe heraus. Die
  Erweiterung MUSS dafür einen Fehler erhalten.
- **FR-030**: Änderungen eines Empfängers MÜSSEN den Eigentümer und alle
  anderen Empfänger erreichen, auch wenn der Eigentümer offline ist (über das
  Postfach, Spec 026). Sind Geräte verschiedener Beteiligter gleichzeitig
  online, DÜRFEN sie die Änderungen dieser Datenfreigabe auch direkt
  austauschen (Spec 027, direkte Verbindung zwischen Mitgliedern).
- **FR-031**: Gleichzeitige Änderungen desselben Feldes MÜSSEN sich bei allen
  Beteiligten gleich auflösen, nach denselben Regeln wie der Sync zwischen
  eigenen Geräten. Konfliktkopien gibt es für Daten nicht.

**Entzug**

- **FR-032**: Der Eigentümer MUSS Fähigkeiten eines Empfängers jederzeit ändern
  und Empfänger entfernen können. Dabei MUSS holzi eine neue Mitgliederliste auf
  das Relay laden, sodass es den Empfänger sofort nach den neuen Rechten
  behandelt, und eine neue Schlüsselgeneration anlegen, die nur an die
  verbliebenen Empfänger geht (wie bei jeder Änderung der Mitglieder,
  FR-010).
- **FR-033**: Nach einem Entfernen DARF der entfernte Empfänger keine Änderung
  mehr lesen können, die mit einer späteren Schlüsselgeneration verschlüsselt
  ist.
- **FR-034**: Eine Änderung gilt, wenn ihr Autor in der Schlüsselgeneration,
  mit der sie verschlüsselt ist, die nötige Fähigkeit hatte; ein Entzug wirkt
  nur nach vorn. Zusätzlich MUSS jedes empfangende Gerät den Autor beim Empfang
  gegen die neueste ihm bekannte Mitgliederliste prüfen, sodass ein entfernter
  Empfänger mit der älteren Generation nicht dauerhaft weiterschreiben kann;
  das Relay sperrt ihn sofort (Spec 026).
  [NEEDS CLARIFICATION: Entzug bei gleichzeitigem Schreiben – Vorschlag: Eine Änderung gilt, wenn ihr Autor in der Schlüsselgeneration, mit der sie verschlüsselt ist, das Recht hatte, und wenn jedes empfangende Gerät den Autor beim Empfang zusätzlich gegen die neueste ihm bekannte Mitgliederliste prüft; das Relay sperrt sofort. Es bleibt ein Zeitfenster für Änderungen, die ein Gerät vor der Nachricht über den Entzug erhalten hat. Alternative: nachträgliche Neuberechnung aus einem Änderungsprotokoll (schließt das Fenster, deutlich aufwändiger). (Gemeinsame Frage für Spec 027 und 028.)]
- **FR-035**: Was ein entfernter Empfänger schon empfangen hat, MUSS in seiner
  Vault bleiben, gekennzeichnet als „Freigabe beendet“ und ohne weitere
  Aktualisierung. Er MUSS es in einem Schritt entfernen können; wie beim
  Austritt erreicht diese Entfernung niemanden sonst.
- **FR-036**: Der Eigentümer MUSS eine ganze Datenfreigabe beenden können. Für
  jeden Empfänger gilt dann FR-035. Dasselbe gilt, wenn die Wurzel gelöscht
  wird.
- **FR-037**: Die Deinstallation einer Erweiterung MUSS vor der Bestätigung die
  betroffenen Datenfreigaben nennen. Beim Eigentümer enden seine Freigaben wie
  in FR-036, beim Empfänger tritt er aus den empfangenen aus wie in FR-026.

**Überlappung**

- **FR-038**: Ein Eintrag MUSS zu mehreren Datenfreigaben desselben Eigentümers
  gehören können (ein Termin einzeln und über seinen Kalender). Jede Freigabe
  hat eigene Empfänger, Rechte und Schlüssel.
- **FR-039**: Eine Änderung MUSS gegen die Freigabe geprüft werden, über die sie
  eingetroffen ist.
- **FR-040**: Die Vault des Eigentümers MUSS eine zugelassene Änderung an einem
  Eintrag, der auch zu anderen seiner Freigaben gehört, in diese weitergeben,
  sobald eines ihrer Geräte online ist und die Änderung hat. Bis dahin sehen die
  Empfänger der anderen Freigaben sie nicht; diese Verzögerung ist für v1
  hingenommen.
- **FR-041**: Die Vault des Eigentümers MUSS eine weitergegebene Änderung als
  neue Änderung im Bereich der anderen Datenfreigabe ausstellen, unterschrieben
  von einem Gerät des Eigentümers. Die Empfänger der anderen Freigabe MÜSSEN
  sie wie jede Änderung des Eigentümers prüfen und zulassen. Der ursprüngliche
  Autor MUSS als Anzeige erkennbar bleiben, aber nicht als unterschreibender
  Autor; der Ersteller des Eintrags bleibt unverändert (FR-023 (e)). Das ist
  die ausdrückliche Ausnahme zur Regel aus Spec 024, dass ein Weitergebender
  Autor und Unterschrift nie ändert.
- **FR-042**: Gehört ein Eintrag beim Empfänger zu mehreren empfangenen
  Freigaben, MUSS er nur einmal in dessen Vault liegen. Endet eine dieser
  Freigaben, bleibt er, solange eine andere ihn umfasst.

### Key Entities

- **Freigabetyp-Erklärung**: Teil des signierten Manifests einer Erweiterung.
  Name, Wurzeltabelle mit Schlüssel, Beschriftungsfeld, zugehörige Tabellen mit
  Fremdschlüssel, optionale Verweise auf andere Freigabetypen derselben
  Erweiterung. Bei Installation und Update geprüft (FR-002).
- **Datenfreigabe**: Kennung, Eigentümer (Vault-Identität), Freigabetyp mit
  Erweiterung, Wurzel, Zeitpunkt der Anlage, Zustand (aktiv oder beendet).
  Gehört der Vault des Eigentümers und synchronisiert sich zwischen seinen
  Geräten.
- **Recht**: Datenfreigabe, Vault-Identität des Empfängers, Fähigkeiten (Lesen,
  Schreiben, Löschen), Stand der Einladung. Nur vom Eigentümer geschrieben.
- **Schlüsselgeneration**: je Datenfreigabe; Inhaltsschlüssel verschlüsselt an
  jede berechtigte Vault-Identität, einmal je Vault. Eine neue entsteht bei
  jeder Änderung der Mitglieder (Einladen, Ändern von Fähigkeiten, Entfernen,
  Austreten, Ablehnen); ihre Mitgliederliste steht damit fest. Neue Empfänger
  erhalten auch die Inhaltsschlüssel aller älteren Generationen (FR-011).
- **Mitgliederliste**: die vom Eigentümer unterschriebene Liste der Empfänger
  mit Fähigkeiten zu einer Generation, auf dem Relay (Spec 026). Die Vault des
  Eigentümers steht nicht darin und ist am Relay dennoch mit allen ihren
  Geräten berechtigt (FR-016).
- **Einladung**: eine verschlüsselte Nachricht an die Vault-Identität des
  Empfängers mit Datenfreigabe, Eigentümer, Erweiterung, Freigabetyp,
  Beschriftung und Fähigkeiten, zugestellt auf dem Weg für Einladungen aus
  Spec 027.
- **Zuordnung empfangener Einträge**: beim Empfänger; welcher Eintrag in den
  Tabellen einer Erweiterung zu welcher empfangenen Datenfreigabe gehört. Vom
  Kern geführt, nicht von der Erweiterung.
- **Ersteller**: an jedem Eintrag einer teilbaren Tabelle; unveränderlich
  (FR-005, FR-023).

## Success Criteria _(mandatory)_

### Measurable Outcomes

- **SC-001**: Eine Erweiterung, deren Erklärung eine fremde Tabelle, einen
  Fremdschlüssel aus dem eigenen Präfix hinaus oder eine nicht synchronisierte
  Tabelle nennt, wird bei Installation und Update in 100 % der Testfälle
  abgelehnt, und die vorher installierte Version bleibt in 100 % der Fälle
  unverändert.
- **SC-002**: In Tests mit mehreren Datenfreigaben erreicht kein einziger
  Eintrag außerhalb von Wurzel und zugehörigen Einträgen einen Empfänger,
  geprüft in dessen Vault und im entschlüsselten Inhalt der Änderungspakete.
- **SC-003**: Kein Änderungspaket einer Datenfreigabe enthält je ein Geheimnis
  der Vault (Vault-Identität, Geräteschlüssel, Inhaltsschlüssel anderer
  Bereiche) oder Daten einer Tabelle des Kerns.
- **SC-004**: 100 % der eingeschleusten unzulässigen Änderungen (ohne Fähigkeit,
  mit geändertem Ersteller, in fremder Tabelle, nicht erreichbar, falsche
  Unterschrift) werden von jeder empfangenden Vault abgelehnt.
- **SC-005**: Ein Nutzer teilt einen Eintrag mit einer Person in weniger als 30
  Sekunden, ausgehend vom geöffneten Eintrag in der Erweiterung.
- **SC-006**: Sind Absender und Empfänger online, erscheint ein neuer Eintrag
  unter einer geteilten Wurzel beim Empfänger innerhalb von 10 Sekunden.
- **SC-007**: Nach einem Entfernen kann der entfernte Empfänger 0 % der
  Änderungen lesen, die mit einer späteren Schlüsselgeneration verschlüsselt
  sind.
- **SC-008**: Eine Beispielerweiterung (Einkaufsliste) wird allein durch ihre
  Erklärung teilbar, ohne eigenen Code für Schlüssel, Synchronisierung oder
  Rechte.
- **SC-009**: Nach Austritt oder Entfernen der eigenen Kopie verliert der
  Eigentümer in 100 % der Fälle keinen Eintrag.

## Assumptions

- holzi hat noch keine Tabellen, die einer Erweiterung gehören, ihr Präfix
  tragen und synchronisiert werden (Entwurf §15 Punkt 10). Diese Spec setzt sie
  voraus, ebenso den Erweiterungs-Host mit Manifest, Bestätigung bei der
  Installation und einer Prüfstelle für Anfragen (geplante Spec 017, ADR-0004).
  Nach der Phasenregel der Constitution wird sie erst umgesetzt, wenn diese
  Grundlagen, die Specs 024 und 026 sowie Einladungen und direkte
  Verbindungen zwischen Mitgliedern aus Spec 027 im Einsatz sind.
- Spec 024 liefert Vault-Identität, Gerätebestätigung, Unterschriften mit
  echtem Autor, das Führen des Erstellers durch den Kern, die Nur-direkt-Daten
  und die Erneuerung der Vault-Identität. Spec 026 liefert Relay, Postfach und
  Mitgliederliste. Spec 027 liefert den Weg für Einladungen und die direkte
  Verbindung zwischen Mitgliedern.
- Das konkrete Format der Erklärung im Manifest und die Anfragen der Erweiterung
  an holzi legt der Plan fest. Beides berührt auch die Repositories `vault-sdk`
  und `haextension`; Verweise darauf werden mit vollem Commit-SHA angegeben.
- Eigentümer und Empfänger nutzen holzi und dieselbe Erweiterung (gleicher
  öffentlicher Schlüssel und Name), nicht zwingend dieselbe Version.
- Personen werden über ihre Vault-Identität ausgewählt. Ein Adressbuch gibt es
  nicht; den Namen in der Einladung gibt der Eigentümer für sich selbst an.
- Ein entfernter Empfänger behält, was er schon entschlüsseln konnte, wie beim
  Entfernen aus einem Space (Entwurf §13; haex-vault ADR 0002, Repository
  `https://github.com/haex-space/haex-vault`, Revision
  `8dce379d94e18fcd42c3b73686a06f984ca3f574`, Pfad
  `docs/adr/0002-shared-space-authenticity-and-confidentiality.md`).
- Anlegen von Datenfreigaben und Ändern von Rechten sind dem Nutzer
  vorbehalten, bis Spec 021 etwas anderes für Agenten festlegt.
- Die Kosten einer neuen Schlüsselgeneration wachsen mit der Zahl der
  Empfänger-Vaults, nicht ihrer Geräte. Für Freigaben mit wenigen Personen ist
  das unkritisch; gemessen wird im Plan.

## Nicht im Umfang

- Dateien teilen; das machen Spaces (Spec 027). Dateien, auf die ein Eintrag
  einer Erweiterung verweist, gehen mit einer Datenfreigabe nicht mit.
- Daten des Kerns teilen (Chat-Verlauf, Einstellungen, Modelle). Geteilt werden
  nur Daten von Erweiterungen.
- Weiterteilen durch Empfänger und mehrere Admins (D6, D7), ebenso das
  Verhindern, dass ein Empfänger Inhalte kopiert und selbst neu teilt.
- Übergabe der Eigentümerschaft, auch wenn die Vault des Eigentümers verloren
  ist.
- Rechte feiner als je Datenfreigabe (je Eintrag oder Feld) und Gruppen als
  Empfänger.
- Teilen mit Personen ohne holzi, öffentliche Links.
- Verbergen der Teilnehmer vor dem Relay (D9), nachträgliches Neuverschlüsseln
  alter Daten und Schutz nach einer Kompromittierung.
- MLS (D13).
