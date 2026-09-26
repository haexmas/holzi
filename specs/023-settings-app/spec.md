# Feature Specification: Einstellungs-App mit Kategorien

**Feature Branch**: `023-settings-app`
**Created**: 2026-09-26
**Status**: Draft
**Input**: Die Einstellungen von holzi bekommen den Aufbau aus haex-vault: links
eine Seitenleiste mit Kategorien, rechts der Inhalt der gewählten Kategorie mit
Titel und Beschreibung, Übersichten mit Unteransichten. Föderation wird eine
Kategorie der Einstellungen statt einer eigenen App, Darstellung kommt als
Kategorie hinzu. (Präzisiert in den Clarifications: Die Föderations-App
entfällt, ihre Kategorie folgt mit der Föderation selbst.) Referenz: haex-vault @ `8dce379d94e18fcd42c3b73686a06f984ca3f574`,
`src/components/haex/system/settings/` und
`src/components/haex/system/settings-layout/`.

## Beziehung zu bestehenden Specs

- [`015-workspace-shell`](../015-workspace-shell/spec.md): Die Einstellungen
  bleiben die App „Einstellungen“ mit einer einzigen Instanz. Die App
  „Föderation“ entfällt (FR-016 bis FR-018 dieser Spec); die Weiterleitung der
  früheren Vollseiten-Adresse `/federation` (FR-004 dort) führt künftig in die
  Einstellungen.
- [`020-tab-navigation`](../020-tab-navigation/spec.md): Jede Kategorie und jede
  Unteransicht ist ein Ort im Tab. Vor, Zurück, Verlaufsliste, Öffnen an einem
  Ort und Deep-Links gelten ohne Sonderregeln. Das haex-vault-Muster
  `useDrillDownNavigation` wird nicht übernommen (Begründung in Spec 020). Jede
  Änderung einer Einstellung bleibt eine Aktion im Katalog von Spec 020.
- [`022-session-restore`](../022-session-restore/spec.md): Die Einstellung
  „Sitzung wiederherstellen“ bekommt einen festen Platz in einer Kategorie. Mit
  eingeschalteter Wiederherstellung kommt ein Einstellungs-Tab samt Ort und
  Historie zurück; das ist eine Funktion der Sitzung, keine Merk-Funktion der
  Einstellungen.
- [`002-onboarding-model-prefs`](../002-onboarding-model-prefs/spec.md),
  [`005-huggingface-model-discovery`](../005-huggingface-model-discovery/spec.md),
  [`007-cli-delegate`](../007-cli-delegate/spec.md),
  [`009-autonomous-delegate-mode`](../009-autonomous-delegate-mode/spec.md),
  [`010-stt-model-choice`](../010-stt-model-choice/spec.md): Diese Specs bleiben
  maßgeblich dafür, was die einzelnen Einstellungen tun. Diese Spec ordnet sie
  nur neu an.
- Die geplante Spec **Befehle und Tastenkürzel** bekommt später eine eigene
  Kategorie. Die geplante Spec **Desktop-Symbole und Raster** ergänzt
  „Darstellung“ um den Hintergrund des Arbeitsbereichs. Eine künftige Spec zur
  **Föderation** legt die Kategorie „Föderation“ an.

## Clarifications

### Session 2026-09-26

- Q: Was zeigt die Kategorie „Föderation“, solange holzi keine
  Föderationsfunktionen hat? → A: Es gibt sie noch nicht. Die App „Föderation“
  entfällt trotzdem; alte Wege dorthin führen in die Kategorie „Allgemein“. Die
  Kategorie entsteht mit der ersten Spec, die Föderationsfunktionen bringt.
- Q: Gehört ein Hintergrund des Arbeitsbereichs (Bild oder Farbverlauf wie in
  haex-vault) zu dieser Spec? → A: Nein. „Darstellung“ enthält hier nur das
  Farbschema; der Hintergrund kommt mit der Spec Desktop-Symbole und Raster.

## User Scenarios & Testing _(mandatory)_

### User Story 1 - Einstellungen nach Kategorien finden (Priority: P1)

