# Feature Specification: Modellsuche im Chat

**Feature Branch**: `031-chat-model-search`

**Created**: 2026-09-29

**Status**: Draft

**Input**: Nutzerbeobachtung: Bei der Auswahl des Modells im Chat verliert man
bei vielen installierten bzw. verbundenen Modellen schnell den Überblick.
Ein Fuzzy-Suchfeld in der Modellauswahl soll das Finden eines bestimmten
Modells wieder einfach machen.

## Beziehung zu bestehenden Specs

- [`002-onboarding-model-prefs`](../002-onboarding-model-prefs/spec.md) und
  [`012-unified-model-capabilities`](../012-unified-model-capabilities/spec.md)
  legen fest, wie Modelle und ihre Anbieter dem Nutzer zur Auswahl angeboten
  werden (Gruppierung nach Anbieter). Diese Spec ändert daran nichts, sie
  ergänzt nur die Bedienung der bestehenden Liste um eine Suche.
- [`011-composer-toolbar-parity`](../011-composer-toolbar-parity/spec.md)
  betrifft dieselbe Modellauswahl im Chat-Composer. Diese Spec ändert nur die
  Bedienung der Liste innerhalb der Auswahl, nicht deren Platzierung in der
  Werkzeugleiste.
- [`005-huggingface-model-discovery`](../005-huggingface-model-discovery/spec.md)
  betrifft das Finden und Herunterladen neuer Modelle aus einem
  Online-Katalog. Diese Spec betrifft ausschließlich die Auswahl unter
  bereits installierten bzw. verbundenen Modellen im Chat und hat damit
  keine Überschneidung.

## User Scenarios & Testing _(mandatory)_

### User Story 1 - Ein Modell im Chat per Suchbegriff finden (Priority: P1)

Ein Nutzer mit vielen installierten oder über mehrere Anbieter verbundenen
Modellen öffnet im Chat die Modellauswahl und tippt einen Teil des Namens,
den er sucht. Die Liste schränkt sich sofort auf passende Treffer ein, auch
wenn der Suchbegriff nicht exakt oder nicht zusammenhängend im Namen
vorkommt.

**Why this priority**: Das ist die gemeldete Einschränkung — mit vielen
Modellen ist die reine Liste nicht mehr überschaubar. Ohne diese Story bleibt
das Problem ungelöst.

**Independent Test**: Modellauswahl mit mindestens zehn Modellen über
mehrere Anbieter öffnen, einen Teil eines bestimmten Modellnamens eintippen:
nur passende Modelle (bzw. deren Anbieter-Gruppen) bleiben sichtbar; das
gesuchte Modell lässt sich ohne Scrollen durch die volle Liste auswählen.

**Acceptance Scenarios**:

1. **Given** eine geöffnete Modellauswahl mit mehreren Anbietern und
   Modellen, **When** der Nutzer einen Teil eines Modellnamens eintippt,
   **Then** bleiben nur Modelle sichtbar, deren Name oder Anbietername zum
   Suchbegriff passt; Anbieter-Gruppen ohne Treffer verschwinden vollständig.
2. **Given** ein Suchbegriff, der nicht exakt und nicht zusammenhängend im
   Namen eines Modells vorkommt (z. B. „gpt4o" für „GPT-4o", oder ein
   einzelner Tippfehler), **When** der Nutzer ihn eintippt, **Then** wird
   das gemeinte Modell trotzdem als Treffer gezeigt.
3. **Given** ein Suchbegriff, zu dem kein Modell passt, **When** der Nutzer
   ihn eintippt, **Then** zeigt die Auswahl einen kurzen, unaufdringlichen
   Hinweis statt einer leeren Fläche.
4. **Given** ein eingetippter Suchbegriff, **When** der Nutzer ihn löscht,
   **Then** erscheinen wieder alle Modelle in der gewohnten Gruppierung.
5. **Given** ein bereits gewähltes Modell, das durch einen neuen Suchbegriff
   aus der sichtbaren Liste herausfällt, **When** der Nutzer die Auswahl
   schließt, ohne ein anderes Modell zu wählen, **Then** bleibt das zuvor
   gewählte Modell unverändert aktiv.

