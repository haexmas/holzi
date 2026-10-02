# Vertrag: Bundle-Format `haextension-bundle/2`

Gilt für holzi (Prüfen) und das Werkzeug `haex` im vault-sdk (Erzeugen). Begründung:
[research.md](../research.md) R2, R3. Die Testvektoren liegen im vault-sdk und werden mit Repository, voller
Revision und Pfad nach `src-tauri/tests/fixtures/extension_bundles/` kopiert (Constitution IV).

## Archiv

- Zip, Dateiendung `.xt`, höchstens 64 MiB.
- Einträge nur `Stored` oder `Deflate`, keine Verschlüsselung, keine Symlinks, keine Verzeichniseinträge mit
  Daten. Deterministisch erzeugt: feste Änderungszeit, keine Verzeichniseinträge.
- Eintragszahl im Archivende = Zahl der gelesenen Einträge (doppelte Namen sind sonst unsichtbar).
- Höchstens 2.000 Einträge; je Eintrag ≤ 25 MiB entpackt und Verhältnis ≤ 200:1; gesamt ≤ 64 MiB entpackt.
  Ein Eintrag, der mehr Bytes liefert als angegeben, ist ein Fehler.

## Pfade

- Gültiges UTF-8 in NFC; nur `/` als Trenner; kein führendes `/`; kein `\`, `:`, NUL, Steuerzeichen.
- Keine leeren, `.`- oder `..`-Teile; Teil ≤ 255 Bytes; Pfad ≤ 1024 Bytes.
- Keine zwei Pfade, die nach NFC und Kleinschreibung gleich sind.

## Inhalt

| Pfad                                                                           | Pflicht          | Inhalt                                                     |
| ------------------------------------------------------------------------------ | ---------------- | ---------------------------------------------------------- |
| `haextension/manifest.json`                                                    | ja               | Manifest, byte-genau JCS (RFC 8785), ohne Feld `signature` |
| `haextension/signature.json`                                                   | ja               | Signaturdatei, JCS                                         |
| `<migrationsDir>/…`                                                            | wenn Migrationen | Drizzle-SQL und `meta/_journal.json`                       |
| alles andere                                                                   | –                | App-Dateien; Einstieg laut `entry` (Standard `index.html`) |
| `haextension.config.json`, `haextension/public.key`, `haextension/private.key` | **verboten**     |                                                            |

Eingeschränktes JSON in beiden Steuerdateien: nur ASCII-Schlüssel, nur ganze Zahlen im Bereich
−2^53 … 2^53, keine Gleitkommazahlen, keine doppelten Schlüssel. Nicht kanonische Bytes werden abgelehnt.

## `signature.json`

Hier lesbar umbrochen; die Datei selbst ist kompakt (JCS, ohne Leerzeichen).

```json
{
  "files": [
    { "path": "haextension/manifest.json", "sha256": "<64 hex>", "size": 1234 }
  ],
  "format": "haextension-bundle/2",
  "publicKey": "<64 hex>",
  "signature": "<128 hex>"
}
```

- `files`: jeder Eintrag des Archivs außer `signature.json`, sortiert nach UTF-8-Bytes des Pfads
  (JavaScript: `Buffer.compare`, nicht `.sort()`).
- `publicKey` = `manifest.publicKey`.
- **Signierte Nachricht**: `"haextension-bundle/2\n"` gefolgt von der JCS-Form von `signature.json` ohne den
  Schlüssel `signature`. Ed25519 (rein), Prüfung mit `verify_strict`, schwache Schlüssel abgelehnt.

## Prüfung in holzi

1. Archiv und Pfade nach den Regeln oben lesen (nie `extract`).
2. Für jeden Eintrag Pfad, Größe und SHA-256 berechnen; die Menge muss genau `files` entsprechen.
3. `manifest.publicKey == signature.publicKey`; Manifest nach [permissions.md](./permissions.md) gültig;
   `name` nach FR-004.
4. Signatur prüfen.
5. Erkennung des alten Formats: Manifest mit Feld `signature` und keine `signature.json` → Fehler
   `legacy_signature_format` („mit dem aktuellen Werkzeug `haex` neu signieren“).

Fehlerarten (für Installation und Start): `archive_too_large`, `archive_invalid`, `entry_path_invalid`,
`entry_duplicate`, `entry_too_large`, `entry_ratio`, `entry_kind`, `manifest_not_canonical`,
`manifest_invalid`, `file_mismatch { path }`, `public_key_mismatch`, `signature_invalid`,
`legacy_signature_format`. Jede Art hat einen Test mit einem schlechten Bundle aus den Testvektoren.

## Werkzeug `haex` (Änderung im vault-sdk, L0)

- `haex sign <dist>` schreibt v2: durchläuft mit `lstat` und bricht bei Symlinks ab, wendet die Pfadregeln an,
  schreibt das Manifest als JCS ohne `signature`, erzeugt `signature.json`, packt deterministisch.
- `haex verify <datei.xt>` prüft wie holzi.
- Schlüsselformat bleibt (öffentlicher Schlüssel roh als Hex), damit Präfixe der Tabellen gleich bleiben.
- Programmname einheitlich `haex` (heute `haexhub` in `src/cli/index.ts:13`).
- Gemeinsame Testvektoren: gute Bundles und je Regel ein schlechtes.
- Veröffentlichung als Breaking Change (Hauptversion 4).
