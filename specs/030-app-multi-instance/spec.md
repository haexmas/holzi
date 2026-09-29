# Feature Specification: Mehrfachinstanzen für Apps

**Feature Branch**: `030-app-multi-instance`

**Created**: 2026-09-29

**Status**: Draft

**Input**: Nutzerbeobachtung: Aktuell lässt sich von jeder App (System-App oder
künftig haextension) nur ein Fenster/Tab gleichzeitig öffnen. Standardmäßig
soll das möglich sein — mehrere Tabs derselben App gleichzeitig, zum Beispiel
mehrere Chat-Unterhaltungen nebeneinander. Ausnahmen bleiben möglich: die
Einstellungen sollen weiterhin nur einmal geöffnet werden können. Künftige
haextensions sollen selbst festlegen können, ob sie einzeln oder mehrfach
geöffnet werden dürfen.

## Beziehung zu bestehenden Specs

- [`015-workspace-shell`](../015-workspace-shell/spec.md) legt in FR-016 fest,
  dass jede App-Definition angibt, ob sie einmal oder mehrfach geöffnet werden
  darf, und in FR-017, dass Chat, Einstellungen und Föderation „in dieser
  Spec" Einzelinstanz-Apps sind. Diese Spec ändert FR-017 für Chat (künftig
  Mehrfachinstanz) und lässt Einstellungen unverändert Einzelinstanz-App
  (Föderation ist mit [`023-settings-app`](../023-settings-app/spec.md)
  FR-016 entfallen und daher nicht mehr betroffen). FR-016 selbst — das
  Datenmodell, das die Instanzpolitik je App festlegt — bleibt unverändert;
  diese Spec nutzt es nur anders.
- [`020-tab-navigation`](../020-tab-navigation/spec.md) geht in seiner
  Annahmen-Liste davon aus, dass „alle ausgelieferten Apps Einzelinstanz-Apps
  sind", und leitet daraus unter anderem ab, dass Mittelklick/Strg+Klick zum
  Öffnen in einem neuen Tab nicht gebraucht wird. Diese Annahme gilt nach
  dieser Spec nicht mehr uneingeschränkt; FR-012 dieser Vorgänger-Spec
  (Öffnen an einem Ort für Einzelinstanz-Apps) bleibt für Einstellungen
  unverändert gültig und wird für Mehrfachinstanz-Apps durch FR-008 dieser
  Spec ergänzt.
- Die geplanten Specs **017/018** (haextension-Host, Extension-Tools) werden
  eigene App-Definitionen für haextensions einführen. Diese Spec ändert das
  Datenmodell aus FR-016 (Spec 015) nicht und bestätigt nur, dass es bereits
  erlaubt, dass eine künftige haextension ihre eigene Instanzpolitik
  unabhängig von den hier behandelten Apps festlegt (FR-007). Die Integration
  von haextensions selbst ist nicht Teil dieser Spec.

## User Scenarios & Testing _(mandatory)_

### User Story 1 - Mehrere Chat-Unterhaltungen gleichzeitig offen (Priority: P1)

Ein Nutzer arbeitet an einer Unterhaltung im Chat und möchte, ohne diese zu
verlassen, eine zweite, unabhängige Unterhaltung parallel offen haben — zum
Beispiel um zwei Themen nebeneinander zu vergleichen. Er öffnet Chat über den
Launcher oder über „+" ein zweites Mal: es entsteht eine neue, unabhängige
Chat-Instanz statt dass die vorhandene aktiviert wird.

**Why this priority**: Das ist die eigentliche vom Betreiber gemeldete
Einschränkung. Ohne diese Story bleibt das Kernproblem ungelöst.

**Independent Test**: Chat öffnen, eine Nachricht senden. Chat über „+" im
selben Fenster ein zweites Mal öffnen: ein zweiter Tab mit einer neuen,
leeren Unterhaltung entsteht; die erste Unterhaltung bleibt im ersten Tab
unverändert sichtbar und bedienbar.

**Acceptance Scenarios**:

1. **Given** ein offener Chat-Tab mit einer laufenden Unterhaltung, **When**
   der Nutzer über „+" im selben Fenster Chat wählt, **Then** entsteht ein
   zweiter Tab mit einer neuen Unterhaltung; der erste Tab und seine
   Unterhaltung bleiben unverändert.
2. **Given** ein offener Chat-Tab, **When** der Nutzer Chat über den Launcher
   wählt, **Then** entsteht ein neues Fenster mit einer neuen
   Chat-Unterhaltung, unabhängig vom bereits offenen Tab (kein Aktivieren des
   vorhandenen Tabs).
3. **Given** zwei offene Chat-Instanzen, **When** der Nutzer in einer eine
   Nachricht sendet, im Verlauf eine andere Unterhaltung öffnet oder darin
   navigiert (Zurück/Vor, Spec 020), **Then** bleibt die andere Chat-Instanz
   davon vollständig unberührt.
