# Planungsübersicht

Erstellt am 2026-09-07 als Antwort auf den Wunsch nach einem ersten Tauri-MVP mit SQLite, CRDT-Sync und lokalem LLM. Am 2026-09-08 überarbeitet, nachdem der Betreiber den MVP-Schnitt präzisiert hat: Sync ist nicht mehr Teil des MVP, Anbietermodelle sind es ab Tag 1.

Ablage im Repo: `plans/` — beratende Roadmap dieses Verzeichnisses; `specs/` — verbindliche Speckit-Artefakte; `docs/plans/` — v1-Architekturentwürfe. Dieser Ordner ist Beratungsgrundlage für die vorhandenen Speckit-Reviewstufen und überschreibt weder die v1-Entwürfe noch die Onboarding-Spezifikation.

| Plan | Priorität | Aufwand | Status | Gate |
| --- | --- | --- | --- | --- |
| [001: MVP mit lokalem Chat und Anbietermodellen](./001-desktop-mvp.md) | P1 | L | Draft (überarbeitet 2026-09-08) | Übernahme einer neuen Geräte-UUID in haex-crdt bleibt offen (blockiert nur den späteren Kopierweg, nicht den MVP) |

Legende: Priorität `P1` (höchste) – `P3` (nachrangig); Aufwand `S` (klein), `M` (mittel), `L` (groß).

Reihenfolge im Plan: Integrationsbasis → Instanzlebenszyklus → Anbieter und Modellkatalog → nutzbarer Chat → Paketabnahme. **Damit endet der MVP.** Der Zwei-Geräte-Sync folgt als eigene Etappe, sobald der Betreiber den Crate-Ausbau geliefert hat; Holzi ist bis dahin auf einem Gerät vollständig nutzbar. Nach jeder Etappe gilt deren überprüfbarer Abnahmeumfang.

Bewusst nicht als neue Arbeit geplant: erneute `haex-crdt`-Extraktion (bereits erfolgt), Wechsel des Frontend-Stacks (kein belegter Nutzen), vollständiges Multi-Plattform-/MCP-/Sharing-v1 vor dem ersten nutzbaren Chat (zu großer erster Umfang).

Klarstellung vom Betreiber: Secrets dürfen in der verschlüsselten Instanz-SQLite liegen, aber niemals in Git. Das gilt auch für Anbieter-Zugangsdaten; eine zweite Verschlüsselungsschicht über SQLCipher gibt es nicht. Die zu weit gehende kanonische Keychain-Formulierung wird als separater Korrekturvorschlag im Plan geführt; Harness-Dateien wurden nicht verändert. Holzi besitzt und verdrahtet den v1-Sync-Transport einschließlich Scanner-/Apply-APIs; konkrete Transportwahl, Vollabgleich-Verhalten und Cursorstrategie bleiben offene Vertragsentscheidungen.

Die Legacy-Konfiguration `.haex-hive.json` bleibt bis zu einer ausdrücklich durchgeführten Migration auf der v2-Atoms-Revision `ff6fda2180563479497e0bd5a25144653d3175fb`. Die neue Spaex-v4-Konfiguration verwendet die gepinnte Revision `97d2260db3cf0ca8abf3ea2cf1e5a312bc387727`; `.spaex/constitution.md` wurde gegen diese Revision geprüft und ist bytegleich mit dem enthaltenen Atom.
