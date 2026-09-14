# Feature Specification: Chatfenster und Session-Handling

**Feature Branch**: `004-chat-window-handling`
**Created**: 2026-09-13
**Status**: Draft
**Input**: Überarbeitung des Chatfensters und seines Einstiegs auf Basis der bestehenden Chat-, Modell- und Agent-Specs.

## Beziehung zu bestehenden Specs

Diese Spec erweitert die Chat-UX, ersetzt aber weder das Chat-Datenmodell noch
den Agent-Tool-Loop.

- [`002-onboarding-model-prefs`](../002-onboarding-model-prefs/spec.md) bleibt
  zuständig für Onboarding, Modellpräferenzen und die Fallback-Kette. Die dort
  beschriebene Ausführung des Session-Resolvers nur beim Mounten der Chat-Seite
  wird durch diese Spec dahingehend präzisiert, dass der Resolver bereits nach
  erfolgreichem Vault-Open im Hintergrund startet.
- [`003-agent-tool-loop`](../003-agent-tool-loop/spec.md) bleibt zuständig für
  Tool-Aufrufe, Freigaben, Abbruch, Retries und Antwort-Lifecycle.
- Wo diese Spec Aussagen zur bisherigen unveränderten Chat-UX aus Spec 002
  konkretisiert, ist diese Spec für Chat-Einstieg, Composer und Reasoning-
  Darstellung maßgeblich.

## Clarifications

### Session 2026-09-13

- Q: Welche Modelle werden beim Vault-Open vorgeladen? → A: Nur das von der
  Fallback-Kette bestimmte lokale Modell; Anbieter-Modelle werden erst bei
  tatsächlicher Auswahl oder Nutzung aktiviert.
- Q: Wie verhält sich ein laufender Preload bei Vault-/Modellwechsel oder beim
  Sperren? → A: Er wird abgebrochen und sein Ende abgewartet; der Ablauf wartet
  nicht auf den vollständigen Load.
- Q: Wie weit darf die Textarea wachsen? → A: Bis maximal 8 sichtbare Zeilen;
  darüber beginnt ein interner vertikaler Scrollbereich.
- Q: Soll Reasoning als Composer-Einstellung abschaltbar sein? → A: Nein. Wenn
  das gewählte Modell Reasoning unterstützt, bleibt es automatisch aktiviert;
  das Frontend zeigt vorhandenes Reasoning im Accordion standardmäßig
  eingeklappt.
- **Chat-Session** bedeutet eine neue leere Unterhaltung. Bestehende Threads und
  Nachrichten werden nicht gelöscht und bleiben über die Historie erreichbar.
- Das Öffnen des Chats bedeutet den Eintritt in die Chat-Route bzw. den Chat-
  Overlay aus dem Workspace. Das Wechseln zwischen Threads innerhalb eines
  bereits geöffneten Chats startet keine zusätzliche Session.
- Der Modell-Load startet nach dem erfolgreichen Öffnen oder Wechseln einer
  Vault. Das Öffnen der Vault darf nicht auf den Abschluss des Loads warten.
- Beim Vault-Open wird ausschließlich ein von der Fallback-Kette bestimmtes
  lokales Modell vorgeladen. Anbieter-Modelle werden erst bei tatsächlicher
  Auswahl oder Nutzung aktiviert.
- Reasoning wird zunächst nicht in `chat_messages` persistiert. Es ist während
  der aktuellen Chat-Ansicht sichtbar und standardmäßig eingeklappt.

## User Scenarios & Testing

### User Story 1 - Jeder Chat-Einstieg beginnt mit einer neuen Unterhaltung (Priority: P1)

Ein Nutzer öffnet den Chat aus dem Workspace oder ruft die Chat-Route erneut
auf. Er möchte jedes Mal mit einem leeren Eingabefeld und einem neuen
Unterhaltungskontext beginnen, ohne dass die bisherige Historie verloren geht.

**Why this priority**: Ein Chat-Einstieg mit der zuletzt geöffneten Unterhaltung
führt zu versehentlichen Antworten im falschen Kontext und macht den
Arbeitsbereich unvorhersehbar.

