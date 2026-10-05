# Quickstart: Speicherverbindungen (S3) prüfen

Prüfanleitung für Spec 038. Verträge: [contracts/bridge.md](./contracts/bridge.md),
[contracts/tauri-commands.md](./contracts/tauri-commands.md); Datenmodell: [data-model.md](./data-model.md).

## Voraussetzungen

- Ein Worktree mit `nix develop`; Rust über `scripts/with-nix-host-bridge.sh` (siehe CLAUDE.md).
- Für §4 und §5 ein lokales RustFS im Container, z. B. `docker run -p 9000:9000 rustfs/rustfs` mit einem
  Bucket `holzi-test` und einem Zugangsschlüssel nur für diesen Bucket. Die automatischen Tests (§1–§3)
  brauchen kein Netz.

## 1. Regeln und Bridge ohne Anbieter

```sh
scripts/with-nix-host-bridge.sh cargo test -j 4 --manifest-path src-tauri/Cargo.toml --lib -- \
  remote_storage extensions::remote_storage passwords::access
```

Erwartet: grün. Abgedeckt: Z14 (Eintrag mit Eigentümer unsichtbar für Erweiterung mit `*`, auch als
Verweisquelle), Schlüsselprüfung (Katalog aus R5, SC-003), Liste nur mit Namen (FR-009a), Lesen gegen
Lesen und Schreiben, Aufruf mit Zugangsdaten → 3001 ohne Dialog, Dialog bestätigt/abgebrochen, Grenzen.

## 2. S3-Umsetzung gegen einen lokalen Mock

```sh
scripts/with-nix-host-bridge.sh cargo test -j 4 --manifest-path src-tauri/Cargo.toml --lib -- remote_storage::s3
```

Erwartet: signierte Anfragen, Antworten gelesen, 403/404/5xx richtig abgebildet, Größen- und Zeitgrenze
streamend durchgesetzt, keine Weiterleitung gefolgt.

## 3. Migration und Sync

```sh
scripts/with-nix-host-bridge.sh cargo test -j 4 --manifest-path src-tauri/Cargo.toml --lib -- identity::migrations
```

Erwartet: `0027_storage_connections` läuft, die neuen Tabellen stehen in `SYNCED_TABLES` bzw.
`DEVICE_TABLES`, Trigger-Version erhöht.

## 4. Einstellungen gegen RustFS (manuell)

1. Einstellungen → Speicher → Verbindung hinzufügen: Anbieter „RustFS“, Endpunkt `http://127.0.0.1:9000`
   (als unverschlüsselt gekennzeichnet), Region `us-east-1`, Zugangsdaten, Bucket `holzi-test`.
   Erwartet: Test bestanden, Speicher gespeichert; im Passwortmanager ein Eintrag „S3: RustFS“ mit
   Kennzeichnung „gehört zu: Speicher“; im Bucket kein Testobjekt (SC-004).
2. Zugangsdaten absichtlich falsch → Test „Zugangsdaten falsch“, nichts gespeichert.
3. Zweiten Speicher auf derselben Verbindung anlegen → keine erneute Eingabe der Zugangsdaten.
4. Verbindung entfernen → Vorschau nennt beide Speicher; danach kein Eintrag „S3: RustFS“ mehr.

## 5. Erweiterung gegen RustFS (manuell oder e2e)

Mit einer Test-Erweiterung (Fixture aus `tests/fixtures/extension_e2e`, um `remoteStorage` erweitert):

1. `backends.add` ohne Zugangsdaten → Dialog von holzi, Zugangsdaten eingeben → Kennung zurück, die
   Erweiterung hat „Lesen und Schreiben“.
2. `upload("a/b.txt")`, `list()`, `download("a/b.txt")`, `delete("a/b.txt")` → gelingt; im Bucket liegt das
   Objekt unter `holzi-ext/<extension_id>/a/b.txt`.
3. `upload("../x")` → 3001; `backends.add` mit `accessKeyId` → 3001 ohne Dialog.
4. Passwort-Funktionen der Erweiterung mit Freigabe für `*` → der Eintrag „S3: RustFS“ fehlt in `list`,
   `read` mit seiner Kennung → 1001 (SC-002).
5. Zweites Gerät derselben Vault (Rig aus Spec 033): nach dem Sync ist der Speicher dort nutzbar (SC-005).
