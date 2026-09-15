# Handoff: die drei großen 500-LoC-Splits

Vorbereitet am 2026-09-15 am Ende des Code-Reviews (PR #67). Dieser Text ist
als Prompt für eine frische Session gedacht — er nennt nur, was dort nicht aus
dem Code selbst hervorgeht.

---

## Prompt

> Wir gehen die drei verbliebenen 500-LoC-Splits an. Jede der drei Dateien hat
> einen symbolgenauen, geordneten Split-Plan in ihrem eigenen Datei-Header —
> lies ihn zuerst und folge ihm, statt einen neuen zu erfinden. Die Pläne
> stammen aus dem Review vom 2026-09-15 und wurden gegen den echten Code
> geschrieben.
>
> Reihenfolge und Begründung:
>
> 1. **`src-tauri/tests/chat_tool_loop.rs` (1850)** — zuerst, weil es reine
>    Testdatei ist und nichts anderes blockiert. Fixture nach
>    `tests/common/tool_loop_fixture.rs`, eingebunden per
>    `#[path = "common/tool_loop_fixture.rs"] mod tool_loop_fixture;`, dann in
>    drei Binaries teilen.
>
> 2. **`src-tauri/src/chat/commands.rs` (3032)** — fünf mechanische Schritte in
>    der im Header genannten Reihenfolge, jeder für sich committet, die
>    registrierte Command-Oberfläche unverändert. `events.rs` muss zuerst,
>    sonst dupliziert jeder spätere Schritt die Payload-Structs.
>
> 3. **`src/pages/chat/[instance].vue` (1964) + `scripts/check-chat-state.ts`
>    (665)** — zuletzt und **nur in dieser Reihenfolge**: der Harness
>    extrahiert den `<script setup>`-Block der Seite per Regex und strippt
>    jede `import`-Zeile. Wird vorher etwas in ein Composable ausgelagert,
>    bleiben die 19 Replay-Tests grün und prüfen nichts mehr. Also erst den
>    Harness auf echte Imports umbauen, dann die Seite aufteilen.
>
> Arbeite im Worktree auf einem Topic-Branch (spaex-Constitution). Verifiziere
> nach jedem Schritt mit der Kommandoliste unten — die Testzahlen dürfen sich
> bei einem reinen Move **nicht** ändern; genau das ist der Beweis, dass nichts
> verlorenging.

---

## Ausgangslage

PR #67 ist der Stand, auf dem das aufsetzt. Falls noch offen: erst mergen.

| Datei                                                  | LoC  | Plan steht in |
| ------------------------------------------------------ | ---- | ------------- |
| `src-tauri/src/chat/commands.rs`                       | 3032 | Datei-Header  |
| `src/pages/chat/[instance].vue`                        | 1964 | Datei-Header  |
| `src-tauri/tests/chat_tool_loop.rs`                    | 1850 | Datei-Header  |
| `scripts/check-chat-state.ts`                          | 665  | Datei-Header  |
| `src/components/models/HuggingFaceModelManagement.vue` | 624  | Datei-Header  |

`models/huggingface.rs` (1248) und `models/commands.rs` (952) haben eigene,
ältere Ausnahmen mit Plan und sind hier nicht gemeint.

## Was beim Verifizieren zählt

```sh
pnpm check:chat-state      # 19 Tests
pnpm check:templates       # 24 Templates
pnpm typecheck
pnpm typecheck:scripts
pnpm lint
pnpm format:check
pnpm generate
cargo test  --manifest-path src-tauri/Cargo.toml                        # 227 / 3 ignored
cargo test  --manifest-path src-tauri/Cargo.toml --no-default-features  # 214
cargo clippy --manifest-path src-tauri/Cargo.toml --all-targets -- -D warnings
cargo fmt   --manifest-path src-tauri/Cargo.toml -- --check
```

Bei einem reinen Move müssen diese Zahlen exakt gleich bleiben. Ändert sich
eine, ist etwas verlorengegangen oder doppelt registriert.

## Fallstricke, die Zeit kosten, wenn man sie nicht kennt

**Der Harness ist die Achillesferse.** Siehe Punkt 3 oben. Er wirft seit dem
Review eine sprechende Meldung, wenn seine Regex nicht mehr greift — das ist
die Frühwarnung, keine Absicherung.

**Testdateien gehören neben das Modul**, nicht in ein `#[cfg(test)] mod tests
{ … }`. Ein einzeiliges `#[cfg(test)] #[path = "x_tests.rs"] mod tests;` im
Produktionsmodul ist erlaubt und der richtige Weg, wenn die Tests private
Items brauchen — sonst müsste man Produktionssichtbarkeit nur für den Test
aufweiten.

**Mehrzeilige Inline-Handler in Vue-Templates sind verboten.** Prettier
(`semi: false`) entfernt genau die `;`, die Vues Template-Parser braucht.
Immer eine benannte Funktion im `<script setup>`. `pnpm check:templates` fängt
es, `lint`/`typecheck`/`format:check` nicht.

**`cargo test` macht `src/types/bindings/InstanceInfo.ts` per
Trailing-Whitespace dreckig.** Vor jedem Commit reverten.

**CI-Job umbenennen heißt `required_status_checks` mitziehen**, sonst
blockiert GitHub jeden Merge auf einen Kontext, den es nicht gibt. Aktuell
required: `Documentation consistency`, `Frontend build`,
`Rust checks (default)`, `Rust checks (no-default)`.

**Node ≥ 22.19** wird gebraucht (`.nvmrc`), sonst laufen die `.ts`-Skripte
nicht.

## Was bewusst offen bleibt

- Kein Tauri-/Rust-Release-Build in CI — nur `pnpm generate` fürs Frontend.
- `eslint.config.mjs` bleibt `.mjs`; eine TS-Config bräuchte `jiti` als neue
  direkte Dependency.
- `/usr/local/bin/node` zeigt als root-Symlink weiter auf v22.17.0 (sudo).
