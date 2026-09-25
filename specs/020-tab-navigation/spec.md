# Feature Specification: Navigation im Tab (Vor/Zurück je Tab)

**Feature Branch**: `020-tab-navigation`
**Created**: 2026-09-25
**Status**: Draft
**Input**: Jeder Tab der Workspace-Shell (Spec 015) bekommt einen Ort innerhalb
seiner App und eine eigene Vor/Zurück-Historie wie ein Browser-Tab. Jeder Klick,
der zu einer neuen Seite oder Ansicht führt, ist über Vor und Zurück erreichbar.
Apps lassen sich direkt an einem Ort öffnen. Shell-Aktionen werden als Befehle
mit festen Standard-Tastenkürzeln geführt; das Umbelegen zur Laufzeit folgt in
einer eigenen Spec. Das haex-vault-Modell (`useDrillDownNavigation`,
Navigation-Store) wird dabei bewusst nicht 1:1 übernommen.

## Beziehung zu bestehenden Specs

- [`015-workspace-shell`](../015-workspace-shell/spec.md) bleibt maßgeblich für
  Fenster, Tabs, Arbeitsbereiche und deren Persistenz. Diese Spec ergänzt den
  Tab um einen Ort und eine Historie und erweitert die Titelleiste (FR-007 aus
  Spec 015) um Zurück und Vor. Die Weiterleitungen der früheren
  Vollseiten-Adressen (FR-004 aus Spec 015) laufen künftig über das Öffnen an
  einem Ort (FR-012 dieser Spec).
- [`004-chat-window-handling`](../004-chat-window-handling/spec.md) und
  [`006-chat-history-management`](../006-chat-history-management/spec.md)
  bleiben maßgeblich für den Chat-Inhalt: jeder Chat-Einstieg beginnt mit einer
  neuen Unterhaltung, Verlaufseinträge lassen sich öffnen, umbenennen und
  löschen. Diese Spec legt nur fest, dass das Öffnen eines Verlaufseintrags und
  das Beginnen einer neuen Unterhaltung Navigationen im Chat-Tab sind.
- Die geplante Folge-Spec zur **Einstellungs-App** (Kategorien-Seitenleiste,
  Unteransichten) baut auf dieser Spec auf; ihre Kategorien und Unteransichten
  sind Orte im Sinne dieser Spec.
- Die geplante Folge-Spec **Befehle und Tastenkürzel** baut auf der
  Befehls-Registry dieser Spec auf und macht die Belegung durch den Nutzer
  änderbar.
- Die geplanten Folge-Specs **Tabs per Drag & Drop** und **Native Fenster**
  setzen voraus, dass die Historie mit dem Tab mitwandert (FR-010).

## User Scenarios & Testing _(mandatory)_

### User Story 1 - Zurück und Vor innerhalb eines Tabs (Priority: P1)

Ein Nutzer klickt sich in einem Tab durch mehrere Ansichten einer App, zum
Beispiel in den Einstellungen von der Übersicht zu den Modellen und von dort zu
einem einzelnen Modell. Mit Zurück gelangt er Schritt für Schritt wieder zur
Übersicht, mit Vor wieder zum Modell — genau wie in einem Browser-Tab.

**Why this priority**: Das ist der Kern des Features. Ohne Vor/Zurück müssen
Apps mit Unteransichten eigene Zurück-Knöpfe erfinden, und genau das hat in
haex-vault zu inkonsistentem Verhalten geführt.

**Independent Test**: Einen Tab öffnen, drei Ansichten nacheinander aufrufen,
zweimal Zurück, einmal Vor: der Tab zeigt jeweils die erwartete Ansicht, und die
Knöpfe sind genau dann bedienbar, wenn es ein Ziel gibt.

**Acceptance Scenarios**:

1. **Given** ein frisch geöffneter Tab, **When** der Nutzer die Titelleiste
   betrachtet, **Then** stehen links vor dem Tab-Bereich die Schaltflächen
   Zurück und Vor, beide nicht bedienbar.
2. **Given** ein Tab zeigt Ansicht A, **When** der Nutzer eine Aktion wählt,
   die zu Ansicht B führt, **Then** zeigt der Tab B und Zurück ist bedienbar.