**Independent Test**: Eine Vault öffnen, eine Nachricht senden, den Chat
verlassen, ihn erneut öffnen und eine zweite Nachricht senden. Die zweite
Nachricht muss in einer neuen Unterhaltung erscheinen; die erste Unterhaltung
bleibt in der Historie erhalten.

**Acceptance Scenarios**:

1. **Given** eine geöffnete Vault und mindestens ein bestehender Thread,
   **When** der Nutzer den Chat öffnet, **Then** wird eine neue leere Chat-
   Session als aktiver Kontext angezeigt; kein bestehender Thread wird
   automatisch ausgewählt.
2. **Given** der Nutzer verlässt den Chat und öffnet ihn erneut, **When** die
   Chat-Ansicht erscheint, **Then** beginnt sie wieder mit einer neuen leeren
   Session — auch wenn zuvor ein anderer Thread aktiv war.
3. **Given** der Nutzer hat eine neue Session geöffnet, **When** er eine
   Nachricht sendet, **Then** wird die Nachricht in einem neuen persistierten
   Thread gespeichert und nicht an einen vorherigen Thread angehängt.
4. **Given** frühere Threads existieren, **When** der Nutzer einen Thread
   explizit aus der Historie auswählt, **Then** werden dessen Nachrichten
   geladen; dieser bewusste Historienzugriff startet keine neue Session.
5. **Given** der Chat wird geöffnet und anschließend ohne Nachricht verlassen,
   **Then** darf kein nutzer-sichtbarer leerer Historieneintrag zurückbleiben.
   Eine Implementierung darf die neue Session deshalb bis zur ersten Nachricht
   nur als Entwurf führen.

### User Story 2 - Das Modell wird bereits beim Vault-Open vorbereitet (Priority: P1)

Ein Nutzer öffnet eine Vault und möchte beim späteren Aufrufen des Chats nicht
erst mehrere Sekunden auf den lokalen Modell-Load warten. Das von der
Fallback-Kette bestimmte lokale Modell wird deshalb unmittelbar nach dem
Vault-Open im Hintergrund vorbereitet. Anbieter-Modelle werden dabei nicht
proaktiv verbunden.

**Why this priority**: Der Modell-Load ist der längste Teil des Chat-Einstiegs
und soll nicht an die Navigation oder an die erste Nachricht gekoppelt sein.

**Independent Test**: Eine Vault mit einem installierten lokalen Modell öffnen,
im Workspace bleiben und anschließend den Chat öffnen. Der Chat muss denselben
bereits laufenden oder abgeschlossenen Load verwenden und darf keinen zweiten
Load starten.

**Acceptance Scenarios**:

1. **Given** eine Vault wird erfolgreich geöffnet und die Fallback-Kette aus
   Spec 002 liefert ein ladbares lokales Modell, **When** das Öffnen
   abgeschlossen ist, **Then** startet dessen Load im Hintergrund, ohne dass
   der Nutzer den Chat öffnen muss.
2. **Given** der Hintergrund-Load läuft noch, **When** der Nutzer den Chat
   öffnet, **Then** übernimmt die Chat-Ansicht den bestehenden Ladezustand und
   startet keinen parallelen zweiten Load desselben Modells.
3. **Given** der Hintergrund-Load ist abgeschlossen, **When** der Nutzer den
   Chat öffnet, **Then** ist das vorbereitete Modell direkt als aktives Modell
   verfügbar.
4. **Given** der Hintergrund-Load läuft, **When** der Nutzer den Workspace
   betrachtet, **Then** bleibt der Workspace bedienbar und zeigt höchstens
   einen unaufdringlichen, verständlichen Lade- oder Bereitschaftszustand.
5. **Given** der Hintergrund-Load schlägt fehl, **When** der Nutzer den Chat
   öffnet, **Then** bleibt die Vault geöffnet, der Fehler wird verständlich
   angezeigt und der Nutzer kann ein anderes verfügbares Modell auswählen.
6. **Given** es gibt kein Modell, das die Fallback-Kette laden kann, **When**
   die Vault geöffnet wird, **Then** wird kein fehlerhafter Hintergrund-Load
   gestartet; die bestehende Modell-Auswahl bzw. der Katalog bleibt erreichbar.
