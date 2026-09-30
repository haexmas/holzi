# Research: Modell bedient holzi über die Aktionen

**Spec**: [spec.md](./spec.md) | **Plan**: [plan.md](./plan.md) | **Stand**: 2026-09-30

Jede Entscheidung: **Entscheidung**, **Begründung**, **verworfene Alternativen**.
Belege sind Dateien im Repository (Stand `main` am 2026-09-30). Wo etwas aus
allgemeinem Wissen und nicht aus dem Repository stammt, steht es dabei.

## R1 — Die Aktionen kommen einzeln als Werkzeuge ins Rust-Register, die Definitionen schiebt das Frontend hinein

**Entscheidung**: Das Frontend ist die einzige Quelle der Aktionsdefinitionen
(`src/lib/actions/catalog.ts`). Nach dem Start der Tresor-Sitzung (und bei
Sprachwechsel) ruft es den neuen Command `set_agent_actions` mit allen für den
eingebauten Agenten aufrufbaren Definitionen auf. Rust legt je Aktion ein
`ActionTool` im bestehenden `ToolRegistry` an (Quelle `action`). Ausgeführt
wird eine Aktion über den Umlauf Rust → Ereignis `action-call-request` →
Frontend `runAction(id, input, { kind: 'builtinAgent' })` → Command
`respond_action_call` → Rust (R2).

**Begründung**: FR-001 verbietet eine zweite Werkzeugbeschreibung; der Katalog
liegt in TypeScript, die Schleife in Rust (`chat/tools/mod.rs`). Ein
`ActionTool` je Aktion gibt dem Modell flache Schemas mit dem echten
Eingabeschema der Aktion. Das ist für kleine Modelle entscheidend.

**Verworfen**:

- _Zwei Meta-Werkzeuge_ (`list_actions`, `call_action(id, input)`): verschachtelte
  Argumente, das Modell sieht das Schema der Zielaktion nicht. Kleine Modelle
  scheitern daran deutlich öfter.
- _Katalog in Rust duplizieren_: zwei Wahrheiten, Drift garantiert; verletzt FR-001.
- _Weg über MCP im selben Prozess_: Serialisierung und zusätzlicher Server ohne
  Nutzen (siehe ADR-0006, Annahmen der Spec); Spec 021 baut den MCP-Zugang auf
  derselben Definition.

## R2 — Der Umlauf Rust ↔ Frontend folgt dem bestehenden Freigabe-Muster

**Befund**: Ein „Rust ruft TS und wartet auf den Rückgabewert“ gibt es im Code
nicht. Vorbilder für „Ereignis senden, auf Command-Antwort warten“:
`pending_tool_approvals` + `respond_tool_permission`
(`chat/turn/tool_round.rs`, `chat/commands.rs`) und `sync/link/host_task.rs`
(mit `DECISION_TIMEOUT` und `select!` gegen Abbruch). Es gibt keine
Webview-Bereitschaftsprüfung; `emit` meldet keinen Fehler ohne Listener. Es
gibt genau ein Fenster (`main`), Chat und Window Manager teilen eine Webview.

**Entscheidung**: Neues Modul `chat/tools/action_bridge.rs` mit

- einem `EventEmitter` (`Arc<dyn Fn(&str, Value) + Send + Sync>`, wie in
  `adapters/cli_delegate/mod.rs`), der in `setup()` gesetzt wird (`ChatState`
  wird ohne `AppHandle` verwaltet, `lib.rs:133`),
- einer eigenen `HashMap<Uuid, oneshot::Sender<ActionReply>>`,
- einem Timeout von 60 s (Tab-Handler warten bis zu 5 s,
  `TAB_HANDLER_TIMEOUT_MS`; Aktionen wie Modell-Download sollen nicht
  sinnlos abbrechen) und `select!` gegen das Abbruch-Token des Zuges,
- einer Sperre (`tokio::sync::Mutex`), die das _Ausführen_ der Aktionen
  serialisiert, weil `execute_plans` die Aufrufe einer Runde mit `join_all`
  gleichzeitig startet und zwei gleichzeitige Fenster-Aktionen sich sonst
  ins Gehege kommen.

