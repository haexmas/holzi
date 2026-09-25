# Feature Specification: Shell ohne Persistenz

**Feature Branch**: `022-session-only-shell`
**Created**: 2026-09-25
**Status**: Draft
**Input**: Betreiberentscheidung vom 2026-09-25 beim Test von Spec 020:
Arbeitsbereiche, Fenster und Tabs leben nur so lange wie die Vault-Session.
Beim Öffnen der Vault beginnt die Shell immer leer, nichts aus einer früheren
Session wird wiederhergestellt. Layouts, die frühere Versionen gespeichert
haben, erscheinen nie wieder und werden aus der Vault entfernt.

## Beziehung zu bestehenden Specs

- [`015-workspace-shell`](../015-workspace-shell/spec.md): Diese Spec zieht
  User Story 5 („Layout bleibt über Neustarts erhalten“) und die Anforderungen
  FR-023, FR-024 und FR-025 zurück. In User Story 7 entfällt, dass Instanzen
  „einen Neustart überleben“ (Independent Test und Acceptance Scenario 3). Alle
  übrigen Anforderungen an Fenster, Tabs und Arbeitsbereiche innerhalb einer
  Session gelten unverändert, auch die Anpassung der Fenstergeometrie an einen
  kleineren sichtbaren Bereich (FR-026 sinngemäß für den laufenden Betrieb).
- [`013-vault-lifecycle-isolation`](../013-vault-lifecycle-isolation/spec.md)
  und ADR-0003: Eine Vault-Session ist genau ein App-Prozess; Sperren und
  Schließen beenden sie. Diese Spec bindet die Lebensdauer des Layouts an
  genau diese Session. Sie passt zum Internet-Café-Gedanken aus Spec 013: Die
  Vault-Datei trägt keine Spuren davon, welche Fenster zuletzt offen waren.
- [`020-tab-navigation`](../020-tab-navigation/spec.md): Die Tab-Historie war
  schon bisher nicht persistiert (Betreiberentscheidung dort). Mit dieser Spec
  gilt dasselbe für das ganze Layout. Das Öffnen einer App an einem Ort
  (Deep-Link) bleibt unverändert.
- ADR-0001 (gerätebezogene Daten): Die Layoutdaten waren bisher nach dieser
  Konvention gerätebezogen gespeichert. Mit dieser Spec gibt es keine
  gespeicherten Layoutdaten mehr, die Konvention selbst bleibt unberührt.
- Tab-Inhalte sind nicht betroffen: Chat-Verlauf, Einstellungen, Modelle und
  alle anderen Daten der Apps bleiben wie bisher in der Vault.

## User Scenarios & Testing _(mandatory)_

### User Story 1 - Jeder Start beginnt leer (Priority: P1)

Ein Nutzer arbeitet mit mehreren Arbeitsbereichen und Fenstern, schließt holzi
und öffnet die Vault später wieder. Er findet einen einzigen, leeren
Arbeitsbereich vor und öffnet die Apps, die er gerade braucht. Nichts von der
letzten Sitzung taucht ungefragt wieder auf.

**Why this priority**: Das ist die Entscheidung selbst. Wiederhergestellte
Fenster wirken beim Start unaufgeräumt und verraten, woran zuletzt gearbeitet
wurde.

**Independent Test**: Zwei Arbeitsbereiche anlegen, im zweiten ein Fenster mit
zwei Tabs öffnen, ein Fenster verschieben und verkleinern, holzi beenden und die
Vault erneut öffnen: Es gibt genau einen Arbeitsbereich, er ist aktiv und hat
kein Fenster.

**Acceptance Scenarios**:

1. **Given** eine Session mit mehreren Arbeitsbereichen, Fenstern und Tabs,
   **When** der Nutzer holzi beendet und die Vault erneut öffnet, **Then** zeigt
   die Shell genau einen Arbeitsbereich ohne Fenster.
2. **Given** eine Session, in der der Nutzer die Vault sperrt, **When** er sie
   wieder entsperrt, **Then** beginnt die neue Session ebenfalls mit genau einem
   leeren Arbeitsbereich.
3. **Given** eine Session, in der der Nutzer den dritten von drei
   Arbeitsbereichen aktiv hatte, **When** er die Vault erneut öffnet, **Then**
   gibt es keinen Hinweis auf die frühere Anzahl, Reihenfolge oder Auswahl der
   Arbeitsbereiche.
4. **Given** der Nutzer öffnet holzi über eine Adresse, die eine App an einem Ort
   öffnet (Spec 020), **When** die Shell erscheint, **Then** ist genau diese App
   im einzigen Arbeitsbereich geöffnet, und sonst nichts.

---

### User Story 2 - Alte gespeicherte Layouts verschwinden (Priority: P1)

