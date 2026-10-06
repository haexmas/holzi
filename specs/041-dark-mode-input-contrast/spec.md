# Feature Specification: Dark-Mode-Input-Kontrast

**Feature Branch**: `fix/dark-mode-input-contrast`

**Created**: 2026-10-06

**Status**: Ready for planning

**Input**: User description: "Im Dark Mode sind die Input-Felder, wenn sie nicht fokussiert sind, schwer zu erkennen; im Light Mode sieht das besser aus."

## User Scenarios & Testing

### User Story 1 - Nicht fokussierte Eingabefelder erkennen (Priority: P1)

Als Nutzer möchte ich auch im Dark Mode die Begrenzung nicht fokussierter Eingabefelder klar erkennen, damit ich weiß, wo ich Text eingeben kann.

**Why this priority**: Die aktuelle Darstellung erschwert eine grundlegende Interaktion und betrifft alle Formulare im Dark Mode.

**Independent Test**: Mehrere Formulare im Dark Mode öffnen, den Fokus aus einem ausgefüllten und einem leeren Eingabefeld entfernen und prüfen, dass dessen Begrenzung klar vom umgebenden Hintergrund unterscheidbar bleibt.

**Acceptance Scenarios**:

1. **Given** die Anwendung ist im Dark Mode und ein Eingabefeld ist nicht fokussiert, **When** der Nutzer das Feld betrachtet, **Then** ist seine Begrenzung sichtbar und vom Feldhintergrund sowie vom umgebenden Hintergrund unterscheidbar.
2. **Given** die Anwendung ist im Light Mode, **When** der Nutzer ein Eingabefeld betrachtet, **Then** bleibt die bisherige erkennbare Darstellung unverändert.
3. **Given** ein Eingabefeld erhält den Fokus, **When** der Nutzer darin arbeitet, **Then** bleibt der Fokusindikator deutlich sichtbar und wird nicht durch die Verbesserung des Ruhe-Zustands ersetzt.

### Edge Cases

- Die Verbesserung gilt für leere und ausgefüllte Eingabefelder gleichermaßen.
- Spezifische Eingabefelder mit eigener Rahmenfarbe dürfen ihre semantische Hervorhebung behalten.
- Die Änderung darf keine Farbwahl des Nutzers in der bestehenden Appearance-Konfiguration überschreiben.

## Requirements

### Functional Requirements

- **FR-001**: System MUST render non-focused input-field boundaries in Dark Mode with at least the WCAG contrast ratio against every surface (background, card, popover, sidebar, muted) that the same boundary has in Light Mode.
- **FR-002**: System MUST preserve the existing Light Mode appearance of input fields.
- **FR-003**: System MUST preserve visible focus indicators for focused input fields.
- **FR-004**: System MUST apply the improvement consistently to the shared input styling used by the application.

## Success Criteria

### Measurable Outcomes

- **SC-001**: In the Dark Mode visual check, the boundary of every checked non-focused input field is immediately distinguishable from both its fill and its surrounding surface.
- **SC-002**: The Light Mode visual check shows no unintended change to input-field boundaries.
- **SC-003**: The application build and existing automated checks complete successfully after the styling change.
- **SC-004**: `pnpm check:appearance` measures the input boundary against every surface in both schemes, for every component colour preset, and fails if Dark Mode falls below Light Mode.

## Assumptions

- Die betroffenen Eingabefelder verwenden die vorhandenen gemeinsamen Theme-Farbvariablen für Rahmen und Input-Flächen.
- Eine kleine Anpassung der bestehenden Dark-Mode-Theme-Werte ist ausreichend; neue Komponenten oder Abhängigkeiten sind nicht erforderlich.
- Die visuelle Abnahme erfolgt in mindestens einem repräsentativen Formular mit leerem und ausgefülltem Feld sowie mit Fokuswechsel.