Der Listener liegt **global** im Frontend (neues Plugin
`src/plugins/agentActions.client.ts`), nicht in `ChatApp.vue`: dessen
Listener existieren nur, solange ein Chat-Tab gemountet ist, und Aktionen wie
`wm.app.open` sollen auch dann laufen, wenn der Chat-Tab im Hintergrund liegt.

**Fehlerpfade**: Webview nicht bereit oder keine Antwort → Timeout →
Werkzeugfehler `action_timeout`. Tresor schließt (`reset_for_close`) →
ausstehende Absender werden verworfen → `tool_call_cancelled` (wie heute bei
Freigaben). Doppelte oder späte Antwort → stilles Nichts (Muster
`cancelled_tool_approvals`).

**Verworfen**: Ergebnis über Dateien/DB austauschen (Latenz, Aufräumen);
`eval`/`emit_to` mit Rückgabewert (Tauri 2.11 hat keinen).

## R3 — Werkzeugnamen: Punkte ersetzen

**Befund**: Aktions-IDs enthalten Punkte (`wm.tab.back`). Anthropic erlaubt
Werkzeugnamen nur aus `[a-zA-Z0-9_-]` mit höchstens 64 Zeichen (allgemeines
Wissen, nicht im Repository belegt). Im Repository gibt es keine
Namensbereinigung; bestehende MCP-Namen (`mcp:server:name`) verstoßen schon
dagegen, das ist nicht Teil dieser Spec.

**Entscheidung**: `toolName = id.replaceAll('.', '_')`. IDs verwenden
camelCase und keine Unterstriche, die Abbildung ist damit eindeutig und
umkehrbar; ein Prüfskript (`check:agent-actions`) erzwingt Eindeutigkeit,
Zeichenmenge und Länge ≤ 64 für alle Katalog-IDs. Die Abbildung ist eine
reine TS-Funktion (`src/lib/actions/agentTools.ts`); Rust erhält den Namen
fertig mit der Definition.

**Verworfen**: Namenstabelle in Rust (zweite Stelle); `__` als Trenner
(länger, kein Mehrwert).

## R4 — Risiko: drei Wirkungsarten statt zwei

**Befund**: `RiskClass { Safe, Risky }` (`chat/tools/mod.rs:26`) und
`decide(mode, risk)` (`chat/tools/permission.rs`). Verwendet in `events.rs`,
`adapters/cli_delegate/approval_bridge.rs`, `cli.rs`, `mcp.rs`, in Tests und im
Frontend (`useChat.ts`, `PermissionPrompt.vue`).

**Entscheidung**: `RiskClass` bekommt die Stufe `Change` zwischen `Safe` und
`Risky`. Abbildung: `read` → `Safe`, `write` → `Change`, `destructive` →
`Risky`. Matrix:

| Modus   | Safe  | Change | Risky |
| ------- | ----- | ------ | ----- |
| Manuell | fragt | fragt  | fragt |
| Auto    | läuft | läuft  | fragt |
| Plan    | läuft | Abl.   | Abl.  |

`run_command` und MCP-Werkzeuge bleiben `Risky`, ihr Verhalten ändert sich
nicht (Spec, Annahmen). Die Wire-Zeichenkette heißt `"change"`; das Frontend
(`RiskClass` in `useChat.ts`, `PermissionPrompt.vue`) kennt sie dann. Für
CLI-Delegates bleibt die Freigabe-Brücke unverändert: die Delegates
klassifizieren selbst, der `match` in `approval_bridge.rs` wird um die neue
Stufe ergänzt (Ergebnis wie `Safe` → unverändert).

**Zusatz `alwaysAsk`**: Drei Aktionen sind als `write` markiert, sind aber
teuer und laden Gigabyte aus dem Netz (`settings.models.downloadCatalog`,
`settings.models.downloadFromHf`, `settings.models.installUpdate`). Ohne
Gegenmaßnahme liefen sie im Modus „Auto“ ohne Rückfrage. Der Katalog bekommt
das optionale Feld `alwaysAsk: true`; der Umlauf ordnet solche Aktionen
`Risky` zu, egal welche Wirkungsart sie haben. Das ändert an Spec 020 nur ein
optionales Feld und hilft Spec 021 später ebenfalls. **Dies ist eine
Ergänzung des Plans, nicht der Spec** — im PR ausdrücklich zur Prüfung
markiert.