7. **Given** die Fallback-Kette liefert beim Vault-Open ausschließlich ein
   Anbieter-Modell, **When** das Öffnen abgeschlossen ist, **Then** wird kein
   Anbieter-Modell proaktiv verbunden; es bleibt bis zur tatsächlichen
   Auswahl/Nutzung unbelastet.
8. **Given** eine zweite Vault wird geöffnet, **When** der aktive Vault-Wechsel
   abgeschlossen ist, **Then** wird das Modell der neuen Vault geladen und ein
   Modell der vorherigen Vault nicht weiter als aktive Chat-Session verwendet.

### User Story 3 - Kompakte Chat-Konfiguration direkt im Composer (Priority: P1)

Ein Nutzer möchte Modell, Effort und Freigabe schnell ändern, ohne
unterhalb des Eingabefelds eine zweite Reihe großer Konfigurationskarten lesen
zu müssen. Modell und Effort werden deshalb über einen gemeinsamen, kompakten
Settings-Button in einem Popover gebündelt. Die Freigabe bleibt als eigenes
Dropdown daneben bestehen; alle Controls liegen in einer Reihe unterhalb der
Textarea, ähnlich dem Bedienmuster aus Codex.

**Why this priority**: Die Konfiguration gehört funktional zur nächsten
Nachricht und soll visuell nicht mit dem eigentlichen Chat konkurrieren.

**Independent Test**: Den Composer auf Desktop und in schmaler Fensterbreite
öffnen. Der gemeinsame Modell-/Effort-Button muss ein Popover öffnen, die
Freigabe muss als separates Dropdown erreichbar sein, und alle Controls müssen
verständlich beschriftet in einer Reihe unterhalb der Textarea angeordnet sein.

**Acceptance Scenarios**:

1. **Given** der Composer ist sichtbar, **Then** befinden sich alle Controls
   in einer durchgehenden Reihe direkt unterhalb der Textarea und innerhalb
   derselben Composer-Oberfläche.
2. **Given** der Settings-Button ist geschlossen, **Then** zeigt er kompakt
   das aktuelle Modell und den aktuellen Effort-Wert an.
3. **Given** der Nutzer öffnet den Settings-Button, **Then** öffnet sich ein
   Popover mit Modellwahl und Effort-Auswahl; beide Werte können dort geändert
   werden, ohne zwei separate Controls in der Composer-Reihe zu rendern. Die
   Effort-Auswahl wird als deutlich greifbarer Slider dargestellt.
4. **Given** der Nutzer öffnet das Freigabe-Control, **Then** bleibt dieses
   ein eigenständiges Dropdown mit den Modi Plan, Manuell und Automatisch in
   genau dieser Reihenfolge.
5. **Given** der Nutzer öffnet ein Control, **When** er eine Option auswählt,
   **Then** wird der Wert unmittelbar aktualisiert und das Control wieder
   kompakt dargestellt.
6. **Given** der Nutzer schreibt oder sendet eine Nachricht, **Then** bleiben
   die Controls zugänglich, werden aber während eines laufenden, nicht
   unterbrechbaren Vorgangs entsprechend deaktiviert.
7. **Given** ein schmales Fenster oder ein mobiles Layout, **Then** bleibt die
   Control-Reihe horizontal bedienbar; das Settings-Popover passt sich der
   verfügbaren Breite an, öffnet sichtbar oberhalb der Reihe und darf nicht von
   einem scrollenden Composer-Container abgeschnitten werden.
8. **Given** ein Control wird nur geöffnet und ohne Auswahl geschlossen,
   **Then** bleibt der bisherige Wert unverändert.
9. **Then** müssen alle Controls Tastatur- und Screenreader-bedienbar sein und
   einen zugänglichen Namen sowie ihren aktuellen Wert vermitteln. Auch
   deaktivierte Buttons müssen lesbaren Text mit ausreichendem Kontrast zeigen.

### User Story 4 - Das Eingabefeld wächst mit mehrzeiligem Text (Priority: P1)

Ein Nutzer schreibt längere oder strukturierte Prompts. Das Eingabefeld soll
mit den eingegebenen Zeilen wachsen, damit der Text sichtbar bleibt, ohne dass
der Nutzer sofort in einem kleinen festen Feld scrollen muss.

