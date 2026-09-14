# Research: Chatfenster und Session-Handling

## Ausgangslage

Die bestehende Chat-Seite lädt Threads und selektiert automatisch den ersten
Thread. Das Modell wird erst beim Mounten der Chat-Seite über
`resolve_default_model` und `load_model` geladen. Die Konfigurations-Controls
stehen in einer separaten Reihe unterhalb des Composer-Containers. Eine
`textarea` existiert bereits, ist aber auf drei Zeilen festgelegt.

Reasoning-Deltas sind im bestehenden `TokenEvent.reasoning` vorhanden und
werden in `reasoningByMessage` gesammelt. Die Seite hat bereits einen einfachen
Show/Hide-Mechanismus. Bei `reasoningMode = on` wird der Inhalt jedoch momentan
automatisch geöffnet; Reasoning wird außerdem nicht persistiert.

## Entscheidung 1: Neue Session als transienter Entwurf

**Entscheidung**: Beim Chat-Einstieg wird kein leerer Thread sofort in SQLite
angelegt. Der Frontend-Kontext bleibt zunächst ohne `threadId`. Das bestehende
`send_message(threadId: null)` erzeugt den Thread beim ersten Senden.

**Rationale**:

- verhindert leere Historieneinträge beim bloßen Öffnen und Verlassen;
- nutzt den bereits vorhandenen Backend-Vertrag;
- trennt „neuer Chat-Einstieg“ klar von „persistierter Thread“;
- alte Threads bleiben unverändert und explizit auswählbar.

**Verworfene Alternative**: `create_thread` bei jedem Mount. Das würde für
Abbrüche und Reloads leere Threads erzeugen und die Historie verschmutzen.

## Entscheidung 2: Preload nach Vault-Publikation im Backend

**Entscheidung**: `create_instance` und `open_instance` starten nach dem
Veröffentlichen der aktiven Datenbank einen internen Preload des von Spec 002
aufgelösten lokalen Modells. Anbieter-Modelle werden dabei nicht proaktiv
verbunden. Der Open-Command wartet nicht auf den Load.

**Rationale**:

- der Load beginnt unabhängig davon, ob der Nutzer den Chat öffnet;
- Workspace, Chat und direkte Route nutzen denselben globalen Runtime-State;
- ein Frontend-Aufruf aus mehreren Routen könnte sonst doppelte Loads erzeugen;
- Genesis und bestehende Vaults erhalten dasselbe Verhalten.

Der Preload darf nicht den Vault-Wechsel blockieren. Die Implementierung
braucht dafür zwei unabhängige, sich ergänzende Mechanismen:

1. Eine `vaultGeneration`, die verhindert, dass ein veralteter Load nach einem
   Switch noch `ChatState.session` oder den sichtbaren Status der neuen Vault
   überschreibt (Stale-Result-Schutz, siehe `data-model.md`).
2. Eine verpflichtende Cancellation (`cancel_preload_and_wait`, FR-013), die
   den noch laufenden Modell-Load selbst abbricht und ihr Ende abwartet, statt
   nur sein Ergebnis zu verwerfen. Ohne Cancellation liefe ein nicht mehr
   benötigter Load-Prozess weiter und würde Ressourcen (VRAM/RAM) belegen, die
   das neue Modell braucht.

Die `vaultGeneration` schützt vor spät eintreffenden Ergebnissen; die
Cancellation beendet die zugrundeliegende Arbeit. Beide sind erforderlich und
ersetzen einander nicht.

**Verworfene Alternative**: Preload ausschließlich in der Workspace-Seite.
Das wäre zwar einfach, würde aber bei direkter Chat-Navigation oder schnellem
Vault-Wechsel zu einer Race zwischen mehreren Frontend-Aufrufern führen.

## Entscheidung 3: Status-Events plus Snapshot-Abfrage

**Entscheidung**: Der bestehende Event-Kanal für Load-Phasen bleibt bestehen.
Zusätzlich liefert ein Status-Command den aktuellen Snapshot. Frontend-Seiten
können dadurch einen Status sehen, auch wenn sie ein frühes Event verpasst haben.

**Rationale**: Tauri-Events sind nicht rückwirkend. Ein Chat, der erst nach dem
Start des Preloads gemountet wird, darf nicht auf einem dauerhaft falschen
Spinner oder einem fehlenden Status landen.

## Entscheidung 4: Composer als einheitlicher Container

**Entscheidung**: Eingabe, Controls und Senden-/Abbrechen-Aktion bleiben in
einem gemeinsamen Container. Controls zeigen initial nur Icon, Kurzlabel und
aktuellen Wert; Optionen werden in einem zugänglichen Select, Popover oder
Sheet geöffnet.

Die fachlichen Semantiken von Modellwahl, Effort und Freigabe ändern sich nicht.
Ein separates Reasoning-Control entfällt: Wenn das gewählte Modell Reasoning
unterstützt, wird es automatisch aktiviert. Es ändert sich nur die Anordnung
und Interaktion der verbleibenden Controls.

## Entscheidung 5: Auto-Growth mit begrenzter Höhe

**Entscheidung**: Die Textarea startet mit einer Zeile, wächst anhand ihrer
Scrollhöhe bis zu maximal 8 sichtbaren Zeilen und wird danach intern scrollbar.
`Enter` sendet weiterhin, `Shift+Enter` erzeugt einen Zeilenumbruch.

Die konkrete CSS-Höhe pro Zeile ist eine Implementierungsentscheidung; die
fachliche Obergrenze sind 8 sichtbare Zeilen, damit der Chat sichtbar bleibt.

## Entscheidung 6: Reasoning als lokales Disclosure-Element

**Entscheidung**: Nicht-leeres Reasoning wird pro Assistant-Nachricht in einem
standardmäßig geschlossenen Accordion angeboten. Der Zustand liegt nur in der
aktuellen Frontend-Ansicht und wird nicht persistiert.

**Rationale**:

- der normale Antworttext bleibt der primäre Lesefluss;
- Nutzer können technische Details bei Bedarf einsehen;
- bestehende Reasoning-Deltas können ohne Schema-Migration verwendet werden;
- kein versehentliches Offenlegen alter Reasoning-Inhalte nach einem Reload.

Reasoning wird nicht mehr über ein Composer-Setting abgeschaltet. Wenn das
Modell Reasoning unterstützt, werden die Deltas verwendet und angezeigt; wenn
keine Reasoning-Deltas vorliegen, wird kein leeres Accordion gerendert.

## Verifikation der bestehenden Anforderungen

- Die Fallback-Reihenfolge und der Write von `last_active_model_id` bleiben in
  Spec 002 autoritativ.
- Tool-Loop, Freigaben, Retries, Abbruch und Turn-Abschluss bleiben in Spec 003
  autoritativ.
- Keine neue Chat-Message-Spalte ist nötig, weil Reasoning ausdrücklich nicht
  persistiert wird.
