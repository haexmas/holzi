# Research: Mehrfachinstanzen für Apps

## R1: Umfang der Änderung

**Decision**: Nur `src/lib/wm/apps.ts` ändern (`system.chat`:
`multiInstance: false` → `true`); keine Änderung an `openApp`, `addTab`,
`hydrate`, `NewTabMenu.vue` oder der Sitzungspersistenz (spec 022).

**Rationale**: Spec 015 (User Story 7, FR-016) hat den Mehrfachinstanz-Fall
bereits beim Bau der Shell mitgedacht und generisch implementiert:

- `src/lib/wm/apps.ts`s eigener Doku-Kommentar sagt wörtlich, der `apps`-
  Parameter jedes Reducers existiere, „so a test can substitute exactly this
  [eine `multiInstance: true`-App] (T052)" — ohne die ausgelieferte Registry
  anzufassen.
- `scripts/check-wm-state.ts` (Abschnitt „Multi-instance apps (User Story 7,
  T051/T052)") hat bereits eine synthetische `multiInstance: true`-App und
  beweist damit: `openApp` öffnet bei jedem Aufruf ein neues Fenster,
  `addTab` hängt einen zweiten Tab an statt anderswo zu aktivieren, und
  `hydrate` stellt mehrere Fenster mit derselben mehrfachen App-Id wieder
  her, ohne sie zusammenzuführen.
- `src/components/wm/NewTabMenu.vue`s `isOpenElsewhere()` gibt für
  `multiInstance` sofort `false` zurück — das „bereits geöffnet"-Label
  (FR-004 dieser Spec) erscheint für Chat schon heute nie, sobald das Flag
  steht.
- `src/components/wm/Launcher.vue` hat gar keine Einzelinstanz-Sonderlogik;
  es ruft nur `openApp` auf, das die Verzweigung schon in `layoutState.ts`
  trägt.
- Chat-Identität (aktuelle Unterhaltung, Navigation) lebt bereits pro Tab
  (`useChatTab.ts`/`useChat.ts` als gewöhnliche, pro Komponenteninstanz neu
  aufgerufene Composables, keine globale Store-Singleton für den
  Gesprächszustand) — nur die Modell-/Anbieterliste selbst ist bewusst ein
  Pinia-Singleton (`stores/models.ts`), was richtig ist: alle Chat-Tabs
  MÜSSEN dieselben installierten/verbundenen Modelle sehen.

Eine gezielte Suche nach `singleton`/`Einzelinstanz` in `src/` fand keine
weitere Stelle, die eine feste Anzahl von Chat-Tabs voraussetzt.

**Alternatives considered**:

- **Neue Dedizierte Prüfung, ob `wm.app.open`/`wm.tab.new` ihre
  Beschreibung anpassen müssen** (spec.md FR-007): Geprüft — die
  bestehenden Beschreibungen in `src/lib/actions/wmActions.ts` sagen bereits
  nur „a single-instance app that is already open is activated ... instead"
  (bedingt auf Einzelinstanz-Apps), nicht, dass alle Apps so funktionieren.
  Keine Textänderung nötig; als Regressionsprüfung in Tasks aufgenommen statt
  als Codeänderung.
- **`openApp`/`addTab`/`hydrate` erneut anfassen**: verworfen — sie sind
  bereits generisch und durch T051/T052 abgedeckt; eine Änderung dort wäre
  unnötiges Risiko für bereits ausgeliefertes, getestetes Verhalten
  (ponytail: kleinste sichere Änderung).

## R2: Spec-Amendments statt stiller Umschreibung

**Decision**: FR-017 in Spec 015 und der Assumptions-Eintrag in Spec 020
bekommen je einen Änderungsvermerk im selben Stil, den Spec 022 bereits für
FR-023 aus Spec 015 verwendet hat: ein Blockzitat („Seit Spec 030 ...")
direkt vor der betroffenen Anforderung, der ursprüngliche Wortlaut bleibt
als historischer Stand erhalten.

**Rationale**: Der historische Wortlaut („Chat, Einstellungen und
Föderation MÜSSEN in dieser Spec Einzelinstanz-Apps sein") war zum
Zeitpunkt von Spec 015 richtig; ihn zu löschen würde die Nachvollziehbarkeit
zerstören, warum Chat je Einzelinstanz war. Spec 022 hat für genau diesen
Fall (eine spätere Spec ändert eine FR einer früheren) schon ein Muster
etabliert.

**Alternatives considered**:

- **FR-017 direkt umschreiben** (Chat aus der Liste streichen): verworfen,
  weil es keine Spur hinterlässt, dass sich das Verhalten je geändert hat,
  und vom etablierten Amendment-Muster (Spec 022) abweicht.