3. **Given** ein Tab hat die Folge A → B → C durchlaufen, **When** der Nutzer
   zweimal Zurück wählt, **Then** zeigt der Tab A; Zurück ist nicht mehr
   bedienbar, Vor ist bedienbar.
4. **Given** der Tab zeigt nach Zurück wieder A und Vor führt zu B, **When** der
   Nutzer von A aus eine andere Ansicht D aufruft, **Then** entfallen die
   Vor-Einträge B und C; Vor ist nicht mehr bedienbar.
5. **Given** eine Ansicht bietet Filter, Suche oder Sortierung, **When** der
   Nutzer diese ändert, **Then** entsteht kein neuer Historien-Eintrag; der
   aktuelle Eintrag übernimmt den neuen Zustand, und Zurück führt zur vorigen
   Ansicht, nicht zum vorigen Filter.
6. **Given** der Nutzer kehrt per Zurück zu einer Ansicht zurück, **When** sie
   erscheint, **Then** zeigt sie denselben Ort mit denselben Parametern wie
   zuvor (z. B. dasselbe Modell, derselbe Filter).
7. **Given** eine App mit verschachtelten Ansichten (Seitenleiste mit
   Kategorien, darin Unteransichten), **When** der Nutzer zwischen Kategorien
   und Unteransichten wechselt, **Then** bilden alle Wechsel eine einzige,
   lineare Historie dieses Tabs, und die Seitenleiste markiert immer die
   Kategorie des aktuellen Orts.

---

### User Story 2 - Jeder Tab hat seine eigene Historie (Priority: P1)

Ein Nutzer hat mehrere Fenster und Tabs offen. Zurück wirkt immer nur auf den
Tab, den er gerade meint; die Historie anderer Tabs bleibt unberührt, auch wenn
er Tabs wechselt, Fenster minimiert oder den Arbeitsbereich wechselt.

**Why this priority**: Eine geteilte Historie ist der Hauptfehler des
haex-vault-Modells: Zurück landet dort im falschen Tab oder bewirkt gar nichts.
Ohne saubere Trennung ist User Story 1 im Mehrfensterbetrieb wertlos.

**Independent Test**: Zwei Fenster mit je einem Tab öffnen, in beiden je zwei
Ansichten aufrufen. Zurück im ersten Fenster ändert nur das erste; der Wechsel
zum zweiten Fenster und Zurück dort ändert nur das zweite.

**Acceptance Scenarios**:

1. **Given** zwei Tabs mit eigener Historie, **When** der Nutzer in einem Tab
   Zurück wählt, **Then** ändert sich nur dieser Tab; Ort und Historie des
   anderen bleiben unverändert.
2. **Given** ein Fenster mit zwei Tabs, **When** der Nutzer zwischen den Tabs
   wechselt, **Then** zeigen Zurück und Vor in der Titelleiste den Zustand des
   jeweils aktiven Tabs.
3. **Given** ein Tab mit Historie, **When** der Nutzer das Fenster minimiert und
   wiederherstellt, den Arbeitsbereich wechselt oder das Fenster in einen
   anderen Arbeitsbereich verschiebt, **Then** ist die Historie des Tabs
   unverändert.
4. **Given** ein Tab mit Historie, **When** der Nutzer den Tab schließt, **Then**
   ist seine Historie verworfen; ein neu geöffneter Tab derselben App beginnt
   ohne Historie.
5. **Given** ein Tab ohne Zurück-Eintrag, **When** der Nutzer eine
   Zurück-Eingabe auslöst (außer der Android-Geste in der Kompaktdarstellung, siehe
   User Story 3), **Then**
   passiert nichts: kein Fenster und kein Tab wird geschlossen, und die Vault
   wird nicht verlassen.

---

### User Story 3 - Vor/Zurück per Maus, Tastatur und Geste (Priority: P1)

Ein Nutzer navigiert zurück, wie er es aus dem Browser kennt: mit den
Seitentasten der Maus, mit dem gewohnten Tastenkürzel oder auf dem Handy mit der
Zurück-Geste. Die Eingabe wirkt auf das Fenster, das er meint.

