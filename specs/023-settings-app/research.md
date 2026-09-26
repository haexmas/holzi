# Research: Einstellungs-App mit Kategorien

**Spec**: [spec.md](./spec.md) | **Plan**: [plan.md](./plan.md) | **Datum**:
2026-09-26

Referenz haex-vault @ `8dce379d94e18fcd42c3b73686a06f984ca3f574`
(`src/components/haex/system/settings/`,
`src/components/haex/system/settings-layout/`).

## Graphify-Konsultation

Graph `graphify-out/graph.json` im Worktree, Stand 2026-09-21 (verknüpfter
Worktree: Snapshot wird ohne Auffrischung genutzt). Abfragen: „settings app
layout sections“, „tab location routing inside an app“, „color scheme dark mode
theme“, „federation app placeholder“, „app definition registry and deep link
open at location“.

Der Snapshot ist älter als die Window-Manager-Arbeit (015, 020, 022) und kennt
`useTabRouter`, `routeMatch.ts` und `lib/wm/apps.ts` nicht; die Kandidaten
wurden deshalb zusätzlich direkt im Code geprüft. Ergebnis:

- **Erweitert statt neu gebaut**: `lib/wm/apps.ts` (Alias der entfallenen App,
  R7), `stores/models.ts` (Download-Fortschritt, R6), `components/wm/appRoutes.ts`
  (Routen der Einstellungen, R1), `usePreferences` (Farbschema, R8),
  `scripts/check-vue-templates.ts` (Farbprüfung, R9).
- **Kein Kandidat**: Kategorien-Register, Seitenleiste, Kopf mit Zurück,
  Farbschema-Anwendung. Es gibt kein Farbschema-Handling und keine
  Seitenleiste in holzi.
- **Geräteliste** (R12): `storage/known_devices.rs` und `device/commands.rs`
  werden erweitert; es gibt dort nur Lesen des eigenen Geräts und Umbenennen.
- Zur späteren manuellen Prüfung vermerkt: Abfragen gegen einen veralteten
  Graphen.

## R1 Orte und Routen der Einstellungen

**Entscheidung**: Die App `system.settings` bekommt eine Routentabelle mit einem
Wurzel-Eintrag `/` (Komponente `SettingsApp.vue`, das Gerüst) und flachen
Kindern, alle auf Tiefe 1:

| Ort                                  | Ansicht                                            | Kategorie   |
| ------------------------------------ | -------------------------------------------------- | ----------- |
| `/`                                  | Allgemein: Gerätename, Sitzung wiederherstellen    | Allgemein   |
| `/appearance`                        | Darstellung: Farbschema                            | Darstellung |
| `/models`                            | Übersicht Modelle                                  | Modelle     |
| `/models/default`                    | Standardmodell                                     | Modelle     |
| `/models/installed`                  | Installierte Modelle (Laden, Updates, Löschen)     | Modelle     |
| `/models/download`                   | Empfohlene Modelle, Zeile „Auf HuggingFace suchen“ | Modelle     |
| `/models/download/search?q=…`        | HuggingFace-Suche (Suchfeld und Ergebnisse)        | Modelle     |
| `/models/download/repo/:owner/:name` | Dateiauswahl eines HuggingFace-Repos               | Modelle     |
| `/models/speech`                     | Spracherkennungsmodell                             | Modelle     |
| `/agents`                            | Übersicht Agenten                                  | Agenten     |
| `/agents/providers`                  | Anbieter verbinden                                 | Agenten     |
| `/agents/autonomy`                   | Autonomiemodus                                     | Agenten     |
| `/agents/deny-rules`                 | Deny-Regeln                                        | Agenten     |
| `/federation`                        | Föderation: Geräte der Vault                       | Föderation  |

- Der Start-Ort `/` eines neuen Tabs ist die erste Kategorie „Allgemein“
  (FR-010). Es gibt keinen zweiten Ort `/general` und keine Umleitung.
- Die Kinder sind flache Muster mit mehreren Segmenten (`routeMatch.ts`
  unterstützt das). Eine Übersicht wird so nie zusammen mit ihrer Unteransicht
  gerendert; `SettingsApp.vue` bleibt montiert und rendert das Kind über ein
  verschachteltes `WmRouterView`.