**Verworfen**: Wirkungsart der drei Aktionen auf `destructive` setzen (falsche
Aussage über die Daten, verändert 020); alle `write` fragen lassen (nimmt
Auto seinen Sinn, widerspricht der Klärung vom 2026-09-30).

## R5 — Chat-Aktionen, die sich selbst auslösen würden

**Befund**: `chat.message.send`, `chat.message.retry` und `chat.reply.cancel`
sind für Agenten aufrufbar (Spec 020, für externe Agenten gedacht). Läuft
ein eingebauter Zug, würde `send_message` durch `acquire_operation()`
abgelehnt; `chat.reply.cancel` würde den eigenen Zug abbrechen.

**Entscheidung**: Der Katalog bekommt das optionale Feld
`builtinAgentCallable` (Standard `true`); diese drei Aktionen setzen es auf
`false`. Sie bleiben für externe Agenten (021) unverändert aufrufbar. Der
Runner lehnt den Aufruf mit Aufrufer `builtinAgent` dann mit dem vorhandenen
Code `forbidden_for_agents` ab, und `set_agent_actions` enthält sie nicht.

**Verworfen**: Sie im Bridge-Code ausblenden (versteckte zweite Liste);
`agentCallable` auf `false` setzen (sperrt sie auch für 021).

## R6 — Werkzeug-Auswahl pro Antwort (FR-011 bis FR-013)

**Befund**: Werkzeuge werden in `send_message` (`commands.rs` ~438–446) einmal
je Zug in `ChatRequest.tools` gesetzt (`tool_specs()`, `commands.rs:189`);
`append_round_to_request` ändert sie nicht. Der Lookup einer Werkzeug-Anfrage
läuft gegen das **Register**, nicht gegen die angebotene Liste
(`tool_round.rs:plan_calls`).

**Entscheidung**: Neues Modul `chat/tools/select.rs` mit der reinen Funktion
`select_tools(defs, context_text, limit) -> Vec<ToolSpec>`. Ein Zug ruft sie
einmal auf. Verfahren (deterministisch, ohne Modellaufruf):

1. Immer angeboten: ein kleiner Kern lesender Aktionen (`wm_state_get`,
   `wm_apps_list`, `settings_get`) und das Meta-Werkzeug `list_actions`.
2. Rest nach Trefferzahl: Wörter der letzten Nutzernachricht (und der
   vorherigen, als „Lage“) gegen die Wörter aus ID-Segmenten (camelCase
   zerlegt), Beschreibung und den Titeln in Deutsch und Englisch, die das
   Frontend mitliefert. Gleichstand: Bereich des zuletzt genutzten
   Werkzeugs, dann stabile ID-Reihenfolge.
3. Auf die Obergrenze kürzen. Startwert: 10 für lokale Modelle, 24 für
   API-Key-Anbieter; je Modell in den Fähigkeiten überschreibbar
   (`toolUse.maxTools`).

`list_actions(query?)` liefert ID, eine Zeile Beschreibung und das
Eingabeschema der passenden Aktionen aus dem vollständigen Register. Ruft das
Modell danach eine nicht angebotene, aber registrierte Aktion, läuft sie: der
Lookup geht gegen das Register (FR-013).

**Grenze** (`ponytail:`-Kommentar am Code): „Lage“ ist in der ersten Fassung
nur der Gesprächstext, nicht die Vordergrund-App; die Erweiterung ist ein
weiteres Feld im Aufruf.

**Verworfen**: Embedding-Suche (neue Abhängigkeit, Start-Latenz, für ≈ 60
Aktionen unverhältnismäßig); zweiter Modellaufruf zur Auswahl (Kosten,
Nichtdeterminismus, bei lokalen Modellen doppelte Wartezeit).

## R7 — Fähigkeit „Werkzeugnutzung“

**Befund**: `ModelCapabilities` (`model_capabilities.rs`) ist als
`capabilities_json` in `models` gespeichert (Migration
`0018_models_add_capabilities`); neue Felder lesen sich ohne Migration als
`None` (`#[serde(default)]`). `models` ist eine CRDT-Tabelle: der Wert gilt
tresorweit und synchronisiert. `upsert_model` überschreibt
`capabilities_json` bei Konflikt vollständig. Anthropic liefert kein
Werkzeugfeld (`anthropic_capabilities.rs`).

