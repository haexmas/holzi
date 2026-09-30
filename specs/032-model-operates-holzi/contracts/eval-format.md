# Vertrag: Beispielsatz-Satz und Messbericht

**Spec**: [../spec.md](../spec.md) (FR-019 bis FR-022) | **Datenmodell**: [../data-model.md](../data-model.md)

## Dateien

- `src-tauri/src/chat/eval/eval_set.json`: der versionierte Satz, eingebettet
  per `include_str!`.
- `src-tauri/src/chat/eval/tools.json`: Schnappschuss der
  `AgentActionDef`-Liste aus dem Katalog, erzeugt von
  `scripts/export-eval-tools.ts`. `pnpm check:agent-actions` schlägt fehl,
  wenn der Schnappschuss vom Katalog abweicht.
- Bericht: `target/eval/<modell>.json` (nicht versioniert).

## `eval_set.json`

```json
{
  "version": 1,
  "sentences": [
    {
      "id": "read-tabs-de-1",
      "lang": "de",
      "kind": "read",
      "text": "Welche Tabs sind gerade offen?",
      "expect": [{ "tool": "wm_state_get", "args": {} }],
      "selfTest": true
    },
    {
      "id": "change-scheme-en-1",
      "lang": "en",
      "kind": "change",
      "text": "Switch to the dark color scheme.",
      "expect": [
        {
          "tool": "settings_appearance_setColorScheme",
          "args": { "scheme": "dark" }
        }
      ],
      "selfTest": false
    },
    {
      "id": "smalltalk-de-1",
      "lang": "de",
      "kind": "smalltalk",
      "text": "Wie wird das Wetter morgen?",
      "expect": "none",
      "selfTest": true
    }
  ]
}
```

Regeln:

- `lang` ∈ `de`, `en`; `kind` ∈ `read`, `change`, `smalltalk`.
- `expect` ist `"none"` (kein Werkzeugaufruf erwartet) oder eine nicht leere
  Liste. Die Reihenfolge der Liste zählt nicht. Der Satz bewertet nur den
  **ersten Schritt** des Modells (ein Aufruf oder Text), keine Mehrschritt-Ketten.
- `args` enthält nur die Felder, auf die es ankommt; zusätzliche, schema-gültige
  Felder des Modells schaden nicht.
- `selfTest: true` markiert die Teilmenge (≈ 5 Sätze, jeweils Deutsch und
  Englisch, mit mindestens einem `smalltalk`), die der Selbsttest in der App
  nutzt.
- Zielwerte, die von Laufzeitdaten abhängen (Tab-IDs), kommen nicht vor: Sätze
  mit Ziel bewerten nur die Wahl der Aktion und der festen Felder.
- Jeder `expect.tool` muss in `tools.json` vorkommen (Prüfung in
  `check:agent-actions`).
- Erhöhung von `version` bei jeder inhaltlichen Änderung; der Selbsttest wird
  dann einmal neu durchgeführt, wenn die Änderung die `selfTest`-Teilmenge
  betrifft.

## Bewertung je Satz

| Ergebnis      | Bedingung                                                                                                                                                            |
| ------------- | -------------------------------------------------------------------------------------------------------------------------------------------------------------------- |
| `pass`        | erwartet `expect` als Liste: jede erwartete Aktion genau einmal gerufen, Eingaben schema-gültig und die erwarteten `args` stimmen; bei `"none"`: kein Werkzeugaufruf |
| `wrong_tool`  | andere Aktion als erwartet                                                                                                                                           |
| `bad_args`    | richtige Aktion, Eingaben ungültig oder falsch                                                                                                                       |
| `missed`      | erwartet Aufruf, Modell antwortet nur mit Text                                                                                                                       |
| `spurious`    | erwartet `"none"`, Modell ruft trotzdem ein Werkzeug                                                                                                                 |
| `not_offered` | erwartete Aktion war **nicht** im Angebot der Auswahl (zählt für den Recall, nicht gegen das Modell)                                                                 |

## `EvalReport`

```json
{
  "setVersion": 1,
  "model": "qwen3-4b-instruct-q4_k_m",
  "total": { "pass": 22, "of": 28, "rate": 0.786 },
  "perLang": {
    "de": { "pass": 11, "of": 14, "rate": 0.786 },
    "en": { "pass": 11, "of": 14, "rate": 0.786 }
  },
  "perKind": {
    "read": { "...": "..." },
    "change": { "...": "..." },
    "smalltalk": { "...": "..." }
  },
  "selectionRecall": 0.964,
  "spuriousCalls": 1,
  "failures": [
    {
      "id": "change-scheme-en-1",
      "result": "bad_args",
      "got": { "tool": "...", "args": {} }
    }
  ]
}
```

- `rate` = `pass / of` ohne `not_offered`-Sätze im Nenner des Modells;
  `selectionRecall` = Anteil der Sätze mit erwarteter Aktion, bei denen die
  Auswahl sie angeboten hat (SC-005).
- Zwei Läufe mit demselben Modell und festem Sampler weichen um höchstens 10
  Prozentpunkte in `total.rate` ab (SC-008). Bei Modellen, die deterministisches
  Sampling nicht zulassen (Cloud), ist das die Mess­grenze und der Bericht
  vermerkt `"deterministic": false`.