Ein Nutzer hat mit einer früheren holzi-Version gearbeitet, die sein Layout
gespeichert hat. Nach dem Update öffnet er die Vault. Das alte Layout erscheint
nicht mehr, und die Vault enthält es danach auch nicht mehr.

**Why this priority**: Ohne diesen Schritt bliebe das alte Layout als tote
Information in der Vault liegen. Wer die Vault-Datei weitergibt oder auf einem
fremden Rechner öffnet, trüge sie weiter mit sich.

**Independent Test**: Eine Vault, in der eine frühere Version ein Layout
gespeichert hat, mit der neuen Version öffnen: Die Shell ist leer, und eine
Untersuchung der Vault-Datei danach findet keine Layoutdaten mehr, weder
Arbeitsbereiche noch Fenster noch Tabs, auf keinem Gerät der Vault.

**Acceptance Scenarios**:

1. **Given** eine Vault mit gespeichertem Layout aus einer früheren Version,
   **When** der Nutzer sie mit dieser Version öffnet, **Then** erscheint die
   Shell mit genau einem leeren Arbeitsbereich.
2. **Given** dieselbe Vault, **When** das Öffnen abgeschlossen ist, **Then**
   enthält die Vault keine gespeicherten Arbeitsbereiche, Fenster oder Tabs
   mehr, auch nicht die anderer Geräte.
3. **Given** eine Vault ohne gespeichertes Layout (neu angelegt oder schon
   bereinigt), **When** der Nutzer sie öffnet, **Then** verhält sich die Shell
   genauso, und das Öffnen dauert nicht spürbar länger.
4. **Given** die Bereinigung schlägt fehl, **When** die Shell erscheint,
   **Then** startet sie trotzdem mit einem leeren Arbeitsbereich, die Vault
   bleibt nutzbar, und der Fehler wird protokolliert, ohne den Nutzer mit einem
   Fehlerbildschirm aufzuhalten.

---

### User Story 3 - Innerhalb einer Session bleibt alles wie gewohnt (Priority: P1)

Solange die Session läuft, verhält sich die Shell wie mit Spec 015 und 020:
Arbeitsbereiche anlegen und wechseln, Fenster verschieben, minimieren,
maximieren, Tabs öffnen und schließen, Einzelinstanz-Apps, Vor und Zurück im Tab.

**Why this priority**: Die Änderung betrifft nur, was über das Ende einer
Session hinaus bleibt. Im laufenden Betrieb darf der Nutzer keinen Unterschied
merken.

**Independent Test**: Die manuellen Szenarien aus Spec 015 und 020, die keinen
Neustart enthalten, laufen unverändert durch.

**Acceptance Scenarios**:

1. **Given** eine laufende Session, **When** der Nutzer Fenster anordnet und
   zwischen Arbeitsbereichen wechselt, **Then** bleibt die Anordnung bis zum
   Ende der Session erhalten.
2. **Given** eine Einzelinstanz-App ist in einem Fenster geöffnet, **When** der
   Nutzer sie erneut öffnet, **Then** wird ihr vorhandener Tab aktiv, wie in
   Spec 015.
3. **Given** der sichtbare Bereich wird kleiner, **When** ein Fenster nicht mehr
   hineinpasst, **Then** wird es so angepasst, dass es vollständig erreichbar
   bleibt.

---

### Edge Cases

- Der Nutzer beendet holzi hart (Absturz, Prozess beendet). Beim nächsten
  Öffnen ist die Shell leer wie nach einem normalen Beenden. Es gibt keinen
  Wiederherstellungsdialog.
- Die Vault wurde auf einem anderen Gerät bereits bereinigt, dieses Gerät öffnet
  sie zum ersten Mal mit der neuen Version. Das Öffnen verläuft normal, es gibt
  nichts mehr zu entfernen.
- Eine Vault mit Layoutdaten wird auf mehreren Geräten genutzt, die nacheinander
  auf die neue Version wechseln. Jedes Gerät entfernt beim ersten Öffnen mit der
  neuen Version alle Layoutdaten seiner Vault-Datei. Über die Föderation werden
  keine Layoutdaten mehr weitergegeben (FR-006). Ein Gerät, das noch eine ältere
  Version nutzt, ist nicht unterstützt (siehe Assumptions).
- Die Vault wird nach der Bereinigung mit einer älteren holzi-Version geöffnet.
  Das ist nicht unterstützt (siehe Assumptions).
- Eine App wird während der Session aus dem Fenster geschlossen. Am Verhalten
  der Session ändert sich nichts gegenüber Spec 015.

## Requirements _(mandatory)_

### Functional Requirements

- **FR-001**: Die Shell DARF Arbeitsbereiche, Fenster, Tabs, deren Anordnung,
  Reihenfolge, Geometrie, Minimierungs- oder Maximierungszustand und den aktiven
  Arbeitsbereich NICHT über das Ende einer Vault-Session hinaus speichern.
