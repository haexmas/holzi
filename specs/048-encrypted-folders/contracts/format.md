# Contract: Format verschlüsselter Ordner `HXEF` v1

Dieses Dokument ist die Quelle für FR-042. In PR B zieht es nach
`docs/formats/encrypted-folder-v1.md` um und wird von dort gepflegt; die Testvektoren liegen in
`src-tauri/tests/fixtures/encrypted/v1-vectors.json`. Begründungen stehen in
[research.md](../research.md) R1 bis R8.

## Bausteine

- **AEAD**: XChaCha20-Poly1305 (24-Byte-Nonce, 16-Byte-Tag).
- **KDF**: HKDF-SHA256. `kdf(ikm, info)` heißt immer: Salt = die 5 Byte `holzi`, Länge 32.
- **Kennungen**: 16 zufällige Bytes; als Objektname in Base32 nach RFC 4648 ohne Auffüllung,
  Kleinbuchstaben (26 Zeichen).
- **Zahlen**: Little Endian.
- **Konstanten**: `MAGIC` = `HXEF` (4 Byte), `VERSION` = `0x01`, `CHUNK` = 65 536 Byte Klartext.
- Typ-Bytes: `0x00` Kopf, `0x01` Inhalt, `0x02` Begleitdatei.

## Schlüssel

| Name     | Herkunft                                                          | Zweck                                   |
| -------- | ----------------------------------------------------------------- | --------------------------------------- |
| `VCK_g`  | Inhaltsschlüssel der Vault, Generation `g` (`sync::content_keys`) | nur Eingabe für `KEK_g`                 |
| `KEK_g`  | `kdf(VCK_g, "holzi/encrypted-folder/kek/v1")`                     | verpackt `FK` im Kopf                   |
| `FK`     | 32 zufällige Bytes je Ordner                                      | nur Eingabe für die zwei Unterschlüssel |
| `K_meta` | `kdf(FK, "holzi/encrypted-folder/meta/v1")`                       | Name im Kopf, Begleitdateien            |
| `K_wrap` | `kdf(FK, "holzi/encrypted-folder/wrap/v1")`                       | verpackt Dateischlüssel                 |
| `DEK`    | 32 zufällige Bytes je Inhaltsobjekt (jede Fassung neu)            | Inhalt                                  |

`KEK_g` und `VCK_g` verlassen die Vault nie. Teilen (FR-040) fügt dem Kopf einen weiteren Empfänger
hinzu, der `FK` unter einem anderen Schlüssel trägt.

## Objekte im Speicher

Ein verschlüsselter Ordner mit der Kennung `fid` liegt im Elternpräfix `E` (leer an der Wurzel des
Buckets) unter `P = E + base32(fid) + ".hxef/"`:

| Objekt        | Name                     | Inhalt                    |
| ------------- | ------------------------ | ------------------------- |
| Kopf          | `P + "h"`                | siehe unten               |
| Begleitdatei  | `P + "m/" + base32(sid)` | je Fassung eines Eintrags |
| Inhaltsobjekt | `P + "c/" + base32(cid)` | je Fassung einer Datei    |

Sonst liegt nichts unter `P`. Content-Type aller Objekte ist `application/octet-stream`; holzi setzt
keine eigenen Metadaten.

## Kopf

```text
MAGIC(4) | VERSION(1) | 0x00 | fid(16) | n(1) | Empfänger × n | name_nonce(24) | name_ct
Empfänger = art(1) | key_id(16) | wrap_nonce(24) | wrapped_fk(48)
```

- `art` `0x00` = eigene Vault; `key_id` ist die Kennung von `VCK_g` (`content_keys`, 16 Byte). Andere
  Arten sind für Spaces reserviert; ein Leser überspringt Empfänger, die er nicht kennt.
- `wrapped_fk = AEAD(KEK_g, wrap_nonce, FK, aad = MAGIC|VERSION|0x00|fid|art|key_id)`.
- `name_ct = AEAD(K_meta, name_nonce, utf8(Name), aad = MAGIC|VERSION|0x00|fid|"name")`.
- Umbenennen schreibt den Kopf mit neuem `name_nonce` neu; die Empfänger bleiben.
- Ein Leser lehnt ab: andere `MAGIC`, `VERSION` > 1 (Zustand „neuere Formatversion“), `fid` passt nicht
  zum Präfix, kein entpackbarer Empfänger (Zustand „andere Vault“), Name nicht zu öffnen
  („beschädigt“).

## Inhaltsobjekt

```text
Kopf:   MAGIC(4) | VERSION(1) | 0x01 | cid(16) | nonce_prefix(16)        = 38 Byte
Block i: AEAD(DEK, nonce_prefix | u64(i), block_i, aad = Kopf(38) | last_i)
```

