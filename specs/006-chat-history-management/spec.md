# Feature Specification: Chat-Historie verwalten

**Feature Branch**: `006-chat-history-management`
**Created**: 2026-09-15
**Status**: Draft
**Input**: Chat-Historieneinträge sollen am rechten Rand die vergangene Dauer
seit ihrer Eröffnung anzeigen sowie beim Hover ein Bearbeiten- und ein
Löschen-Control anbieten.

## Beziehung zu bestehenden Specs

Diese Spec erweitert die Historien-UX aus Spec 004. Sie ändert nicht den
Chat-Einstieg, das Nachrichtenformat oder den Agent-Tool-Loop.

- [`004-chat-window-handling`](../004-chat-window-handling/spec.md) bleibt
  maßgeblich für neue Chat-Entwürfe, die Auswahl bestehender Threads und die
  Nachrichtenanzeige.
- Ein Historieneintrag entspricht einem persistierten Thread mit seinen
  Nachrichten. Das Umbenennen verändert nur den Titel; das Löschen entfernt
  den gesamten Eintrag und seine zugehörigen Nachrichten.
- Die bestehende Vault- und Synchronisationssemantik für per-Vault-Daten bleibt
  erhalten.

## Clarifications

### Session 2026-09-15

- Q: Was soll beim Löschen eines Threads mit laufendem Turn oder offenem
  Tool-Approval passieren? → A: Vor dem Löschen wird der laufende Turn
  abgebrochen und sein Abschluss abgewartet. Erst danach wird der Thread
  gelöscht; bei fehlgeschlagenem oder nicht abgeschlossenem Abbruch bleibt er
  erhalten.

## User Scenarios & Testing

### User Story 1 - Eröffnungszeit eines Verlaufs erkennen (Priority: P1)

Ein Nutzer möchte im Verlauf sofort erkennen, wann eine Unterhaltung begonnen
hat, ohne den Eintrag erst öffnen zu müssen. Jeder Eintrag zeigt deshalb seine
Eröffnungszeit kompakt am rechten Rand.

**Why this priority**: Die zeitliche Einordnung hilft, ähnliche oder alte
Unterhaltungen schnell wiederzufinden und gibt dem Verlauf mehr Orientierung.

**Independent Test**: Mehrere Threads mit unterschiedlichen
Eröffnungszeitpunkten anzeigen, darunter einen gerade eröffneten und einen
älteren Thread. Jeder Eintrag muss rechts eine kompakte Dauer wie `1min`, `2h`
oder `5d` zeigen.

**Acceptance Scenarios**:

1. **Given** mehrere persistierte Threads sind im Verlauf sichtbar, **When**
   der Verlauf gerendert wird, **Then** zeigt jeder Eintrag am rechten Rand
   die vergangene Dauer seit seiner Eröffnung.
2. **Given** ein Thread wurde vor wenigen Minuten, Stunden oder Tagen
   eröffnet, **When** der Nutzer den Verlauf betrachtet, **Then** wird die
   Dauer im kompakten Format `<Ganzzahl><Einheit>` angezeigt, zum Beispiel
   `1min`, `2h` oder `5d`.
3. **Given** der Nutzer fokussiert oder verweilt über der Dauerangabe, **Then**
   ist zusätzlich der vollständige lokale Eröffnungszeitpunkt zugänglich.
4. **Given** ein Thread wird umbenannt oder erhält neue Nachrichten, **When**
   der Verlauf aktualisiert wird, **Then** bleibt seine angezeigte
   Eröffnungszeit unverändert.
5. **Given** der Verlauf enthält lange Titel oder ein schmales Fenster, **Then**
   bleibt die Dauerangabe am rechten Rand sichtbar und wird nicht vom Titel
   überdeckt.

### User Story 2 - Verlaufseintrag umbenennen (Priority: P1)

Ein Nutzer möchte einen automatisch oder unklar benannten Verlauf später
verständlich benennen können. Beim Hover oder bei Tastaturfokus erscheint dafür
ein Pencil-/Bearbeiten-Control direkt am Eintrag.