**Independent Test**: Einen einzeiligen, mehrzeiligen und sehr langen Prompt
eingeben. Die ersten Zeilen müssen das Feld vergrößern; bei Überschreiten der
Maximalhöhe muss nur innerhalb des Felds gescrollt werden.

**Acceptance Scenarios**:

1. **Given** das Eingabefeld enthält eine Zeile, **When** der Nutzer weitere
   Zeilen über `Shift+Enter` einfügt, **Then** wächst die Textarea automatisch
   bis auf maximal 8 sichtbare Zeilen.
2. **Given** die Maximalhöhe ist erreicht, **When** weitere Zeilen eingegeben
   werden, **Then** bleibt die Composer-Höhe stabil und die Textarea erhält
   einen internen vertikalen Scrollbereich.
3. **Given** der Nutzer sendet den Prompt, **Then** wird der vollständige
   Inhalt unverändert übernommen und die Textarea auf ihre minimale Höhe
   zurückgesetzt.
4. **Given** der Nutzer drückt `Enter` ohne Shift, **Then** wird die Nachricht
   gesendet. `Shift+Enter` erzeugt ausschließlich einen Zeilenumbruch.
5. **Given** die Textarea ist leer, **Then** bleibt sie auf der minimalen Höhe
   und der Senden-Button ist deaktiviert.
6. **Then** darf das automatische Wachstum weder den Nachrichtenbereich noch
   den gesamten Viewport unkontrolliert aus dem sichtbaren Bereich drücken.

### User Story 5 - Reasoning ist nachvollziehbar, aber standardmäßig verborgen (Priority: P1)

Ein Nutzer möchte nachvollziehen können, ob und welches Reasoning das Modell zu
seiner Antwort geliefert hat. Gleichzeitig soll dieses Detail die normale
Antwort nicht dominieren. Reasoning wird deshalb pro Assistant-Nachricht in
einem Accordion angeboten, das standardmäßig eingeklappt ist.

**Why this priority**: Reasoning ist für die Diagnose und das Vertrauen in eine
Antwort nützlich, soll aber den Lesefluss der eigentlichen Antwort nicht
stören.

**Independent Test**: Eine Antwort mit Reasoning erzeugen, das Accordion öffnen
und wieder schließen, anschließend eine zweite Antwort erzeugen. Beide
Reasoning-Bereiche müssen unabhängig voneinander und standardmäßig geschlossen
sein.

**Acceptance Scenarios**:

1. **Given** eine Assistant-Nachricht enthält nicht-leeres Reasoning, **Then**
   wird unter oder bei der Nachricht ein dezentes Accordion mit einer klaren
   Beschriftung angezeigt; sein Inhalt ist zunächst verborgen.
2. **Given** das Reasoning-Accordion ist geschlossen, **When** der Nutzer es
   aktiviert, **Then** wird der vollständige bisher empfangene Reasoning-Text
   angezeigt.
3. **Given** Reasoning wird noch gestreamt und das Accordion ist geöffnet,
   **When** weitere Reasoning-Deltas eintreffen, **Then** wächst der sichtbare
   Inhalt mit der Antwort mit und die Position des Nachrichtenbereichs bleibt
   benutzbar.
4. **Given** mehrere Assistant-Nachrichten besitzen Reasoning, **When** der
   Nutzer nur eines der Accordions öffnet, **Then** bleiben die anderen
   Accordions geschlossen.
5. **Given** eine Nachricht enthält kein Reasoning, **Then** wird kein leeres
   Accordion angezeigt.
6. **Given** der Nutzer öffnet einen neuen Chat-Einstieg oder lädt die Ansicht
   neu, **Then** werden Reasoning-Accordions wieder standardmäßig eingeklappt.
7. **Then** muss das Accordion per Tastatur und Screenreader bedienbar sein und
   seinen geöffneten Zustand über `aria-expanded` kommunizieren.
8. **Then** bleibt die normale Assistant-Antwort auch bei geschlossenem
   Accordion vollständig sichtbar.

## Edge Cases

- Öffnet der Nutzer den Chat mehrfach sehr schnell, darf daraus nur ein aktiver
  neuer Session-Entwurf entstehen; parallele leere Threads sind zu vermeiden.