- Der Suchbegriff steht in der Query (`q`), gesetzt mit `setQuery` (ersetzt den
  Eintrag, Spec 020 FR-005). Der Wechsel von der Suche zu einem Repo ist ein
  `push`, Zurück kehrt zu den Ergebnissen zurück.
- HuggingFace-Repo-Kennungen haben die Form `owner/name` und belegen deshalb
  zwei Segmente.
- Unbekannte Orte fängt `wm/RouterView.vue` schon ab (Rückfall auf `/` mit
  Hinweis, Spec 020 FR-014), was FR-012 erfüllt.

**Begründung**: Spec 020 macht jede Ansicht zu einem Ort; die flache Tabelle
hält Übersicht und Unteransicht getrennt, ohne neue Router-Fähigkeiten.

**Alternativen**: Verschachtelte Kinder unter `/models` (würde die Übersicht
über der Unteransicht montieren oder eine leere Zwischenkomponente brauchen);
Kategorie `/general` mit Umleitung von `/` (zwei Orte für dieselbe Ansicht,
doppelte Historieneinträge).

## R2 Kategorien-Register

**Entscheidung**: Ein reines Modul `src/lib/settings/registry.ts` beschreibt
Kategorien und Orte: Kennung, Pfadmuster, Kategorie, Symbol, i18n-Schlüssel für
Titel und Beschreibung, übergeordneter Ort. Daraus entstehen:

- die Routenmuster (`settingsRoutePatterns()`), an die `appRoutes.ts` die
  Komponenten hängt (so bleibt das Register unter Node testbar),
- die Seitenleiste (Kategorien in fester Reihenfolge),
- die Zeilen der Übersichten,
- Kopf (Titel, Beschreibung, Zurück) und Tab-Titel (`titleKey` je Route, Spec
  020 R9).

**Begründung**: Eine Quelle für Reihenfolge, Titel und Hierarchie; FR-005,
FR-002, FR-003 und die Tab-Titel können nicht auseinanderlaufen.

**Alternativen**: Kategorien in der Vue-Komponente fest verdrahten (nicht
testbar, dreifache Pflege).

## R3 Zurück im Kopf

**Entscheidung**: Reine Funktion `headerBack(history, parentPath)`:

- Ist der vorige Historieneintrag des Tabs ein Ort mit dem Pfad `parentPath`
  (Query egal), wirkt der Pfeil wie Zurück im Tab (`goTab(tab, -1)`); die Query
  des Eintrags bleibt, etwa der Suchbegriff.
- Sonst `push(parentPath)`.

Übergeordnet: Unteransicht → Übersicht der Kategorie; Suche → Download; Repo →
Suche. Kategorien selbst haben keinen Pfeil. Ein Klick in der Seitenleiste ist
immer ein `push` auf den Ort der Kategorie, auch aus einer Unteransicht derselben
Kategorie (FR-010).

**Begründung**: FR-009 verlangt „kein doppelter Eintrag“, wenn die übergeordnete
Ansicht die vorige Station ist, und einen Weg zur Übersicht nach einem
Deep-Link.

## R4 Seitenleiste in schmalen Fenstern

**Entscheidung**: CSS-Container-Abfragen von Tailwind v4 (`@container` am
Einstellungs-Gerüst). Unterhalb von `@2xl` (42rem, 672 px) zeigt die
Seitenleiste nur Symbole (Breite 3.5rem) mit Tooltip (`UiButton` hat
`tooltip`), darüber Symbol und Name (Breite 16rem).

**Begründung**: FR-004 verlangt die Fensterbreite, nicht die Bildschirmbreite.
haex-vault schaltet bei `@3xl` (48rem) um, das Einstellungsfenster in holzi
startet aber mit 760 px (`lib/wm/apps.ts`) und würde sonst immer nur Symbole
zeigen. Bei 360 px (SC-004) bleiben neben der Leiste rund 300 px für den Inhalt.

**Alternativen**: `ResizeObserver` in JavaScript (mehr Code, gleiche Wirkung);
Medienabfragen (falsche Bezugsgröße).

## R5 Einstellungen ohne Speichern-Knopf (FR-021)

**Entscheidung**: Die vorhandenen Einstellungskomponenten werden umgestellt,
ihre Aktionen bleiben:

| Einstellung     | Bedienung neu                                                                                                     | Aktion                                         |
| --------------- | ----------------------------------------------------------------------------------------------------------------- | ---------------------------------------------- |
| Gerätename      | Textfeld, speichert beim Verlassen und mit Enter; leer → Hinweis, kein Speichern                                  | `settings.device.setAlias`                     |
| Sitzung         | unverändert (Spec 022, Vorbild)                                                                                   | `settings.sessionRestore.*`                    |
| Farbschema      | zwei Auswahlen: „Dieses Gerät“ (Wie alle Geräte / Hell / Dunkel / System), „Alle Geräte“ (System / Hell / Dunkel) | neu, R8                                        |
| Standardmodell  | zwei Auswahlen: „Dieses Gerät“ (Wie alle Geräte / Modelle), „Alle Geräte“ (Keins / Modelle)                       | `settings.models.setDefault` / `.clearDefault` |
| Spracherkennung | Auswahl speichert beim Wählen; ein nicht installiertes Modell wird dabei geladen, mit Fortschritt                 | `settings.models.setStt`                       |
| Autonomiemodus  | Optionen speichern beim Wählen                                                                                    | `settings.autonomy.setMode`                    |
| Deny-Regeln     | Textfeld speichert beim Verlassen; ungültige Regeln → Hinweis am Feld, kein Speichern                             | `settings.delegate.setDenyRules`               |

Knöpfe bleiben für Handlungen: Anbieter verbinden, Modell laden, herunterladen,
Updates prüfen und installieren, löschen.

„Wie alle Geräte“ bzw. „Keins“ ist die Option „nicht festgelegt“ aus FR-021 und
ruft die `clear`-Aktion. Eine Rückmeldung „Gespeichert.“ bleibt als kurzer
Status, ein Fehler erscheint an der Einstellung und setzt die Anzeige auf den
gespeicherten Wert zurück.

**Begründung**: Betreiberentscheidung 2026-09-26 (Clarification, FR-021).

## R6 Download-Fortschritt überlebt die Navigation

**Entscheidung**: `stores/models.ts` bekommt `downloads` (Fortschritt je
Modellkennung) und `watchDownloads()`, das Fortschritt und Abschluss einmal je
Vault-Session abonniert (idempotent, ohne `stopListening` des Chats).
`pages/workspace/[instance].vue` ruft es beim Öffnen auf. Die Download-Ansichten
lesen nur noch den Store; ihre eigenen Abonnements
(`HuggingFaceModelManagement.vue`, `HuggingFaceFilePicker.vue`) entfallen.

**Begründung**: Edge Case der Spec: Ein Download läuft weiter, und die Ansicht
zeigt seinen Stand, wenn sie wieder offen ist. Heute hält die Komponente den
Fortschritt lokal und verliert ihn beim Verlassen; das Chat-Abonnement endet mit
dem Chat-Tab.

**Alternativen**: Referenzzählung pro Ansicht (verliert Ereignisse zwischen zwei
Ansichten); Fortschritt im Backend abfragen (neuer Befehl ohne Not).

## R7 Föderations-App entfällt

**Entscheidung**:

- `system.federation` verschwindet aus `WM_APPS` und `appRoutes.ts`;
  `components/apps/FederationApp.vue` und ihre Tests in
  `scripts/check-vault-lifecycle.ts` entfallen (FR-016, FR-018).
- `lib/wm/apps.ts` bekommt `LEGACY_APP_ALIASES`:
  `system.federation` → `{ appId: 'system.settings', at: '/federation' }` und
  `resolveAppAlias(appId)`. `wm.app.open` und `wm.tab.new` lösen den Alias vor
  der Prüfung auf; damit führen der Deep-Link `?open=system.federation`, Agenten
  und alte Aufrufe in die Kategorie „Föderation“ (FR-017).
- `pages/federation/[instance].vue` leitet direkt auf
  `?open=system.settings&at=/federation` um.
- Ein gespeicherter Föderations-Tab (Spec 022) wird wie jede unbekannte App beim
  Wiederherstellen verworfen (Spec 015 FR-025); das deckt der bestehende Test
  für unbekannte Apps ab.

**Begründung**: Kleinste Änderung, die jeden Weg abdeckt; die Kennung
`system.federation` bleibt für Agenten und alte Links gültig.