**Entscheidung**: Neues Feld `tool_use: Option<ToolUse>` mit
`ToolUse { support: Supported | Unsupported, basis: Provider | Curated |
Template | SelfTest, max_tools: Option<u32> }`; `None` heißt „unbekannt“.

- **Anthropic**: `map_capabilities` setzt `Supported`/`Provider`. Die Wire-API
  kennt kein Feld; alle aktuellen Claude-Modelle unterstützen Werkzeuge
  (allgemeines Wissen). Ein Anbieter, der das Gegenteil meldet, hat Vorrang
  (FR-015): derzeit gibt es dafür kein Signal, der Code ist dafür vorbereitet.
- **Lokal**: `ModelCapabilities::local()` setzt `tool_use` aus dem
  Katalogeintrag (`CatalogEntry.tool_use`, neues optionales Feld, nach dem
  ersten Messlauf gefüllt), sonst `None`.
- **Schreiben** nach Probe oder Selbsttest über eine neue Funktion
  `storage::models::set_tool_use(tx, id, ToolUse)`, die das Feld in der
  bestehenden JSON ersetzt, statt die ganze Zeile zu überschreiben. Ein
  erneuter Download derselben Datei setzt das Feld zurück (gewollt: neue
  Datei, neuer Test, FR-018 / Szenario 8).
- **Keine Migration**, aber alle Struct-Literale anpassen (`..Default::default()`).

**Verworfen**: eigene Tabelle/Spalte (Migration und Trigger-Version
`HOLZI_TRIGGER_VERSION` 13 → 14 ohne Not); Wert nur im Speicher (verliert das
Ergebnis bei jedem Start, Selbsttest liefe immer wieder).

## R8 — Vorlagenprüfung bei lokalen Modellen (FR-018a)

**Befund**: mistralrs 0.8.1 hat **kein** `supports_tools`. Die einzige Prüfung
steckt in `apply_chat_template_to`: bei einer Vorlage aus benannten Teilen ohne
`tool_use` gibt es den Fehler „does not handle tool usage“; eine Vorlage als
Zeichenkette bekommt `tools` übergeben und **ignoriert sie stillschweigend**,
wenn sie nicht darauf verweist. `Model::tokenize(text, tools, …)` und
`detokenize` sind öffentlich.

**Entscheidung**: `llm/local/probe.rs` (hinter `#[cfg(feature = "llm-cpu")]`)
rendert eine feste Testnachricht einmal ohne und einmal mit einem Dummy-Werkzeug
über `tokenize` + `detokenize`. Gleicher Text oder der „does not handle tool
usage“-Fehler → `Unsupported`/`Template`. Sonst bleibt der Wert offen (die
Vorlage _kann_ Werkzeuge, ob das Modell sie gut nutzt, klärt R9). Der Aufruf
läuft einmal beim ersten Laden eines Modells mit `tool_use == None`
(`chat/model_loading.rs`).

**Verworfen**: Vorlagentext selbst nach `tools` durchsuchen (kein
Zugriff auf die Vorlage über die öffentliche API, `ChatTemplate` ist
pipeline-intern).

## R9 — Selbsttest und Messlauf teilen dieselbe Bewertung (FR-018b, FR-019 bis FR-022)

**Befund**: Kein Test ruft heute ein echtes Modell mit Werkzeugen
(`tests/local_inference.rs` setzt `tools: Vec::new()`, ist `#[ignore]` und
braucht `HOLZI_TEST_GGUF`). `LocalModel::load` und `stream_chat` laufen ohne
Tresor. mistralrs bietet `set_deterministic_sampler`,
`set_sampler_temperature`, `set_tool_choice` und `set_constraint` (Regex,
Lark, JSON-Schema, Llguidance).

**Entscheidung**: Neues Modul `chat/eval/` mit

- dem versionierten Satz `eval_set.json` (eingebettet per `include_str!`),
  Format in [contracts/eval-format.md](./contracts/eval-format.md),
- `tools.json`, einem Schnappschuss der Werkzeugdefinitionen, den
  `scripts/export-eval-tools.ts` aus dem Katalog erzeugt; `check:agent-actions`
  prüft, dass er aktuell ist (der Test misst so das echte, nicht ein
  veraltetes Angebot),
