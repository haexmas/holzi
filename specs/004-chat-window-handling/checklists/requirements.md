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

- [X] Modell-, Effort- und Freigabe-Control liegen im gemeinsamen Composer; Reasoning hat keinen eigenen Schalter.
- [X] Controls sind kompakt, responsive und zugänglich.
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
- [ ] Quickstart auf Desktop und schmalem Viewport durchgeführt.

## Deferred validation

- Die GUI-Szenarien im Quickstart (T018, T024, T031, T036, T043, T048)
  benötigen einen laufenden Tauri-Desktop und wurden in dieser Umgebung nicht
  ausgeführt.
- Die dedizierte Preload-Lifecycle-Suite (T012/T023) ist noch offen; die
  Zustands- und Race-Invarianten sind durch `session_tests.rs` abgedeckt.
