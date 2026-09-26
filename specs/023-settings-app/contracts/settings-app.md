# Vertrag: Einstellungs-App

**Spec**: [spec.md](../spec.md) | **Research**: [research.md](../research.md) |
**Datenmodell**: [data-model.md](../data-model.md)

## 1. Orte

App `system.settings`, Register in `src/lib/settings/registry.ts`, Komponenten
in `src/components/wm/appRoutes.ts`.

| Ort-`id`                 | Pfad                                 | Kategorie  | Übergeordnet             | Übersichtszeile | Komponente                              |
| ------------------------ | ------------------------------------ | ---------- | ------------------------ | --------------- | --------------------------------------- |
| `general`                | `/`                                  | general    | —                        | —               | `settings/GeneralView.vue`              |
| `appearance`             | `/appearance`                        | appearance | —                        | —               | `settings/ColorSchemeSetting.vue`       |
| `models`                 | `/models`                            | models     | —                        | —               | `settings/OverviewView.vue`             |
| `models.default`         | `/models/default`                    | models     | `models`                 | ja              | `settings/DefaultModelSetting.vue`      |
| `models.installed`       | `/models/installed`                  | models     | `models`                 | ja              | `settings/InstalledModels.vue`          |
| `models.download`        | `/models/download`                   | models     | `models`                 | ja              | `settings/DownloadModels.vue`           |
| `models.download.search` | `/models/download/search`            | models     | `models.download`        | —               | `models/HuggingFaceSearch.vue`          |
| `models.download.repo`   | `/models/download/repo/:owner/:name` | models     | `models.download.search` | —               | `models/HuggingFaceFilePicker.vue`      |
| `models.speech`          | `/models/speech`                     | models     | `models`                 | ja              | `settings/SttModelSetting.vue`          |
| `agents`                 | `/agents`                            | agents     | —                        | —               | `settings/OverviewView.vue`             |
| `agents.providers`       | `/agents/providers`                  | agents     | `agents`                 | ja              | `settings/ConnectDelegateProvider.vue`  |
| `agents.autonomy`        | `/agents/autonomy`                   | agents     | `agents`                 | ja              | `settings/AutonomyModeSetting.vue`      |
| `agents.denyRules`       | `/agents/deny-rules`                 | agents     | `agents`                 | ja              | `settings/DelegateDenyRulesSetting.vue` |

Query: nur `models.download.search` nutzt `q` (Suchbegriff). Deep-Link:
`/workspace/<vault>?open=system.settings&at=/models/download/search?q=qwen`
(URL-kodiert).

Route-Komponenten bekommen keine Props. Was eine Einstellung vom Gerät braucht
(`vaultDeviceUuid`, Alias), stellt `SettingsApp.vue` per `provide` bereit
(`SETTINGS_DEVICE_KEY`, geladen einmal beim Montieren).

## 2. Gerüst (`components/apps/SettingsApp.vue`)

```text
┌──────────────┬───────────────────────────────────────────┐
│ Seitenleiste │ Kopf: [←] Titel                           │
│  ⚙ Allgemein │       eine Zeile Beschreibung             │
│  ◐ Darstell. ├───────────────────────────────────────────┤
│  ▣ Modelle   │ Inhalt (WmRouterView, scrollt allein)     │
│  ✦ Agenten   │                                           │
└──────────────┴───────────────────────────────────────────┘
```

- Wurzel mit `@container`; unter `@2xl` (672 px) ist die Seitenleiste 3.5rem
  breit und zeigt nur Symbole mit Tooltip, darüber 16rem mit Symbol und Name
  (FR-004).
- **Seitenleiste** (`settings/Sidebar.vue`): eine Schaltfläche je Kategorie in
  Registerreihenfolge; hervorgehoben ist die Kategorie des aktuellen Orts
  (`aria-current="page"`). Ein Klick ist `router.push(category.path)`, auch wenn
  die Kategorie schon aktiv ist und eine Unteransicht offen ist (FR-010).
- **Kopf**: Titel und Beschreibung aus dem Ort (`locationFor`), Parameter
  eingesetzt. Hat der Ort ein `parent`, steht links ein Zurück-Pfeil mit
  Beschriftung „Zurück zu <Titel des übergeordneten Orts>“; er führt
  `headerBack` aus: `back` → `router.back()`, `push` → `router.push(path)`
  (FR-009).