**Why this priority**: Aussagekräftige Titel machen den Verlauf dauerhaft
durchsuch- und unterscheidbar, ohne den Gesprächsinhalt zu verändern.

**Independent Test**: Einen bestehenden Verlauf fokussieren, die
Bearbeiten-Aktion öffnen, einen neuen Titel eingeben, speichern und den Verlauf
neu laden. Der neue Titel muss erhalten bleiben, während die Nachrichten und
die Eröffnungszeit unverändert bleiben.

**Acceptance Scenarios**:

1. **Given** ein Verlaufseintrag ist nicht aktiv, **When** der Nutzer mit dem
   Mauszeiger darüber fährt oder ihn per Tastatur fokussiert, **Then** wird ein
   eindeutig als Bearbeiten/Pencil erkennbares Control angeboten.
2. **Given** der Nutzer aktiviert Bearbeiten, **Then** wird der aktuelle Titel
   in einem fokussierten Eingabefeld editierbar, ohne den Eintrag zu öffnen.
3. **Given** ein gültiger neuer Titel wurde eingegeben, **When** der Nutzer
   speichert, **Then** wird der neue Titel im Verlauf und nach einem Neuladen
   angezeigt.
4. **Given** der Nutzer bearbeitet einen Titel, **When** er `Enter` drückt
   oder die Eingabe bestätigt, **Then** wird der Titel gespeichert und der
   Eintrag verlässt den Editiermodus.
5. **Given** der Nutzer bearbeitet einen Titel, **When** er `Escape` drückt,
   **Then** wird die Änderung verworfen und der vorherige Titel bleibt
   erhalten.
6. **Given** die Eingabe ist leer oder besteht nur aus Leerzeichen, **When**
   der Nutzer speichert, **Then** wird die Speicherung abgelehnt, ein
   verständlicher Fehler angezeigt und der bisherige Titel nicht ersetzt.
7. **Given** das Umbenennen schlägt fehl, **Then** bleibt der bisherige Titel
   sichtbar und der Nutzer kann die Aktion erneut versuchen.
8. **Given** der Titel wird geändert, **Then** bleiben Nachrichten,
   Eröffnungszeit und aktiver Gesprächskontext unverändert.

### User Story 3 - Verlaufseintrag löschen (Priority: P1)

Ein Nutzer möchte nicht mehr benötigte Unterhaltungen aus dem Verlauf
entfernen. Beim Hover oder bei Tastaturfokus erscheint dafür ein Delete-
Control.

**Why this priority**: Ein kontrollierbarer Verlauf verhindert, dass alte oder
versehentlich angelegte Gespräche die Übersicht dauerhaft belasten.

**Independent Test**: Einen nicht benötigten Thread löschen, die Bestätigung
ausführen und den Verlauf sowie die Unterhaltung nach einem Neuladen prüfen.
Der Eintrag und seine Nachrichten dürfen nicht mehr erscheinen.

**Acceptance Scenarios**:

1. **Given** ein Verlaufseintrag ist nicht aktiv, **When** der Nutzer mit dem
   Mauszeiger darüber fährt oder ihn per Tastatur fokussiert, **Then** wird ein
   eindeutig als Löschen erkennbares Control angeboten.
2. **Given** der Nutzer aktiviert Löschen, **Then** wird vor der endgültigen
   Aktion eine Bestätigung mit ausreichendem Bezug auf den betroffenen Titel
   angezeigt.
3. **Given** der Nutzer bricht die Bestätigung ab, **Then** bleiben Eintrag,
   Titel, Nachrichten und Eröffnungszeit unverändert.
4. **Given** der Nutzer bestätigt die Löschung, **Then** verschwinden der
   Verlaufseintrag und alle zugehörigen Nachrichten aus der Ansicht und aus
   der persistierten Historie.
