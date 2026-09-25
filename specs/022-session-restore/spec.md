# Feature Specification: Sitzung wiederherstellen (wählbar)

**Feature Branch**: `022-session-restore`
**Created**: 2026-09-25
**Status**: Draft
**Input**: Betreiberentscheidung vom 2026-09-25 beim Test von Spec 020: Welche
Arbeitsbereiche, Fenster und Tabs offen sind, wird standardmäßig nicht mehr
über das Ende einer Vault-Session hinaus gespeichert. Wer seine Sitzung
behalten möchte, schaltet das in den Einstellungen ein, wahlweise für dieses
Gerät oder für die ganze Vault. Sitzungen, die frühere Versionen ungefragt
gespeichert haben, werden beim Update entfernt.

## Begriffe

- **Sitzung**: welche Arbeitsbereiche es gibt und in welcher Reihenfolge,
  welcher davon aktiv ist, welche Fenster in welchem Arbeitsbereich offen sind
  (mit Position, Größe, minimiert oder maximiert) und welche Tabs jedes Fenster
  hat (welche App, Reihenfolge, aktiver Tab). Das ist genau das, was Spec 015
  bisher bei jedem Neustart wiederhergestellt hat (FR-023 dort).
- **Gespeicherte Sitzung**: eine Sitzung, die in der Vault liegt, damit der
  nächste Start sie wiederherstellen kann. Sie gehört immer zu genau einem
  Gerät.
- **Vault-Session**: die Zeitspanne vom Entsperren bis zum Sperren oder
  Schließen einer Vault, gleichbedeutend mit einem App-Prozess (Spec 013,
  ADR-0003).

## Beziehung zu bestehenden Specs

- [`015-workspace-shell`](../015-workspace-shell/spec.md): User Story 5
  („Layout bleibt über Neustarts erhalten“) und FR-023 bis FR-025 gelten nur
  noch, wenn der Nutzer die Wiederherstellung eingeschaltet hat (FR-001 dieser
  Spec). Ohne diese Einstellung beginnt jede Vault-Session leer. Dasselbe gilt
  für den Neustart-Teil von User Story 7. Alle übrigen Anforderungen an
  Fenster, Tabs und Arbeitsbereiche innerhalb einer Vault-Session gelten
  unverändert.
- [`013-vault-lifecycle-isolation`](../013-vault-lifecycle-isolation/spec.md)
  und ADR-0003: Sperren und Schließen beenden die Vault-Session. Ohne
  Einstellung lebt die Sitzung genau so lange. Das passt zum
  Internet-Café-Gedanken aus Spec 013: Die Vault-Datei verrät dann nicht,
  welche Fenster zuletzt offen waren.
- [`002-onboarding-model-prefs`](../002-onboarding-model-prefs/spec.md): Die
  neue Einstellung folgt demselben Muster wie das Standardmodell. Es gibt einen
  Wert für die ganze Vault und optional einen Wert nur für dieses Gerät, der den
  Vault-Wert überschreibt.
- [`020-tab-navigation`](../020-tab-navigation/spec.md): Die Vor-/Zurück-
  Historie eines Tabs wird auch mit eingeschalteter Wiederherstellung nicht
  gespeichert (Betreiberentscheidung dort). Die Einstellung ist eine Aktion im
  Katalog von Spec 020 (FR-024 dort), wie jede andere Änderung einer
  Einstellung.
- ADR-0001 (gerätebezogene Daten): Eine gespeicherte Sitzung bleibt
  gerätebezogen, auch wenn die Einstellung für die ganze Vault gilt. Jedes Gerät
  speichert und sieht nur seine eigene.
- Synchronisierung (haex-crdt): Gelöschte Einträge synchronisierter Daten
  hinterlassen einen Löschvermerk, der an andere Geräte weitergegeben und dort
  angewendet wird. Eine Löschung ist dort also nicht spurlos. Diese Spec
  verlangt deshalb, dass gespeicherte Sitzungen gar nicht erst an andere Geräte
  gehen (FR-010), und erlaubt für die schon synchronisierten Altdaten einen
  Löschvermerk ohne Inhalt (FR-011).
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
kein Fenster. Die Vault enthält keine gespeicherte Sitzung.

**Acceptance Scenarios**:

