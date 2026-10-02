# Vertrag: Aktionen der Darstellung

Alle im Aktionskatalog (`src/lib/actions/settingsActions.ts`), `scope: 'settings.device'`,
Handler in `src/stores/settingsActionHandlers.ts`. Fehler tragen die Schlüssel aus
`contracts/appearance-file.md` beziehungsweise `settings.appearance.failed`.

## `settings.appearance.set` — effect: write

Setzt ein oder mehrere Felder der Darstellung (Teilwerte). Wirkt sofort und gilt für die Vault.

Eingabe (mindestens ein Feld):

```json
{
  "accent":     { "preset": "blue" },
  "window":     { "custom": "#223344" },
  "container":  { "preset": "warm" },
  "text":       { "preset": "neutral" },
  "component":  { "preset": "cool" },
  "windowHint": true
}
```

Ergebnis: die gespeicherte Darstellung und `adjustments` (Liste, kann leer sein):

```json
{ "appearance": { "v": 1, "accent": {…}, … }, "adjustments": [ { "control": "window", "kind": "chroma", "reason": "text-contrast" } ] }
```

Fehler: `invalid` (Feld ungültig, nichts geschrieben).

## `settings.appearance.reset` — effect: write

Keine Eingabe. Setzt die Darstellung auf den Standard; das Schema bleibt. Die Oberfläche fragt
vor dem Aufruf nach Bestätigung (FR-017); die Aktion selbst ist unmittelbar (ein Modell im Chat
hat, wie bei anderen schreibenden Einstellungen, die Bestätigung über die Freigaberegeln aus 032).

Ergebnis: `{ "appearance": {…Standard…} }`.

## `settings.appearance.export` — effect: read

Keine Eingabe. Ergebnis: `{ "file": "<JSON-Text nach appearance-file.md>" }`. Die Oberfläche
schreibt den Text über den Speichern-Dialog.

## `settings.appearance.import` — effect: write

Eingabe: `{ "file": "<JSON-Text>" }`. Prüft wie in `appearance-file.md`, schreibt Schema und
Darstellung, wendet an. Ergebnis wie `set`. Fehler: die Schlüssel der Importprüfung.

## `settings.get` (erweitert)

Das Ergebnis erhält `appearance` (die Darstellung, mit Standard für ungesetzt). Keine weitere
Änderung der bestehenden Felder.

## Vom Katalog erwartet

`check-agent-actions.ts` prüft, dass jede Aktion Beschreibung, Eingabe-Schema, Ergebnis-Schema,
`scope` und `effect` hat; die vier Aktionen kommen mit englischer Beschreibung wie die
bestehenden (die Oberfläche ist deutsch, der Katalog für Modelle englisch).