## R8 Farbschema

**Entscheidung**:

- Präferenz `appearance.color_scheme` mit `light`, `dark` oder `system`, Gerät
  vor Vault, ohne Wert `system` (FR-013, FR-014). Kein Backend-Code: Lesen und
  Schreiben über die vorhandenen `get_pref`/`set_pref`/`clear_pref`.
- Reines Modul `src/lib/settings/colorScheme.ts`: `parseColorScheme`,
  `effectiveColorScheme({ device, vault })`, `isDark(scheme, systemDark)`.
- Composable `useColorScheme` (modulweiter Zustand): hält beide Werte, hört auf
  `prefers-color-scheme` und setzt die Klasse `dark` an `<html>` (das Theme des
  haex-ui-Layers definiert `.dark`, `tailwind.css` hat die Variante).
- Plugin `plugins/colorScheme.client.ts` wendet beim Start „System“ an (vor dem
  Entsperren, FR-014); `pages/workspace/[instance].vue` lädt nach dem Öffnen die
  Werte der Vault.
- Aktionen `settings.appearance.setColorScheme` (`{ scope, scheme }`) und
  `settings.appearance.clearColorScheme` (`{ scope }`), Bereich
  `settings.device`, für Agenten aufrufbar (keine Leitplanke). Die Handler
  aktualisieren den Zustand von `useColorScheme`, die App wechselt sofort
  (SC-005). `settings.get` meldet `colorScheme` mit.
- Alle Fenster des Window Managers liegen in einer Webview, eine Klasse genügt
  (US4 AS1).

**Begründung**: Folgt dem Muster „Gerät vor Vault“ (Spec 002, 022); nutzt das
vorhandene Theme. `@nuxtjs/color-mode` wäre eine neue Abhängigkeit für eine
Klasse und eine Medienabfrage.

**Alternativen**: Farbschema im `localStorage` (liefe am Vault-Muster vorbei und
wäre vor dem Entsperren eine Spur der Vault).

## R9 Feste Farben auf Theme-Farben umstellen

**Entscheidung**: Rund 170 feste Tailwind-Farben in 28 Vue-Dateien werden auf
die Theme-Farben des Layers umgestellt, in einem eigenen, mechanischen Commit:

| heute                                                          | neu                                                            |
| -------------------------------------------------------------- | -------------------------------------------------------------- |
| `text-neutral-500`                                             | `text-muted-foreground`                                        |
| `border-neutral-200`, `border-neutral-300`                     | `border-border` bzw. `border-input`                            |
| `text-red-500`                                                 | `text-destructive`                                             |
| `text-green-600`, `text-green-800`, `bg-green-100`             | `text-success`, `bg-success/10`                                |
| `bg-amber-500`, `text-amber-600`                               | `bg-warning`, `text-warning`                                   |
| `ring-blue-500`, `border-blue-500`, `text-blue-*`, `bg-blue-*` | `ring-ring`, `border-primary`, `text-primary`, `bg-primary/10` |
| `bg-emerald-500`                                               | `bg-success`                                                   |
| `bg-white/10`, `bg-black/10`                                   | `bg-foreground/10`                                             |

`scripts/check-vue-templates.ts` bekommt eine Sperrliste für Palettenfarben
(`(text|bg|border|ring|…)-(neutral|gray|red|green|amber|blue|emerald|white|black|…)`),
damit keine neuen dazukommen.

**Begründung**: Ohne Umstellung wäre das dunkle Schema an diesen Stellen
unlesbar (graue Schrift auf dunklem Grund, weiße Flächen); US4 AS1 verlangt die
ganze App.

**Alternativen**: `dark:`-Varianten neben jede Farbe setzen (doppelter Pflegeaufwand,
heute genau eine Stelle).

## R10 Aufteilung der Modellverwaltung

**Entscheidung**: `HuggingFaceModelManagement.vue` (580 Zeilen, Ausnahme mit
Aufteilungsplan) zerfällt entlang der Orte aus R1:

- `settings/InstalledModels.vue`: Liste, Laden, Löschen, Updates prüfen und
  installieren, Integritätsdialog.
- `settings/DownloadModels.vue`: empfohlene Modelle mit Download-Fortschritt aus
  dem Store (R6), Zeile zur Suche.
