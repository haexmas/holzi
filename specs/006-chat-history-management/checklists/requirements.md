# Specification Quality Checklist: Chat-Historie verwalten

**Purpose**: Qualität und Vollständigkeit der Anforderungen vor der Planung
prüfen
**Created**: 2026-09-15
**Feature**: [../spec.md](../spec.md)

## Content Quality

- [x] Keine unnötigen Implementierungsdetails; die Spec beschreibt Nutzerwert
      und beobachtbares Verhalten.
- [x] Die Spec ist auf Verlauf, Daueranzeige, Umbenennen und Löschen begrenzt.
- [x] Die Formulierungen sind für Produkt- und UX-Entscheidungen verständlich.
- [x] Alle Pflichtabschnitte sind ausgefüllt.

## Requirement Completeness

- [x] Keine `[NEEDS CLARIFICATION]`-Marker verbleiben.
- [x] Funktionale Anforderungen sind testbar und eindeutig formuliert.
- [x] Das Dauerformat, die Einheiten, Rundung und das Verhalten bei
      Uhrabweichungen sind explizit festgelegt.
- [x] Die Unix-Millisekunden-Zeitbasis und der `0min`-Fallback für unbrauchbare
      Zeitstempel sind in Spec, Datenmodell und Contract konsistent festgelegt.
- [x] Erfolgskriterien sind messbar und technologieagnostisch.
- [x] Primär-, Abbruch-, Fehler- und Recovery-Szenarien sind abgedeckt.
- [x] Edge Cases für Zeit, lange Titel, Touch, laufende Turns und Sync sind
      beschrieben.
- [x] Die Historienzeile beschreibt die flexible Titelspalte, die
      rechtsbündige Dauer und die Aktionsposition direkt links neben der Zeit.
- [x] Scope, Annahmen und Nicht-Ziele sind dokumentiert.

## Requirement Consistency

- [x] Die sichtbare Dauer basiert unabhängig von Titeländerungen und
      Nachrichtenalter auf `created_at`.
- [x] Löschen des aktiven Threads steht im Einklang mit dem neuen leeren
      Chat-Entwurf aus Spec 004.
- [x] Spec 004 verweist für Verlaufsverwaltung und Daueranzeige auf Spec 006.
- [x] Das Verhalten beim Löschen eines aktiven Threads mit laufendem Turn ist
      eindeutig als Abbruch-vor-Löschung festgelegt.
- [x] Bestehende Nachrichten-, Tool-Loop- und Vault-Semantik wird nicht
      ungewollt ersetzt.

## Accessibility & Internationalization

- [x] Hover-Aktionen sind auch über Tastaturfokus erreichbar.
- [x] Bearbeiten und Löschen erscheinen zwischen Titel und Dauer, ohne im
      ausgeblendeten Zustand dauerhaft Titelplatz zu verbrauchen.
- [x] Zugängliche Namen, Fokuszustände und Screenreader-Semantik sind
      spezifiziert.
- [x] Deutsch und Englisch sind für neue sichtbare Texte und Fehlerfälle
      vorgesehen.

## Notes

- Die Spec wurde als Umsetzungsgrundlage verwendet. GUI-Details wie exakte
  Icongröße und konkrete Dialoggestaltung bleiben der Implementierung
  überlassen, sofern sie die beschriebenen Verträge erfüllen.
- Implementierung validiert: `cargo test --no-default-features` mit 151 Unit-
  und allen Integrationstests, inklusive der neuen Thread-Management-Tests,
  sowie `node scripts/check-chat-state.mjs` mit 19 bestandenen Tests.
- Frontend-Prettier, ESLint und Nuxt-Typecheck wurden mit der installierten
  Repository-Toolchain erfolgreich ausgeführt; der manuelle Desktop-/Narrow-
  Viewport-Durchlauf bleibt für die Abnahme offen.