Ein Nutzer öffnet die Einstellungen. Links sieht er die Kategorien, rechts den
Inhalt der ersten Kategorie mit Titel und einer Zeile Beschreibung. Er klickt
auf „Modelle“ und sieht dort alles, was Modelle betrifft, statt eine lange Seite
mit allen Einstellungen durchzuscrollen.

**Why this priority**: Das ist der Kern der Spec. Die heutige Einstellungsseite
ist eine lange Liste, die mit jeder neuen Einstellung unübersichtlicher wird.

**Independent Test**: Einstellungen öffnen, jede Kategorie anklicken: Jede zeigt
Titel, Beschreibung und nur ihre eigenen Einstellungen; jede bisherige
Einstellung ist in genau einer Kategorie zu finden.

**Acceptance Scenarios**:

1. **Given** die Einstellungen sind geschlossen, **When** der Nutzer sie öffnet,
   **Then** zeigt die Seitenleiste alle Kategorien, die erste ist ausgewählt und
   ihr Inhalt ist sichtbar.
2. **Given** die Einstellungen sind offen, **When** der Nutzer eine andere
   Kategorie wählt, **Then** ist diese in der Seitenleiste hervorgehoben und ihr
   Inhalt ersetzt den vorherigen.
3. **Given** eine Kategorie, **When** sie angezeigt wird, **Then** steht oben ihr
   Titel und eine Zeile Beschreibung, darunter ihr Inhalt, der für sich scrollt,
   während Kopf und Seitenleiste stehen bleiben.
4. **Given** die Einstellungen vor dieser Spec, **When** der Nutzer eine
   beliebige frühere Einstellung sucht, **Then** findet er sie in genau einer
   Kategorie (Zuordnung in FR-005).

---

### User Story 2 - Unteransichten mit Übersicht und Zurück (Priority: P1)

Eine Kategorie mit mehreren Bereichen, zum Beispiel „Modelle“, zeigt zuerst eine
Übersicht: Zeilen mit Symbol, Titel, einer Zeile Beschreibung und einem Pfeil.
Der Nutzer wählt „Modelle herunterladen“, sucht auf HuggingFace, öffnet ein
Ergebnis und wählt eine Datei. Mit dem Zurück-Pfeil im Kopf kommt er Schritt für
Schritt zurück zur Übersicht; die Zurück-Taste des Tabs (Spec 020) wirkt
genauso.

**Why this priority**: Ohne Unteransichten müssten große Bereiche wie die
Modellverwaltung wieder auf einer Seite stehen. Die Navigation muss sich dabei
verhalten wie überall sonst im Tab.

**Independent Test**: In „Modelle“ zwei Ebenen tief gehen, dann einmal den
Zurück-Pfeil im Kopf und einmal die Zurück-Taste des Tabs benutzen: Beide führen
jeweils eine Ebene zurück. Vor im Tab führt wieder hinein.

**Acceptance Scenarios**:

1. **Given** eine Kategorie mit mehreren Bereichen, **When** der Nutzer sie
   wählt, **Then** sieht er ihre Übersicht mit einer Zeile je Bereich.
2. **Given** die Übersicht, **When** der Nutzer eine Zeile wählt, **Then** öffnet
   sich die Unteransicht mit eigenem Titel und einem Zurück-Pfeil im Kopf.
3. **Given** eine Unteransicht, die der Nutzer von der Übersicht aus geöffnet
   hat, **When** er den Zurück-Pfeil im Kopf wählt, **Then** ist die Übersicht
   wieder zu sehen, und Vor im Tab führt zurück in die Unteransicht.
4. **Given** eine Unteransicht, die über einen Deep-Link direkt geöffnet wurde,
   **When** der Nutzer den Zurück-Pfeil im Kopf wählt, **Then** führt er zur
   Übersicht der Kategorie, nicht aus den Einstellungen heraus.
5. **Given** der Nutzer wechselt aus einer Unteransicht in eine andere Kategorie
   und wieder zurück, **When** er die frühere Kategorie wählt, **Then** beginnt
   sie mit ihrer Übersicht, nicht mit der zuletzt offenen Unteransicht.

---

### User Story 3 - Direkt an eine Stelle springen (Priority: P2)

Ein Hinweis in holzi, ein anderer Teil der App oder ein Agent will den Nutzer zu
einer bestimmten Einstellung schicken, zum Beispiel zum Spracherkennungsmodell.
Die Einstellungen öffnen sich genau dort. Sind sie schon offen, springt der
vorhandene Tab dorthin.