- `last_i` = `0x01` für den letzten Block, sonst `0x00`.
- Blöcke haben `CHUNK` Byte Klartext, der letzte 0 bis `CHUNK`. Jedes Objekt hat mindestens einen
  Block; eine leere Datei ist ein Kopf und ein leerer letzter Block (16 Byte).
- Länge: `38 + 16 × k + Größe` mit `k = max(1, ⌈Größe / CHUNK⌉)`.
- Teilbereich `[a, b)` des Klartexts: Blöcke `⌊a / CHUNK⌋` bis `⌊(b − 1) / CHUNK⌋`, Block `i` beginnt
  bei `38 + i × (CHUNK + 16)`. Der Kopf wird einmal je geöffneter Datei gelesen.
- Ein Leser prüft: `cid` im Kopf = `cid` aus der Begleitdatei; Länge des Objekts passt zur Größe aus
  der Begleitdatei; der letzte Block trägt `last = 0x01`, kein anderer. Beim vollständigen Lesen
  (Herunterladen, Transfer) zusätzlich SHA-256 des Klartexts.

## Begleitdatei

```text
MAGIC(4) | VERSION(1) | 0x02 | sid(16) | nonce(24) | AEAD(K_meta, nonce, json, aad = MAGIC|VERSION|0x02|fid|sid)
```

`json` ist UTF-8 ohne Leerraum, Felder in dieser Reihenfolge:

| Feld       | Typ                | Regel                                                       |
| ---------- | ------------------ | ----------------------------------------------------------- |
| `v`        | Zahl               | `1`                                                         |
| `entry`    | Base32             | Kennung des Eintrags, bleibt über Fassungen gleich          |
| `parent`   | Base32             | Kennung des Elterneintrags; `fid` an der Wurzel des Ordners |
| `name`     | Text               | Name nach den Regeln von 044 (`check_name`)                 |
| `kind`     | `file` \| `folder` |                                                             |
| `revision` | Zahl               | 1 bei der ersten Fassung, dann +1                           |
| `base`     | Base32 \| `null`   | `sid` der Begleitdatei, die diese ersetzt                   |
| `written`  | Zahl               | Zeit des Schreibens, ms seit 1970                           |
| `size`     | Zahl               | nur `file`: Größe des Klartexts                             |
| `modified` | Zahl               | nur `file`: Änderungszeit der Datei, ms                     |
| `type`     | Text \| `null`     | nur `file`: Inhaltstyp, wenn bekannt                        |
| `sha256`   | Hex                | nur `file`: Prüfsumme des Klartexts                         |
| `content`  | Base32             | nur `file`: `cid`                                           |
| `dek`      | Base64             | nur `file`: `nonce(24)                                      | AEAD(K_wrap, nonce, DEK, aad = fid | entry | cid)` |

## Regeln für Leser und Schreiber

- **Ansicht**: alle Begleitdateien laden. Gibt es für eine `entry` mehrere, gilt die Fassung, die keine
  andere als `base` hat. Haben zwei Fassungen dieselbe `base`, ist das ein Konflikt: beide gelten, die
  mit dem kleineren `written` wird als „Name (Konflikt JJJJ-MM-TT hh-mm)“ gezeigt. Gleiche Namen
  verschiedener Einträge im selben Elterneintrag werden genauso benannt.
- **Einträge ohne Eltern** (Elterneintrag gelöscht, während ein anderes Gerät etwas hineinlegte)
  erscheinen im virtuellen Ordner „Wiederhergestellt“ an der Wurzel des verschlüsselten Ordners.
- **Schreiben**: zuerst das neue Inhaltsobjekt ganz, dann die neue Begleitdatei, danach alte
  Begleitdatei und altes Inhaltsobjekt löschen.
- **Aufräumen**: Begleitdateien, die eine andere als `base` hat, und Inhaltsobjekte ohne Verweis
  löscht holzi, wenn ihre Änderungszeit beim Anbieter älter als 24 Stunden ist.
- **Löschen eines Eintrags**: Inhaltsobjekte und Begleitdateien aller Nachkommen, zuletzt die eigene.
- **Löschen des Ordners**: alle Objekte unter `P`, der Kopf zuletzt.

## Testvektoren

`v1-vectors.json` enthält mit festen Schlüsseln, Kennungen und Nonces:

1. `KEK_g`, `K_meta`, `K_wrap` aus festen `VCK_g` und `FK`.
2. Einen Kopf mit einem Empfänger und Namen „Unterlagen“.
3. Inhaltsobjekte mit 0, 1, 65 536 und 65 537 Byte Klartext.
4. Eine Begleitdatei für eine Datei und eine für einen Ordner.
5. Negativfälle mit erwarteter Ablehnung: verändertes Byte, gekürzt an einer Blockgrenze, verlängert,
   zwei Blöcke vertauscht, `last` am falschen Block, Inhaltsobjekt einer anderen Datei, Begleitdatei
   eines anderen Ordners, Begleitdatei unter anderem `sid`.
