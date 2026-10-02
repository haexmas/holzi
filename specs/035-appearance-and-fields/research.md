# Research: Darstellung und Eingabefelder

Alle Punkte sind entschieden; „NEEDS CLARIFICATION“ bleibt nicht offen. Stand der Quellen:
holzi `main` @ `ef4998c` plus Branch `035-ui-foundation`, haex-ui @ `2dcb8bc`.

## R1 – Wohin gehört die Darstellung?

**Entscheidung**: Eine zusätzliche Vault-Einstellung `appearance.theme` (JSON-String, Scope
`vault`); das Schema bleibt unter dem bestehenden Schlüssel `appearance.color_scheme`.

**Begründung**: `usePreferences.setPrefAsync/getPrefAsync` und die Tabelle `preferences` tragen
beliebige String-Werte und sind schon CRDT-gesynct; `pages/workspace/[instance].vue` ruft
`onVaultTablesChanged(['preferences'], …)` und liest nach einem Sync neu (Spec 024 FR-032). Das
Schema hat bereits einen Schlüssel, den e2e `settings-color-scheme` prüft; ihn umzuziehen würde
Daten und Prüfungen ohne Nutzen anfassen. Eine zweite Einstellung statt vieler einzelner hält
den Export einfach und schreibt bei „Zurücksetzen“ nur einmal.

**Verworfen**: Je Regler ein Schlüssel (mehr Schreibvorgänge, Teilzustände beim Sync, ein
Import müsste mehrere Schreibvorgänge atomar machen); Schema in den neuen Wert ziehen
(Migration ohne Nutzen).

## R2 – Modell: Tönung statt Absolutfarbe

**Entscheidung** (Klärung vom 2026-10-02): Fenster-, Container-, Text- und Komponententönung
sind je ein Paar aus OKLCH-Farbton `h` (0–360) und Sättigung `c` (0 bis zu einer Obergrenze je
Regler). Die **Helligkeit** kommt weiter vom Standardwert des Schemas. Eine eigene Farbe
(`<input type="color">` liefert Hex) wird nach OKLCH gewandelt, Helligkeit verworfen,
Sättigung auf die Obergrenze gekürzt. Die Akzentfarbe ist ein Farbton und eine Sättigung; ihre
Helligkeit wird aus dem Kontrast je Schema gelöst (R4).

**Begründung**: Eine Wahl gilt in Hell und Dunkel, Hell bleibt hell und Dunkel dunkel. Weil nur
Farbton und gedeckelte Sättigung wechseln, bleiben Kontraste nahe an den heute gemessenen und
sind für jede Wahl vorab berechenbar.

**Verworfen**: Absolute Farbe je Regler (Kontrast bricht beim Schemawechsel); getrennte Wahl je
Schema (doppelte Einstellungen, Export doppelt, kein Gewinn für den Nutzer, der in COSMIC
ohnehin meist einen Ton wählt).

## R3 – Farbrechnung ohne Bibliothek

**Entscheidung**: Eigene Funktionen in `src/lib/appearance/oklch.ts` und `contrast.ts`: OKLCH →
lineares sRGB (Matrizen nach Ottosson), Begrenzung auf den sRGB-Raum durch Senken der Sättigung
bei gleicher Helligkeit, Relative Luminanz und Kontrast nach WCAG 2.

**Begründung**: Die Vorab-Prüfung (`check-appearance.ts`) und die App müssen dieselbe Rechnung
benutzen, sonst misst die Prüfung etwas anderes als der Nutzer sieht. ~120 Zeilen sind
kleiner als jede Farbbibliothek (Constitution `ponytail`: keine neue Abhängigkeit, wenn es ohne
geht). Die e2e-Messung am DOM (`textContrast`, `backgroundContrast`) bleibt die unabhängige
Gegenprobe.

## R4 – Kontrast erzwingen

Gemessener Ausgangsstand des heutigen Akzents (Rechnung mit den Funktionen aus R3, oklch(L 0.17
180)): hell `L 0.65`: weißer Text auf Akzent **2,67:1**, Akzent gegen Hintergrund **2,45:1**;
dunkel `L 0.70`: schwarzer Text **8,2:1**, gegen Hintergrund **7,4:1**. Der heutige helle Akzent
erfüllt also die Spec-Grenzen (FR-016: 4,5:1 und 3:1) **nicht**.

**Entscheidung**: Die Ableitung löst für jeden Akzent und jedes Schema die Helligkeit `L` so,
dass (a) die bessere der beiden Textfarben (`0.985` weiß oder `0.145` schwarz) auf dem Akzent
mindestens 4,5:1 erreicht und (b) der Akzent gegen Fenster- und Container-Fläche mindestens 3:1
erreicht (Ring, Schalterspur, Rand). Gewählt wird die kleinste Änderung der Helligkeit gegenüber
dem Ausgangswert; bei Gleichstand gewinnt die weiße Schrift. Im hellen Schema liegt das Fenster
bei `L ≈ 0.55–0.60` mit dunkler Schrift oder `L ≈ 0.50` mit weißer; im dunklen bleibt der Standard.

