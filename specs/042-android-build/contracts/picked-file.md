# Contract: gewählte Datei (`PickedFile`)

**Anforderungen**: FR-015, FR-022, FR-002a | **Research**: R3

## Form an der Grenze

```text
PickedFile = String   // tauri_plugin_fs::FilePath: Pfad (Desktop) oder content://-Adresse (Android)
```

Die Oberfläche bekommt den Wert aus `@tauri-apps/plugin-dialog` (`open`, `save`) über
`usePickedFile()` und reicht ihn unverändert an den Kern. Sie zerlegt ihn nicht, zeigt nur den
Anzeigenamen und leitet nie einen Pfad daraus ab.

## Baustein im Kern: `files::picked`

```text
read(app, file) -> Vec<u8>                       // ganze Datei
open_read(app, file) -> std::fs::File            // für große Dateien (Tresor, Modell)
copy_into(app, file, target: &Path) -> u64       // .tmp + rename, Rückbau bei Fehler
write(app, file, bytes)                          // Speichern-Dialog: Modus "wt"
display_name(file) -> String
```

- Alle Funktionen öffnen über `app.fs().open(FilePath, OpenOptions)`; auf Android liefert das
  Plugin einen Dateideskriptor aus dem `ContentResolver`, am Desktop einen geöffneten Pfad.
- Auf Android lehnen Commands, die eine gewählte Datei erwarten, Pfade außerhalb des
  app-eigenen Speichers ab (`invalid_input`), damit keine freien Pfade durch die Hintertür
  entstehen (FR-016). Pfade im app-eigenen Speicher sind erlaubt (dort legt auch die e2e-Suite
  ihre Testdateien ab).
- Fehler: `Unreadable` (Anbieter liefert nichts, Berechtigung entzogen, Cloud-Datei offline),
  `NotEnoughSpace` (beim Kopieren).

## Umgestellte Abläufe

| Ablauf                                               | Command (neu oder geändert)                                                             |
| ---------------------------------------------------- | --------------------------------------------------------------------------------------- |
| Anhang im Passwortmanager                            | `passwords_attachment_add(entry, file: PickedFile)`                                     |
| Anhang im Chat                                       | `chat_attachment_add(thread, file: PickedFile)`                                         |
| Import aus haex-vault                                | `passwords_import_*(file: PickedFile, …)` (Begleitdatei: R3-Grenze, siehe unten)        |
| Darstellung importieren/exportieren, Hintergrundbild | `appearance_import(file)`, `appearance_export(file)`, `appearance_background_set(file)` |
| Erweiterung aus Datei                                | `extension_install_from_file(file: PickedFile)`                                         |
| Modell aus Datei                                     | `import_model_from_file(file: PickedFile)`                                              |
| Dateidialoge der Erweiterungen                       | Bridge `filesystem.open/save` → Inhalt bzw. Schreiben über `file`                       |
| Tresordatei                                          | `import_instance(file: PickedFile, passphrase)`                                         |

Grenze: Der Import aus haex-vault liest am Desktop eine Begleitdatei neben der Vault-Datei
(Spec 037 FR-002). Über `content://` gibt es kein „daneben“; auf Android wird die Begleitdatei
deshalb nicht gelesen, und der Import-Assistent weist darauf hin (Spec 037 bleibt am Desktop
unverändert).

## Tests

- Rust: `files::picked` mit Pfaden (Desktop-Test); die Prüffunktion „Pfad im app-eigenen
  Speicher“ als Unit-Test mit Pfaden innerhalb, außerhalb und mit `..`.
- e2e: Die vorhandenen Fälle (`passwords-attachments`, `appearance-basic`,
  `extension-install-open`, …) prüfen die Abläufe am Desktop weiter; auf Android legt die
  Plattform die Datei per `run-as` in die Sandbox und übergibt deren Pfad über einen Test-Seam,
  weil chromedriver die System-Dateiauswahl nicht bedienen kann. Die echte Auswahl prüft der
  Quickstart (§3) von Hand.
