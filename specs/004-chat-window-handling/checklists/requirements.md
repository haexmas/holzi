# Requirements Checklist: Chatfenster und Session-Handling

**Feature**: [../spec.md](../spec.md)

## Scope und Verhalten

- [x] Neue Chat-Session ist eindeutig von persistiertem Thread unterschieden.
- [x] Historie bleibt erhalten und wird nicht automatisch fortgesetzt.
- [x] Modell-Preload startet nach Genesis, Unlock und Vault-Wechsel.
- [x] Vault-Open wartet nicht auf den Modell-Load.
- [x] Veraltete Loads können keinen neuen aktiven Status überschreiben.
- [x] Ein erst nach Preload-Start gemounteter Chat liest den aktuellen Snapshot und bleibt nicht in einem falschen Ladezustand hängen.

## Composer

- [x] Ein gemeinsamer Settings-Button bündelt Modell und Effort in einem Popover.
- [x] Der Modellname im geschlossenen Settings-Button ist auf maximal 20 Zeichen inklusive Ellipsis begrenzt, hat zusätzlich eine responsive Maximalbreite und bleibt vollständig zugänglich.
- [x] Das Freigabe-Control bleibt ein eigenes Dropdown in der Reihenfolge Plan, Manuell, Automatisch.
- [x] Modell- und Freigabeauswahl verwenden die Shadcn-Select-Komponenten; Effort verwendet einen verstärkten Shadcn-Slider.
- [x] Settings-Button, Freigabe-Dropdown und Senden/Abbrechen liegen in einer Reihe unterhalb der Textarea.
- [x] Das Settings-Popover öffnet sichtbar oberhalb der Reihe und wird nicht vom scrollenden Composer-Container abgeschnitten.
- [x] Controls sind kompakt, responsive und zugänglich.
- [x] Deaktivierte Buttons bleiben lesbar, verlieren nicht durch globale Transparenz ihren Textkontrast und sind visuell klar von aktiven Buttons unterschieden.
- [x] Textarea wächst bis maximal 8 sichtbare Zeilen und scrollt danach intern.
- [x] `Enter` und `Shift+Enter` behalten die festgelegte Semantik.

## Reasoning

- [x] Nicht-leeres Reasoning wird pro Assistant-Nachricht angeboten.
- [x] Accordion ist beim ersten Rendern immer geschlossen.
- [x] Accordions sind unabhängig voneinander bedienbar.
- [x] Normale Antwort bleibt bei geschlossenem Accordion vollständig sichtbar.
- [x] Reasoning und Accordion-Zustände werden nicht persistiert.

## Qualität

- [x] Rust-Tests für Load-Status und Race-Verhalten vorhanden.
- [x] `pnpm typecheck` erfolgreich.
- [x] Deutsch und Englisch enthalten alle neuen UI-Schlüssel.
- [x] PR-Review-Fixes für Modell-Capabilities, Anthropic-Thinking-Constraints,
      Preload-Cancellation/DB-Lifetime, Status-Watermark und Active-Model-Refresh
      umgesetzt und durch Tests abgesichert.
- [x] PR-42-CI vollständig erfolgreich: Formatierung, Default-/No-Default-
      Rust-Tests, beide Clippy-Läufe, Chat-State-/Dokumentationscheck und
      Vault→Model→CLI-E2E.
- [ ] Quickstart auf Desktop und schmalem Viewport durchgeführt.

## Deferred validation

- Die GUI-Szenarien im Quickstart (T018, T024, T031, T036, T043, T048)
  benötigen einen laufenden Tauri-Desktop und wurden in dieser Umgebung nicht
  ausgeführt.
- Die dedizierte Preload-Lifecycle-Suite (T012/T023) ist noch offen; die
  Zustands- und Race-Invarianten sind durch `session_tests.rs` abgedeckt.