**Why this priority**: Schaltflächen allein reichen für ein Browser-ähnliches
Gefühl nicht; die Maustasten und die Android-Geste sind die häufigsten Wege
zurück. Ohne klare Zielregel entsteht wieder das Problem aus User Story 2.

**Independent Test**: Zwei nebeneinander sichtbare Fenster mit Historie.
Maustaste „Zurück“ über dem nicht fokussierten Fenster ändert dieses; das
Tastenkürzel ändert das fokussierte. In der Kompaktdarstellung auf Android führt die
Zurück-Geste im aktiven Tab zurück und öffnet bei leerer Historie die
Fensterübersicht.

**Acceptance Scenarios**:

1. **Given** zwei sichtbare Fenster mit Historie, **When** der Nutzer mit dem
   Zeiger über dem nicht fokussierten Fenster die Maustaste „Zurück“ drückt,
   **Then** navigiert der aktive Tab dieses Fensters zurück; der Fokus wechselt
   nicht.
2. **Given** ein fokussiertes Fenster mit Historie, **When** der Nutzer das
   Standard-Tastenkürzel für Zurück bzw. Vor drückt (FR-017), **Then**
   navigiert der aktive Tab des fokussierten Fensters.
3. **Given** der Tastaturfokus liegt in einem Texteingabefeld, **When** der
   Nutzer Alt+Pfeil drückt und diese Kombination im Feld eine Textbedeutung hat
   (unter macOS: Sprung zum nächsten Wort), **Then** behält das Feld diese
   Bedeutung und der Tab navigiert nicht.
4. **Given** die Kompaktdarstellung auf Android und ein aktiver Tab mit
   Zurück-Einträgen, **When** der Nutzer die Zurück-Geste ausführt, **Then**
   navigiert der aktive Tab des obersten Fensters zurück.
5. **Given** die Kompaktdarstellung auf Android und ein aktiver Tab ohne
   Zurück-Einträge, **When** der Nutzer die Zurück-Geste ausführt, **Then**
   öffnet sich die Fensterübersicht; eine weitere Zurück-Geste schließt sie
   wieder, ohne die App zu verlassen oder ein Fenster zu schließen.
6. **Given** die Zurück-Schaltfläche, **When** der Nutzer sie lange drückt
   oder mit der rechten Maustaste anklickt, **Then** erscheint eine Liste der
   Zurück-Einträge (neueste zuerst) mit Titel; die Auswahl springt direkt zu
   diesem Eintrag. Für Vor gilt dasselbe mit den Vor-Einträgen.

---

### User Story 4 - Apps direkt an einem Ort öffnen (Priority: P2)

Ein Nutzer wird aus einer App heraus an eine bestimmte Stelle einer anderen App
geschickt, zum Beispiel vom Chat-Hinweis „kein Modell ausgewählt“ direkt in die
Modell-Einstellungen. Ist die Ziel-App schon offen, springt ihr vorhandener Tab
dorthin, und der Nutzer kommt mit Zurück wieder an seine vorige Stelle in dieser
App.

**Why this priority**: Direkte Sprünge sparen Klicks und ersetzen die
Weiterleitungen der früheren Vollseiten. Die Grundnavigation (User Story 1–3)
ist aber auch ohne sie nutzbar, deshalb P2.

**Independent Test**: Einstellungen offen auf der Übersicht, im Chat den Hinweis
„Modell wählen“ anklicken: der vorhandene Einstellungs-Tab zeigt die Modelle,
Zurück führt zur Übersicht. Einstellungen geschlossen, derselbe Klick: ein neuer
Einstellungs-Tab öffnet direkt bei den Modellen, Zurück ist nicht bedienbar.

**Acceptance Scenarios**:

1. **Given** eine App ist nicht geöffnet, **When** der Nutzer eine Aktion wählt,
   die sie an einem bestimmten Ort öffnet, **Then** entsteht ein Fenster mit
   einem Tab dieser App, der direkt diesen Ort zeigt; seine Historie beginnt
   mit genau diesem Eintrag.
2. **Given** eine Einzelinstanz-App ist bereits geöffnet (in einem beliebigen
   Fenster oder Arbeitsbereich), **When** der Nutzer sie an einem anderen Ort
   öffnen lässt, **Then** wird der vorhandene Tab aktiviert und fokussiert (wie
   FR-016 aus Spec 015) und navigiert zu diesem Ort; Zurück führt zum vorigen
   Ort des Tabs.