---

### User Story 2 - Weiterhin vollständig per Tastatur bedienbar (Priority: P2)

Ein Nutzer bedient die Modellauswahl ausschließlich über die Tastatur: tippen
zum Filtern, Pfeiltasten zum Bewegen innerhalb der Treffer, Eingabe zum
Übernehmen, Escape zum Schließen.

**Why this priority**: Ohne das bliebe die Suche eine reine
Maus-Bequemlichkeit und würde die bestehende Tastaturbedienung der Auswahl
verschlechtern. Wichtig, aber nachrangig zur eigentlichen Suche (P1).

**Independent Test**: Modellauswahl per Tastatur öffnen, einen Suchbegriff
tippen, mit den Pfeiltasten zu einem Treffer bewegen, mit Eingabe übernehmen:
das erwartete Modell wird ausgewählt, ohne die Maus zu benutzen.

**Acceptance Scenarios**:

1. **Given** eine geöffnete Modellauswahl, **When** der Nutzer einen
   Suchbegriff tippt und danach die Pfeiltasten benutzt, **Then** bewegt
   sich die Markierung nur innerhalb der aktuell sichtbaren Treffer.
2. **Given** ein markierter Treffer, **When** der Nutzer Eingabe drückt,
   **Then** wird dieses Modell übernommen und die Auswahl schließt sich, wie
   beim bisherigen Verhalten ohne Suchbegriff.
3. **Given** eine geöffnete Modellauswahl mit Suchbegriff, **When** der
   Nutzer Escape drückt, **Then** schließt sich die Auswahl, ohne das
   aktive Modell zu ändern (bestehendes Verhalten, keine Regression).

---

### Edge Cases

- **Kein Treffer**: Ein Suchbegriff, zu dem kein Modell oder Anbieter passt,
  zeigt einen Hinweis statt einer leeren Fläche (User Story 1, Szenario 3).
- **Nur ein Anbieter oder wenige Modelle installiert**: Das Suchfeld
  erscheint unabhängig von der Anzahl der Modelle immer, auch wenn eine
  Suche dort keinen praktischen Nutzen hat.
- **Erneutes Öffnen**: Die Modellauswahl beginnt bei jedem Öffnen mit leerem
  Suchfeld und vollständiger Liste, unabhängig vom zuletzt eingetippten
  Suchbegriff einer vorherigen Sitzung der Auswahl.
- **Ein Modell wird während offener Auswahl entfernt** (z. B. Anbieter
  getrennt): Es verschwindet aus der (gefilterten oder ungefilterten)
  Liste wie beim bisherigen Verhalten ohne Suche; diese Spec ändert daran
  nichts.

## Requirements _(mandatory)_

### Functional Requirements

- **FR-001**: Die Modellauswahl im Chat-Composer MUSS ein Suchfeld zeigen,
  sobald sie geöffnet wird, unabhängig von der Anzahl installierter oder
  verbundener Modelle.
- **FR-002**: Jede Eingabe in das Suchfeld MUSS die angezeigten Modelle ohne
  gesonderte Bestätigung (kein Enter nötig) in Echtzeit auf Treffer
  einschränken, deren Modellname oder Anbietername zum Suchbegriff passt.
- **FR-003**: Der Treffer-Abgleich MUSS unscharf (fuzzy) sein: Die Zeichen
  des Suchbegriffs MÜSSEN nicht zusammenhängend im Anzeigenamen vorkommen,
  aber in derselben Reihenfolge; Groß-/Kleinschreibung DARF KEINE Rolle
  spielen.
- **FR-004**: Sichtbare Treffer MÜSSEN weiterhin nach Anbieter gruppiert
  bleiben; eine Anbieter-Gruppe ohne Treffer MUSS vollständig verschwinden.
  Die Reihenfolge der Modelle innerhalb einer sichtbaren Gruppe MUSS
  unverändert zur ungefilterten Liste bleiben (keine Neusortierung nach
  Trefferqualität).
