# Feature Specification: Sitzung wiederherstellen (wählbar)

**Feature Branch**: `022-session-restore`
**Created**: 2026-09-25
**Status**: Draft
**Input**: Betreiberentscheidung vom 2026-09-25 beim Test von Spec 020:
Arbeitsbereiche, Fenster und Tabs werden standardmäßig nicht mehr über das Ende
einer Vault-Session hinaus gespeichert. Wer seine Sitzung behalten möchte,
schaltet das in den Einstellungen ein, wahlweise für dieses Gerät oder für die
ganze Vault. Layouts, die frühere Versionen ungefragt gespeichert haben, werden
beim Update entfernt.

## Beziehung zu bestehenden Specs

- [`015-workspace-shell`](../015-workspace-shell/spec.md): User Story 5
  („Layout bleibt über Neustarts erhalten“) und FR-023 bis FR-025 gelten nur
  noch, wenn der Nutzer die Wiederherstellung eingeschaltet hat (FR-001 dieser
  Spec). Ohne diese Einstellung beginnt jede Session leer. Dasselbe gilt für den
  Neustart-Teil von User Story 7. Alle übrigen Anforderungen an Fenster, Tabs
  und Arbeitsbereiche innerhalb einer Session gelten unverändert.
- [`013-vault-lifecycle-isolation`](../013-vault-lifecycle-isolation/spec.md)
  und ADR-0003: Eine Vault-Session ist genau ein App-Prozess; Sperren und
  Schließen beenden sie. Ohne Einstellung lebt das Layout genau so lange. Das
  passt zum Internet-Café-Gedanken aus Spec 013: Die Vault-Datei trägt dann
  keine Spuren davon, welche Fenster zuletzt offen waren.
- [`002-onboarding-model-prefs`](../002-onboarding-model-prefs/spec.md): Die
  neue Einstellung folgt demselben Muster wie das Standardmodell. Es gibt einen
  Wert für die ganze Vault und optional einen Wert nur für dieses Gerät, der den
  Vault-Wert überschreibt.
- [`020-tab-navigation`](../020-tab-navigation/spec.md): Die Tab-Historie wird
  auch mit eingeschalteter Wiederherstellung nicht gespeichert
  (Betreiberentscheidung dort). Die Einstellung ist eine Aktion im Katalog von
  Spec 020 (FR-024 dort), wie jede andere Änderung einer Einstellung.
- ADR-0001 (gerätebezogene Daten): Ein gespeichertes Layout bleibt
  gerätebezogen, auch wenn die Einstellung für die ganze Vault gilt. Jedes Gerät
  speichert und sieht nur sein eigenes Layout.
- Die geplante Spec zur **Einstellungs-App** (Seitenleiste mit Kategorien)
  übernimmt diese Einstellung in eine passende Kategorie. Bis dahin steht sie in
  der heutigen Einstellungsansicht.
- Tab-Inhalte sind nicht betroffen: Chat-Verlauf, Einstellungen, Modelle und
  alle anderen Daten der Apps bleiben wie bisher in der Vault.

## User Scenarios & Testing _(mandatory)_

### User Story 1 - Standardmäßig beginnt jeder Start leer (Priority: P1)

Ein Nutzer arbeitet mit mehreren Arbeitsbereichen und Fenstern, schließt holzi
und öffnet die Vault später wieder. Er hat nichts eingestellt. Er findet einen
einzigen, leeren Arbeitsbereich vor und öffnet die Apps, die er gerade braucht.

**Why this priority**: Das ist der neue Standard. Wiederhergestellte Fenster
wirken beim Start unaufgeräumt und verraten, woran zuletzt gearbeitet wurde.

**Independent Test**: Ohne die Einstellung einzuschalten zwei Arbeitsbereiche
anlegen, im zweiten ein Fenster mit zwei Tabs öffnen, holzi beenden und die
Vault erneut öffnen: Es gibt genau einen Arbeitsbereich, er ist aktiv und hat
kein Fenster. Die Vault-Datei enthält kein gespeichertes Layout.

**Acceptance Scenarios**:

1. **Given** die Wiederherstellung ist weder für dieses Gerät noch für die Vault
   eingeschaltet, **When** der Nutzer holzi beendet und die Vault erneut öffnet,
   **Then** zeigt die Shell genau einen Arbeitsbereich ohne Fenster.
2. **Given** dieselbe Ausgangslage, **When** der Nutzer die Vault sperrt und
   wieder entsperrt, **Then** beginnt die neue Session ebenfalls leer.
