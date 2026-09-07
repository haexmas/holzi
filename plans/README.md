# Planungsübersicht

Erstellt am 2026-09-07 als Antwort auf den Wunsch nach einem ersten Tauri-MVP mit SQLite, CRDT-Sync und lokalem LLM.

Ablage im Repo: `plans/` — beratende Roadmap dieses Verzeichnisses; `specs/` — verbindliche Speckit-Artefakte; `docs/plans/` — v1-Architekturentwürfe. Dieser Ordner ist Beratungsgrundlage für die vorhandenen Speckit-Reviewstufen und überschreibt weder die v1-Entwürfe noch die Onboarding-Spezifikation.

| Plan | Priorität | Aufwand | Status | Gate |
| --- | --- | --- | --- | --- |
| [001: Desktop-MVP](001-desktop-mvp.md) | P1 | L | Draft | Sync-Ausbau von haex-crdt einplanen (Schlüsselhaltung geklärt) |

Legende: Priorität `P1` (höchste) – `P3` (nachrangig); Aufwand `S` (klein), `M` (mittel), `L` (groß).

Reihenfolge im Plan: Integrationsbasis → Instanzlebenszyklus → lokaler Chat → Integration des haex-crdt-Sync → Paketabnahme. Der Betreiber entwickelt den Crate-Sync separat; Holzi kann währenddessen lokal nutzbar werden. Nach jeder Etappe gilt deren überprüfbarer Abnahmeumfang. Die lokale Chat-Zwischenversion ist noch nicht der vollständige MVP mit Sync.

Bewusst nicht als neue Arbeit geplant: erneute `haex-crdt`-Extraktion (bereits erfolgt), Wechsel des Frontend-Stacks (kein belegter Nutzen), vollständiges Multi-Plattform-/MCP-/Sharing-v1 vor dem ersten nutzbaren Chat (zu großer erster Umfang).

Klarstellung vom Betreiber: Secrets dürfen in der verschlüsselten Instanz-SQLite liegen, aber niemals in Git. Die zu weit gehende kanonische Keychain-Formulierung wird als separater Korrekturvorschlag im Plan geführt; Harness-Dateien wurden nicht verändert. Holzi besitzt und verdrahtet den v1-Sync-Transport einschließlich Scanner-/Apply-APIs; konkrete Transportwahl, Vollabgleich-Verhalten und Cursorstrategie bleiben offene Vertragsentscheidungen.
