# Contract: e2e-Suite auf Android

**Anforderungen**: FR-033a, FR-033b, FR-033c, SC-007, FR-021 | **Research**: R14

## Aufruf

```text
pnpm test:e2e --platform android [--shard i/n] [--grep <muster>]
```

- `--platform linux` bleibt der Standard; die Szenarien ändern sich nicht.
- `--shard i/n` verteilt nach der zuletzt gemessenen Dauer (`artifacts/e2e-durations.json`,
  Fallback: alphabetisch reihum) und gilt für beide Plattformen.
- Voraussetzungen (Preflight): `adb` mit genau einem verbundenen Emulator oder
  `ANDROID_SERIAL`, installiertes Debug-APK (`com.haex.holzi`, debuggable), `chromedriver`
  passend zur WebView-Hauptversion, `iroh-relay`. Fehlt etwas, bricht der Preflight mit dem
  Namen des Fehlenden ab.

## Plattform `lib/platform/android.ts` (implementiert `DeviceHost`)

| Operation         | Umsetzung                                                                                                |
| ----------------- | -------------------------------------------------------------------------------------------------------- |
| `newData`         | `pm clear com.haex.holzi`; Bildschirmschutz aus (Geräte-Einstellung nach dem ersten Entsperren)          |
| `copyVaultFile`   | Quelle gestoppt; `adb exec-out run-as … tar c` bzw. Linux-Datei → `adb exec-in run-as … sh -c 'cat > …'` |
| `keep`            | App-Daten per `run-as` als Tar in die Fehlerunterlagen                                                   |
| `dispose`         | `pm clear`                                                                                               |
| `start`           | `am start -W -n com.haex.holzi/.MainActivity`, warten auf `webview_devtools_remote_<pid>`, Sitzung       |
| `stop`            | `input keyevent KEYCODE_HOME`, `am kill`, sonst `am force-stop`                                          |
| `kill`            | `am force-stop`                                                                                          |
| `alive`           | `adb shell pidof com.haex.holzi` (synchron)                                                              |
| `screenshot`      | chromedriver `GET /screenshot`                                                                           |
| Fenster schließen | Aufgabe entfernen (`am stack remove` bzw. `cmd activity` je nach API), Ende innerhalb der Frist prüfen   |

Sitzung: `POST /session` an chromedriver mit
`{ "goog:chromeOptions": { "androidPackage": "com.haex.holzi", "androidDeviceSerial": <serial>,
"androidUseRunningApp": true } }`. URLs: `tauri://localhost` → `http://tauri.localhost`,
`holzi-ext://localhost` → `http://holzi-ext.localhost`.

Dienste: Für jeden Port eines Dienstes auf dem Testrechner `adb reverse tcp:P tcp:P`; alle
Geräte benutzen `127.0.0.1`. iroh-Relays in Android-Läufen: `http://127.0.0.1:3340`
(`iroh-relay --dev`); Linux-Läufe bleiben beim geschlossenen Port.

Darstellung: `wm size 2560x1600`, `wm density 320` für Desktop-Szenarien; Android-Fälle für
die Telefonbreite setzen `wm size 1080x2400`, `wm density 480` (360 dp) und setzen danach zurück.

Gemischte Gruppen: Gerät `phone` → Emulator, alle anderen → Linux; ohne `phone` das erste
Gerät. Höchstens ein Android-Gerät pro Gruppe und Shard.

## Ausnahmeliste `scripts/e2e/platform-exclusions.ts`

Form in [data-model.md](../data-model.md#ausnahmeliste-der-e2e-suite-fr-033b-sc-007-vertrag-e2e-androidmd).
Anfangsstand (endgültig):

| Szenario              | Grund                                                    | Gegenfall                         |
| --------------------- | -------------------------------------------------------- | --------------------------------- |
| `extension-files`     | freie Pfade und Ordner beobachten gibt es nicht (FR-016) | `android-not-available`           |
| `extension-dev-mode`  | lädt eine Erweiterung aus einem Ordner (FR-016)          | `android-not-available`           |
| `relaunch-after-lock` | Schließen beendet die App, kein Neustart (FR-006)        | `lock-twice` (Android: App endet) |

`pending`-Einträge entstehen in Stufe 1b für alles, was erst eine spätere Stufe zum Laufen
bringt, jeweils mit Stufe. `pnpm check:e2e-exclusions` schlägt fehl, wenn ein Eintrag ohne
Begründung/Gegenfall/Stufe ist, ein Szenario nicht existiert, die Abdeckung unter 80 % fällt
oder (ab Stufe 5, Schalter in der Datei) `pending` nicht leer ist.

## Neue Android-Fälle (`scenarios/android-*.test.ts`)

| Fall                     | prüft                                                                                                   |
| ------------------------ | ------------------------------------------------------------------------------------------------------- |
| `android-phone-layout`   | 360 dp: kein waagrechtes Scrollen, Bedienelemente innerhalb der Ränder (FR-010, FR-011)                 |
| `android-back-gesture`   | `KEYCODE_BACK`: Tab zurück, Fensterübersicht, schließen, App bleibt (FR-013)                            |
| `android-screen-capture` | Einstellung standardmäßig an, ausschaltbar, Wert pro Gerät (FR-011a)                                    |
| `android-not-available`  | Ordnerauswahl, Terminal, Ordner beobachten, freie Pfade, Entwicklungsmodus → „nicht verfügbar“ (FR-016) |
| `android-close-ends-app` | Tresor schließen → Prozess weg; Neustart → Tresorauswahl (FR-006)                                       |
| `android-process-death`  | `am kill` im Hintergrund → Neustart ohne „anderswo geöffnet“ (FR-007)                                   |
| `vault-file-import`      | Desktop und Android: Tresordatei übernehmen (FR-002a)                                                   |

Diese Fälle laufen nur auf Android (außer `vault-file-import`); sie stehen nicht in der
Abdeckungsrechnung der Desktop-Fälle.