3. **Given** dieselbe Ausgangslage, **When** die Session läuft, **Then** wird zu
   keinem Zeitpunkt ein Layout in der Vault gespeichert.
4. **Given** der Nutzer öffnet holzi über eine Adresse, die eine App an einem Ort
   öffnet (Spec 020), **When** die Shell erscheint, **Then** ist genau diese App
   im einzigen Arbeitsbereich geöffnet, und sonst nichts.

---

### User Story 2 - Wiederherstellung einschalten (Priority: P1)

Ein Nutzer möchte auf seinem Laptop jeden Morgen dort weitermachen, wo er
aufgehört hat. Er öffnet die Einstellungen, schaltet „Sitzung wiederherstellen“
für dieses Gerät ein und arbeitet weiter. Beim nächsten Start sind seine
Arbeitsbereiche, Fenster und Tabs wieder da.

**Why this priority**: Ohne diesen Weg verlören Nutzer, die ihre Sitzung
behalten wollen, eine Fähigkeit, die sie heute haben.

**Independent Test**: Die Einstellung für dieses Gerät einschalten, zwei
Arbeitsbereiche mit Fenstern und Tabs einrichten, holzi beenden und die Vault
erneut öffnen: Das Verhalten entspricht Spec 015 User Story 5 (Arbeitsbereiche,
Fenster mit Position, Größe und Zustand, Tabs mit Reihenfolge und aktivem Tab,
zuletzt aktiver Arbeitsbereich).

**Acceptance Scenarios**:

1. **Given** die Einstellungsansicht, **When** der Nutzer die Wiederherstellung
   für dieses Gerät einschaltet, **Then** gilt sie sofort: Das aktuelle Layout
   wird ab diesem Moment gespeichert, ohne Neustart.
2. **Given** die Wiederherstellung ist eingeschaltet, **When** der Nutzer die
   Vault erneut öffnet, **Then** erscheint das zuletzt gespeicherte Layout
   dieses Geräts gemäß Spec 015 User Story 5.
3. **Given** ein wiederhergestellter Tab, **When** die Shell erscheint,
   **Then** beginnt er an der Startansicht seiner App ohne Vor-/Zurück-Historie
   (Spec 020); ein Chat-Tab beginnt mit einer neuen Unterhaltung (Spec 004).
4. **Given** die Einstellungsansicht, **When** der Nutzer sie öffnet, **Then**
   sieht er, welcher Wert für dieses Gerät und welcher für die ganze Vault
   gesetzt ist, und welcher davon auf diesem Gerät gerade gilt.

---

### User Story 3 - Für alle Geräte oder nur für dieses (Priority: P2)

Eine Nutzerin verwendet ihre Vault auf ihrem Arbeitsrechner, ihrem Laptop und
gelegentlich auf fremden Rechnern. Sie schaltet die Wiederherstellung für die
ganze Vault ein, damit ihre eigenen Geräte ihr Layout behalten. Auf dem Laptop,
den sie oft unterwegs nutzt, schaltet sie sie für dieses Gerät gezielt aus.

**Why this priority**: Wer mehrere eigene Geräte hat, soll die Einstellung nicht
auf jedem einzeln setzen müssen. Die Ausnahme je Gerät ist aber nötig, sonst
würde die Einstellung für die Vault auch auf Geräten gelten, auf denen man das
nicht will.

**Independent Test**: Die Einstellung für die Vault einschalten und auf einem
zweiten Gerät öffnen: Dort wird das Layout ebenfalls gespeichert, aber
getrennt. Auf dem zweiten Gerät die Einstellung für dieses Gerät ausschalten:
Dort beginnt jeder Start leer, das erste Gerät speichert weiter.

**Acceptance Scenarios**:

1. **Given** die Wiederherstellung ist für die Vault eingeschaltet und für
   dieses Gerät nicht gesetzt, **When** der Nutzer die Vault auf diesem Gerät
   öffnet, **Then** gilt die Einstellung der Vault.
2. **Given** die Wiederherstellung ist für die Vault eingeschaltet und für
   dieses Gerät ausgeschaltet, **When** der Nutzer die Vault auf diesem Gerät
   öffnet, **Then** beginnt die Session leer, und es wird kein Layout
   gespeichert.
3. **Given** die Wiederherstellung ist für die Vault ausgeschaltet und für
   dieses Gerät eingeschaltet, **When** der Nutzer die Vault auf diesem Gerät
   öffnet, **Then** wird das Layout dieses Geräts wiederhergestellt.
