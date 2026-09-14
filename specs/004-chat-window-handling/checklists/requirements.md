# Requirements Checklist: Chatfenster und Session-Handling

**Feature**: [../spec.md](../spec.md)

## Scope und Verhalten

- [X] Neue Chat-Session ist eindeutig von persistiertem Thread unterschieden.
- [X] Historie bleibt erhalten und wird nicht automatisch fortgesetzt.
- [X] Modell-Preload startet nach Genesis, Unlock und Vault-Wechsel.
- [X] Vault-Open wartet nicht auf den Modell-Load.
- [X] Veraltete Loads können keinen neuen aktiven Status überschreiben.
- [X] Ein erst nach Preload-Start gemounteter Chat liest den aktuellen Snapshot und bleibt nicht in einem falschen Ladezustand hängen.

## Composer

- [X] Ein gemeinsamer Settings-Button bündelt Modell und Effort in einem Popover.
- [X] Der Modellname im geschlossenen Settings-Button ist auf maximal 20 Zeichen inklusive Ellipsis begrenzt, hat zusätzlich eine responsive Maximalbreite und bleibt vollständig zugänglich.
- [X] Das Freigabe-Control bleibt ein eigenes Dropdown für Plan, Manuell und Automatisch.
- [X] Modell- und Freigabeauswahl verwenden die Shadcn-Select-Komponenten; Effort verwendet einen verstärkten Shadcn-Slider.
- [X] Settings-Button, Freigabe-Dropdown und Senden/Abbrechen liegen in einer Reihe unterhalb der Textarea.
- [X] Das Settings-Popover öffnet sichtbar oberhalb der Reihe und wird nicht vom scrollenden Composer-Container abgeschnitten.
- [X] Controls sind kompakt, responsive und zugänglich.
- [X] Deaktivierte Buttons bleiben lesbar und verlieren nicht durch globale Transparenz ihren Textkontrast.
- [X] Textarea wächst bis maximal 8 sichtbare Zeilen und scrollt danach intern.
- [X] `Enter` und `Shift+Enter` behalten die festgelegte Semantik.

## Reasoning

- [X] Nicht-leeres Reasoning wird pro Assistant-Nachricht angeboten.
- [X] Accordion ist beim ersten Rendern immer geschlossen.
- [X] Accordions sind unabhängig voneinander bedienbar.
- [X] Normale Antwort bleibt bei geschlossenem Accordion vollständig sichtbar.
- [X] Reasoning und Accordion-Zustände werden nicht persistiert.

## Qualität

- [X] Rust-Tests für Load-Status und Race-Verhalten vorhanden.
- [X] `pnpm typecheck` erfolgreich.
- [X] Deutsch und Englisch enthalten alle neuen UI-Schlüssel.
- [X] PR-Review-Fixes für Modell-Capabilities, Anthropic-Thinking-Constraints,
  Preload-Cancellation/DB-Lifetime, Status-Watermark und Active-Model-Refresh
  umgesetzt und durch Tests abgesichert.
- [X] PR-42-CI vollständig erfolgreich: Formatierung, Default-/No-Default-
  Rust-Tests, beide Clippy-Läufe, Chat-State-/Dokumentationscheck und
  Vault→Model→CLI-E2E.
- [ ] Quickstart auf Desktop und schmalem Viewport durchgeführt.

## Deferred validation

- Die GUI-Szenarien im Quickstart (T018, T024, T031, T036, T043, T048)
  benötigen einen laufenden Tauri-Desktop und wurden in dieser Umgebung nicht
  ausgeführt.
- Die dedizierte Preload-Lifecycle-Suite (T012/T023) ist noch offen; die
  Zustands- und Race-Invarianten sind durch `session_tests.rs` abgedeckt.