3. **Given** die Einzelinstanz-App steht bereits an genau diesem Ort, **When**
   sie dort erneut geöffnet wird, **Then** entsteht kein zusätzlicher
   Historien-Eintrag.
4. **Given** der Nutzer ruft eine frühere Vollseiten-Adresse auf (Chat,
   Einstellungen, Föderation), **When** die Seite lädt, **Then** öffnet die
   zugehörige App an ihrem Start-Ort, wie in FR-004 aus Spec 015.
5. **Given** ein Ort, den die App nicht kennt (etwa ein veralteter Verweis),
   **When** sie dort geöffnet werden soll, **Then** zeigt der Tab den Start-Ort
   der App mit einem kurzen, unaufdringlichen Hinweis statt eines
   Fehlerbildschirms.

---

### User Story 5 - Tab-Titel folgt der Ansicht (Priority: P2)

Ein Nutzer erkennt an Tab, Tab-Liste und Fensterübersicht, wo er in einer App
gerade steht, zum Beispiel „Einstellungen › Modelle“ oder den Titel des offenen
Chat-Verlaufs.

**Why this priority**: Macht die Historie und viele Tabs überhaupt
unterscheidbar; ohne sie funktioniert die Navigation aber trotzdem, deshalb P2.

**Independent Test**: In den Einstellungen zu den Modellen wechseln: der
Tab-Titel ändert sich; die Einträge der Verlaufsliste tragen die Titel der
jeweiligen Ansichten.

**Acceptance Scenarios**:

1. **Given** ein Ort liefert einen eigenen Titel, **When** der Tab ihn zeigt,
   **Then** tragen Tab, Tab-Liste (Chevron) und Fensterübersicht diesen Titel;
   liefert der Ort keinen, gilt der App-Name.
2. **Given** der Nutzer öffnet die Verlaufsliste (FR-016), **When** sie
   erscheint, **Then** trägt jeder Eintrag den Titel, den sein Ort beim Besuch
   hatte.

---

### User Story 6 - Navigation im Chat (Priority: P3)

Ein Nutzer wechselt im Chat über den Verlauf zwischen Unterhaltungen oder
beginnt eine neue. Mit Zurück kommt er zur vorigen Unterhaltung.

**Why this priority**: Nützlich, aber der Chat ist bislang eine einzige Ansicht
und funktioniert ohne Historie; deshalb P3.

**Independent Test**: Im Chat eine Nachricht senden, eine ältere Unterhaltung
aus dem Verlauf öffnen, Zurück: die zuvor aktive Unterhaltung ist wieder offen.

**Acceptance Scenarios**:

1. **Given** der Chat zeigt Unterhaltung A, **When** der Nutzer im Verlauf
   Unterhaltung B öffnet, **Then** entsteht ein Historien-Eintrag; Zurück zeigt
   wieder A.
2. **Given** der Chat zeigt eine Unterhaltung, **When** der Nutzer eine neue
   Unterhaltung beginnt, **Then** entsteht ein Historien-Eintrag.
3. **Given** ein Historien-Eintrag verweist auf eine inzwischen gelöschte
   Unterhaltung, **When** der Nutzer dorthin zurück- oder vornavigiert,
   **Then** wird der Eintrag übersprungen bzw. entfernt; der Nutzer sieht keine
   leere oder kaputte Ansicht.
4. **Given** eine Antwort läuft oder eine Freigabe steht aus, **When** der
   Nutzer im Chat-Tab zurück- oder vornavigiert, **Then** gelten dieselben
   Regeln wie beim Wechsel über den Verlauf (Spec 003/006); die Navigation
   selbst bricht keine Antwort stillschweigend ab.

### Edge Cases

- **Schnelles Mehrfach-Zurück**: Mehrere Zurück-Eingaben kurz hintereinander
  bewegen die Historie um genau so viele Schritte, ohne Einträge zu
  überspringen oder doppelt zu zählen.