1. **Given** die Wiederherstellung ist weder für dieses Gerät noch für die Vault
   eingeschaltet, **When** der Nutzer holzi beendet und die Vault erneut öffnet,
   **Then** zeigt holzi genau einen Arbeitsbereich ohne Fenster.
2. **Given** dieselbe Ausgangslage, **When** der Nutzer die Vault sperrt und
   wieder entsperrt, **Then** beginnt die neue Vault-Session ebenfalls leer.
3. **Given** dieselbe Ausgangslage, **When** die Vault-Session läuft, **Then**
   wird zu keinem Zeitpunkt eine Sitzung in der Vault gespeichert.
4. **Given** der Nutzer öffnet holzi über eine Adresse, die eine App an einem Ort
   öffnet (Spec 020), **When** holzi erscheint, **Then** ist genau diese App
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
   für dieses Gerät einschaltet, **Then** gilt sie sofort: Die aktuelle Sitzung
   wird ab diesem Moment gespeichert, ohne Neustart.
2. **Given** die Wiederherstellung ist eingeschaltet, **When** der Nutzer die
   Vault erneut öffnet, **Then** erscheint die zuletzt gespeicherte Sitzung
   dieses Geräts gemäß Spec 015 User Story 5.
3. **Given** ein wiederhergestellter Tab, **When** holzi erscheint,
   **Then** beginnt er an der Startansicht seiner App ohne Vor-/Zurück-Historie
   (Spec 020); ein Chat-Tab beginnt mit einer neuen Unterhaltung (Spec 004).
4. **Given** die Einstellungsansicht, **When** der Nutzer sie öffnet, **Then**
   sieht er, welcher Wert für dieses Gerät und welcher für die ganze Vault
   gesetzt ist, und welcher davon auf diesem Gerät gerade gilt.

---

### User Story 3 - Für alle Geräte oder nur für dieses (Priority: P2)

Eine Nutzerin verwendet ihre Vault auf ihrem Arbeitsrechner, ihrem Laptop und
gelegentlich auf fremden Rechnern. Sie schaltet die Wiederherstellung für die
ganze Vault ein, damit ihre eigenen Geräte ihre Sitzung behalten. Auf dem
Laptop, den sie oft unterwegs nutzt, schaltet sie sie für dieses Gerät gezielt
aus.

**Why this priority**: Wer mehrere eigene Geräte hat, soll die Einstellung nicht
auf jedem einzeln setzen müssen. Die Ausnahme je Gerät ist aber nötig, sonst
würde die Einstellung für die Vault auch auf Geräten gelten, auf denen man das
nicht will.

**Independent Test**: Die Einstellung für die Vault einschalten und die Vault
auf einem zweiten Gerät öffnen: Dort wird die Sitzung ebenfalls gespeichert,
aber getrennt. Auf dem zweiten Gerät die Einstellung für dieses Gerät
ausschalten: Dort beginnt jeder Start leer, das erste Gerät speichert weiter.

**Acceptance Scenarios**:

1. **Given** die Wiederherstellung ist für die Vault eingeschaltet und für
   dieses Gerät nicht gesetzt, **When** der Nutzer die Vault auf diesem Gerät
   öffnet, **Then** gilt die Einstellung der Vault.
2. **Given** die Wiederherstellung ist für die Vault eingeschaltet und für
   dieses Gerät ausgeschaltet, **When** der Nutzer die Vault auf diesem Gerät
   öffnet, **Then** beginnt die Vault-Session leer, und es wird keine Sitzung
   gespeichert.
3. **Given** die Wiederherstellung ist für die Vault ausgeschaltet und für
   dieses Gerät eingeschaltet, **When** der Nutzer die Vault auf diesem Gerät
   öffnet, **Then** wird die Sitzung dieses Geräts wiederhergestellt.
4. **Given** zwei Geräte mit eingeschalteter Wiederherstellung, **When** jedes
   seine Vault öffnet, **Then** sieht jedes nur seine eigene Sitzung.
5. **Given** ein Wert für dieses Gerät ist gesetzt, **When** der Nutzer ihn
   zurücksetzt, **Then** gilt auf diesem Gerät wieder der Wert der Vault.

---

### User Story 4 - Ausschalten entfernt die gespeicherte Sitzung (Priority: P1)

Ein Nutzer hatte die Wiederherstellung eingeschaltet und entscheidet sich um. Er
schaltet sie aus. Die bisher gespeicherte Sitzung verschwindet sofort aus der
Vault, die offenen Fenster bleiben, wie sie sind.

