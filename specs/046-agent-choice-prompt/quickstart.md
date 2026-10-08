# Quickstart: Agent-Rückfrage prüfen

Voraussetzung: App gestartet (`pnpm tauri:dev:cuda` oder `pnpm tauri:dev`), Vault entsperrt, ein lokales
Modell mit Werkzeugnutzung gewählt (empfohlen: Qwen3-4B), Erweiterungen haex-mail, haex-notes und
haex-files installiert.

1. **Exakter Name (US1/1)**: „öffne haex-mail“ → haex-mail öffnet sich, keine Rückfrage.
2. **Tippfehler (US1/2, SC-002)**: „öffne haex-mial“ → haex-mail öffnet sich, keine Rückfrage.
3. **System-App deutsch (US1/3)**: „öffne die Einstellungen“ → Einstellungen öffnen sich.
4. **Geratene ID (US1/4)**: Im Verlauf prüfen, dass ein Aufruf mit `system.notes` haex-notes öffnete
   (ggf. „öffne notes“ versuchen, bis das Modell eine ID rät).
5. **Mehrdeutig (US2/1–2, SC-003)**: „öffne haex“ → Rückfrage „Meintest du …?“ mit haex-mail,
   haex-notes, haex-files; noch nichts geöffnet. haex-notes wählen, bestätigen → haex-notes öffnet sich.
6. **Freitext (US2/3, US2/5)**: „öffne haex“ → „Etwas anderes …“ → „haex“ → neue Rückfrage;
   abbrechen → „Etwas anderes …“ → „Einstellungen“ → Einstellungen öffnen sich.
7. **Abbrechen (US2/4)**: „öffne haex“ → Abbrechen → nichts öffnet sich, der Agent sagt das.
8. **Manueller Modus (FR-012)**: Freigabe-Modus „Manuell“, „öffne haex“ → erst Freigabe, dann
   Rückfrage, nach der Wahl keine zweite Freigabe.
9. **Turn abbrechen (FR-011)**: „öffne haex“, Rückfrage offen lassen, Stopp → Rückfrage verschwindet,
   nichts öffnet sich.
10. **Thread-Wechsel**: Rückfrage offen, anderen Chat öffnen und zurück → Rückfrage wieder sichtbar.
11. **Verlauf (FR-010)**: Chat neu öffnen → Tool-Ergebnis zeigt Angabe und Antwort.
12. **Nicht verfügbar (R8)**: Erweiterung auf diesem Gerät im Zustand „Wird übertragen“ → als Kandidat
    deaktiviert mit Grund.
13. **Allgemeine Rückfrage (US3)**: „mach das dunkler“ → Rückfrage mit Möglichkeiten (z. B.
    dunkles Farbschema); wählen → wird umgesetzt. Kleines Modell darf hier scheitern (SC-006: 7/10).
14. **Modellprüfung (FR-017)**: `cargo test --test model_tool_eval` gegen Qwen3-4B; Quoten für
    `change` (Tippfehler) und `clarify` notieren.