- Öffnet der Nutzer den Chat während eines Vault-Wechsels, muss die Navigation
  entweder warten, bis der neue aktive Vault veröffentlicht ist, oder klar
  blockiert werden. Nachrichten dürfen niemals an die vorherige Vault gesendet
  werden.
- Wird während des Hintergrund-Loads ein anderes Modell ausgewählt, darf der
  alte Load das neu ausgewählte Modell nicht wieder als aktiv veröffentlichen.
- Wird der Workspace geschlossen, die Vault gesperrt oder das Modell gewechselt,
  wird ein laufender Preload abgebrochen und sein Ende abgewartet. Der Ablauf
  darf dadurch nur kurz auf die Cancellation reagieren, nicht auf den
  vollständigen Modell-Load warten.
- Liefert ein Modell Reasoning-Deltas ohne abschließende normale Antwort, muss
  der Reasoning-Text trotzdem sicher angezeigt werden können; das bestehende
  Antwort- und Fehlerhandling aus Spec 003 bleibt maßgeblich.
- Ein Reasoning-Accordion darf bei sehr langem Inhalt nicht den gesamten Chat-
  Viewport übernehmen; der Inhalt braucht eine sinnvolle Begrenzung oder einen
  internen Scrollbereich.
- Die Controls dürfen bei langen Modellnamen, übersetzten Labels oder kleinen
  Viewports nicht wichtige Teile des Senders oder des Eingabetextes verdecken.
- Während ein Tool-Approval aus Spec 003 offen ist, bleiben Composer und
  Freigabe-Control konsistent zum laufenden Turn; diese Spec ändert nicht die
  Freigabeentscheidung.

## Requirements

### Functional Requirements

#### Chat-Session und Historie

- **FR-001**: Jeder neue Chat-Einstieg MUSS mit einer neuen leeren Chat-Session
  beginnen.
- **FR-002**: Der neue Chat-Einstieg DARF keinen bestehenden Thread automatisch
  als aktiven Kontext auswählen.
- **FR-003**: Eine neue Session MUSS beim ersten Senden einen neuen persistierten
  Thread erzeugen; sie DARF bis dahin als nicht persistierter Entwurf geführt
  werden.
- **FR-004**: Bestehende Threads und Nachrichten DÜRFEN durch den neuen
  Chat-Einstieg nicht gelöscht, verändert oder automatisch fortgesetzt werden.
- **FR-005**: Eine explizite Auswahl eines bestehenden Threads aus der Historie
  MUSS weiterhin möglich sein und dessen Nachrichten laden.

#### Modell-Load beim Vault-Open

- **FR-006**: Nach jedem erfolgreichen Öffnen oder Wechseln einer Vault MUSS
  der Session-Resolver aus Spec 002 im Hintergrund gestartet werden, ohne dass
  der Nutzer zuerst den Chat öffnen muss. Nur ein lokaler Kandidat darf daraus
  einen Modell-Preload auslösen.
- **FR-007**: Der Resolver MUSS weiterhin die in Spec 002 definierte
  Fallback-Reihenfolge und die dort definierte Persistenzsemantik verwenden.
- **FR-008**: Ein lokales, von der Fallback-Kette ausgewähltes Modell MUSS
  bereits während des Vault-Open-Lifecycles geladen werden.
- **FR-009**: Der Vault-Open-Vorgang DARF NICHT auf den Abschluss des
  Hintergrund-Loads warten.
- **FR-010**: Die Chat-Ansicht MUSS einen bereits laufenden oder abgeschlossenen
  Hintergrund-Load übernehmen und DARF keinen parallelen zweiten Load für
  dasselbe Modell starten.
- **FR-011**: Der Hintergrund-Load MUSS einen strukturierten Status mit Modell,
  Phase und Fehlerzustand bereitstellen. Die bestehenden Ladephasen aus Spec
  002 (`connecting`, `loading`, `cuda-jit-warmup`, `ready`) bleiben gültig.
- **FR-012**: Ein fehlgeschlagener Hintergrund-Load MUSS die Vault geöffnet
  lassen und eine erneute Modellwahl oder einen erneuten Load ermöglichen.
- **FR-013**: Bei einem Vault-Wechsel, manuellen Modellwechsel oder dem
  Sperren der Vault MUSS ein laufender Preload abgebrochen und sein Ende
  abgewartet werden; der Ablauf DARF NICHT auf den vollständigen Load warten.