- **Klick auf den aktuellen Ort**: Eine Aktion, die zum bereits angezeigten Ort
  mit denselben Parametern führt, erzeugt keinen neuen Eintrag.
- **Sehr lange Historie**: Je Tab werden höchstens 50 Einträge behalten; beim
  Überschreiten entfällt der älteste. Die Verlaufsliste zeigt höchstens die
  letzten 15 Einträge (wie Firefox).
- **Ort einer Unteransicht verschwindet**: Verweist ein Eintrag auf etwas, das
  es nicht mehr gibt (gelöschtes Modell, gelöschter Verlauf), zeigt die App
  statt einer kaputten Ansicht die nächsthöhere sinnvolle Ansicht mit
  Hinweis, oder überspringt den Eintrag (Chat, User Story 6).
- **Nicht gespeicherte Eingaben**: Navigiert der Nutzer von einer Ansicht mit
  nicht gespeicherten Änderungen weg, entscheidet die App, ob sie nachfragt;
  ohne Nachfrage dürfen die Eingaben verloren gehen (siehe Annahmen).
- **Kompaktdarstellung** (Spec 015: schmales Anwendungsfenster, Fenster im
  Vollbild): Zurück und Vor bleiben in der Titelleiste sichtbar.
- **Maustaste über einem Bereich außerhalb von Fenstern** (Launcher, freie
  Fläche des Arbeitsbereichs): wirkt auf nichts.
- **Neustart**: Nach einem Neustart beginnen alle wiederhergestellten Tabs an
  ihrem Start-Ort und ohne Historie (Betreiberentscheidung).
- **Vault sperren oder schließen**: Alle Historien werden verworfen; keine
  Historie gelangt in eine spätere Vault-Session.
- **Tab zieht in ein anderes Fenster** (heute nur als Teil eines Fensters beim
  Verschieben in einen anderen Arbeitsbereich, später per Drag & Drop oder in
  ein natives Fenster): Ort und Historie ziehen mit.

## Requirements _(mandatory)_

### Functional Requirements

**Orte und Historie**

- **FR-001**: Jeder Tab MUSS jederzeit genau einen aktuellen Ort innerhalb
  seiner App haben. Ein Ort besteht aus der Ansicht und ihren Parametern und
  ist vollständig als Daten beschreibbar, ohne Bezug auf den momentanen
  Bildschirmzustand.
- **FR-002**: Jede App MUSS ihre Ansichten als Orte anmelden, einschließlich
  eines Start-Orts. Ansichten DÜRFEN verschachtelt sein (etwa Kategorie mit
  Unteransichten); eine verschachtelte Ansicht ist ein eigener Ort.
- **FR-003**: Jeder Tab MUSS eine eigene, lineare Historie aus Orten führen,
  mit einer aktuellen Position darin. Verschachtelte Ansichten eines Tabs
  teilen sich diese eine Historie.
- **FR-004**: Führt eine Nutzeraktion zu einer anderen Ansicht oder zu
  anderen Parametern, die eine eigene Seite darstellen (etwa ein anderes
  Element), MUSS ein neuer Eintrag hinter der aktuellen Position entstehen;
  alle bisherigen Vor-Einträge MÜSSEN dabei entfallen.
- **FR-005**: Änderungen, die dieselbe Ansicht nur anders darstellen (Filter,
  Suche, Sortierung, Aufklappzustand), MÜSSEN den aktuellen Eintrag ersetzen
  statt einen neuen anzulegen. Jede App MUSS für ihre Ansichten festlegen,
  welche Aktionen Einträge anlegen und welche ersetzen.
- **FR-006**: Eine Aktion, die zum aktuellen Ort mit identischen Parametern
  führt, DARF KEINEN neuen Eintrag erzeugen.
- **FR-007**: Zurück MUSS die aktuelle Position um einen Eintrag zum
  früheren, Vor um einen Eintrag zum späteren Eintrag verschieben; der Tab MUSS
  danach den Ort dieses Eintrags mit seinen Parametern anzeigen. Zurück und Vor
  MÜSSEN wirkungslos sein, wenn kein Eintrag in dieser Richtung existiert;
  sie DÜRFEN NIE einen Tab oder ein Fenster schließen oder die Vault verlassen.