**Why this priority**: Kontextbezogene Hinweise („Kein Modell installiert —
Modelle herunterladen“) werden erst nützlich, wenn sie an die richtige Stelle
führen.

**Independent Test**: holzi über eine Adresse mit Kategorie und Unteransicht
öffnen: Die Einstellungen zeigen genau diese Unteransicht. Dasselbe bei schon
offenen Einstellungen: kein zweiter Tab, der vorhandene springt dorthin.

**Acceptance Scenarios**:

1. **Given** die Einstellungen sind geschlossen, **When** eine Stelle in holzi
   sie an einer Kategorie oder Unteransicht öffnet, **Then** erscheinen sie
   genau dort.
2. **Given** die Einstellungen sind offen, **When** sie an einer anderen Stelle
   geöffnet werden, **Then** springt der vorhandene Tab dorthin, und Zurück
   führt zur vorherigen Stelle (Spec 020 FR-013).
3. **Given** eine Adresse mit einer unbekannten Kategorie oder Unteransicht,
   **When** sie geöffnet wird, **Then** zeigt holzi die erste Kategorie und einen
   Hinweis, statt eines Fehlers.

---

### User Story 4 - Farbschema wählen (Priority: P2)

Ein Nutzer arbeitet abends und möchte holzi dunkel. In „Darstellung“ wählt er
„Dunkel“. Die ganze App wechselt sofort. Ein anderer Nutzer lässt „System“
eingestellt, und holzi folgt dem Farbschema des Betriebssystems.

**Why this priority**: Die Oberflächenbibliothek bringt ein dunkles Schema
schon mit, holzi bietet es nur nicht an. Es ist die naheliegende erste
Einstellung für „Darstellung“.

**Independent Test**: Farbschema auf „Dunkel“, dann „Hell“, dann „System“
stellen und dabei das Farbschema des Betriebssystems umschalten: holzi folgt
jeweils sofort, ohne Neustart.

**Acceptance Scenarios**:

1. **Given** die Kategorie „Darstellung“, **When** der Nutzer „Hell“, „Dunkel“
   oder „System“ wählt, **Then** übernimmt die ganze App das Schema sofort, in
   allen offenen Fenstern.
2. **Given** „System“ ist gewählt, **When** das Betriebssystem sein Farbschema
   wechselt, **Then** folgt holzi ohne Zutun.
3. **Given** ein gewähltes Farbschema, **When** der Nutzer die Vault erneut
   öffnet, **Then** gilt dasselbe Schema wieder.
4. **Given** die Wahl des Farbschemas, **When** der Nutzer sie trifft, **Then**
   kann er sie wie das Standardmodell für dieses Gerät oder für die ganze Vault
   treffen; der Gerätewert geht vor.

---

### User Story 5 - Keine eigene Föderations-App mehr (Priority: P2)

Die Föderation ist keine eigene App mehr. Sie gehört künftig in die
Einstellungen, bekommt dort aber erst eine Kategorie, wenn es
Föderationsfunktionen gibt. Bis dahin führen alte Wege dorthin
(Launcher-Eintrag, frühere Adresse) in die Kategorie „Allgemein“.

**Why this priority**: Betreiberentscheidung vom 2026-09-25. Föderation ist eine
Konfiguration der Vault, keine Arbeitsumgebung wie der Chat. Die heutige App ist
nur ein Platzhalter mit Sperr-Knopf, und eine Kategorie ohne Inhalt widerspricht
FR-006.

**Independent Test**: Launcher öffnen: kein Eintrag „Föderation“ mehr. Die
frühere Föderations-Adresse öffnen: Die Einstellungen öffnen sich in der
Kategorie „Allgemein“.

**Acceptance Scenarios**:

1. **Given** der Launcher, **When** der Nutzer ihn öffnet, **Then** gibt es keine
   App „Föderation“ mehr.
2. **Given** die frühere Adresse der Föderation, **When** holzi sie öffnet,
   **Then** erscheinen die Einstellungen in der Kategorie „Allgemein“.
3. **Given** die Seitenleiste der Einstellungen, **When** der Nutzer sie
   ansieht, **Then** gibt es keine Kategorie „Föderation“.