- **FR-014**: Ein Load, der zu einer nicht mehr aktiven Vault oder zu einem
  veralteten Modellwechsel gehört, DARF keinen aktiven Chat-Status mehr
  veröffentlichen.
- **FR-015**: Wenn kein ladbares Modell gefunden wird, MUSS der Chat ohne
  Modell-Load-Fehler geöffnet werden können und die bestehende Modellwahl
  anzeigen.

#### Composer und Konfiguration

- **FR-016**: Der Composer MUSS Eingabefeld, Konfigurations-Controls und
  Senden-/Abbrechen-Aktion in einer zusammengehörigen Oberfläche darstellen.
- **FR-017**: Modell und Effort MÜSSEN über genau einen gemeinsamen,
  unaufdringlichen Settings-Button innerhalb des Composer-Containers erreichbar
  sein.
- **FR-017a**: Der Settings-Button MUSS ein Popover mit Modellwahl und
  Effort-Auswahl öffnen; Modell und Effort DÜRFEN nicht als zwei separate
  Controls in der Composer-Reihe erscheinen.
- **FR-017b**: Der geschlossene Settings-Button MUSS den Modellnamen auf
  maximal 20 Zeichen inklusive Ellipsis begrenzen und zusätzlich eine
  responsive visuelle Maximalbreite verwenden; der vollständige Name MUSS über
  den zugänglichen Namen und/oder einen Tooltip erreichbar sein.
- **FR-018**: Die Freigabe MUSS als eigenes Dropdown mit den Modi Plan, Manuell
  und Automatisch neben dem Settings-Button erreichbar sein. Die Optionen
  MÜSSEN in genau dieser Reihenfolge erscheinen.
- **FR-018a**: Die Controls MÜSSEN ihren aktuellen Wert kompakt anzeigen und
  Details bzw. Auswahloptionen erst nach Interaktion öffnen.
- **FR-018b**: Modell- und Freigabeauswahl MÜSSEN die vorhandenen Shadcn-
  Select-Komponenten verwenden und dürfen keine nativen Browser-Selectboxen
  rendern. Die Effort-Auswahl MUSS die vorhandene Shadcn-Slider-Komponente mit
  einem sichtbar verstärkten Track und Thumb verwenden.
- **FR-019**: Settings-Button, Freigabe-Dropdown sowie Senden-/Abbrechen-Aktion
  MÜSSEN in einer Reihe direkt unterhalb der Textarea angeordnet sein. Auf
  schmalen Viewports MUSS diese Reihe bedienbar bleiben; das Popover MUSS sich
  an die verfügbare Breite anpassen, sichtbar oberhalb der Reihe öffnen und darf
  nicht durch einen scrollenden Composer-Container abgeschnitten werden.
- **FR-020**: Alle Controls MÜSSEN einen zugänglichen Namen, Tastaturbedienung
  und eine Zustandsansage für Screenreader anbieten. Buttons dürfen auch im
  deaktivierten Zustand nicht durch globale Transparenzregeln unleserlich
  werden.
- **FR-021**: Die bestehenden fachlichen Semantiken für Modellwahl, Effort,
  Freigabe, Abbruch und Tool-Loop DÜRFEN durch die neue Anordnung nicht
  verändert werden. Unterstützt das gewählte Modell Reasoning, wird es ohne
  separates Composer-Setting automatisch aktiviert.
- **FR-021a**: Ob ein Modell Reasoning unterstützt, MUSS als serverseitige
  Modell-Capability bekannt sein und über `ChatRequest` an den jeweiligen
  Adapter weitergereicht werden, damit Adapter, die Reasoning nur nach
  explizitem Request-Flag liefern (z. B. Anthropic `thinking`), es tatsächlich
  aktivieren. Ist die Capability für das gewählte Modell unbekannt oder nicht
  vorhanden, DARF kein Reasoning angefordert werden; FR-031 bleibt in diesem
  Fall gültig.

#### Wachsende Textarea

- **FR-022**: Das Eingabefeld MUSS eine mehrzeilige Textarea sein, deren Höhe
  sich automatisch an die eingegebenen Zeilen anpasst.