5. **Given** der aktive Thread wird gelöscht, **Then** wird der aktive
   Gesprächskontext geleert und eine neue leere Session angezeigt; ein anderer
   Thread wird nicht automatisch ausgewählt.
6. **Given** der aktive Thread hat einen laufenden Turn oder offenen
   Tool-Approval, **When** der Nutzer die Löschung bestätigt, **Then** wird die
   Chat-Ausführung zuerst abgebrochen und ihr terminaler Zustand abgewartet;
   erst danach wird der Thread gelöscht.
7. **Given** der Abbruch eines laufenden Turns schlägt fehl oder erreicht keinen
   terminalen Zustand, **Then** wird der Thread nicht gelöscht und der Nutzer
   erhält eine verständliche Fehlermeldung.
8. **Given** die Löschung schlägt fehl, **Then** bleibt der Verlaufseintrag
   sichtbar, die Nachrichten bleiben erreichbar und der Nutzer erhält eine
   verständliche Fehlermeldung.
9. **Given** ein Thread wird auf einem Gerät umbenannt oder gelöscht, **When**
   die bestehende Vault-Synchronisation den Vorgang auf ein anderes Gerät
   überträgt, **Then** wird dieselbe Änderung dort ebenfalls wirksam.

## Edge Cases

- Ein Verlaufseintrag ohne für die Anzeige verwendbaren Eröffnungszeitpunkt
  darf den Verlauf nicht unbrauchbar machen; die UI zeigt eine verständliche
  Ersatzangabe und meldet keinen technischen Rohfehler.
- Bei einer lokalen Uhrzeitabweichung darf die Dauer nicht negativ oder
  irreführend erscheinen; ein zukünftiger Eröffnungszeitpunkt wird als `0min`
  behandelt.
- Der Titel darf weder durch sehr lange Eingaben das Zeitfeld verdrängen noch
  durch führende oder nachgestellte Leerzeichen ungewollt leer wirken.
- Bei einem bestätigten Löschen während eines laufenden Turns oder offenen
  Tool-Approvals wird zuerst abgebrochen und der terminale Zustand abgewartet.
  Erst ein erfolgreicher Abbruch erlaubt die Löschung; bei einem Fehler bleibt
  der Thread erhalten.
- Wenn zwei Geräte denselben Thread nahezu gleichzeitig ändern, gelten die
  vorhandenen Vault-/CRDT-Konfliktregeln. Diese Spec führt keine zusätzliche
  Konfliktauflösung ein.
- Hover ist auf Touch-Geräten nicht verfügbar. Die Aktionen müssen deshalb
  über Tastaturfokus und die vorhandene fokussierbare Eintragsinteraktion
  ebenfalls erreichbar bleiben.

## Requirements

### Functional Requirements

- **FR-001**: Der Verlauf MUSS jeden persistierten Chat-Thread als eigenen
  Eintrag anzeigen.
- **FR-002**: Jeder Verlaufseintrag MUSS dauerhaft, also nicht nur beim Hover,
  eine Dauerangabe am rechten Rand anzeigen.
- **FR-003**: Die Dauerangabe MUSS als vergangene Dauer aus dem unveränderlichen
  Eröffnungszeitpunkt des Threads berechnet werden und DARF nicht aus dem
  Zeitpunkt der letzten Nachricht oder der letzten Titeländerung stammen.
- **FR-004**: Die sichtbare Dauer MUSS ausschließlich als ganze Zahl mit einer
  kompakten Einheit dargestellt werden: `min` für weniger als eine Stunde, `h`
  für weniger als einen Tag und `d` ab einem Tag. Die Dauer MUSS abgerundet
  werden; Beispiele sind `0min`, `1min`, `2h` und `5d`.
- **FR-005**: Die Dauer MUSS sich während der geöffneten Ansicht spätestens an
  der nächsten Einheiten-Grenze aktualisieren. Ein zukünftiger oder wegen
  lokaler Uhrabweichung negativer Wert MUSS als `0min` erscheinen. Der
  vollständige lokale Eröffnungszeitpunkt MUSS als ergänzende zugängliche
  Information verfügbar sein.