4. **Given** zwei Geräte mit eingeschalteter Wiederherstellung, **When** jedes
   seine Vault öffnet, **Then** sieht jedes nur sein eigenes Layout.
5. **Given** ein Wert für dieses Gerät ist gesetzt, **When** der Nutzer ihn
   zurücksetzt, **Then** gilt auf diesem Gerät wieder der Wert der Vault.

---

### User Story 4 - Ausschalten entfernt das gespeicherte Layout (Priority: P1)

Ein Nutzer hatte die Wiederherstellung eingeschaltet und entscheidet sich um. Er
schaltet sie aus. Das bisher gespeicherte Layout verschwindet sofort aus der
Vault, die laufende Session bleibt, wie sie ist.

**Why this priority**: Wer die Wiederherstellung ausschaltet, erwartet, dass
nichts mehr liegen bleibt. Sonst hätte das Ausschalten nur eine halbe Wirkung.

**Independent Test**: Die Wiederherstellung einschalten, ein Layout einrichten,
sie wieder ausschalten: Die Fenster bleiben offen, die Vault-Datei enthält kein
gespeichertes Layout dieses Geräts mehr, und der nächste Start beginnt leer.

**Acceptance Scenarios**:

1. **Given** die Wiederherstellung gilt auf diesem Gerät, **When** der Nutzer
   eine Änderung vornimmt, nach der sie auf diesem Gerät nicht mehr gilt,
   **Then** wird das gespeicherte Layout dieses Geräts sofort aus der Vault
   entfernt.
2. **Given** dieselbe Änderung, **When** sie wirksam ist, **Then** bleiben die
   offenen Arbeitsbereiche, Fenster und Tabs der laufenden Session unverändert.
3. **Given** die Wiederherstellung wird für die Vault ausgeschaltet, **When**
   ein anderes Gerät, auf dem sie nur über die Vault galt, die Vault das nächste
   Mal öffnet, **Then** entfernt es sein gespeichertes Layout und beginnt leer.
4. **Given** die Wiederherstellung wird für die Vault ausgeschaltet, **When** ein
   Gerät sie für sich selbst eingeschaltet hat, **Then** speichert dieses Gerät
   weiter.

---

### User Story 5 - Ungefragt gespeicherte Layouts verschwinden beim Update (Priority: P1)

Ein Nutzer hat mit einer früheren holzi-Version gearbeitet, die sein Layout
ungefragt gespeichert hat. Nach dem Update öffnet er die Vault. Das alte Layout
erscheint nicht mehr, und die Vault enthält es danach auch nicht mehr.

**Why this priority**: Der neue Standard ist „nicht speichern“. Ohne diesen
Schritt blieben alte Layouts als tote Information in der Vault liegen und
reisten mit der Vault-Datei weiter.

**Independent Test**: Eine Vault, in der eine frühere Version ein Layout
gespeichert hat, mit der neuen Version öffnen: Die Shell ist leer, und die
Vault-Datei enthält danach keine Layoutdaten mehr, auf keinem Gerät der Vault.

**Acceptance Scenarios**:

1. **Given** eine Vault mit gespeichertem Layout aus einer früheren Version,
   **When** der Nutzer sie mit dieser Version öffnet, **Then** erscheint die
   Shell mit genau einem leeren Arbeitsbereich.
2. **Given** dieselbe Vault, **When** das Öffnen abgeschlossen ist, **Then**
   enthält die Vault keine gespeicherten Arbeitsbereiche, Fenster oder Tabs
   mehr, auch nicht die anderer Geräte.
3. **Given** die Bereinigung schlägt fehl, **When** die Shell erscheint,
   **Then** startet sie trotzdem leer, die Vault bleibt nutzbar, und der Fehler
   wird protokolliert, ohne den Nutzer mit einem Fehlerbildschirm aufzuhalten.

---

### Edge Cases

- Der Nutzer beendet holzi hart (Absturz, Prozess beendet), während die
  Wiederherstellung eingeschaltet ist. Beim nächsten Öffnen erscheint das
  zuletzt gespeicherte Layout, höchstens mit den Änderungen der letzten
  Augenblicke vor dem Absturz verloren (wie in Spec 015).
- Derselbe Absturz bei ausgeschalteter Wiederherstellung: Die Shell ist leer wie
  nach einem normalen Beenden. Es gibt keinen Wiederherstellungsdialog.
- Der Nutzer schaltet die Wiederherstellung ein und sofort wieder aus. Danach
  liegt kein Layout in der Vault.