- `scoring.rs` (reine Bewertung: richtige Aktion, gültige und richtige
  Eingaben, keine unnötigen Aufrufe, Quoten gesamt, je Sprache, je Art,
  Recall der Auswahl),
- `runner.rs`: nimmt ein `Arc<dyn ProviderAdapter>`, ruft je Satz **einen**
  Schritt mit der Auswahl aus R6 (deterministischer Sampler, wo der Adapter es
  zulässt), wertet den ersten Werkzeugaufruf bzw. den Text aus. Ersatzhandler
  gibt es nicht: der Lauf führt nie eine Aktion aus, er bewertet nur den
  Aufruf, darum kann er keine Daten verändern (FR-021).

Einstiege:

- **Selbsttest in der App**: `chat/tools/selftest.rs` startet als
  Hintergrundaufgabe (`tokio::spawn`) mit einer festen Teilmenge (Markierung
  `selfTest: true` im Satz, ≈ 5 Sätze), bricht ab, wenn die Sitzung das Modell
  wechselt, schreibt das Ergebnis über `set_tool_use`, sendet
  `model-tool-use-updated`.
- **Vollständiger Messlauf**: `tests/model_tool_eval.rs`, `#[ignore]`, liest
  `HOLZI_TEST_GGUF` (lokal) oder `HOLZI_EVAL_PROVIDER` + Schlüssel aus der
  Umgebung (Cloud), schreibt den Bericht nach `target/eval/<modell>.json`.
  Kein Tresor nötig (FR-022). Schlüssel stehen nie im Repository.

**Schwellen**: Selbsttest „unterstützt“ ab der Mindestquote aus dem ersten
Messlauf (Annahme der Spec); bis dahin ein Platzhalter `SELF_TEST_PASS =
0.6`, markiert mit `ponytail:`-Kommentar und Verweis auf Aufgabe T046.

**Verworfen**: Den kompletten Zug (`run_turn`) durchlaufen lassen (braucht
Tresor und Datenbank, langsamer, misst die Schleife statt das Modell);
Constrained Decoding im Eval erzwingen (würde den Messwert verfälschen, weil
der Lauf in der App ohne es arbeitet — siehe R10).

## R10 — Constrained Decoding bleibt außerhalb dieser Spec

**Befund**: mistralrs kann Werkzeugaufrufe per Grammatik erzwingen
(`set_constraint`, `set_tool_choice`), holzi nutzt das bisher nicht. Die
Aufgabenliste aus Spec 003 (T014) versprach `set_tool_choice`, im Code ist es
nicht zu finden.

**Entscheidung**: Nicht Teil von 032. Der erste Messlauf (T046) zeigt, ob die
Quoten ohne Erzwingung reichen. Falls nicht, wird Constrained Decoding eine
eigene kleine Spec; der Runner aus R9 bekommt dann einen Schalter, um beide
Varianten zu messen. Vermerkt in den Annahmen des Plans.

**Begründung**: Erst messen, dann eine Maßnahme einführen, die in jedem
Adapter anders aussieht (Anthropic braucht sie nicht).

## R11 — Hinweise „einmalig je Unterhaltung“ (US4, FR-016/017/023)

**Befund**: Es gibt keinen Mechanismus für Hinweiszeilen. Vorhanden:
vorübergehende Ereignisse (`chat-retry`, `chat-message-error`), das
`StatusBanners.vue`-Banner, und die Rolle `System` in `chat_messages`, die
niemand anlegt und die `history_to_messages` verwirft. Backend-Texte dürfen
nicht lokalisiert sein (`CONTEXT.md`).

**Entscheidung**: `send_message` bestimmt je Zug den Zustand
`ToolAvailability = Offered | OfferedUnverified | Unsupported | Delegate`
und sendet das Ereignis `chat-tool-availability { threadId, state }`. Das
Frontend zeigt für `OfferedUnverified`, `Unsupported` und `Delegate` ein
schließbares Hinweisbanner in `StatusBanners.vue` (Schlüssel
`chat.toolNotice.*` in `de.json` und `en.json`), **einmal je Unterhaltung und
Zustand pro App-Sitzung** (Menge im Speicher, `ponytail:`-Kommentar: nach einem
App-Neustart erscheint der Hinweis erneut; Upgrade-Pfad ist eine
Systemzeile in `chat_messages`).