**Why this priority**: Wer die Wiederherstellung ausschaltet, erwartet, dass
nichts mehr liegen bleibt. Sonst hätte das Ausschalten nur eine halbe Wirkung.

**Independent Test**: Die Wiederherstellung einschalten, Fenster und Tabs
öffnen, sie wieder ausschalten: Die Fenster bleiben offen, die Vault enthält
keine gespeicherte Sitzung dieses Geräts mehr, und der nächste Start beginnt
leer.

**Acceptance Scenarios**:

1. **Given** die Wiederherstellung gilt auf diesem Gerät, **When** der Nutzer
   eine Änderung vornimmt, nach der sie auf diesem Gerät nicht mehr gilt,
   **Then** wird die gespeicherte Sitzung dieses Geräts sofort aus der Vault
   entfernt.
2. **Given** dieselbe Änderung, **When** sie wirksam ist, **Then** bleiben die
   offenen Arbeitsbereiche, Fenster und Tabs unverändert.
3. **Given** die Wiederherstellung wird für die Vault ausgeschaltet, **When**
   ein anderes Gerät, auf dem sie nur über die Vault galt, die Vault das nächste
   Mal öffnet, **Then** entfernt es seine gespeicherte Sitzung und beginnt leer.
4. **Given** die Wiederherstellung wird für die Vault ausgeschaltet, **When** ein
   Gerät sie für sich selbst eingeschaltet hat, **Then** speichert dieses Gerät
   weiter.

---

### User Story 5 - Ungefragt gespeicherte Sitzungen verschwinden beim Update (Priority: P1)

Ein Nutzer hat mit einer früheren holzi-Version gearbeitet, die seine Sitzung
ungefragt gespeichert hat. Nach dem Update öffnet er die Vault. Die alte Sitzung
erscheint nicht mehr, und die Vault enthält sie danach auch nicht mehr.

**Why this priority**: Der neue Standard ist „nicht speichern“. Ohne diesen
Schritt blieben alte Sitzungen als tote Information in der Vault liegen und
reisten mit der Vault-Datei weiter.

**Independent Test**: Eine Vault, in der eine frühere Version eine Sitzung
gespeichert hat, mit der neuen Version öffnen: holzi ist leer, und die Vault
enthält danach keine Inhalte früherer Sitzungen mehr, von keinem Gerät der
Vault.

**Acceptance Scenarios**:

1. **Given** eine Vault mit einer Sitzung, die eine frühere Version gespeichert
   hat, **When** der Nutzer sie mit dieser Version öffnet, **Then** erscheint
   holzi mit genau einem leeren Arbeitsbereich.
2. **Given** dieselbe Vault, **When** das Öffnen abgeschlossen ist, **Then**
   enthält die Vault keine Inhalte früherer Sitzungen mehr, auch nicht die
   anderer Geräte, die frühere Versionen dorthin synchronisiert haben.
3. **Given** die Bereinigung schlägt fehl, **When** holzi erscheint,
   **Then** startet sie trotzdem leer, die Vault bleibt nutzbar, und der Fehler
   wird protokolliert, ohne den Nutzer mit einem Fehlerbildschirm aufzuhalten.

---

### Edge Cases

- Der Nutzer beendet holzi hart (Absturz, Prozess beendet), während die
  Wiederherstellung eingeschaltet ist. Beim nächsten Öffnen erscheint die
  zuletzt gespeicherte Sitzung, höchstens mit den Änderungen der letzten
  Augenblicke vor dem Absturz verloren (wie in Spec 015).
- Derselbe Absturz bei ausgeschalteter Wiederherstellung: holzi ist leer wie
  nach einem normalen Beenden. Es gibt keinen Wiederherstellungsdialog.
- Der Nutzer schaltet die Wiederherstellung ein und sofort wieder aus. Danach
  liegt keine Sitzung in der Vault.
- Ein anderes Gerät hat den Vault-Wert geändert, während dieses Gerät eine
  Vault-Session offen hat. Die Änderung gilt auf diesem Gerät spätestens ab dem
  nächsten Öffnen; bis dahin gilt, was beim Öffnen galt.
- Die Einstellung lässt sich nicht lesen. holzi verhält sich wie bei
  ausgeschalteter Wiederherstellung und startet leer.
