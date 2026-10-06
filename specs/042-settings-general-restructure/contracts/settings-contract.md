# Contracts: Allgemein mit Grundeinstellung und Erscheinungsbild

## §1 Settings-Locations (`src/lib/settings/registry.ts`)

Kategorien in Sidebar-Reihenfolge: `general` (`/`), `models`, `agents`, `storage`, `extensions`,
`federation`. Keine Kategorie `appearance`.

| Location-ID | Pfad | Art | Icon | Inhalt |
|---|---|---|---|---|
| `general` | `/` | Kategorie, Übersicht | `lucide:sliders-horizontal` | Zeilen `general.basic`, `general.appearance` |
| `general.basic` | `/general/basic` | `overviewRow` | `lucide:settings-2` | Sprache, Zeile → Passwort, Gerätename, Sitzung wiederherstellen |
| `general.basic.password` | `/general/basic/password` | `row` | `lucide:key-round` | Passwort-Formular |
| `general.appearance` | `/general/appearance` | `overviewRow` | `lucide:palette` | Farbschema, Hintergrund, bisherige `AppearanceView` |

`settingKeys` (Suche, FR-005):

- `general.basic`: `settings.language.label`, `settings.alias.label`, `settings.sessionRestore.title`
- `general.appearance`: `settings.colorScheme.label`, `settings.background.label`,
  `settings.appearance.accent`, `.window`, `.container`, `.text`, `.component`, `.windowHint`

i18n: `settings.locations.general.{basic,appearance}.{title,description,keywords}`,
`settings.locations.general.basic.password.{title,description,keywords}`; `settings.categories.appearance.*`
entfällt.

## §2 Tauri-Command `change_vault_passphrase`

```text
invoke('change_vault_passphrase', { args: { current: string, new: string } }) -> null
```

| Fehler (`kind`) | Wann | Anzeige |
|---|---|---|
| `WrongPassphrase` | aktuelles Passwort öffnet die Datei nicht | „Das aktuelle Passwort ist falsch.“ |
| `WeakPassphrase` | neues < 8 Zeichen oder gleich dem aktuellen | Grund aus `reason`, lokalisiert im Frontend |
| `InvalidInput` | eine andere Vault-Operation läuft | „Gerade läuft eine andere Vault-Operation …“ |
| sonst | SQLCipher-/IO-Fehler | `useErrorString` |

Nicht im Action-Katalog, nicht agent-aufrufbar. Während eines Vault-Schließens abgelehnt
(`APP_SCOPED_COMMANDS`-Default-Deny, `vault_gate/invoke.rs:15-24`).

## §3 Aktionen (`src/lib/actions/settingsActions.ts`)

| ID | Input | Result | Scope | Effect |
|---|---|---|---|---|
| `settings.general.setLanguage` | `{ language: 'de' \| 'en' }` | `{ language }` | `settings.device` | `write` |
| `settings.appearance.removeBackground` | — | `DONE` | `settings.device` | `write` |

`settings.get` ergänzt `language`. Kein `setBackground` (research R8); Agenten öffnen
`/general/appearance` bzw. `/general/basic/password` mit `wm.app.open`.

## §4 Startbildschirm

`pages/index.vue`: Sprachwahl oben rechts (`absolute top-4 right-4`), `data-testid="landing-language"`,
Optionen Deutsch / English, ruft nur `setLocale`.