- Ein anderes Gerät hat die Vault-Einstellung geändert, während dieses Gerät
  eine Session offen hat. Die Änderung gilt auf diesem Gerät spätestens ab dem
  nächsten Öffnen; bis dahin gilt, was beim Öffnen galt.
- Die Einstellung lässt sich nicht lesen. Die Shell verhält sich wie bei
  ausgeschalteter Wiederherstellung und startet leer.
- Ein gespeichertes Layout ist nicht lesbar oder verweist auf eine unbekannte
  App. Es gilt Spec 015 FR-025: Start mit dem verwertbaren Rest oder leer, kein
  Fehlerbildschirm.
- Die Vault wird nach dem Update mit einer älteren holzi-Version geöffnet. Das
  ist nicht unterstützt (siehe Assumptions).

## Requirements _(mandatory)_

### Functional Requirements

**Einstellung**

- **FR-001**: holzi MUSS eine Einstellung „Sitzung wiederherstellen“ anbieten.
  Gilt sie auf einem Gerät, werden dort Arbeitsbereiche, Fenster und Tabs
  gemäß Spec 015 FR-023 bis FR-025 gespeichert und beim nächsten Öffnen
  wiederhergestellt. Gilt sie nicht, DARF NICHTS davon über das Ende der
  Vault-Session hinaus gespeichert werden.
- **FR-002**: Die Einstellung MUSS einen Wert für die ganze Vault und einen Wert
  nur für dieses Gerät haben können. Auf einem Gerät gilt der Gerätewert, falls
  gesetzt, sonst der Vault-Wert, sonst „aus“.
- **FR-003**: Der Standard MUSS „aus“ sein: Eine neue Vault und eine
  aktualisierte Vault haben weder einen Vault- noch einen Gerätewert gesetzt.
- **FR-004**: Die Einstellungsansicht MUSS den Gerätewert, den Vault-Wert und
  den auf diesem Gerät geltenden Wert zeigen. Der Nutzer MUSS jeden der beiden
  Werte einschalten, ausschalten und zurücksetzen können. Die Bedienung folgt
  dem Muster der Einstellung zum Standardmodell (Spec 002).
- **FR-005**: Eine Änderung der Einstellung MUSS auf diesem Gerät sofort
  wirken, ohne Neustart: Beginnt sie zu gelten, wird das aktuelle Layout ab
  sofort gespeichert; hört sie auf zu gelten, gilt FR-007.
- **FR-006**: Das Setzen, Ändern und Zurücksetzen der Einstellung MUSS eine
  Aktion im Katalog von Spec 020 sein, mit derselben Wirkung aus Oberfläche,
  Tastenkürzel und für Agenten mit passender Berechtigung.

**Entfernen gespeicherter Layouts**

- **FR-007**: Hört die Einstellung auf diesem Gerät auf zu gelten, MUSS das
  gespeicherte Layout dieses Geräts sofort aus der Vault entfernt werden. Die
  laufende Session bleibt unverändert.
- **FR-008**: Öffnet ein Gerät die Vault und die Einstellung gilt dort nicht,
  MUSS ein noch vorhandenes gespeichertes Layout dieses Geräts entfernt werden,
  bevor die Shell erscheint (zum Beispiel, weil ein anderes Gerät den
  Vault-Wert ausgeschaltet hat).
- **FR-009**: Beim ersten Öffnen einer Vault mit dieser Version MÜSSEN alle
  Layoutdaten, die frühere Versionen gespeichert haben, entfernt werden:
  Arbeitsbereiche, Fenster und Tabs aller Geräte der Vault.
- **FR-010**: Ein entferntes Layout DARF sich aus der Vault-Datei nicht
  wiederherstellen und nicht an andere Geräte weitergeben lassen, auch nicht
  als gelöschter Eintrag.
- **FR-011**: Schlägt ein Entfernen fehl, MUSS die Shell trotzdem normal
  starten beziehungsweise weiterlaufen. Die Vault MUSS nutzbar bleiben, der
  Fehler MUSS protokolliert werden, und das Entfernen MUSS beim nächsten Öffnen
  erneut versucht werden. Ein nicht entferntes Layout DARF bei ausgeschalteter
  Einstellung NICHT wiederhergestellt werden.

**Verhalten der Shell**

- **FR-012**: Ohne geltende Einstellung MUSS jede Vault-Session (Öffnen,
  Entsperren, Neustart nach Absturz) mit genau einem Arbeitsbereich ohne Fenster
  beginnen, es sei denn, der Start öffnet ausdrücklich eine App (FR-013).
