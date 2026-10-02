# Datenmodell: Darstellung und Eingabefelder

Es gibt keine neue Tabelle. Die Darstellung ist ein Wert der bestehenden Tabelle `preferences`
(Scope `vault`), CRDT-gesynct wie alle Einstellungen.

## Schlüssel

| Schlüssel                 | Scope | Wert                                  | Besitzer         |
| ------------------------- | ----- | ------------------------------------- | ---------------- |
| `appearance.color_scheme` | vault | `light` \| `dark` \| `system`         | 023, unverändert |
| `appearance.theme`        | vault | JSON-Text der **Darstellung** (unten) | diese Spec       |

Fehlt ein Schlüssel oder ist der Wert kein JSON beziehungsweise `v` unbekannt, gilt der ganze
Standard (FR-020). Ist nur ein einzelnes Feld ungültig, fällt dieses Feld auf seinen Standard und
die übrigen bleiben; der Import (Datei) ist dagegen streng und nimmt nur ganz gültige Dateien an.

## Darstellung (`appearance.theme`)

```json
{
  "v": 1,
  "accent": { "preset": "teal" },
  "window": { "preset": "neutral" },
  "container": { "preset": "neutral" },
  "text": { "preset": "neutral" },
  "component": { "preset": "neutral" },
  "windowHint": false
}
```

Jede Farbangabe (`accent`, `window`, `container`, `text`, `component`) ist genau eins von:

- `{ "preset": "<id>" }` – ein Farbfeld aus `presets.ts`;
- `{ "custom": "#rrggbb" }` – eine eigene Farbe (Hex, sechsstellig, Kleinbuchstaben, ohne Alpha).

| Feld         | Typ           | Regel                                                    | Standard             |
| ------------ | ------------- | -------------------------------------------------------- | -------------------- |
| `v`          | Ganzzahl      | muss `1` sein; andere Werte → ganze Darstellung ungültig | `1`                  |
| `accent`     | Farbangabe    | Preset aus der Akzentreihe oder Hex                      | `{preset:"teal"}`    |
| `window`     | Farbangabe    | Preset aus der Tönungsreihe oder Hex                     | `{preset:"neutral"}` |
| `container`  | Farbangabe    | wie `window`                                             | `{preset:"neutral"}` |
| `text`       | Farbangabe    | wie `window`                                             | `{preset:"neutral"}` |
| `component`  | Farbangabe    | wie `window`                                             | `{preset:"neutral"}` |
| `windowHint` | Wahrheitswert | Fensterhinweis (FR-024)                                  | `false`              |

Unbekannte Felder werden beim Lesen ignoriert und beim Schreiben weggelassen (Vorwärtsverträglichkeit
von einer neueren Version). Ein unbekannter Preset-Name zählt als ungültiges Feld; dieses Feld
fällt auf seinen Standard, die übrigen bleiben (nur beim Lesen aus der Vault; ein **Import**
prüft strenger, siehe Datei).

Jede eigene Farbe gehört zu **ihrem** Regler; es gibt keine gemeinsame Palette. Der Nutzer
sieht seine eigene Farbe als zusätzliches Feld in der Reihe (FR-012) und kann sie durch
Wählen eines Presets ablegen; sie geht dabei verloren.

## Farbfelder (`presets.ts`)

**Akzent** (Standard zuerst; Farbton in OKLCH, Sättigung 0.17 sofern nicht angegeben):
`teal` 180 (Standard), `blue` 255, `violet` 300, `pink` 350, `red` 25, `orange` 55,
`yellow` 95, `green` 150, `neutral` Sättigung 0.0 (Grau). Neun Felder; die Reihe zeigt zusätzlich
die eigene Farbe, falls gesetzt, und „+“.

**Tönungen** (Farbton / Sättigung; Sättigung begrenzt durch die Obergrenze des Reglers):
`neutral` 0 / 0 (Standard), `warm` 60 / 0.02, `cool` 240 / 0.02, `green` 150 / 0.02,
`violet` 300 / 0.02, `rose` 10 / 0.02. Sechs Felder.

Die Namen sind Sprachschlüssel unter `settings.appearance.preset.*` (Deutsch und Englisch).

## Abgeleiteter Zustand (nicht gespeichert)

`derive(appearance, scheme) → { tokens: Record<TokenName, string>, adjustments: Adjustment[] }`

- `tokens`: die CSS-Variablen (siehe `contracts/token-map.md`), als `oklch(L C H)`.
- `adjustments`: Liste der Anpassungen, die der Kontrast erzwungen hat, je mit Regler
  (`accent | window | container | text | component`), Art (`lightness | chroma`) und
  Grund (Schlüssel für die Meldung „angepasst“ in der Zeile; FR-017).

Zustand der Oberfläche: `useAppearance().appearance` (die gelesene Darstellung), `.tokens`
(letzte Ableitung), `.adjustments`. Beides ist rein abgeleitet und wird nach jedem Wechsel von
Darstellung, Schema oder System-Schema neu berechnet.

## Zustandsübergänge

| Auslöser                                       | Wirkung                                                                                                               |
| ---------------------------------------------- | --------------------------------------------------------------------------------------------------------------------- |
| Vault geöffnet                                 | `loadAsync`: Standard anwenden, Darstellung lesen, anwenden                                                           |
| Auswahl eines Reglers                          | Teilwert schreiben (`setPrefAsync`), dann anwenden; bei Schreibfehler bleibt der alte Zustand, Zeile zeigt den Fehler |
| `preferences` geändert (Sync, anderes Fenster) | `refreshAsync`: neu lesen, ohne Zwischenzustand anwenden                                                              |
| Schema oder System-Schema wechselt             | neu ableiten und anwenden                                                                                             |
| Zurücksetzen (nach Bestätigung)                | Darstellung auf Standard schreiben (Schema bleibt), anwenden                                                          |
| Import                                         | Datei ganz prüfen; bei Erfolg Darstellung **und** Schema schreiben, anwenden; bei Fehler nichts ändern                |
| Vault gesperrt/gewechselt                      | Variablen entfernen, Standard (`tailwind.css`) gilt                                                                   |

## Standardwerte (Tabelle wird bei der Umsetzung gefüllt)

Die Standardwerte der Tokens bleiben die aus `src/assets/css/tailwind.css`, bis auf den
Akzent im hellen Schema (research R4). `check-appearance.ts` schreibt die gemessenen Kontraste
dieser Standardwerte in die Beschreibung des PRs; Abweichungen von den heutigen Werten sind
dort aufgelistet.
