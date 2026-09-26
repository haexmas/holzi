# Data Model: Einstellungs-App mit Kategorien

**Spec**: [spec.md](./spec.md) | **Research**: [research.md](./research.md)

Diese Spec legt keine Tabellen und keine Rust-Typen an. Neu sind ein
Frontend-Register, eine Präferenz und Zustand im Modell-Store.

## Kategorie (`SettingsCategory`)

Reines Datum in `src/lib/settings/registry.ts` (R2).

| Feld             | Typ                                                 | Regel                                                       |
| ---------------- | --------------------------------------------------- | ----------------------------------------------------------- |
| `id`             | `'general' \| 'appearance' \| 'models' \| 'agents'` | eindeutig; Reihenfolge des Arrays = FR-005                  |
| `path`           | `string`                                            | Ort der Kategorie: `/`, `/appearance`, `/models`, `/agents` |
| `icon`           | `string`                                            | Iconify-Name (`lucide:*`)                                   |
| `titleKey`       | `string`                                            | `settings.categories.<id>.title`                            |
| `descriptionKey` | `string`                                            | `settings.categories.<id>.description`                      |

Eine Kategorie mit Unteransichten zeigt an ihrem Ort die Übersicht; eine ohne
(Allgemein, Darstellung) zeigt ihren Inhalt direkt (FR-003).

## Ort (`SettingsLocation`)

| Feld             | Typ            | Regel                                                                                |
| ---------------- | -------------- | ------------------------------------------------------------------------------------ |
| `id`             | `string`       | eindeutig, z. B. `models.download.search`                                            |
| `pattern`        | `string`       | Routenmuster relativ zu `/`, z. B. `models/download/repo/:owner/:name`; `''` für `/` |
| `category`       | Kategorie-`id` | bestimmt die Hervorhebung in der Seitenleiste                                        |
| `parent`         | Ort-`id` \| —  | nur Unteransichten; Ziel des Zurück-Pfeils (R3)                                      |
| `icon`           | `string` \| —  | nur Orte, die als Zeile einer Übersicht erscheinen                                   |
| `titleKey`       | `string`       | Kopf und Tab-Titel; darf Routenparameter nutzen (`{owner}/{name}`)                   |
| `descriptionKey` | `string`       | eine Zeile unter dem Titel und in der Übersichtszeile                                |
| `overviewRow`    | `boolean`      | erscheint als Zeile in der Übersicht seiner Kategorie                                |

Die vollständige Liste steht in R1 und im [Vertrag](./contracts/settings-app.md#1-orte).

Abgeleitet:

- `settingsRoutePatterns()`: `RoutePattern[]` für `routeMatch.ts`, Wurzel `/`
  mit einem Kind je Ort.
- `locationFor(path)`: der Ort eines Pfads über `matchRoute`, `undefined` für
  Unbekanntes.
- `overviewRows(categoryId)`: Orte mit `overviewRow` in Registerreihenfolge.
- `headerBack(history, parentPath)`: `{ kind: 'back' }` oder
  `{ kind: 'push', path }` (R3).

## Farbschema (Präferenz)

| Eigenschaft        | Wert                                                                             |
| ------------------ | -------------------------------------------------------------------------------- |
| Schlüssel          | `appearance.color_scheme`                                                        |
| Werte              | `light`, `dark`, `system`; jeder andere gespeicherte Wert gilt als nicht gesetzt |
| Scopes             | `device` (vault_device_uuid) und `vault`, Gerät vor Vault                        |
| Standard           | `system`, wenn keiner der beiden gesetzt ist                                     |
| Sync               | wie alle Präferenzen (Tabelle `preferences`)                                     |
| Vor dem Entsperren | `system` (keine Vault offen)                                                     |

`ColorSchemeState = { device: Scheme | null, vault: Scheme | null, effective: Scheme }`
mit `Scheme = 'light' | 'dark' | 'system'`. Dunkel ist die Ansicht, wenn
`effective` `dark` ist oder `system` und das Betriebssystem dunkel meldet.

## Download-Fortschritt (Modell-Store)

In `stores/models.ts` (R6):

| Feld               | Typ                                     | Regel                                                                                |
| ------------------ | --------------------------------------- | ------------------------------------------------------------------------------------ |
| `downloads`        | `Record<string, DownloadProgressEvent>` | Schlüssel Modellkennung; gesetzt bei Fortschritt, entfernt bei Abschluss oder Fehler |
| `watchDownloads()` | `() => Promise<void>`                   | abonniert einmal je Vault-Session; weitere Aufrufe tun nichts                        |

Die vorhandenen Felder `downloadingId`, `downloadProgressBytes`,
`downloadTotalBytes` (Download aus dem Katalog im Chat) bleiben.

## Alias entfallener Apps

In `lib/wm/apps.ts` (R7):

```text
LEGACY_APP_ALIASES: Record<string, { appId: string; at: string }>
  'system.federation' → { appId: 'system.settings', at: '/' }
resolveAppAlias(appId) → { appId, at: string | null }
```

## Entfällt

- App-Definition `system.federation`, `components/apps/FederationApp.vue`,
  i18n `wm.apps.federation`.
- Lokaler Zustand in `HuggingFaceModelManagement.vue` (`activeTab`,
  `selectedRepo`, `downloadStates` und Abonnements); die Komponente selbst wird
  aufgeteilt (R10).
- i18n `settings.header.forDevice` (der Kopf zeigt die Kategorie).