- `models/HuggingFaceSearch.vue`: liest und setzt `q` über den Tab-Router statt
  über ein Ereignis; ein Ergebnis navigiert zum Repo-Ort.
- `models/HuggingFaceFilePicker.vue`: liest `owner`/`name` aus den Routenparametern.

Die Ausnahme-Begründung am Kopf entfällt. Die gemeinsame Zustandshaltung, die den
Split bisher verhinderte (`downloadStates`, Abonnements), liegt nach R6 im Store.

**Begründung**: Complexity Tracking aus Spec 020; die Orte der Spec geben die
Schnitte vor.

## R12 Geräte der Vault (Kategorie „Föderation“)

**Entscheidung**:

- `storage/known_devices.rs` bekommt `list_devices(conn)`: alle Zeilen außer
  der internen Vault-Bereichszeile (`installation_uuid = VAULT_SCOPE_UUID`, von
  `identity::bootstrap` mit `first_seen = 0` angelegt), sortiert nach
  `first_seen`.
- Neuer Befehl `list_vault_devices` in `device/commands.rs`, registriert in
  `lib.rs`: `Vec<VaultDevicePayload>` mit `vaultDeviceUuid`, `alias`
  (`null` ohne Namen), `firstSeenMs` (Unix-Millisekunden, wie gespeichert) und
  `isCurrent`; dieses Gerät zuerst. Wire-Form camelCase wie
  `DeviceInfoPayload`, TypeScript-Typ von Hand in `useDevice.ts` (Muster des
  Moduls).
- Lese-Aktion `settings.devices.list` (Bereich `settings.read`, Wirkung
  `read`), damit Agenten die Liste abrufen können (FR-022).
- Ansicht `settings/FederationView.vue`: eine Zeile je Gerät mit Name oder
  „Unbenanntes Gerät“, Datum des ersten Öffnens (lokal formatiert) und der
  Markierung „Dieses Gerät“. Kein Umbenennen hier: der eigene Name steht in
  „Allgemein“.
- Die Liste wird beim Öffnen der Kategorie geladen; neue Geräte anderer
  Rechner erscheinen, sobald ihre `known_devices`-Zeile synchronisiert ist und
  die Kategorie erneut geöffnet wird (US5 AS4). Kein Live-Abonnement.

**Begründung**: Betreiberentscheidung beim Plan-Review (Clarification). Die
Daten liegen schon in der Vault; ein Lesebefehl genügt.

**Alternativen**: Liste aus `settings.get` (vermischt Einstellungen mit einer
Geräteliste); Live-Aktualisierung über Sync-Ereignisse (ohne Bedarf in der
Spec).

## R11 Tests

- **Neu `src-tauri/src/storage/known_devices_tests.rs`**: `list_devices` ohne
  Vault-Bereichszeile, Sortierung nach `first_seen`, Gerät ohne Namen.
- **Neu `scripts/check-settings.ts`** (`pnpm check:settings`, in CI): Register
  (eindeutige Kennungen und Pfade, jede Kategorie hat einen Ort, jeder Ort hat
  Titel und Beschreibung, Reihenfolge nach FR-005, `categoryOf` für jeden Ort),
  `settingsRoutePatterns` gegen `matchRoute`, `headerBack` (vorige Station ist
  übergeordnet → zurück, mit Query; sonst `push`; Deep-Link-Fall),
  `effectiveColorScheme`/`isDark`, `resolveAppAlias`, und dass jeder
  i18n-Schlüssel des Registers in `de.json` und `en.json` existiert (FR-020;
  holzi schaltet die Sprache zur Laufzeit nicht um, eine manuelle Prüfung auf
  Englisch ist deshalb nicht möglich).
- **Erweitert `scripts/check-vue-templates.ts`**: Sperrliste für Palettenfarben;
  die neuen Aktionen laufen durch die vorhandene Prüfung auf direkte Schreibzugriffe.
- **Angepasst `scripts/check-vault-lifecycle.ts`**: Föderations-Tests entfallen.
- **Regression**: `check:wm-state`, `check:wm-navigation`, `check:chat-state`,
  `typecheck`, `lint`, `format:check`, e2e-Suite.
- **Manuell**: [quickstart.md](./quickstart.md).