**Folge, sichtbar**: Der Standardakzent bleibt Blaugrün (Farbton 180), wird aber im **hellen**
Schema dunkler (oder bekommt dunkle Schrift) als heute. Das ist die ehrliche Folge der
Kontrastgrenzen aus der Spec; die genauen Werte stehen nach der Umsetzung in `data-model.md`
(Tabelle „Standard“) und im PR. Dunkel bleibt unverändert.

**Tönungen**: Obergrenze der Sättigung je Regler (Fenster `0.03`, Container `0.03`, Komponenten
`0.04`, Text `0.04`). Nach der Ableitung wird jedes Paar (Text gegen Fenster, Container und
Komponenten; gedämpfter Text; Rand gegen Fläche) gemessen; liegt eines unter der Grenze, wird
erst die Sättigung der verursachenden Tönung gesenkt, dann die Helligkeit des Textes
nachgeführt. Die Anpassung wird dem Nutzer gemeldet (FR-017): die Zeile zeigt „angepasst“ mit
Grund.

## R5 – Anwendung auf die Oberfläche

**Entscheidung**: `useAppearance` setzt die abgeleiteten Variablen mit
`documentElement.style.setProperty` (Inline-Stil schlägt `:root` und `.dark` in
`tailwind.css`); „Zurücksetzen“ entfernt sie. Die Standardwerte stehen weiter in `tailwind.css`
und gelten, solange kein Wert gesetzt ist (Start, Sperrbildschirm, Fehler). Wechselt das
Schema (Klasse `dark`, Systembeobachtung), rechnet `useAppearance` neu, indem es auf denselben
Beobachter hört, den `useColorScheme` schon betreibt.

**Begründung**: Alle Fenster teilen ein Webview (023); ein Satz Variablen genügt. Kein
Neuladen, kein eigenes Stylesheet, und der Standard bleibt in einer Datei sichtbar.

**Kein Zwischenspeicher im Browser**: Die Darstellung gehört zur Vault; ein lokaler
Zwischenspeicher könnte beim Vault-Wechsel die der vorherigen zeigen (Edge Case der Spec). Der
kurze Wechsel Standard → eigene Farbe beim Öffnen wird durch Warten auf `loadAsync` an der
Stelle ausgeblendet, an der das Farbschema schon gelesen wird, bevor der Arbeitsbereich
gezeichnet wird; das wird in der Umsetzung geprüft und, falls es blitzt, dort gelöst, nicht
durch Zwischenspeicherung.

## R6 – Welche Felder, was daraus wird

Bestandsaufnahme (`git grep` nach `ShadcnInput|ShadcnTextarea|ShadcnSelect|<input|<textarea|<select`):

| Bereich | Dateien | Ziel |
|---|---|---|
| Passwortmanager | `EntryEditor` (7 Felder, 1 mehrzeilig, 1 Passwort), `GeneratorPanel` (3 Felder, `<select>`, Regler), `FolderDialog` (2), `KeyValues` (2), `Passkeys`, `TagManager`, `Attachments` (je 1), `SelectionToolbar` (`<select>` ×2, 1 Feld), `Toolbar` (Suche) | `UiInput`/`UiInputPassword`/`UiTextarea`/`UiSelect`; Suche ohne Label mit `clearable`; Regler bleibt |
| Einstellungen | `AliasSetting`, `ConnectDelegateProvider`, `ServerList`, `Toolbar` (Suche), `Select` (Zeilen-Auswahl), `OptionRow` (Optionsfeld) | Felder → `UiInput`, Suche ohne Label; **`Select.vue` bleibt** (siehe unten); Optionsfeld/Auswahlknopf bleibt nativ |
| Chat | `Composer` (`<textarea>`), `ComposerControl` und `ComposerSettingsPopover` (`ShadcnSelect`, Optionsfelder), `ThreadSidebar` (Umbenennen) | Auswahllisten → `UiSelect` ohne Label; Umbenennen → `UiInput` ohne Label; `Composer` bleibt (wächst mit dem Inhalt, Enter/Umschalt-Enter, Senden) |
| Modelle | `HuggingFaceSearch`, `HuggingFaceFilePicker` | `UiInput` (Suche, `clearable`) |
| Einrichtung | `AliasStep`, `CreateSheet`, `LinkSheet` (3) | `UiInput`/`UiInputPassword` mit Label |

**Ausnahmen der Positivliste** (`scripts/check-fields.ts`, je mit Grund, FR-001):

1. `Composer.vue`: wachsendes Eingabefeld des Chats mit eigenem Tastaturverhalten.
2. `settings/Select.vue`: nimmt gruppierte Optionen, einen Leer-Wert („Keins“) und `data-value`
   an den Einträgen für die e2e-Prüfungen; das Label ist der Titel der Zeile. Wird aufgehoben,
   sobald haex-ui `UiSelect` das kann (Upstream-Folgeänderung A1 unten).