- **FR-023**: Die Textarea MUSS eine minimale Höhe und eine Maximalhöhe von
  8 sichtbaren Zeilen besitzen; nach Erreichen der Maximalhöhe MUSS sie intern
  vertikal scrollen.
- **FR-024**: `Shift+Enter` MUSS einen Zeilenumbruch einfügen. `Enter` ohne
  Shift MUSS die Nachricht senden.
- **FR-025**: Nach erfolgreichem Senden MUSS die Textarea geleert werden und auf
  ihre minimale Höhe zurückkehren.
- **FR-026**: Die Textarea DARF den Nachrichtenbereich oder den Viewport nicht
  unkontrolliert aus dem sichtbaren Bereich schieben.

#### Reasoning-Accordion

- **FR-027**: Für jede Assistant-Nachricht mit nicht-leerem Reasoning MUSS das
  Frontend einen einklappbaren Reasoning-Bereich anbieten.
- **FR-028**: Jeder Reasoning-Bereich MUSS standardmäßig eingeklappt sein,
  unabhängig davon, ob die Nachricht gestreamt, abgeschlossen oder aus der
  aktuellen Ansicht wiederhergestellt wurde.
- **FR-029**: Der Nutzer MUSS jeden Reasoning-Bereich unabhängig von allen
  anderen öffnen und schließen können.
- **FR-030**: Bei geschlossenem Reasoning-Bereich MUSS die vollständige normale
  Assistant-Antwort sichtbar bleiben.
- **FR-031**: Wenn kein Reasoning vorhanden ist, DARF kein leerer Accordion-
  Container gerendert werden. Eine separate Einstellung zum Abschalten der
  Reasoning-Anzeige ist nicht Bestandteil des Composers.
- **FR-032**: Während des Streamings MUSS ein geöffnetes Accordion eintreffende
  Reasoning-Deltas anzeigen können, ohne den Inhalt abzuschneiden.
- **FR-033**: Der geöffnete/geschlossene Zustand MUSS über native Disclosure-
  Semantik oder ein gleichwertiges `aria-expanded`-Muster zugänglich sein.
- **FR-034**: Reasoning-Zustände MÜSSEN beim neuen Chat-Einstieg und beim
  Neuladen standardmäßig geschlossen starten.
- **FR-035**: Reasoning MUSS in dieser Spec nicht dauerhaft in `chat_messages`
  gespeichert werden. Eine spätere Persistenz ist ein separates Feature.

#### Internationalisierung und Nicht-Regression

- **FR-036**: Alle neuen nutzersichtbaren Texte für Composer-Controls,
  Ladezustände, Fehler und Reasoning-Accordion MÜSSEN über `@nuxtjs/i18n` in
  Deutsch und Englisch gepflegt werden.
- **FR-037**: Backend-Events MÜSSEN strukturierte Zustände und Parameter statt
  lokalisierter UI-Texte liefern.
- **FR-038**: Die Anforderungen aus Spec 003 für Streaming, Tool-Nutzung,
  Freigaben, Abbruch, Retries und Turn-Abschluss MÜSSEN unverändert gelten.
- **FR-039**: Eine neue Chat-Session MUSS die bestehende Nachrichten-Persistenz
  und die Idempotenz von `send_message` weiterhin einhalten.

## Key Entities

- **Chat-Session-Entwurf**: Der zunächst leere, aktive Kontext eines neuen
  Chat-Einstiegs. Er wird beim ersten Senden zu einem persistierten Thread oder
  verwirft sich beim Verlassen ohne Nachricht.
- **Thread**: Eine persistierte Unterhaltung mit eigener ID und ihren
  Nachrichten. Bestehende Threads bleiben über die Historie erreichbar.
- **Composer-Settings-Popover**: Ein gemeinsamer kompakter Settings-Button
  für Modell und Effort. Er öffnet ein Popover mit beiden Einstellungen.
- **Composer-Control**: Ein kompaktes Bedienelement für die separate Freigabe
  innerhalb des Composer-Containers.
- **Model-Load-Status**: Der global zum aktiven Vault gehörende strukturierte
  Zustand eines Hintergrund-Loads inklusive Modell-ID, Modellname, Phase und
  optionalem Fehler.