- **FR-008**: Die Historie eines Tabs MUSS unabhängig von allen anderen Tabs
  sein; keine Navigation in einem Tab DARF Ort oder Historie eines anderen
  Tabs verändern.
- **FR-009**: Je Tab MÜSSEN höchstens 50 Einträge behalten werden; beim
  Überschreiten entfällt der älteste.
- **FR-010**: Ort und Historie MÜSSEN beim Tab-Wechsel, Minimieren,
  Wiederherstellen, Maximieren, Arbeitsbereichswechsel und beim Verschieben des
  Fensters in einen anderen Arbeitsbereich erhalten bleiben. Sie gehören zum
  Tab, nicht zum Fenster, und MÜSSEN so beschaffen sein, dass sie mit dem Tab in
  ein anderes Fenster umziehen können.
- **FR-011**: Ort und Historie DÜRFEN NICHT über einen Neustart oder das
  Sperren bzw. Schließen der Vault hinaus erhalten werden; ein
  wiederhergestellter Tab (Spec 015, FR-023) beginnt an seinem Start-Ort ohne
  Historie. Schließen eines Tabs verwirft seine Historie.

**Öffnen an einem Ort**

- **FR-012**: Die Shell MUSS erlauben, eine App an einem bestimmten Ort zu
  öffnen. Ist keine passende Instanz offen, entsteht ein neuer Tab mit diesem
  Ort als einzigem Eintrag. Ist eine Einzelinstanz-App bereits offen, MUSS ihr
  vorhandener Tab wie in FR-016 aus Spec 015 aktiviert werden und zu diesem Ort
  navigieren (neuer Eintrag nach FR-004, außer bei identischem Ort nach
  FR-006).
- **FR-013**: Die Weiterleitungen der früheren Vollseiten-Adressen (Spec 015,
  FR-004) MÜSSEN über das Öffnen am Start-Ort der jeweiligen App erfolgen.
- **FR-014**: Ein unbekannter Ort DARF NICHT zu einem Fehlerbildschirm führen;
  der Tab MUSS den Start-Ort der App zeigen und einen unaufdringlichen Hinweis
  geben. Verweist ein vorhandener Eintrag auf ein nicht mehr existierendes
  Element, MUSS die App die nächsthöhere sinnvolle Ansicht zeigen oder den
  Eintrag überspringen.

**Bedienung**

- **FR-015**: Die Titelleiste jedes Fensters MUSS links vor dem Tab-Bereich die
  Schaltflächen Zurück und Vor enthalten, die sich auf den aktiven Tab des
  Fensters beziehen und genau dann bedienbar sind, wenn es in ihrer Richtung
  einen Eintrag gibt. Die übrige Anordnung aus Spec 015 (FR-007) bleibt
  unverändert. Beide Schaltflächen MÜSSEN auch in der Kompaktdarstellung
  (Spec 015: Anwendungsfenster schmaler als 768 CSS-Pixel, jedes Fenster füllt
  den Arbeitsbereich) sichtbar bleiben.
- **FR-016**: Langes Drücken oder Rechtsklick auf Zurück bzw. Vor MUSS eine
  Verlaufsliste der Einträge in dieser Richtung zeigen (nächster zuerst, höchstens 15),
  jeweils mit Titel; die Auswahl springt direkt zu diesem Eintrag.
- **FR-017**: Zurück und Vor MÜSSEN per Alt+Pfeil links / Alt+Pfeil rechts auf
  dem aktiven Tab des fokussierten Fensters auslösbar sein, auf allen
  Plattformen; unter macOS zusätzlich per Cmd+[ / Cmd+]. Hat die Kombination in
  einem fokussierten Texteingabefeld eine Textbedeutung (unter macOS
  Alt+Pfeil = Wortsprung), MUSS diese Vorrang haben.
- **FR-018**: Die Maustasten „Zurück“ und „Vor“ MÜSSEN auf den aktiven Tab des
  Fensters unter dem Zeiger wirken, ohne den Fokus zu ändern; über einem Bereich
  außerhalb eines Fensters wirken sie auf nichts.
