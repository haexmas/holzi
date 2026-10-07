# Contract: CI für Android

**Anforderungen**: FR-031, FR-032, FR-033, FR-033a, SC-007, SC-008 | **Research**: R15

## Jobs in `.github/workflows/ci.yml`

| Job             | läuft bei         | tut                                                                                                                                                                                                                   | Ergebnis                                     |
| --------------- | ----------------- | --------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------- | -------------------------------------------- |
| `android-build` | PR, Push auf main | Java 17, SDK-Pakete per `sdkmanager`, Rust 1.98.1 + Ziele, `pnpm tauri android build --debug --apk --split-per-abi --target aarch64 --target x86_64`                                                                  | Artefakt `holzi-android-debug` (APK)         |
| `android-e2e`   | PR, Push auf main | Matrix `shard: [1, 2, 3]`; KVM freischalten, Emulator (API 35, `google_apis`, x86_64, Snapshot-Cache), APK installieren, Linux-App für gemischte Gruppen bauen, `pnpm test:e2e --platform android --shard ${shard}/3` | Fehlerunterlagen als Artefakt bei Fehlschlag |
| `android`       | PR, Push auf main | `needs: [android-build, android-e2e]`, scheitert, wenn einer scheitert                                                                                                                                                | **Pflicht-Check** im Branch-Schutz           |

Festgelegte Versionen (dieselben wie die devShell, geprüft von `pnpm check:android-versions`):
`platforms;android-36`, `platforms;android-37.0`, `build-tools;36.0.0`, `build-tools;37.0.0`,
`ndk;28.2.13676358`, Rust `1.98.1`, Ziele `aarch64-linux-android`, `x86_64-linux-android`.

Caches: `Swatinem/rust-cache` mit Schlüssel `android`, `gradle/actions/setup-gradle`, AVD-Snapshot.
Der Job `documentation` führt zusätzlich `pnpm check:e2e-exclusions` und
`pnpm check:android-versions` aus.

## Release `.github/workflows/android-release.yml`

- Auslöser: Tag `v*` und `workflow_dispatch` (zum Prüfen der Signatur ohne Release-Anhang).
- Schritte: wie `android-build`, aber `pnpm tauri android build --apk --target aarch64 --target
x86_64` (Release); vorher `src-tauri/gen/android/keystore.properties` und den Keystore aus
  den Geheimnissen nach `$RUNNER_TEMP` schreiben; danach `apksigner verify` gegen den erwarteten
  Fingerabdruck des Zertifikats (Variable `ANDROID_CERT_SHA256`, öffentlich).
- Geheimnisse: `ANDROID_KEY_BASE64` (Keystore, Base64), `ANDROID_KEY_ALIAS`,
  `ANDROID_KEY_PASSWORD`. Fehlen sie, scheitert der Workflow; es entsteht nie ein unsigniertes
  oder mit Debug-Schlüssel signiertes Release.
- `versionCode` aus der Version in `tauri.conf.json` (Tauri-Standard: Major × 1 000 000 +
  Minor × 1 000 + Patch), damit jedes Release ein Update des vorigen ist.
- Ergebnis: APKs als Release-Anhänge (Store-Veröffentlichung ist nicht Teil der Spec).

## Aufgaben für die Person, die das Repo betreut (nicht im Code)

- Keystore einmalig erzeugen und sicher verwahren (Verlust = keine Updates mehr möglich).
- Die drei Geheimnisse und die Variable `ANDROID_CERT_SHA256` im Repo setzen.
- Den Pflicht-Check `android` im Branch-Schutz von `main` eintragen (ab Stufe 1b).