---

### User Story 6 - Schmale Fenster (Priority: P2)

Ein Nutzer verkleinert das Einstellungsfenster oder nutzt holzi im Kompaktmodus
(Spec 015). Die Seitenleiste schrumpft auf eine Leiste mit Symbolen; beim
Überfahren oder langen Drücken zeigt ein Hinweis den Namen der Kategorie. Der
Inhalt bekommt den Platz.

**Why this priority**: Fenster lassen sich frei verkleinern, und im Kompaktmodus
ist wenig Platz. Eine volle Seitenleiste würde den Inhalt dort erdrücken.

**Independent Test**: Das Einstellungsfenster schrittweise schmaler ziehen: Ab
einer bestimmten Breite zeigt die Seitenleiste nur noch Symbole mit Hinweis, der
Inhalt bleibt ohne waagerechtes Scrollen bedienbar.

**Acceptance Scenarios**:

1. **Given** ein breites Einstellungsfenster, **When** es angezeigt wird,
   **Then** zeigt die Seitenleiste Symbol und Namen jeder Kategorie.
2. **Given** das Fenster wird schmaler als eine feste Grenze, **When** es
   angezeigt wird, **Then** zeigt die Seitenleiste nur Symbole, und jeder Name
   erscheint als Hinweis.
3. **Given** die Breite des Fensters (nicht des Bildschirms), **When** sie sich
   ändert, **Then** entscheidet sie allein über die Darstellung der Seitenleiste.

---

### Edge Cases

- Eine Kategorie wird geöffnet, während eine Einstellung darin gerade lädt oder
  ein Download läuft: Der Download läuft weiter, die Anzeige zeigt seinen Stand,
  sobald die Unteransicht wieder offen ist.
- Ein Nutzer ist mitten in einer Eingabe (Gerätename, Deny-Regeln) und wechselt
  die Kategorie: Nicht gespeicherte Eingaben gehen verloren, wie beim Navigieren
  in Spec 020 („Inhalte bleiben nicht zwingend erhalten“).
- Eine gespeicherte Sitzung (Spec 022) enthält einen Tab der entfallenen App
  „Föderation“: Er wird beim Wiederherstellen verworfen wie jede unbekannte App
  (Spec 015 FR-025).
- Eine Unteransicht hängt von Daten ab, die es nicht mehr gibt (ein gelöschtes
  Modell, ein getrennter Anbieter): Sie zeigt einen Hinweis und den Weg zurück
  zur Übersicht, keinen Fehlerbildschirm.
- Das Farbschema lässt sich nicht lesen oder die Vault ist noch gesperrt
  (Entsperr- und Einrichtungsseiten): holzi folgt dem Farbschema des
  Betriebssystems.
- Eine Kategorie hat nur einen Bereich: Sie zeigt den Inhalt direkt, ohne
  Übersicht mit einer einzigen Zeile.

## Requirements _(mandatory)_

### Functional Requirements

**Aufbau**

- **FR-001**: Die Einstellungen MÜSSEN links eine Seitenleiste mit den
  Kategorien und rechts den Inhalt der gewählten Kategorie zeigen. Die
  Seitenleiste ist eine flache Liste ohne Gruppen; die gewählte Kategorie ist
  hervorgehoben.
- **FR-002**: Jede Kategorie und jede Unteransicht MUSS oben einen Kopf mit Titel
  und einer Zeile Beschreibung haben; Unteransichten zusätzlich einen
  Zurück-Pfeil. Nur der Inhalt darunter scrollt.
- **FR-003**: Eine Kategorie mit mehreren Bereichen MUSS eine Übersicht zeigen:
  eine Zeile je Bereich mit Symbol, Titel, einer Zeile Beschreibung und einem
  Pfeil. Eine Kategorie mit nur einem Bereich MUSS diesen direkt zeigen.
- **FR-004**: Die Seitenleiste MUSS unterhalb einer festen Breite des Fensters
  nur Symbole zeigen, mit dem Namen als Hinweis beim Überfahren oder langen
  Drücken. Maßgeblich ist die Breite des Einstellungsfensters, nicht die des
  Bildschirms.

**Kategorien**