4. **Given** zwei offene Chat-Instanzen im selben oder in verschiedenen
   Fenstern, **When** der Nutzer eine davon schließt, **Then** bleibt die
   andere mit ihrer Unterhaltung und Historie offen.
5. **Given** zwei offene Chat-Instanzen mit unterschiedlichen Unterhaltungen,
   **When** der Nutzer die Tab-Liste (Chevron) oder die Fensterübersicht
   öffnet, **Then** sind beide anhand des Titels ihrer jeweils aktuellen
   Unterhaltung unterscheidbar (Spec 020 FR-021).

---

### User Story 2 - Einstellungen bleiben Einzelinstanz (Priority: P1)

Ein Nutzer hat die Einstellungen offen und öffnet sie erneut (Launcher, „+"
oder eine alte Adresse). Wie bisher wird der vorhandene Einstellungs-Tab
aktiviert statt ein zweiter zu entstehen.

**Why this priority**: Regressionsschutz. Mehrere gleichzeitig offene
Einstellungs-Tabs würden auf denselben globalen Einstellungen arbeiten und zu
widersprüchlichen Ansichten führen (z. B. zwei Tabs mit derselben Kategorie,
von denen einer eine gerade geänderte Auswahl noch nicht zeigt). Ebenso
wichtig wie User Story 1, deshalb ebenfalls P1.

**Independent Test**: Einstellungen öffnen, dann erneut über den Launcher
öffnen: es bleibt bei einem Tab, der vorhandene wird aktiviert und
fokussiert.

**Acceptance Scenarios**:

1. **Given** ein offener Einstellungs-Tab, **When** der Nutzer Einstellungen
   erneut über Launcher oder „+" wählt, **Then** wird der vorhandene Tab
   aktiviert, sein Fenster fokussiert; es entsteht kein zweiter Tab (wie
   bisher, Spec 015 FR-016/FR-033).

---

### Edge Cases

- **Viele gleichzeitig offene Chat-Instanzen** (z. B. zehn): Diese Spec setzt
  kein eigenes Limit; bestehende Grenzen (Arbeitsspeicher, Anzahl Fenster)
  gelten unverändert.
- **Agent-Aufruf**: Ruft ein Agent `wm.tab.new` oder `wm.app.open` mit
  `appId: system.chat` mehrfach hintereinander auf, entsteht jedes Mal eine
  neue Instanz; es wird nie eine vorhandene gesucht oder zusammengeführt.
- **Sitzungswiederherstellung** (Spec 022, bei eingeschalteter Einstellung):
  Waren beim Sperren oder Schließen mehrere Chat-Instanzen offen, MUSS jede
  einzeln als eigener Tab wiederhergestellt werden — keine wird beim
  Wiederherstellen zusammengeführt, verworfen oder wie in Spec 015 FR-023
  vorgesehen ohne Historie und mit neuer Unterhaltung gestartet.
- **Direkter Sprung zu einem Ort** (Spec 020 FR-012) für eine
  Mehrfachinstanz-App: Da es keine „die eine" Instanz mehr gibt, MUSS ein
  solcher Sprung — sollte er künftig für Chat gebraucht werden — immer eine
  neue Instanz an diesem Ort öffnen, nie eine vorhandene durchsuchen. Chat
  nutzt solche Sprünge heute nicht; diese Regel hält nur das Prinzip für
  künftige Fälle fest.

## Requirements _(mandatory)_

### Functional Requirements

- **FR-001**: Chat MUSS eine Mehrfachinstanz-App sein: Öffnen über Launcher
  oder „+" MUSS unabhängig davon, wie viele Chat-Instanzen bereits offen
  sind, immer eine neue Instanz (neuer Tab bzw. neues Fenster, neue
  Unterhaltung) erzeugen und nie eine vorhandene aktivieren.
- **FR-002**: Einstellungen MUSS weiterhin eine Einzelinstanz-App bleiben;
  ihr Verhalten ändert sich gegenüber Spec 015 (FR-016, FR-033) nicht.
- **FR-003**: Jede offene Chat-Instanz MUSS ihre eigene Identität, Navigation
  und Historie (Spec 020), aktuelle Unterhaltung und laufende Antwort
  unabhängig von jeder anderen offenen Chat-Instanz führen; keine Aktion in
  einer Instanz DARF eine andere Instanz beeinflussen.
- **FR-004**: Die „+"-Liste MUSS für Chat nie den Hinweis „bereits geöffnet"
  zeigen; dieser bleibt Einzelinstanz-Apps vorbehalten (Spec 015 FR-033).
- **FR-005**: Titel, Tab-Liste (Chevron) und Fensterübersicht MÜSSEN mehrere
  gleichzeitig offene Chat-Instanzen anhand des Titels ihres jeweiligen Orts
  unterscheidbar machen (bestehende Regel aus Spec 020 FR-021, hier für den
  Mehrfachinstanz-Fall bestätigt).