- Eine gespeicherte Sitzung ist nicht lesbar oder verweist auf eine unbekannte
  App. Es gilt Spec 015 FR-025: Start mit dem verwertbaren Rest oder leer, kein
  Fehlerbildschirm.
- Die Vault wird nach dem Update mit einer älteren holzi-Version geöffnet. Das
  ist nicht unterstützt (siehe Assumptions).

## Requirements _(mandatory)_

### Functional Requirements

**Einstellung**

- **FR-001**: holzi MUSS eine Einstellung „Sitzung wiederherstellen“ anbieten.
  Gilt sie auf einem Gerät, wird dort die Sitzung gemäß Spec 015 FR-023 bis
  FR-025 gespeichert und beim nächsten Öffnen wiederhergestellt. Gilt sie
  nicht, DARF die Sitzung NICHT über das Ende der Vault-Session hinaus
  gespeichert werden.
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
  wirken, ohne Neustart: Beginnt sie zu gelten, wird die aktuelle Sitzung ab
  sofort gespeichert; hört sie auf zu gelten, gilt FR-007.
- **FR-006**: Das Setzen, Ändern und Zurücksetzen der Einstellung MUSS eine
  Aktion im Katalog von Spec 020 sein, mit derselben Wirkung aus Oberfläche,
  Tastenkürzel und für Agenten mit passender Berechtigung.

**Entfernen gespeicherter Sitzungen**

- **FR-007**: Hört die Einstellung auf diesem Gerät auf zu gelten, MUSS die
  gespeicherte Sitzung dieses Geräts sofort aus der Vault entfernt werden. Die
  offenen Arbeitsbereiche, Fenster und Tabs bleiben unverändert.
- **FR-008**: Öffnet ein Gerät die Vault und die Einstellung gilt dort nicht,
  MUSS eine noch vorhandene gespeicherte Sitzung dieses Geräts entfernt werden,
  bevor holzi erscheint (zum Beispiel, weil ein anderes Gerät den Vault-Wert
  ausgeschaltet hat).
- **FR-009**: Beim ersten Öffnen einer Vault mit dieser Version MÜSSEN alle
  Sitzungen, die frühere Versionen gespeichert haben, entfernt werden, die
  aller Geräte der Vault.
- **FR-010**: Eine gespeicherte Sitzung DARF NICHT an andere Geräte
  weitergegeben werden, weder beim Speichern noch beim Entfernen. Entfernen
  MUSS sie vollständig aus der Vault-Datei dieses Geräts löschen, ohne einen
  Vermerk zu hinterlassen, der an andere Geräte geht.
- **FR-011**: Für Sitzungen, die frühere Versionen bereits an andere Geräte
  synchronisiert haben, DÜRFEN nach dem Entfernen keine Inhalte in der Vault
  bleiben (welche Apps offen waren, Fensterpositionen und -größen, Anzahl und
  Reihenfolge der Arbeitsbereiche). Ein Löschvermerk, den die Synchronisierung
  zum Weitergeben der Löschung braucht, ist zulässig, solange er nur die
  Kennung des gelöschten Eintrags trägt und keine dieser Inhalte.
- **FR-012**: Schlägt ein Entfernen fehl, MUSS holzi trotzdem normal
  starten beziehungsweise weiterlaufen. Die Vault MUSS nutzbar bleiben, der
  Fehler MUSS protokolliert werden, und das Entfernen MUSS beim nächsten Öffnen
  erneut versucht werden. Eine nicht entfernte Sitzung DARF bei ausgeschalteter
  Einstellung NICHT wiederhergestellt werden.

**Verhalten der Fensterverwaltung**

- **FR-013**: Ohne geltende Einstellung MUSS jede Vault-Session (Öffnen,
  Entsperren, Neustart nach Absturz) mit genau einem Arbeitsbereich ohne Fenster
  beginnen, es sei denn, der Start öffnet ausdrücklich eine App (FR-014).
- **FR-014**: Öffnet der Start eine App an einem Ort (Deep-Link, frühere
  Vollseiten-Adressen, Spec 020 FR-012), MUSS diese App erscheinen: ohne
  geltende Einstellung im einzigen Arbeitsbereich, mit geltender Einstellung
  zusätzlich zur wiederhergestellten Sitzung wie bisher.
- **FR-015**: holzi MUSS während der Vault-Session alle Fähigkeiten aus
  Spec 015 und 020 behalten, unabhängig von der Einstellung.