- **FR-005**: Die Einstellungen MÜSSEN in dieser Reihenfolge diese Kategorien
  haben, jede mit eigenem Symbol:

  | Kategorie   | Inhalt                                                                                                                                                                           |
  | ----------- | -------------------------------------------------------------------------------------------------------------------------------------------------------------------------------- |
  | Allgemein   | Gerätename, Sitzung wiederherstellen (Spec 022)                                                                                                                                  |
  | Darstellung | Farbschema (US4)                                                                                                                                                                 |
  | Modelle     | Übersicht mit: Standardmodell; installierte Modelle mit Updates und Löschen; Modelle herunterladen (Empfehlungen und HuggingFace-Suche mit Dateiauswahl); Spracherkennungsmodell |
  | Agenten     | Übersicht mit: Anbieter verbinden; Autonomiemodus; Deny-Regeln                                                                                                                   |

- **FR-006**: Es DÜRFEN nur Kategorien erscheinen, die Inhalt haben. Kategorien
  aus haex-vault ohne Gegenstück in holzi (Erweiterungen, Kontakte, Identitäten,
  Speicher, Sicherheit, Protokolle, Entwickler und ähnliche) entfallen, bis eine
  eigene Spec sie füllt. Das gilt auch für „Föderation“ (US5).
- **FR-007**: Jede Einstellung, die es vor dieser Spec gab, MUSS mit
  unverändertem Verhalten in genau einer Kategorie erreichbar sein.

**Navigation**

- **FR-008**: Jede Kategorie und jede Unteransicht MUSS ein eigener Ort im Sinne
  von Spec 020 sein. Der Wechsel der Kategorie und das Öffnen einer Unteransicht
  MÜSSEN neue Einträge in der Historie des Tabs erzeugen; Vor, Zurück und die
  Verlaufsliste gelten ohne Sonderregeln.
- **FR-009**: Der Zurück-Pfeil im Kopf einer Unteransicht MUSS zur
  übergeordneten Ansicht führen. War diese die vorige Station der Historie,
  MUSS er wie Zurück im Tab wirken (kein doppelter Eintrag); sonst MUSS er zu
  ihr navigieren.
- **FR-010**: Ein neu geöffneter Einstellungs-Tab MUSS mit der ersten Kategorie
  beginnen. Die zuletzt geöffnete Kategorie oder Unteransicht DARF NICHT von den
  Einstellungen gemerkt werden. Wählt der Nutzer eine Kategorie in der
  Seitenleiste, beginnt sie mit ihrer Übersicht.
- **FR-011**: Die Einstellungen MÜSSEN sich an jeder Kategorie und jeder
  Unteransicht öffnen lassen (Spec 020 FR-012), auch über Deep-Links und durch
  Agenten mit passender Berechtigung. Sind sie schon offen, MUSS der vorhandene
  Tab dorthin navigieren.
- **FR-012**: Ein unbekannter Ort in den Einstellungen MUSS zur ersten Kategorie
  mit Hinweis führen (Spec 020 FR-014).

**Darstellung**

- **FR-013**: holzi MUSS ein Farbschema mit den Werten „Hell“, „Dunkel“ und
  „System“ anbieten; Standard ist „System“. Die Wahl MUSS sofort in der ganzen
  App gelten und bei „System“ dem Betriebssystem folgen.
- **FR-014**: Das Farbschema MUSS sich wie das Standardmodell für dieses Gerät
  oder für die ganze Vault setzen und zurücksetzen lassen; der Gerätewert geht
  vor. Setzen und Zurücksetzen MÜSSEN Aktionen im Katalog von Spec 020 sein.
  Vor dem Entsperren einer Vault folgt holzi dem Betriebssystem.
- **FR-015**: „Darstellung“ MUSS in dieser Spec nur das Farbschema enthalten.
  Ein Hintergrund des Arbeitsbereichs (Bild oder Farbverlauf) gehört zur Spec
  Desktop-Symbole und Raster.

**Föderation**

- **FR-016**: Die App „Föderation“ MUSS entfallen: kein Launcher-Eintrag, kein
  Eintrag im Menü für neue Tabs, kein Öffnen als Fenster.