- **FR-019**: Die Android-Zurück-Geste MUSS auf den aktiven Tab des obersten
  sichtbaren Fensters wirken. Hat er keinen Zurück-Eintrag, MUSS sie in der
  Kompaktdarstellung die Fensterübersicht öffnen; ist die Fensterübersicht
  (oder ein anderes Shell-Overlay) offen, MUSS sie diese schließen. Außerhalb
  der Kompaktdarstellung ist sie bei leerer Historie wirkungslos. Sie DARF die
  App nie verlassen.
- **FR-020**: Die Navigations-Historie des Webviews selbst DARF NICHT als
  Historie eines Tabs dienen; sie MUSS nur so weit beeinflusst werden, dass
  eine Zurück-Eingabe des Systems die Vault nicht verlässt.
- **FR-021**: Liefert ein Ort einen Titel, MÜSSEN Tab, Tab-Liste und
  Fensterübersicht diesen zeigen; sonst den App-Namen. Einträge der
  Verlaufsliste tragen den Titel ihres Orts zum Zeitpunkt des Besuchs.
- **FR-022**: Alle neuen Bedienelemente MÜSSEN per Tastatur erreichbar sein und
  zugängliche Namen tragen; die Verlaufsliste MUSS per Tastatur bedienbar
  sein.
- **FR-023**: Alle neuen sichtbaren Texte MÜSSEN in Deutsch und Englisch im
  Gleichschritt vorliegen.

**Befehle**

- **FR-024**: Jede Shell-Aktion (mindestens: Zurück, Vor, Tab schließen, neuer
  Tab über „+“, Fenster minimieren, maximieren/wiederherstellen und schließen,
  Fensterübersicht, Arbeitsbereich anlegen und wechseln, Launcher öffnen) MUSS
  als Befehl mit fester Kennung und lokalisiertem Namen geführt werden.
  Schaltflächen, Menüs und Tastenkürzel MÜSSEN diese Befehle auslösen statt
  eigene Logik zu enthalten.
- **FR-025**: Ein Befehl MUSS festlegen, worauf er wirkt (etwa aktiver Tab des
  fokussierten Fensters), und DARF eine Standard-Tastenbelegung je Plattform
  haben. In dieser Spec haben nur Zurück und Vor eine Standardbelegung
  (FR-017); die Belegung ist fest, das Ändern durch den Nutzer folgt in einer
  eigenen Spec.
- **FR-026**: Die bestehende Tastatursteuerung der Tab-Leiste (Spec 015,
  FR-038: Pfeiltasten zwischen Tabs) bleibt unverändert und ist keine globale
  Tastenbelegung im Sinne von FR-025.

**Chat**

- **FR-027**: Im Chat-Tab MÜSSEN das Öffnen eines Verlaufseintrags und das
  Beginnen einer neuen Unterhaltung neue Einträge nach FR-004 erzeugen. Ein
  Eintrag zu einer gelöschten Unterhaltung MUSS beim Erreichen übersprungen
  bzw. entfernt werden. Beim Navigieren zu einer anderen Unterhaltung gelten
  für laufende Antworten und ausstehende Freigaben dieselben Regeln wie beim
  Wechsel über den Verlauf (Spec 003/006).

### Key Entities

- **Ort (Location)**: Wo ein Tab innerhalb seiner App steht. Attribute:
  Ansicht, Parameter (etwa gewähltes Element, Filter), optional Titel. Reine
  Daten, gleich auswertbar in jedem Fenster.
- **Ansicht (Route)**: Eine von einer App angemeldete, adressierbare Seite.
  Attribute: Kennung, Parameter, Eltern-Ansicht (bei Verschachtelung),
  Titel, ob sie Start-Ort ist.
- **Tab-Historie**: Geordnete Liste von Orten mit aktueller Position; gehört zu
  genau einem Tab, höchstens 50 Einträge, nur im Speicher.
- **Befehl (Command)**: Eine benannte Shell-Aktion mit Kennung, lokalisiertem
  Namen, Wirkungsziel und optionaler Standard-Tastenbelegung je Plattform.

## Success Criteria _(mandatory)_

### Measurable Outcomes

- **SC-001**: In jeder ausgelieferten App führt jede Ansicht, die per Klick
  erreichbar ist, mit genau einer Zurück-Eingabe zur Ansicht davor (Prüfung
  aller Ansichten, 100 %).