- **FR-006**: Titel und Dauerangabe MÜSSEN auch bei langen Titeln und schmalen
  Viewports getrennt lesbar bleiben; der Titel DARF die Zeitangabe nicht
  überdecken.
- **FR-007**: Jeder Verlaufseintrag MUSS beim Hover und bei Tastaturfokus ein
  zugängliches Bearbeiten-Control mit Pencil-Semantik anbieten.
- **FR-008**: Das Bearbeiten-Control MUSS den Titel editierbar machen, ohne
  Nachrichten, Eröffnungsdauer oder aktiven Gesprächskontext zu verändern.
- **FR-009**: Ein Titel MUSS vor dem Speichern von äußeren Leerzeichen bereinigt
  werden und mindestens ein sichtbares Zeichen enthalten. Die zulässige
  maximale Titellänge beträgt 120 sichtbare Zeichen.
- **FR-010**: Ein bestätigter gültiger Titel MUSS dauerhaft gespeichert und nach
  einem Neuladen sowie auf synchronisierten Vault-Replikaten sichtbar sein.
- **FR-011**: Der Editiermodus MUSS Speichern per `Enter` oder einer
  gleichwertigen Bestätigungsaktion und Verwerfen per `Escape` ermöglichen.
- **FR-012**: Jeder Verlaufseintrag MUSS beim Hover und bei Tastaturfokus ein
  zugängliches Löschen-Control anbieten.
- **FR-013**: Die Löschaktion MUSS vor der endgültigen Ausführung eine
  Bestätigung verlangen und DARF bei Abbruch keine Daten verändern.
- **FR-014**: Eine bestätigte Löschung MUSS den Thread und alle ihm
  zugeordneten Nachrichten als eine zusammengehörige persistente Aktion
  entfernen und DARF keine verwaisten Nachrichten zurücklassen.
- **FR-015**: Wird der aktive Thread gelöscht, MUSS die Chat-Ansicht auf eine
  neue leere Session wechseln, ohne automatisch einen anderen Historien-Thread
  zu öffnen.
- **FR-016**: Fehler beim Laden, Umbenennen oder Löschen MÜSSEN verständlich
  angezeigt werden; ein Fehler DARF weder einen Eintrag stillschweigend
  entfernen noch einen teilweise geänderten Titel anzeigen.
- **FR-017**: Bearbeiten und Löschen MÜSSEN per Tastatur bedienbar sein. Die
  Controls MÜSSEN zugängliche Namen, sichtbare Fokuszustände und eine klare
  Aktionsemantik für Screenreader bereitstellen.
- **FR-018**: Alle neu eingeführten sichtbaren Texte, Fehlermeldungen und
  Bestätigungen MÜSSEN in Deutsch und Englisch über die bestehende
  Internationalisierung gepflegt werden.
- **FR-019**: Das Umbenennen und Löschen MUSS die bestehende Sortierung,
  Nachrichtenpersistenz, Streaming-, Tool-Approval- und
  Synchronisationssemantik respektieren, sofern diese Spec nichts anderes
  festlegt.
- **FR-020**: Wird der aktive Thread mit laufendem Turn oder offenem
  Tool-Approval gelöscht, MUSS die Chat-Ausführung zuerst abgebrochen und ihr
  terminaler Zustand abgewartet werden. Der Lösch-Command DARF erst danach
  ausgeführt werden. Scheitert der Abbruch oder wird kein terminaler Zustand
  erreicht, MUSS der Thread erhalten bleiben und der Nutzer einen Fehler
  erhalten.

### Key Entities

- **Chat-Thread / Verlaufseintrag**: Persistierte Unterhaltung mit stabiler ID,
  Titel, unveränderlichem `created_at`, Änderungszeitpunkt und zugeordneten
  Nachrichten. Der Verlauf zeigt eine UI-Projektion dieses Threads.
