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

**Voraussetzung**: `chat.permission_mode = auto` über den Freigabe-Umschalter in der Chat-Sidebar
setzen. Mindestens ein Tool ist immer verfügbar (Host-CLI, unconditionally registriert) — MCP-Server
sind in diesem Feature nicht konfigurierbar (spec.md Assumptions), daher testet der manuelle Lauf
gegen das Host-CLI-Tool. Für den automatisierten Safe-Pfad wird der test-only Stub aus T022A
verwendet; der manuelle Lauf bestätigt die Freigabe für das (immer `Risky`) CLI-Tool.

**Schritte**:
1. Chat öffnen, eine Frage stellen, die das Host-CLI-Tool erfordert (z.B. einen Kommandoaufruf).
2. **Erwartung**: `chat-tool-call` und `chat-tool-result` Events feuern, bevor die finale Antwort
   erscheint; `list_messages` zeigt die `tool_call`/`tool_result`-Zeilen in der Kette.
3. Die finale Antwort spiegelt das tatsächliche Tool-Ergebnis wider, nicht eine Vermutung.

## Szenario 2: Freigabe-Modi (User Story 2)

**Manual**: über den Sidebar-Umschalter auf "Manuell" stellen, eine Frage stellen, die den
Host-CLI-Aufruf auslöst. **Erwartung**: der Freigabe-Dialog erscheint (`tool-permission-request`),
Antwort bleibt aus, bis er per Erlauben/Ablehnen beantwortet wird — der Turn hängt nicht von selbst
weiter.

**Auto + Host-CLI-Tool**: Umschalter auf "Automatisch" stellen, eine Frage stellen, die einen
Kommandoaufruf braucht. **Erwartung**: der CLI-Tool-Aufruf pausiert immer und zeigt den
Freigabe-Dialog (FR-004/FR-015 — CLI ist immer `Risky`); ein `Safe`-Tool im selben Turn (nur über
den T022A-Test-Stub erreichbar, kein produktives Tool ist `Safe`) liefe ohne Rückfrage.

**Plan**: Umschalter auf "Plan" stellen, dieselbe Frage stellen. **Erwartung**: kein
Freigabe-Dialog erscheint, der CLI-Aufruf wird gar nicht erst versucht, das Tool-Ergebnis zeigt
einen Fehler (`blocked_by_plan_mode`), die Antwort läuft trotzdem weiter.

## Szenario 3: Abbruch mitten in einer Aktion (User Story 3)

**Außerhalb des Umfangs dieses PRs**: Die Abbruchbehandlung für laufende Tool-Aufrufe und offene
Freigabeanfragen wird erst in Phase 5 (T032) implementiert. Die folgenden Schritte dokumentieren
das geplante Verhalten und sind in diesem PR erwartete Fehlversuche.

1. Eine Anfrage stellen, die einen (spürbar langsamen) Host-CLI-Aufruf auslöst, im `auto`-Modus
   freigeben.
2. Während der Befehl läuft: `abort_current_generation` aufrufen (Stop-Button).
3. **Erwartung**: der Kommandoprozess wird sofort beendet (OS-Prozessliste prüfen — kein Zombie-
   Prozess), `finish_reason = cancelled`, keine weitere Runde startet von selbst.
4. Wiederholen, diesmal während eine `tool-permission-request` offen ist (Manual-Modus) statt
   während der Ausführung. **Erwartung**: derselbe sofortige Abbruch, kein Hängenbleiben.

## Szenario 4: Automatischer Retry (User Story 4)

**Außerhalb des Umfangs dieses PRs**: Automatische Retries werden erst in Phase 6 implementiert.
Die folgenden Schritte dokumentieren das geplante Verhalten und sind in diesem PR erwartete
Fehlversuche.

Nicht deterministisch am UI allein zu erzwingen — im Test (`chat_tool_loop`-Suite) über einen
Adapter-Test-Double simuliert, der beim ersten Versuch einen transienten Fehler liefert. Manuell
verifizierbar: Netzwerk kurz kappen (z.B. WLAN aus/an) während eines api_key-Sends. **Erwartung**:
entweder eine normale, vollständige Antwort (Retry griff) oder eine klar als Fehler markierte
Antwort nach Ausschöpfen der Retries — nie eine sichtbare "halbe" fehlgeschlagene Zwischen-Antwort.

## Szenario 5: Rundenlimit (FR-016)

Im Test simulierbar über ein Test-Tool, das das Modell zwingt, immer wieder dasselbe Tool
aufzurufen. **Erwartung**: nach der festen Obergrenze endet die Antwort mit
`finish_reason = tool_limit_reached`, unterscheidbar im UI von `error` und `cancelled`.