- **FR-017**: Die frühere Adresse der Föderation und jeder Aufruf, der die App
  „Föderation“ öffnen will, MÜSSEN die Einstellungen in der Kategorie
  „Allgemein“ öffnen, bis eine Spec zur Föderation eine eigene Kategorie
  anlegt.
- **FR-018**: Der Sperr-Knopf der bisherigen Föderations-App entfällt ersatzlos;
  gesperrt wird weiter über den Chat und die künftige Tastenkürzel-Spec.

**Allgemein**

- **FR-019**: Die Aufteilung DARF keine Einstellung in ihrem Verhalten ändern;
  alle Änderungen bleiben die bestehenden Aktionen aus Spec 020 und 022.
- **FR-020**: Die Kategorien, ihre Titel und Beschreibungen MÜSSEN auf Deutsch
  und Englisch vorliegen.

### Key Entities

- **Kategorie**: ein Eintrag der Seitenleiste mit Kennung, Symbol, Titel,
  Beschreibung und entweder direktem Inhalt oder einer Übersicht mit Bereichen.
  Hat einen Ort im Tab.
- **Bereich / Unteransicht**: ein Teil einer Kategorie mit eigenem Titel, eigener
  Beschreibung und eigenem Ort; kann weitere Unteransichten haben (etwa
  HuggingFace-Suche → Ergebnis → Dateiauswahl).
- **Farbschema**: Einstellung mit den Werten Hell, Dunkel, System; Gerätewert
  vor Vault-Wert, Standard System.

## Success Criteria _(mandatory)_

### Measurable Outcomes

- **SC-001**: Jede bisherige Einstellung ist von den geöffneten Einstellungen aus
  mit höchstens drei Klicks erreichbar.
- **SC-002**: 100 % der bisherigen Einstellungen sind in genau einer Kategorie zu
  finden und verhalten sich wie vorher (die bestehenden automatischen Prüfungen
  und manuellen Szenarien der betroffenen Specs bestehen).
- **SC-003**: Ein Kategoriewechsel und das Öffnen einer Unteransicht zeigen den
  neuen Inhalt in höchstens 100 ms (bei geladenen Daten).
- **SC-004**: Bei einer Fensterbreite von 360 px sind alle Kategorien erreichbar
  und alle Einstellungen ohne waagerechtes Scrollen bedienbar.
- **SC-005**: Ein Wechsel des Farbschemas ist in allen offenen Fenstern in
  weniger als einer Sekunde sichtbar.
- **SC-006**: Kein Weg in holzi öffnet mehr eine eigene Föderations-App.

## Assumptions

- Die Kategorie-Namen „Allgemein“, „Darstellung“, „Modelle“, „Agenten“ sind
  Arbeitstitel; die Beschreibungen formuliert der Plan.
- Der Wechsel der Kategorie ist eine Navigation im Tab wie jeder andere Klick zu
  einer neuen Ansicht (Spec 020); das entspricht auch haex-vault, wo Zurück einen
  Kategoriewechsel rückgängig macht.
- Das Farbschema folgt dem Muster „Gerät vor Vault“ aus Spec 002 und Spec 022,
  weil verschiedene Geräte unterschiedliche Vorlieben nahelegen (heller
  Arbeitsplatz, dunkles Tablet).
- Eine Akzentfarbe, die Schriftgröße und eine Sprachwahl sind nicht Teil dieser
  Spec; haex-vault hat die ersten beiden auch nicht, die Sprache folgt weiter dem
  System.
- Die zu große Komponente der HuggingFace-Modellverwaltung (Complexity Tracking
  aus Spec 020) wird bei der Aufteilung in Bereiche zerlegt; das ist eine Folge
  dieser Spec, keine eigene Anforderung.
- Der PR zu dieser Spec stapelt auf dem PR zu Spec 022, weil 022 die
  Einstellungsansicht um einen Abschnitt erweitert.

## Nicht im Umfang

- Eine Suche über alle Einstellungen und ein globales Zurücksetzen.
- Kategorien ohne heutigen Inhalt (siehe FR-006), darunter „Föderation“.
- Ein Hintergrund des Arbeitsbereichs (FR-015).
- Neue Einstellungen außer dem Farbschema.
- Tastenkürzel für einzelne Kategorien (kommt mit der Tastenkürzel-Spec).
