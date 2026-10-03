# Vertrag: Verweise zwischen Einträgen

**Spec**: [../spec.md](../spec.md) (FR-044 bis FR-050) | **Research**: [../research.md](../research.md) R3, R4, R12, R13

## Grammatik

```text
reference := "{$" item-id ":" kind "}"
item-id   := 36 Zeichen UUID (Hex und Bindestriche, Groß- oder Kleinbuchstaben; gespeichert klein)
kind      := "username" | "password" | "extra:" key
key       := (plain | escaped)+
plain     := jedes Zeichen außer "}" und "\"
escaped   := "\" jedes Zeichen          // "\}" ist "}", "\\" ist "\"
```

- Ein Platzhalter gilt nur in genau dieser Form **und** mit einer UUID als Kennung; alles
  andere (`{$`, `{$abc}`, `{$<uuid>:url}`, `{$<uuid>:extra:}` mit leerem Schlüssel) bleibt
  Text und wird nie aufgelöst.
- Der Schlüssel von `extra:` ist der Schlüssel eines eigenen Felds der Quelle (Groß- und
  Kleinschreibung zählt; mehrere gleiche Schlüssel: der erste nach `sort_order`).
- `passwords_reference_token` (Rust) baut den Platzhalter samt Schutzzeichen; kein Teil der
  Oberfläche baut ihn selbst.
- **Felder, in denen Platzhalter gelten**: `item_details.username`, `.password`, `.url`,
  `.note`, `item_key_values.value`. **Nicht**: Titel, TOTP-Secret, Schlüssel eigener Felder,
  Tags, Ordnernamen.
- **Verweisbare Werte**: `username`, `password`, `extra:<Schlüssel>`.

Beispiele:

| Text                                 | Ergebnis                                  |
| ------------------------------------ | ----------------------------------------- |
| `{$7c1e…:password}`                  | ein Verweis aufs Passwort der Quelle      |
| `admin-{$7c1e…:extra:PIN}`           | Text `admin-` + Wert des Felds „PIN“      |
| `{$7c1e…:extra:a\}b}`                | Verweis aufs Feld mit dem Schlüssel `a}b` |
| `{$7c1e…:username}{$7c1e…:password}` | zwei Verweise                             |
| `{$7c1e…:url}`, `{$ABC:password}`    | Text (kein Platzhalter)                   |

## Auflösen

`references::resolve(q, ctx, text) -> Result<String, ReferenceError>`; `ctx` trägt den
Aufrufer, dessen Freigaben (`Caller`, `Grant[]`), die besuchten (Eintrag, Wert) und die Stufe.

1. Platzhalter von links nach rechts finden; Text dazwischen bleibt.
2. Für jeden: Quelle laden (`items::item_state`). **Sichtbarkeit** prüfen (`access::visible`
   wie für `read_secret_item`): fehlt die Quelle oder ist sie für den Aufrufer nicht sichtbar
   (Papierkorb für Aufrufer außer dem Nutzer, Tags nicht gedeckt), ist das Ergebnis
   `Err(Missing)` — äußerlich dieselbe Meldung (FR-047).
3. Rohwert der Quelle lesen (`username`, `password` oder der Wert des eigenen Felds).
   Fehlt das eigene Feld: `Err(Missing)`.
4. Enthält der Rohwert Platzhalter, mit `stufe + 1` rekursiv auflösen. Stufe 12 überschritten:
   `Err(TooDeep)`. (Eintrag, Wert) schon besucht: `Err(Cycle)`.
5. Ersetzen. **Das Ergebnis eines Teils ist nie leer, weil ein Fehler war**; ein einzelner
   Fehler macht das ganze Feld zum Fehler (kein Teilergebnis).

Fehler → Oberfläche: `Missing` → „Quelle nicht verfügbar“, `Cycle` und `TooDeep` →
„Verweiskreis oder zu tief“; Kopieren, Anzeigen und Benutzen **verwenden den Platzhalter nie
als Text** (FR-045), sie melden den Fehler (Fehlerart `ReferenceError { kind }`).

## Speichern

`references::validate(q, item_id, field_texts)` läuft in `create_item` und `update_item`
(und im Kopieren, R7): findet für jeden neuen Platzhalter eine Tiefensuche (höchstens 12
Stufen) einen Weg, der wieder auf **diesen** Eintrag trifft, lehnt es mit
`ReferenceCycle { source_item_id }` ab. Einen Verweis auf einen nicht vorhandenen Eintrag
lässt es zu (er kann per Sync ankommen) und kennzeichnet ihn (`status: missing`).

## Listen und Aufrufer von außen

- `headers_in_scope` (Aufrufer von außen): Felder `username` und `url`, deren Wert `{$`
  enthält, sind **leer** (FR-047). `read_secret_item` löst auf (Schritt 2 prüft die Quelle);
  ein Feld mit Fehler fehlt in der Antwort.
- `load_overview` (Nutzer): roh; die Oberfläche zeigt Marken (`passwords_references_parse`).
- `agent_headers` (eingebauter Agent): kennt weder Benutzername noch Adresse (034 FR-027),
  keine Änderung.

## Verwendung und Umwandeln beim Löschen (R12)

- `references::targets_of(q, source_ids) -> Vec<ReferenceUsage>`: je Quelle die Zahl der Ziele
  als `text` (Einträge, deren Textspalten `{$<id>:` enthalten) und `passkeyLinks`.
- `references::inline_all(tx, source_ids)`: ersetzt in jedem Ziel jeden Platzhalter auf diese
  Quellen durch den Wert, aufgelöst mit den Rechten des Nutzers (Kette bis zum Ende), und
  nimmt je Ziel einen Stand auf (`take_snapshot`). Passkey-Verbindungen der Quelle werden
  gelöscht.

## Verlauf

`snapshots.rs` speichert die rohen Spalten; ein Platzhalter bleibt Platzhalter. Beim
Wiederherstellen gilt der Platzhalter wieder; fehlt die Quelle, meldet holzi es
(`status: missing`) und stellt den Rest her.

## KeePass-Import (R13)

`{REF:<Feld>@<Suche>:<Text>}` (KeePass): Feld `U` → `username`, `P` → `password`; Suche `I`
mit 32 Hex-Zeichen = Kennung der Quelle in der Datei; `T`, `U`, `P`, `A`, `N`, `O` = Text im
Titel, Benutzernamen, Passwort, der Adresse, den Notizen, einem eigenen Feld, **genau ein**
Treffer in der Datei. Andere Felder (T, A, N, I, O) und alles ohne eindeutigen Treffer
bleiben Text.

## Testvektoren

Die Datei `src-tauri/tests/fixtures/reference_vectors.json` (beim Bau anzulegen) hält die
Beispiele oben und weitere (Kette der Tiefe 12, Kreis A→B→A, Kreis A→A, Quelle fehlt,
Quelle im Papierkorb, Schlüssel mit `}` und `\`, Text ohne Platzhalter, `{$` am Ende).
Die Rust-Tests (`references_tests.rs`) lesen sie; das Frontend hat **keine** Kopie der
Grammatik und braucht keine (R11).
