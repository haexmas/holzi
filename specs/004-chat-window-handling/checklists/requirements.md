# Requirements Checklist: Chatfenster und Session-Handling

**Feature**: [../spec.md](../spec.md)

## Scope und Verhalten

- [ ] Neue Chat-Session ist eindeutig von persistiertem Thread unterschieden.
- [ ] Historie bleibt erhalten und wird nicht automatisch fortgesetzt.
- [ ] Modell-Preload startet nach Genesis, Unlock und Vault-Wechsel.
- [ ] Vault-Open wartet nicht auf den Modell-Load.
- [ ] Veraltete Loads können keinen neuen aktiven Status überschreiben.

## Composer

- [ ] Modell-, Effort- und Freigabe-Control liegen im gemeinsamen Composer; Reasoning hat keinen eigenen Schalter.
- [ ] Controls sind kompakt, responsive und zugänglich.
- [ ] Textarea wächst bis maximal 8 sichtbare Zeilen und scrollt danach intern.
- [ ] `Enter` und `Shift+Enter` behalten die festgelegte Semantik.

## Reasoning

- [ ] Nicht-leeres Reasoning wird pro Assistant-Nachricht angeboten.
- [ ] Accordion ist beim ersten Rendern immer geschlossen.
- [ ] Accordions sind unabhängig voneinander bedienbar.
- [ ] Normale Antwort bleibt bei geschlossenem Accordion vollständig sichtbar.
- [ ] Reasoning und Accordion-Zustände werden nicht persistiert.

## Qualität

- [ ] Rust-Tests für Load-Status und Race-Verhalten vorhanden.
- [ ] `pnpm typecheck` erfolgreich.
- [ ] Deutsch und Englisch enthalten alle neuen UI-Schlüssel.
- [ ] Quickstart auf Desktop und schmalem Viewport durchgeführt.