3. Native Schalter und Regler (`type=radio|checkbox|range|file|color`) sind keine Textfelder und
   unterliegen der Prüfung nicht.

**Upstream-Folgeänderung A1** (haex-space/haextension, klein, vor Stufe 1 oder parallel):
`UiSelect` bekommt `groups`, `data-value` an den Einträgen, Weiterreichen von `data-testid` und
Attributen an den Auslöser (damit `choose(selectHook, value)` aus `scripts/e2e/lib/settings.ts`
unverändert geht) und einen Leer-Wert. Ohne A1 bleiben `UiSelect`-Umstellungen auf Auswahllisten
ohne e2e-Hook und ohne Gruppen; Stufe 1 hängt nicht daran.

**`labelBg`**: Felder in `SettingsGroup`-Boxen liegen auf `bg-muted`/`--card`: dort
`label-bg="var(--card)"`, in Dialogen `var(--popover)`, auf der Seite der Standard
`var(--background)`. Eine kleine Hilfe in `src/lib/` ist nicht nötig, die Werte stehen am Feld.

## R7 – Farbwähler und Dateien, auch mobil

**Entscheidung**: Eigene Farbe über `<input type="color">` (nativ, auf Android und iOS vom
Webview als Farbwähler gezeigt) in einem Popover der „+“-Kachel, daneben ein Hex-Feld
(`UiInput`, geprüft). Export/Import über `@tauri-apps/plugin-dialog` (`save`/`open`) und
`plugin-fs`, wie der Passwortimport in `ImportWizard.vue`; die Dateiauswahl auf Mobilgeräten ist
dort bereits abgedeckt. Die Datei ist klein (< 2 KiB) und wird im Speicher geprüft.

**Verworfen**: Eigener Farbwähler (Aufwand ohne Nutzen für Stufe 2); Anzeigen und Einfügen von
Text statt Datei (der Nutzer wünscht „Export/Import“ wie COSMIC, Datei).

## R8 – Aktionen (Aktionskatalog)

Jede Bedienung steht im Aktionskatalog (Spec 020/032): `settings.appearance.set` (Teilwerte),
`settings.appearance.reset`, `settings.appearance.export` (liefert die Datei als Text),
`settings.appearance.import` (nimmt Text, prüft, wendet an). Rechte und Wirkung wie
`settings.appearance.setColorScheme` (`scope: settings.device`, `effect: write`); `reset` und
`import` sind wie der Rest der Gruppe schreibend. Der Dialog für Datei (Pfad wählen) bleibt in
der Oberfläche; die Aktion kennt nur den Text. Details in `contracts/appearance-actions.md`.

`settings.get` liefert zusätzlich `appearance` (die Darstellung), damit ein Modell im Chat sie
lesen kann.

## R9 – Fensterhinweis

`components/wm/Window.vue` setzt heute `border-foreground/40` für aktive und `border-border` für
inaktive Fenster. Mit eingeschaltetem Hinweis wird für das aktive `border-primary` gesetzt
(Akzent erfüllt gegen die Fläche 3:1 durch R4). Der Wert kommt aus `useAppearance`. Es gibt
immer nur ein aktives Fenster (`wm.focusWindow`), damit trifft FR-024 zu.

## R10 – Prüfstrategie

- `check-appearance.ts`: Farbrechnung (bekannte Werte, Rückwandlung), Ableitung (Standard =
  heutige Tokens für Dunkel; Hell mit den neuen Akzentwerten aus R4), **Kontrastmatrix**: jedes
  vordefinierte Farbfeld und die Extremwerte einer eigenen Farbe (Sättigung = Obergrenze, Farbton
  in 15°-Schritten) × beide Schemata × jedes Paar der Token-Tabelle (`contracts/token-map.md`);
  Datei: gültige Beispiele, jedes fehlerhafte Feld einzeln, „alles oder nichts“.
- `check-fields.ts`: sucht in `src/**/*.vue` die alten Feldarten und meldet jede Fundstelle, die
  nicht in der Positivliste steht; die Liste steht in der Datei, mit Grund je Eintrag.
- e2e: `appearance-basic` (Akzent wählen, Neustart, Zurücksetzen, Export, Import, kaputte Datei),
  `appearance-sync-two-devices` (Wechsel auf Gerät A erscheint auf Gerät B, Rig aus Spec 033),
  `fields-basic` (Label schwebt, Ring in Primärfarbe, Fehlertext, Löschen/Kopieren), erweiterte
  `settings-color-scheme` (Kontrast der Standardwerte nach R4).
- Bestehende e2e (`passwords-*`, `settings-*`, Chat, Einrichtung) laufen unverändert; ein
  Fehlschlag dort ist ein Verhaltensverlust (FR-007).