- **Reasoning-Accordion**: Ein pro Assistant-Nachricht lokaler Disclosure-
  Bereich für nicht-leere Reasoning-Deltas. Sein Zustand wird in dieser Spec
  nicht persistiert.
- **Reasoning-Capability**: Eine serverseitig aus dem gewählten Modell
  abgeleitete Fähigkeit, die bestimmt, ob `ChatRequest` beim Adapter Reasoning
  anfordert. Sie ist kein Composer-State und wird nicht vom Frontend gesetzt.

## Success Criteria

### Measurable Outcomes

- **SC-001**: In 100 % der getesteten neuen Chat-Einstiege wird kein
  bestehender Thread automatisch als aktiver Sendekontext verwendet.
- **SC-002**: Ein lokales Modell beginnt nach erfolgreichem Vault-Open mit dem
  Laden, ohne dass der Nutzer den Chat öffnen muss.
- **SC-003**: Beim Öffnen des Chats während eines laufenden Hintergrund-Loads
  entsteht kein zweiter Load desselben Modells.
- **SC-004**: Der Vault-Open bleibt während des Modell-Loads bedienbar und
  blockiert nicht auf dessen Abschluss.
- **SC-005**: Ein Nutzer kann Modell und Effort über genau einen gemeinsamen
  Settings-Button sowie die Freigabe über ein separates Dropdown aus einer
  Reihe direkt unterhalb der Textarea erreichen.
- **SC-006**: Ein mehrzeiliger Prompt ist bis zu 8 sichtbaren Zeilen ohne
  internes Scrollen sichtbar; längere Prompts bleiben danach vollständig
  erreichbar.
- **SC-007**: 100 % der getesteten Assistant-Nachrichten mit Reasoning zeigen
  einen geschlossenen Reasoning-Bereich beim ersten Rendern.
- **SC-008**: Das Öffnen eines Reasoning-Bereichs verändert weder Sichtbarkeit
  noch Inhalt der normalen Assistant-Antwort.
- **SC-009**: Alle neuen Controls und Accordions sind per Tastatur bedienbar und
  haben in `de` und `en` vollständige Beschriftungen.

## Assumptions

- „Neue Session“ bedeutet ein neuer Gesprächskontext, nicht das Löschen alter
  Chat-Daten und nicht zwingend einen Neustart des bereits geladenen
  Modellprozesses.
- Ein Hintergrund-Load darf den Workspace anzeigen, während der Chat bis zum
  Zustand `ready` entsprechend deaktiviert bleibt. Die genaue Darstellung des
  Workspace-Hinweises ist eine UI-Entscheidung innerhalb dieser Spec.
- Die bestehende Fallback-Kette aus Spec 002 bleibt die einzige autoritative
  Auswahlentscheidung für das Startmodell.
- Der Hintergrund-Preload gilt ausschließlich für lokale Modelle. Ein
  Anbieter-Modell wird erst bei tatsächlicher Auswahl oder Nutzung aktiviert.
- Unterstützt das gewählte Modell Reasoning, wird Reasoning automatisch
  angefordert bzw. verwendet. Es gibt dafür keinen separaten Composer-
  Schalter; liefert das Modell kein Reasoning, wird kein leeres Accordion
  angezeigt.
- Die minimale Textarea-Höhe und die exakten Pixelmaße der Controls werden im
  Plan festgelegt; die maximale Textarea-Höhe ist auf 8 sichtbare Zeilen
  begrenzt.
- Modell und Effort teilen sich im Composer genau einen Settings-Button mit
  Popover. Die Freigabe bleibt ein separates Dropdown; die Controls werden
  nicht auf mehrere Reihen verteilt.

## Explicit Non-Goals

- Persistenz von Reasoning-Text oder Accordion-Zuständen in `chat_messages`.
- Änderung der Provider-Konfiguration oder des Modellkatalogs.
- Änderung der Tool-Aufruf-, Freigabe-, Abbruch- oder Retry-Semantik aus Spec
  003.
- Vollständige Überarbeitung des Workspace-Dashboards.
- Eine neue allgemeine Settings-Seite für Modell-, Reasoning- oder Effort-
  Optionen außerhalb des Chat-Composers.
- Löschen, Zusammenführen oder automatische Umbenennung bestehender Threads.