- **Chat-Nachricht**: Persistierter Inhalt eines Threads. Beim Löschen des
  zugehörigen Threads wird sie gemeinsam mit dem Thread entfernt.
- **Titeländerung**: Eine Nutzeraktion, die ausschließlich den Titel eines
  bestehenden Threads ersetzt und die Eröffnungszeit nicht verändert.

## Success Criteria

### Measurable Outcomes

- **SC-001**: In 100 % der geprüften Zustände zeigt jeder sichtbare
  Verlaufseintrag am rechten Rand eine Dauer im Format `<Ganzzahl><Einheit>`;
  kein Titel verdeckt sie.
- **SC-002**: In 100 % der geprüften Fälle bleibt die Dauer nach Umbenennen,
  neuem Nachrichtenversand und Neuladen auf demselben Eröffnungszeitpunkt
  basiert.
- **SC-003**: Ein Nutzer kann einen Verlaufstitel in höchstens drei klaren
  Interaktionen öffnen, ändern und speichern; der neue Titel ist anschließend
  nach einem Neuladen wieder vorhanden.
- **SC-004**: 100 % der Bearbeiten- und Löschen-Aktionen sind sowohl über
  Mausinteraktion als auch über Tastaturfokus erreichbar und haben in Deutsch
  und Englisch verständliche zugängliche Namen.
- **SC-005**: In 100 % der geprüften abgebrochenen Löschungen bleiben Thread und
  Nachrichten unverändert.
- **SC-006**: In 100 % der geprüften bestätigten Löschungen erscheinen Thread
  und zugehörige Nachrichten weder im Verlauf noch nach einem Neuladen; es
  bleiben keine verwaisten Nachrichten zurück.
- **SC-007**: Nach dem Löschen des aktiven Threads wird in 100 % der geprüften
  Fälle eine leere Session angezeigt und kein anderer Thread automatisch
  geöffnet.
- **SC-008**: In 100 % der geprüften Löschungen eines aktiven Threads mit
  laufendem Turn wird der Turn vor der persistierten Löschung beendet; bei
  fehlgeschlagenem Abbruch bleibt der Thread vollständig erhalten.

## Assumptions

- „Session eröffnet“ bezeichnet den persistierten Eröffnungszeitpunkt des
  Threads, nicht den letzten Zugriff oder die letzte Aktivität.
- Die Standardansicht zeigt ausschließlich die vergangene Dauer. Sie nutzt
  `min`, `h` und `d` als feste kompakte Einheiten: unter einer Stunde Minuten,
  unter einem Tag Stunden, ab einem Tag Tage; die Werte werden abgerundet.
  Der vollständige lokale Zeitpunkt ist nur ergänzende zugängliche Information.
- Der bestehende Verlauf zeigt nur persistierte Threads. Ein leerer,
  nicht gesendeter Chat-Entwurf aus Spec 004 benötigt keinen eigenen
  Historieneintrag und kann nicht gelöscht werden.
- Löschen ist in v1 endgültig; ein Papierkorb oder Undo-Mechanismus ist nicht
  Bestandteil dieser Spec. Die Bestätigung schützt vor versehentlichen
  Löschungen.
- Die maximale Titellänge von 120 sichtbaren Zeichen ist eine UX-Grenze; die
  tatsächliche Darstellung darf längere Inhalte zusätzlich visuell kürzen,
  sofern der gespeicherte Titel zugänglich bleibt.

## Explicit Non-Goals

- Volltextsuche, Filterung, Gruppierung oder Sortieroptionen für den Verlauf.
- Automatische Titelgenerierung oder Änderung des bestehenden Standardtitels.
- Bearbeiten oder Löschen einzelner Nachrichten innerhalb eines Threads.
- Wiederherstellung gelöschter Threads.
- Änderung der bestehenden Chat-, Streaming-, Tool-Approval- oder
  Vault-Synchronisationsarchitektur außerhalb der für Titeländerung und
  Löschung notwendigen Persistenzaktion.