- **SC-002**: Bei fünf offenen Tabs in drei Fenstern verändert eine
  Zurück-Eingabe in 100 % der Fälle nur den gemeinten Tab (Maustaste: Fenster
  unter dem Zeiger; Tastenkürzel: fokussiertes Fenster).
- **SC-003**: Der Wechsel per Zurück oder Vor zeigt die Ziel-Ansicht ohne
  wahrnehmbare Verzögerung (höchstens 100 ms bis zur sichtbaren Ansicht bei
  bereits geladenen Daten).
- **SC-004**: Ein Direktsprung (z. B. Chat → Modell-Einstellungen) erreicht die
  Ziel-Ansicht in einer Interaktion, unabhängig davon, ob die Ziel-App offen ist.
- **SC-005**: Keine Navigations-Eingabe (Schaltfläche, Kürzel, Maustaste,
  Geste) schließt in einem Testlauf über alle Szenarien jemals ein Fenster,
  einen Tab oder verlässt die Vault.
- **SC-006**: Die bisherigen Funktionen der Shell aus Spec 015 bleiben ohne
  Rückschritt erfüllt (alle Quickstart-Szenarien aus Spec 015 bestehen weiter).

## Assumptions

- **Keine Persistenz der Historie** (Betreiberentscheidung, 2026-09-25): Nach
  einem Neustart beginnen Tabs am Start-Ort.
- **Inhalte bleiben beim Navigieren nicht zwingend erhalten.** Zurück stellt Ort
  und Parameter wieder her, aber nicht unbedingt Scrollposition oder nicht
  gespeicherte Eingaben einer verlassenen Ansicht. Tab-Wechsel,
  Minimieren usw. erhalten Inhalte weiterhin vollständig (Spec 015, FR-013).
- **Maustasten verschieben den Fokus nicht**, wie in gängigen Desktop-Umgebungen
  beim Browser-Zurück über einem Hintergrundfenster.
- **Die Standardbelegung** ist Alt+Pfeil auf allen Plattformen (Vorgabe des
  Betreibers). Unter macOS kommt Cmd+[ / Cmd+] hinzu, weil Alt(Option)+Pfeil
  dort in Textfeldern zum nächsten Wort springt und dort Vorrang hat.
- **Grenzen der Historie** (50 Einträge, 15 in der Liste) sind übliche
  Browser-Werte und in der Planung änderbar.
- **Mittelklick oder Strg+Klick zum Öffnen in neuem Tab** ist nicht Teil dieser
  Spec, weil alle ausgelieferten Apps Einzelinstanz-Apps sind.
- **Vorgaben aus dem Projekt** (Randbedingungen an die Planung):
  - Referenz zum Verhalten, nicht zum Übernehmen: `haex-space/haex-vault` @
    `8dce379d94e18fcd42c3b73686a06f984ca3f574`, `src/stores/navigation.ts` und
    `src/composables/useDrillDownNavigation.ts`. Bewusst anders als dort:
    Einträge sind Daten statt Rückgängig-Funktionen, jeder Tab hat seine eigene
    Historie statt einer gemeinsamen Webview-Historie, verschachtelte Ansichten
    teilen eine Historie je Tab, und Orte sind adressierbar.
  - Sichtbare Texte laufen über `@nuxtjs/i18n` in `de.json` und `en.json`
    (CONTEXT.md, Abschnitt Internationalisierung).
  - Ein Design-Beschluss zum Ort-Modell ist bei Bedarf als ADR unter
    `docs/adr/` festzuhalten (Constitution).

## Nicht im Umfang

- Umbelegen von Tastenkürzeln durch den Nutzer und weitere Standardbelegungen
  (Strg+T, Strg+W, Strg+Tab …): Folge-Spec Befehle und Tastenkürzel.
- Neue Ansichten der Einstellungs-App (Kategorien-Seitenleiste, Föderation als
  Kategorie): Folge-Spec Einstellungs-App.
- Persistenz von Ort oder Historie über Neustarts.
- Tabs per Drag & Drop und native Fenster (nur vorbereitet durch FR-010).
- Öffnen eines Orts in einem neuen Tab per Mittelklick.