- **Inhalt**: `<WmRouterView />` auf Tiefe 1; nur dieser Bereich scrollt,
  Kopf und Seitenleiste stehen (US1 AS3).
- **Übersicht** (`settings/OverviewView.vue`): eine Zeile je
  `overviewRows(category)`: Symbol, Titel, eine Zeile Beschreibung, Pfeil;
  Klick `router.push(row.path)` (FR-003).
- Eine Unteransicht, deren Daten fehlen (gelöschtes Modell, getrennter
  Anbieter), zeigt einen Hinweis; der Zurück-Pfeil führt zur Übersicht (Edge
  Case).

## 3. Aktionen

In `src/lib/actions/settingsActions.ts`, Handler in
`src/stores/settingsActionHandlers.ts`.

| Kennung                                | Eingabe                                                                 | Ergebnis            | Bereich           | Wirkung | Agent |
| -------------------------------------- | ----------------------------------------------------------------------- | ------------------- | ----------------- | ------- | ----- |
| `settings.appearance.setColorScheme`   | `{ scope: 'device' \| 'vault', scheme: 'light' \| 'dark' \| 'system' }` | `ColorSchemeResult` | `settings.device` | write   | ja    |
| `settings.appearance.clearColorScheme` | `{ scope: 'device' \| 'vault' }`                                        | `ColorSchemeResult` | `settings.device` | write   | ja    |

`ColorSchemeResult = { effective: Scheme, device?: Scheme, vault?: Scheme }`:
nicht gesetzte Werte fehlen, weil das Schema-Subset kein `null` kennt (wie
`settings.sessionRestore.*`, Spec 022). `settings.get` ergänzt `colorScheme` in
dieser Form.

Alle anderen Einstellungen behalten ihre Aktionen (FR-019); nur die Bedienung
ändert sich (research R5).

## 4. Farbschema (`composables/useColorScheme.ts`)

```text
useColorScheme() → {
  state: Readonly<Ref<ColorSchemeState>>     // device, vault, effective
  loadAsync(deviceUuid): Promise<void>        // liest beide Präferenzen, wendet an
  setAsync(scope, scheme | null): Promise<ColorSchemeState>  // schreibt oder löscht, wendet an
}
```

- Modulweiter Zustand: ein Zustand je Prozess (eine Vault je Prozess, Spec 013).
- Wendet an: `document.documentElement.classList.toggle('dark', isDark(...))`;
  hört auf `matchMedia('(prefers-color-scheme: dark)')` und wendet bei `system`
  neu an (US4 AS2).
- `plugins/colorScheme.client.ts` wendet beim Start `system` an.
  `pages/workspace/[instance].vue` ruft nach dem Öffnen `loadAsync` auf; ein
  Fehler beim Lesen lässt `system` stehen (Edge Case).
- Die Aktions-Handler rufen `setAsync`; die Einstellungsansicht ruft nur
  Aktionen (Spec 020 FR-024).

## 5. Entfallene App (`lib/wm/apps.ts`, `stores/wmActionHandlers.ts`)

- `resolveAppAlias('system.federation')` →
  `{ appId: 'system.settings', at: '/' }`; jede andere Kennung →
  `{ appId, at: null }`.
- `wm.app.open` und `wm.tab.new`: zuerst Alias auflösen, dann `knownApp`; ein
  `at` aus der Eingabe geht vor dem `at` des Alias.
- `pages/federation/[instance].vue` leitet auf `?open=system.settings` um.

## 6. Modell-Store (`stores/models.ts`)

- `downloads: Record<string, DownloadProgressEvent>`; Fortschritt setzt,
  Abschluss entfernt den Eintrag.
- `watchDownloads()`: abonniert `onDownloadProgress`/`onDownloadComplete`
  einmal je Prozess; weitere Aufrufe kehren sofort zurück. Aufruf in
  `pages/workspace/[instance].vue` nach dem Öffnen.

## 7. i18n (de, en)

- `settings.categories.<id>.{title,description}` für die vier Kategorien.
- `settings.locations.<id>.{title,description}` für jeden Ort mit Unteransicht.
- `settings.back` („Zurück zu {title}“).
- `settings.colorScheme.*` (Titel, Optionen, „Wie alle Geräte“).
- `actions.settings.appearance.{setColorScheme,clearColorScheme}`.
- Entfällt: `wm.apps.federation`, `settings.header.forDevice`, die
  Speichern-Texte der umgestellten Einstellungen.