**Begründung**: Kein Schema, kein Sync von Hinweistexten, kein Eingriff in den
Verlauf, den das Modell nie sieht. Die Abweichung von „einmalig je
Unterhaltung“ (Neustart zeigt erneut) ist klein und benannt.

**Verworfen**: `System`-Zeile im Verlauf (Sync über CRDT, Marker-Text plus
Lokalisierung im Frontend, mehr Fläche für Fehler).

## R12 — Freigabe-Dialog in Klartext (FR-008)

**Befund**: `PermissionPrompt.vue` zeigt `toolName`, `toolInput` und
`riskClass`, Titel „Approve: {name}“ (`chat.permission.requestTitle`).

**Entscheidung**: Für Werkzeuge der Quelle `action` schlägt das Frontend die
Aktion über die umgekehrte Namensabbildung (R3) nach und zeigt den
lokalisierten Titel (`actions.<id>`), das Ziel (Tab-/Fenster-/Arbeitsbereichs-
Titel aus dem Store) und die Eingaben als Liste „Feld: Wert“. Kein Backend-
Text. `riskClass` zeigt „Änderung“ für `change`.

## R13 — Ergebnisse und Fehler ohne Geheimnisse und Interna (FR-006, FR-010, SC-004)

**Entscheidung**:

- `respond_action_call` gibt an das Modell nur `ok`/`result` oder `code`,
  `field` und `message` weiter; das Feld `error` der Runner-Antwort (roher
  Fehler) wird nie gesendet. Bei Code `failed` ersetzt ein fester Text die
  Meldung des Handlers (er kann Pfade oder Interna enthalten).
- `check:agent-actions` prüft für jede für den eingebauten Agenten aufrufbare
  Aktion, dass Ergebnisschema und Eingabeschema keine Felder mit Namen wie
  `secret|private|password|passphrase|token|apiKey|credential` enthalten
  (Groß-/Kleinschreibung egal; Ausnahmeliste begründet und kurz). Die 9
  Leitplanken-Aktionen sind bereits gesperrt.
- `settings.devices.identity` gibt laut Beschreibung nur öffentliche
  Schlüssel zurück; ein Test prüft die Antwortform.

**Grenze**: Das Schema-Prüfen fängt nur Felder, die im Ergebnisschema
stehen. Werte, die ein Handler über ein allgemeines Feld durchreicht, fängt es
nicht — das ist dieselbe Grenze wie bei Spec 020 FR-032; der Test bleibt
Stichprobe über den Katalog.

## R14 — CLI-Delegates (FR-023)

**Befund**: `send_message` kennt `session.provider_kind`
(`commands.rs` ~510); Delegates ignorieren `req.tools` und emittieren nie
`ToolCalls`.

**Entscheidung**: Bei `ProviderKind::CliDelegate` bleibt `ChatRequest.tools`
leer (wie heute, jetzt aber ausdrücklich), es läuft weder Auswahl noch
Fähigkeitsprüfung, und der Zustand `Delegate` geht an das Frontend (R11).
Kein neuer Code in `adapters/cli_delegate/`.

## R15 — ADR

**Entscheidung**: ADR-0006 „Aktionen als Werkzeuge des eingebauten Agenten:
im Prozess, nicht über MCP“. Die Nummer 0005 bleibt für Spec 021
reserviert (Spec 020 verweist darauf). Inhalt: eine Definition, zwei
Eingänge; Risiko-Abbildung auf drei Stufen; `alwaysAsk` und
`builtinAgentCallable`; was 021 übernimmt.

## R16 — Offene Stellen, bewusst nicht gelöst

- **Constrained Decoding** (R10): erst nach Messung.
- **Katalog der Empfehlungsliste** (nur Qwen3 0.6B/1.7B/4B): Aufnahme eines
  größeren Modells entscheidet sich nach dem ersten Messlauf, nicht hier.
- **Vordergrund-App als Lage** (R6).
- **Hinweis nach Neustart** (R11).
- **Aktionen aus haextensions** (017–019): erscheinen später als weitere
  Quelle im Register; `source` ist ein freier Text in `chat_messages.tool_source`
  (kein CHECK), das TS-Typfeld `toolSource` wird auf `'mcp' | 'cli' | 'action'`
  erweitert.
