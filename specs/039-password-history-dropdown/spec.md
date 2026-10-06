# Feature Specification: Kompakter Passwortverlauf

**Feature Branch**: `feat/password-history-dropdown`

**Created**: 2026-10-06

**Status**: Draft

**Input**: User description: "Im Passwortmanager soll der Verlauf statt als zweispaltige Ansicht mit Dropdown in einer Spalte angezeigt werden. Die Anzeige von ‚gespeichert am …‘ soll dezenter werden."

## User Scenarios & Testing

### User Story 1 - Frühere Eintragsstände übersichtlich auswählen (Priority: P1)

Ein Nutzer öffnet den Verlauf eines Passwort-Eintrags. Er wählt den gewünschten früheren
Stand aus einem Dropdown und sieht dessen Inhalte darunter in derselben Spalte. Die
Funktion zum Wiederherstellen bleibt erreichbar.

**Why this priority**: Der Verlauf soll bei schmalen und breiten Fenstern gleichermaßen
ruhig und verständlich bleiben, ohne zwei nebeneinanderliegende Inhaltsbereiche.

**Independent Test**: Einen Eintrag mit mindestens zwei Verlaufsständen öffnen, im Dropdown
zwischen den Ständen wechseln und prüfen, dass jeweils der passende Stand darunter angezeigt
wird und wiederhergestellt werden kann.

**Acceptance Scenarios**:

1. **Given** ein Eintrag hat mehrere Verlaufsstände, **When** der Nutzer den Verlauf öffnet,
   **Then** sieht er genau eine vertikale Inhaltsfolge mit einem Dropdown über dem gewählten
   Stand.
2. **Given** ein Stand ist ausgewählt, **When** der Nutzer einen anderen Stand im Dropdown
   wählt, **Then** werden die Daten des neuen Stands unterhalb des Dropdowns angezeigt.
3. **Given** ein Verlaufsstand ist ausgewählt, **When** der Nutzer auf Wiederherstellen klickt,
   **Then** bleibt die bestehende Bestätigung und Wiederherstellungsfunktion unverändert
   erreichbar.
4. **Given** es gibt nur einen Verlaufsstand, **When** der Nutzer den Verlauf öffnet,
   **Then** bleibt der Hinweis erhalten, dass es keinen älteren Stand gibt.

### User Story 2 - Zeitinformation zurückhaltend wahrnehmen (Priority: P2)

Ein Nutzer kann weiterhin erkennen, wann der gewählte Stand gespeichert wurde, ohne dass die
Zeitinformation die eigentlichen Eintragsdaten visuell dominiert.

**Why this priority**: Die Zeitinformation ist nützlich, soll aber gegenüber den Daten und der
Wiederherstellungsaktion sekundär wirken.

**Independent Test**: Einen Verlaufsstand öffnen und prüfen, dass „Gespeichert am …“ sichtbar,
lesbar und deutlich kleiner bzw. weniger betont als die Inhaltsüberschrift dargestellt wird.

**Acceptance Scenarios**:

1. **Given** ein Verlaufsstand ist geladen, **When** der Nutzer die Ansicht betrachtet,
   **Then** ist die Speicherdauer/-zeit als dezente Metainformation sichtbar.
2. **Given** die Zeit ist ungültig oder fehlt, **When** der Stand angezeigt wird,
   **Then** bleibt die bestehende Platzhalterdarstellung erhalten.

### Edge Cases

- Eine lange Liste von Verlaufsständen bleibt im Dropdown auswählbar, ohne die Seite horizontal
  zu verbreitern.
- Die Auswahl ist während des Ladens bzw. bei einem Ladefehler nicht interaktiv sichtbar;
  bestehende Lade- und Fehlerzustände bleiben erhalten.
- Die Auswahl eines Stands blendet weiterhin zuvor aufgedeckte Geheimnisse über den bestehenden
  Reset-Mechanismus aus.

## Requirements

### Functional Requirements

- **FR-001**: Der Verlauf MUSS die verfügbaren Stände über eine einzelne Dropdown-Auswahl
  anbieten.
- **FR-002**: Der Verlauf MUSS die Inhalte des ausgewählten Stands unterhalb der Dropdown-Auswahl
  in einer einzigen vertikalen Spalte anzeigen.
- **FR-003**: Beim Wechsel der Auswahl MUSS der ausgewählte Stand geladen und angezeigt werden;
  die bestehende Wiederherstellung eines ausgewählten Stands MUSS erhalten bleiben.
- **FR-004**: Die Anzeige von „Gespeichert am …“ MUSS sichtbar bleiben, aber als dezente
  Metainformation gegenüber dem eigentlichen Inhalt zurückgenommen werden.
- **FR-005**: Die bestehenden Lade-, Fehler-, leereren Verlauf- und Geheimnis-Sichtbarkeitsregeln
  DÜRFEN durch die Layoutänderung nicht verloren gehen.
- **FR-006**: Die Auswahl MUSS per Tastatur und mit einem zugänglichen Namen bedienbar bleiben.

## Success Criteria

### Measurable Outcomes

- **SC-001**: Bei einem geöffneten Verlauf gibt es auf allen unterstützten Breiten höchstens
  eine vertikale Inhaltsflussrichtung; die Verlaufsstände werden nicht als parallele Spalte
  neben den Daten angezeigt.
- **SC-002**: Ein Nutzer kann einen beliebigen geladenen Verlaufsstand mit höchstens zwei
  Interaktionen auswählen und dessen Inhalte sehen.
- **SC-003**: Die Zeitinformation bleibt in 100 % der geladenen Standansichten vorhanden,
  sofern ein Zeitstempel vorliegt, und verwendet eine visuell sekundäre Darstellung.
- **SC-004**: Bestehende Prüfungen für Passwortmanager-Templates und Typen bleiben erfolgreich.

## Assumptions

- Das bestehende Dropdown- und Übersetzungssystem des Projekts wird wiederverwendet.
- Der bisherige relative/exakte Zeittext und die Reihenfolge „neueste zuerst“ bleiben bestehen.
- Es sind keine Änderungen am Datenmodell, an Backend-Befehlen oder an der Navigation nötig.
- Die Änderung gilt für die bestehende Desktop-/Webview-Passwortmanageransicht; neue responsive
  Breakpoints sind nicht Teil dieses Changes.
