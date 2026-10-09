# Evaluation results (T038)

## Qwen3-4B

Measured on 2026-10-09 with Q4_K_M on CUDA (RTX A2000), deterministic settings, eval set version 3
(43 sentences) and the tool instruction of the application, including the `ask_user` sentence. The
core offer includes `wm.apps.list` again (research R16). One run took 42 seconds; two runs produced
an identical report.

|             | Pass | Of  | Rate  |
| ----------- | ---- | --- | ----- |
| Total       | 25   | 43  | 58 %  |
| German      | 13   | 22  | 59 %  |
| English     | 12   | 21  | 57 %  |
| `read`      | 8    | 12  | 67 %  |
| `change`    | 11   | 19  | 58 %  |
| `smalltalk` | 6    | 6   | 100 % |
| `clarify`   | 0    | 6   | 0 %   |

`reachRate` was 0.73, with one search step (`extraSteps` 1) and no spurious call.

### SC-005: met

All eight sentences that open an app end with a call of `wm_app_open`; none ends with a refusal.
The six sentences with a typo pass. `change-open-de-1` („Öffne die Einstellungen.“) counts as
`bad_args` because the model passed the name `Einstellungen` instead of the id `system.settings`;
since research R5 `wm.app.open` resolves that name to the same app, so in the application it
opens the settings. The scoring still compares the id exactly.

### SC-006: not met

The model called `ask_user` in none of the six ambiguous sentences:

- Three times it asked in plain text instead („Welche Farbe möchtest du verwenden? … 1. Hell 2. Dunkel 3. System“, its English counterpart, and „Könntest du bitte klarer sagen …“). The
  person can answer such a question in the chat, but gets no choice dialog.
- Twice it guessed: „Mach es dunkler.“ and „Make it darker.“ set the dark color scheme.
- Once („Switch it over.“) it searched with `find_actions`.

A sharper instruction („Never write such a question as text: whenever you would ask the user to
choose, call ask_user.“) left every result unchanged and was reverted. This is the case the spec's
assumptions anticipated: small local models do not ask on their own reliably. The fixed path of
User Story 2 (the choice from `wm.app.open`) does not depend on the model and is covered by the
automated tests.

## Not measured

No Claude model was measured, as no API key was available in the measurement environment.
