# Quickstart: Agent Tool Loop manuell verifizieren

Manuelle Verifikations-Schritte für Reviewer/Operator vor dem Merge. Automatisierte Tests laufen in
`cargo test`; Frontend-Verifikation ist manuell (kein Playwright in diesem Repo).

## Vorbereitung

```bash
git switch 003-agent-tool-loop                # oder der tatsächliche Feature-Branch
pnpm install
cd src-tauri
cargo test --lib
cargo test --test chat_message_idempotency    # bestehend, muss weiterhin grün bleiben
cargo test --test chat_tool_loop              # neu in diesem Feature
cd ..
pnpm typecheck
```

Erwartetes Ergebnis: alle Test-Suites grün, `pnpm typecheck` exit 0.

## Szenario 1: Assistant nutzt ein Tool (User Story 1)

**Voraussetzung**: `chat.permission_mode = auto` (Settings), mindestens ein Tool verfügbar (Host-CLI
oder ein konfigurierter MCP-Server). Für den automatisierten Safe-Pfad wird der test-only Stub aus
T022A verwendet; der manuelle Lauf darf ein Risky-Tool verwenden und dessen Freigabe bestätigen.

**Schritte**:
1. Chat öffnen, eine Frage stellen, die das Tool erfordert (z.B. einen Kommandoaufruf oder eine
   MCP-Abfrage).
2. **Erwartung**: `chat-tool-call` und `chat-tool-result` Events feuern, bevor die finale Antwort
   erscheint; `list_messages` zeigt die `tool_call`/`tool_result`-Zeilen in der Kette.
3. Die finale Antwort spiegelt das tatsächliche Tool-Ergebnis wider, nicht eine Vermutung.

## Szenario 2: Freigabe-Modi (User Story 2)

**Manual**: `chat.permission_mode = manual` setzen, eine Frage stellen, die *irgendein* Tool
auslöst (auch ein Safe-Tool). **Erwartung**: `tool-permission-request` feuert, Antwort bleibt aus,
bis `respond_tool_permission` aufgerufen wird — Turn hängt nicht von selbst weiter.

**Auto + Host-CLI-Tool**: `chat.permission_mode = auto` setzen, eine Frage stellen, die einen
Kommandoaufruf braucht. **Erwartung**: Safe-Tools (falls im selben Turn genutzt) laufen ohne
Rückfrage; der CLI-Tool-Aufruf pausiert immer (FR-004/FR-015 — CLI ist immer `Risky`).

**Plan**: `chat.permission_mode = plan` setzen, dieselbe Frage stellen. **Erwartung**: kein
Freigabe-Dialog erscheint, der CLI-Aufruf wird gar nicht erst versucht, `chat-tool-result` zeigt
`isError: true` mit einer "nicht erlaubt"-Meldung, die Antwort läuft trotzdem weiter.

## Szenario 3: Abbruch mitten in einer Aktion (User Story 3)

1. Eine Anfrage stellen, die einen (spürbar langsamen) Host-CLI-Aufruf auslöst, im `auto`-Modus
   freigeben.
2. Während der Befehl läuft: `abort_current_generation` aufrufen (Stop-Button).
3. **Erwartung**: der Kommandoprozess wird sofort beendet (OS-Prozessliste prüfen — kein Zombie-
   Prozess), `finish_reason = cancelled`, keine weitere Runde startet von selbst.
4. Wiederholen, diesmal während eine `tool-permission-request` offen ist (Manual-Modus) statt
   während der Ausführung. **Erwartung**: derselbe sofortige Abbruch, kein Hängenbleiben.

## Szenario 4: Automatischer Retry (User Story 4)

Nicht deterministisch am UI allein zu erzwingen — im Test (`chat_tool_loop`-Suite) über einen
Adapter-Test-Double simuliert, der beim ersten Versuch einen transienten Fehler liefert. Manuell
verifizierbar: Netzwerk kurz kappen (z.B. WLAN aus/an) während eines api_key-Sends. **Erwartung**:
entweder eine normale, vollständige Antwort (Retry griff) oder eine klar als Fehler markierte
Antwort nach Ausschöpfen der Retries — nie eine sichtbare "halbe" fehlgeschlagene Zwischen-Antwort.

## Szenario 5: Rundenlimit (FR-016)

Im Test simulierbar über ein Test-Tool, das das Modell zwingt, immer wieder dasselbe Tool
aufzurufen. **Erwartung**: nach der festen Obergrenze endet die Antwort mit
`finish_reason = tool_limit_reached`, unterscheidbar im UI von `error` und `cancelled`.