- **FR-013**: Öffnet der Start eine App an einem Ort (Deep-Link, frühere
  Vollseiten-Adressen, Spec 020 FR-012), MUSS diese App erscheinen: ohne
  geltende Einstellung im einzigen Arbeitsbereich, mit geltender Einstellung
  zusätzlich zum wiederhergestellten Layout wie bisher.
- **FR-014**: Die Shell MUSS während der Session alle Fähigkeiten aus Spec 015
  und 020 behalten, unabhängig von der Einstellung.
- **FR-015**: Die Dokumentation von Spec 015 MUSS bei User Story 5, FR-023 bis
  FR-025 und dem Neustart-Teil von User Story 7 vermerken, dass sie nur bei
  eingeschalteter Wiederherstellung gelten, und auf diese Spec verweisen.

### Key Entities

- **Vault-Session**: Die Zeitspanne vom Entsperren bis zum Sperren oder
  Schließen einer Vault, gleichbedeutend mit einem App-Prozess (Spec 013,
  ADR-0003).
- **Einstellung „Sitzung wiederherstellen“**: Ein-/Aus-Wert, einmal für die
  Vault und optional je Gerät. Der geltende Wert eines Geräts ergibt sich aus
  FR-002.
- **Gespeichertes Layout**: Arbeitsbereiche, Fenster und Tabs samt Anordnung und
  Zustand eines Geräts (Spec 015 FR-023). Existiert nur, solange die
  Einstellung auf diesem Gerät gilt.
- **Frühere Layoutdaten**: Von Versionen mit Spec 015 ungefragt gespeicherte
  Layouts aller Geräte. Werden beim Update einmalig entfernt.

## Success Criteria _(mandatory)_

### Measurable Outcomes

- **SC-001**: Ohne geltende Einstellung zeigt die Shell in 100 % der Starts,
  ob nach normalem Beenden, Sperren oder Absturz, genau einen Arbeitsbereich
  ohne Fenster (ohne Deep-Link).
- **SC-002**: Mit geltender Einstellung stellt die Shell das Layout in 100 % der
  normalen Neustarts so wieder her, wie Spec 015 User Story 5 es beschreibt.
- **SC-003**: Nach dem Ausschalten und nach dem ersten Öffnen mit dieser Version
  findet eine Untersuchung der Vault-Datei keine Spur eines entfernten Layouts
  mehr.
- **SC-004**: Ein Nutzer findet die Einstellung und schaltet sie in weniger als
  30 Sekunden ein, ausgehend vom geöffneten Arbeitsbereich.
- **SC-005**: Das Öffnen einer Vault dauert höchstens so lange wie vorher. Die
  einmalige Bereinigung beim Update verlängert es um höchstens eine Sekunde.
- **SC-006**: Alle automatischen Prüfungen und die manuellen Szenarien von Spec
  015 und 020 bestehen, die Neustart-Szenarien aus Spec 015 mit eingeschalteter
  Wiederherstellung.

## Assumptions

- Ein Downgrade auf eine ältere holzi-Version nach dem Update ist nicht
  unterstützt, wie bei anderen Schemaänderungen auch.
- Die Einstellung ist keine Leitplanke im Sinne von Spec 020 (FR-032 dort). Sie
  bekommt den Berechtigungsbereich für Geräteeinstellungen, den Spec 021 für
  externe Agenten freigeben kann.
- Mit eingeschalteter Wiederherstellung gilt, was Spec 015 festlegt: kein
  Wiederherstellen von Tab-Inhalten, keine Tab-Historie (Spec 020), keine
  zuletzt geöffnete Ansicht innerhalb einer App.
- Desktop-Symbole und das Raster auf dem Arbeitsbereich (geplante Folge-Spec)
  sind nicht Teil dieser Spec. Ob sie gespeichert werden, entscheidet die
  Folge-Spec.
- Der PR zu dieser Spec setzt auf dem PR zu Spec 020 auf (gestapelt), weil beide
  den Shell-Store ändern.

## Nicht im Umfang

- Mehrere benannte Sitzungen oder ein Sitzungsverlauf. Gespeichert wird genau
  ein Layout je Gerät.
- Ein Wiederherstellungsdialog nach Abstürzen.
- Änderungen an der Speicherung von Tab-Inhalten (Chat-Verlauf, Einstellungen,
  Modelle).
- Die Einordnung der Einstellung in eine Kategorie der künftigen
  Einstellungs-App.