- **FR-002**: Beim Beginn jeder Vault-Session (Öffnen, Entsperren, Neustart
  nach Absturz) MUSS die Shell mit genau einem Arbeitsbereich ohne Fenster
  beginnen, es sei denn, der Start öffnet ausdrücklich eine App (FR-004).
- **FR-003**: Die Shell MUSS während der Session alle Fähigkeiten aus Spec 015
  und 020 behalten, die nicht vom Speichern über die Session hinaus abhängen.
- **FR-004**: Öffnet der Start eine App an einem Ort (Deep-Link, frühere
  Vollseiten-Adressen, Spec 020 FR-012), MUSS genau diese App im einzigen
  Arbeitsbereich erscheinen.
- **FR-005**: Beim ersten Öffnen einer Vault mit dieser Version MÜSSEN alle
  Layoutdaten, die frühere Versionen gespeichert haben, aus der Vault entfernt
  werden: Arbeitsbereiche, Fenster und Tabs aller Geräte der Vault.
- **FR-006**: Nach der Entfernung DARF die Vault-Datei keine Layoutdaten mehr
  enthalten, auch keine gelöschten Einträge, die sich aus der Datei
  wiederherstellen oder an andere Geräte weitergeben lassen.
- **FR-007**: Schlägt die Entfernung fehl, MUSS die Shell trotzdem normal mit
  einem leeren Arbeitsbereich starten. Die Vault MUSS nutzbar bleiben, und der
  Fehler MUSS protokolliert werden. Die Entfernung MUSS beim nächsten Öffnen
  erneut versucht werden.
- **FR-008**: Die Anwendung DARF keine Schnittstelle mehr anbieten, über die
  ein Layout geladen oder gespeichert werden kann.
- **FR-009**: Die Dokumentation von Spec 015 MUSS die zurückgezogenen Teile
  (User Story 5, FR-023 bis FR-025, der Neustart-Teil von User Story 7) als
  zurückgezogen kennzeichnen und auf diese Spec verweisen.

### Key Entities

- **Vault-Session**: Die Zeitspanne vom Entsperren bis zum Sperren oder
  Schließen einer Vault, gleichbedeutend mit einem App-Prozess (Spec 013,
  ADR-0003). Das Layout lebt genau so lange.
- **Layout**: Arbeitsbereiche, Fenster und Tabs samt Anordnung und Zustand.
  Existiert nur noch im Arbeitsspeicher der laufenden Session.
- **Frühere Layoutdaten**: Von Versionen mit Spec 015 in der Vault gespeicherte
  Arbeitsbereiche, Fenster und Tabs, je Gerät. Werden einmalig entfernt.

## Success Criteria _(mandatory)_

### Measurable Outcomes

- **SC-001**: In 100 % der Starts, ob nach normalem Beenden, Sperren oder
  Absturz, zeigt die Shell genau einen Arbeitsbereich ohne Fenster (ohne
  Deep-Link).
- **SC-002**: Nach dem ersten Öffnen mit dieser Version findet eine Untersuchung
  der Vault-Datei keine Spur früherer Arbeitsbereiche, Fenster oder Tabs mehr.
- **SC-003**: Das Öffnen einer Vault dauert höchstens so lange wie vorher. Die
  einmalige Bereinigung verlängert es um höchstens eine Sekunde.
- **SC-004**: Alle automatischen Prüfungen und die manuellen Szenarien von Spec
  015 und 020 ohne Neustart-Bezug bestehen unverändert.
- **SC-005**: Die Anwendung bietet keine Möglichkeit mehr, ein Layout zu laden
  oder zu speichern (nachprüfbar an der Liste ihrer Schnittstellen).

## Assumptions

- Ein Downgrade auf eine ältere holzi-Version nach der Bereinigung ist nicht
  unterstützt, wie bei anderen Schemaänderungen auch.
- Es gibt keinen Wunsch nach einer optionalen Wiederherstellung („Sitzung
  wiederherstellen“). Sollte sie später gewünscht werden, ist das eine eigene
  Spec.
- Die Einzelinstanz-Regeln, die Mehrfachinstanz-Testapp und die
  Geometrie-Anpassung aus Spec 015 bleiben unverändert, nur ohne Neustart-Bezug.
- Desktop-Symbole und das Raster auf dem Arbeitsbereich (geplante Folge-Spec)
  sind nicht Teil dieser Spec. Ob diese gespeichert werden, entscheidet die
  Folge-Spec.
- Der PR zu dieser Spec setzt auf dem PR zu Spec 020 auf (gestapelt), weil beide
  den Shell-Store ändern.

## Nicht im Umfang

- Jede Form von Sitzungswiederherstellung, auch optional.
- Änderungen an der Speicherung von Tab-Inhalten (Chat-Verlauf, Einstellungen,
  Modelle).
- Desktop-Symbole und Raster.