- **FR-005**: Passt kein Modell zum Suchbegriff, MUSS die Auswahl einen
  kurzen, unaufdringlichen Hinweis statt einer leeren Fläche zeigen.
- **FR-006**: Das Suchfeld MUSS bei jedem Öffnen der Modellauswahl leer
  beginnen und alle Modelle zeigen, unabhängig von einem zuvor eingetippten
  Suchbegriff.
- **FR-007**: Die Modellauswahl MUSS nach Einführung der Suche vollständig
  per Tastatur bedienbar bleiben: Tippen filtert, Pfeiltasten bewegen die
  Markierung innerhalb der sichtbaren Treffer, Eingabe übernimmt das
  markierte Modell, Escape schließt die Auswahl ohne das aktive Modell zu
  ändern (keine Regression gegenüber dem bisherigen Verhalten ohne Suche).
- **FR-008**: Verschwindet das aktuell gewählte Modell durch einen
  Suchbegriff aus der sichtbaren Liste, MUSS die Auswahl davon unberührt
  bleiben, solange der Nutzer kein anderes Modell übernimmt.
- **FR-009**: Alle neuen sichtbaren Texte (Platzhalter des Suchfelds,
  Kein-Treffer-Hinweis) MÜSSEN in Deutsch und Englisch im Gleichschritt
  vorliegen.

### Key Entities

- **Modell-Anzeigeeintrag**: Bereits bestehende Entität (Anbieter-Gruppe mit
  Anbietername, darin Modelle mit Name und Kennung). Diese Spec fügt kein
  neues Attribut hinzu; die Suche wertet ausschließlich Anbieter- und
  Modellname aus, die die Auswahl heute schon anzeigt.

## Success Criteria _(mandatory)_

### Measurable Outcomes

- **SC-001**: Ein Nutzer mit 30 installierten bzw. verbundenen Modellen
  findet ein bestimmtes Modell durch Tippen weniger Zeichen in unter 5
  Sekunden, ohne die volle Liste zu durchscrollen.
- **SC-002**: Bei einem Suchbegriff mit einem einzelnen Tippfehler oder in
  nicht zusammenhängender Schreibweise wird das gemeinte Modell in 100 % von
  zehn vorbereiteten Testfällen weiterhin als Treffer angezeigt.
- **SC-003**: Alle bisherigen Tastaturschritte der Modellauswahl (öffnen,
  Pfeiltasten, Eingabe, Escape) funktionieren nach der Änderung unverändert
  — geprüft an den bestehenden Szenarien ohne Suchbegriff (keine Regression).

## Assumptions

- Das Suchfeld ist immer sichtbar, unabhängig von der Anzahl der Modelle;
  es gibt keinen Schwellenwert, ab dem es erst erscheint (einfachstes,
  vorhersagbares Verhalten).
- Treffer werden nur gefiltert, nicht anbieterübergreifend nach
  Trefferqualität neu sortiert (FR-004) — einfacher und vorhersagbarer als
  eine Neusortierung, und ausreichend, um das gemeldete Überblicksproblem zu
  lösen.
- Nur die Modellauswahl im Chat-Composer ist betroffen. Die separate
  Standardmodell-Auswahl in den Einstellungen (Spec 002) hat bei vielen
  Modellen dieselbe Unübersichtlichkeit, ist aber ein anderer
  Anwendungsbereich und nicht Teil dieser Spec (siehe „Nicht im Umfang").
- Es wird ausschließlich über bereits geladene Anbieter- und Modellnamen
  gefiltert; kein Netzwerk- oder Server-Aufruf je Tastenanschlag.

## Nicht im Umfang

- Fuzzy-Suche in der Standardmodell-Auswahl der Einstellungen (könnte eine
  eigene, spätere Änderung sein, siehe Assumptions).
- Neue Metadaten je Modell (Beschreibung, Fähigkeiten, Tags) für die Suche;
  gesucht wird nur über die bereits vorhandenen Namen.
- Serverseitige oder Online-Modellsuche (z. B. Hugging-Face-Katalog-Suche,
  Spec 005); diese Spec betrifft nur die Auswahl unter bereits installierten
  bzw. verbundenen Modellen.