- **FR-006**: Das App-Definitions-Modell MUSS unverändert erlauben, dass eine
  künftige App-Definition (etwa einer haextension, Spec 017/018) ihre eigene
  Instanzpolitik unabhängig von den in dieser Spec geänderten Apps festlegt,
  ohne dass dafür das Modell aus Spec 015 FR-016 geändert werden muss.
- **FR-007**: Aktionen, die eine App an einem Ort oder als neuen Tab öffnen
  (`wm.app.open`, `wm.tab.new`, Spec 020 FR-012), MÜSSEN für Mehrfachinstanz-
  Apps immer eine neue Instanz erzeugen und dürfen nie nach einer
  vorhandenen suchen; ihre Beschreibung für maschinelle Aufrufer (Spec 020
  FR-025) MUSS das für Mehrfachinstanz-Apps widerspiegeln, statt weiterhin
  ausschließlich das Einzelinstanz-Verhalten zu beschreiben.
- **FR-008**: Eine wiederhergestellte Sitzung (Spec 022, bei eingeschalteter
  Einstellung „Sitzung wiederherstellen") MUSS jede vormals offene
  Chat-Instanz einzeln als eigenen Tab wiederherstellen; jede beginnt gemäß
  Spec 015 FR-023 ohne Historie und mit einer neuen Unterhaltung.

### Key Entities

- **App-Definition**: Bereits in Spec 015 (FR-016) beschrieben, Attribut
  „ob mehrfach geöffnet werden darf". Diese Spec ändert nur den Wert dieses
  Attributs für Chat (neu: mehrfach) und bestätigt ihn unverändert für
  Einstellungen (weiterhin einzeln).

## Success Criteria _(mandatory)_

### Measurable Outcomes

- **SC-001**: Ein Nutzer kann zwei oder mehr Chat-Unterhaltungen gleichzeitig
  in getrennten Tabs offen haben und in jeder unabhängig schreiben und
  antworten lassen, ohne dass eine Instanz die andere sichtbar beeinflusst
  (Prüfung: zwei parallele Unterhaltungen, 100 % unabhängig).
- **SC-002**: Erneutes Öffnen der Einstellungen (Launcher, „+", alte Adresse)
  aktiviert in 100 % der Fälle weiterhin den vorhandenen Tab statt einen
  zweiten zu erzeugen — keine Regression gegenüber dem heutigen Verhalten.
- **SC-003**: Öffnen von Chat über Launcher oder „+" erzeugt in 100 % der
  Fälle eine neue, unabhängige Instanz, unabhängig davon, wie viele
  Chat-Tabs bereits offen sind (Prüfung bei 0, 1 und 5 bereits offenen
  Instanzen).
- **SC-004**: Nach einem Neustart mit eingeschalteter Sitzungswiederher-
  stellung erscheinen alle vormals offenen Chat-Instanzen wieder als
  getrennte, je leere Tabs — keine wird beim Wiederherstellen
  zusammengeführt oder verworfen (Prüfung: drei offene Instanzen vor einem
  Neustart, drei danach).

## Assumptions

- Von den heute ausgelieferten Apps wird ausschließlich Chat auf
  Mehrfachinstanz umgestellt; Einstellungen bleibt Einzelinstanz. Föderation
  ist mit Spec 023 bereits entfallen und daher nicht betroffen.
- **Öffnen aus dem Launcher oder über „+" erzeugt für Mehrfachinstanz-Apps
  immer eine neue Instanz** (Betreiberentscheidung, abgeleitet aus dem
  bestehenden Modell: eine Mehrfachinstanz-App sucht per Definition nicht
  nach einer vorhandenen Instanz). Ein Fokussieren der zuletzt aktiven
  Instanz beim Öffnen aus dem Launcher (etwa wie ein Klick auf ein bereits
  offenes App-Symbol in einer Taskleiste) ist eine andere, hier bewusst
  nicht verfolgte Interaktion und könnte bei Bedarf eine eigene, spätere
  Spec sein.
- Künftige haextensions (Spec 017/018) legen ihre Instanzpolitik über
  dieselbe App-Definition fest wie die heutigen System-Apps; diese Spec
  ändert daran nichts, sie bestätigt nur, dass das bestehende Modell dafür
  ausreicht.
- Es entsteht kein neues UI-Element und kein neuer sichtbarer Text; das
  bestehende „bereits geöffnet"-Label bleibt unverändert Einzelinstanz-Apps
  vorbehalten (FR-004).

## Nicht im Umfang

- Fokussieren der zuletzt aktiven Instanz statt Neuerzeugung beim Öffnen aus
  dem Launcher (siehe Assumptions).
- Eine eigene Einstellung oder Aktion, mit der Nutzer selbst zwischen
  Einzel- und Mehrfachinstanz umschalten können.
- Integration von haextensions selbst, deren Host oder deren
  App-Definitionen (Spec 017/018).
- Jede Änderung an Einstellungen oder Föderation (mit Spec 023 entfallen).