- **FR-016**: Die Dokumentation von Spec 015 MUSS bei User Story 5, FR-023 bis
  FR-025 und dem Neustart-Teil von User Story 7 vermerken, dass sie nur bei
  eingeschalteter Wiederherstellung gelten, und auf diese Spec verweisen.

### Key Entities

- **Einstellung „Sitzung wiederherstellen“**: Ein-/Aus-Wert, einmal für die
  Vault und optional je Gerät. Der geltende Wert eines Geräts ergibt sich aus
  FR-002. Die Einstellung selbst wird wie andere Einstellungen synchronisiert.
- **Gespeicherte Sitzung**: siehe Begriffe. Existiert nur, solange die
  Einstellung auf ihrem Gerät gilt, und verlässt dieses Gerät nie (FR-010).
- **Frühere Sitzungsdaten**: Von Versionen mit Spec 015 ungefragt gespeicherte
  und synchronisierte Sitzungen aller Geräte. Werden beim Update einmalig
  entfernt (FR-009, FR-011).

## Success Criteria _(mandatory)_

### Measurable Outcomes

- **SC-001**: Ohne geltende Einstellung zeigt holzi in 100 % der Starts,
  ob nach normalem Beenden, Sperren oder Absturz, genau einen Arbeitsbereich
  ohne Fenster (ohne Deep-Link).
- **SC-002**: Mit geltender Einstellung stellt holzi die Sitzung in 100 %
  der normalen Neustarts so wieder her, wie Spec 015 User Story 5 es
  beschreibt.
- **SC-003**: Nach dem Ausschalten und nach dem ersten Öffnen mit dieser Version
  findet eine Untersuchung der Vault-Datei keine Inhalte einer entfernten
  Sitzung mehr (höchstens Löschvermerke nach FR-011).
- **SC-004**: Eine gespeicherte Sitzung taucht in 0 % der Fälle auf einem
  anderen Gerät auf, auch nicht nach einer Synchronisierung.
- **SC-005**: Ein Nutzer findet die Einstellung und schaltet sie in weniger als
  30 Sekunden ein, ausgehend vom geöffneten Arbeitsbereich.
- **SC-006**: Das Öffnen einer Vault dauert höchstens so lange wie vorher. Die
  einmalige Bereinigung beim Update verlängert es um höchstens eine Sekunde.
- **SC-007**: Alle automatischen Prüfungen und die manuellen Szenarien von Spec
  015 und 020 bestehen, die Neustart-Szenarien aus Spec 015 mit eingeschalteter
  Wiederherstellung.

## Assumptions

- Ein Downgrade auf eine ältere holzi-Version nach dem Update ist nicht
  unterstützt, wie bei anderen Schemaänderungen auch.
- Die Einstellung ist keine Leitplanke im Sinne von Spec 020 (FR-032 dort). Sie
  bekommt den Berechtigungsbereich für Geräteeinstellungen, den Spec 021 für
  externe Agenten freigeben kann.
- Mit eingeschalteter Wiederherstellung gilt, was Spec 015 festlegt: kein
  Wiederherstellen von Tab-Inhalten, keine Vor-/Zurück-Historie (Spec 020),
  keine zuletzt geöffnete Ansicht innerhalb einer App.
- Die Synchronisierung bietet eine Möglichkeit, Daten gerätelokal zu halten, so
  dass sie nicht weitergegeben werden. Wie FR-010 umgesetzt wird, klärt der
  Plan.
- Desktop-Symbole und das Raster auf dem Arbeitsbereich (geplante Folge-Spec)
  sind nicht Teil dieser Spec. Ob sie gespeichert werden, entscheidet die
  Folge-Spec.
- Der PR zu dieser Spec setzt auf dem PR zu Spec 020 auf (gestapelt), weil beide
  den Store der Fensterverwaltung ändern.

## Nicht im Umfang

- Mehrere benannte Sitzungen oder ein Sitzungsverlauf. Gespeichert wird genau
  eine Sitzung je Gerät.
- Ein Wiederherstellungsdialog nach Abstürzen.
- Änderungen an der Speicherung von Tab-Inhalten (Chat-Verlauf, Einstellungen,
  Modelle).
- Die Einordnung der Einstellung in eine Kategorie der künftigen
  Einstellungs-App.
