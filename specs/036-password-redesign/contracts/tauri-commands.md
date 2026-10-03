# Vertrag: Neue und geänderte Commands

**Spec**: [../spec.md](../spec.md) | **Datenmodell**: [../data-model.md](../data-model.md) |
**Grundlage**: [034 tauri-commands](../../034-password-manager/contracts/tauri-commands.md)

Regeln wie in 034: Nutzlasten camelCase, Argumente in `args`, Antworttypen als ts-rs-Strukturen,
Tresor-Gate, Aufrufer ist der Eingang (hier immer `Caller::User`). Kein Command gibt ein
Geheimnis zurück außer den schon bestehenden Aufdeck-Commands, die jetzt Verweise auflösen.

## Neu

### `passwords_copy`

```text
args:   { targets: Target[], intoGroupId: string | null,                // null = Wurzel
          options: { title: { exact: string } | { suffix: string },     // exact nur bei genau einem Eintrag
                     history: bool,
                     usernameAsReference: bool, passwordAsReference: bool,
                     passkeysAsLinks: bool } }
result: { itemsCreated, groupsCreated, passkeyLinks, skippedMissing }
errors: NotFound | InvalidInput{field} | IntoTrash      // Ziel im Papierkorb oder darin
```

Eine Schreibtransaktion (alles oder nichts). Fehlende Quellen (gelöscht) werden übersprungen und
in `skippedMissing` gezählt. `exact` bei mehr als einem Ziel oder bei einem Ordner:
`InvalidInput{field:'options.title'}`. Kopieren in den Papierkorb oder einen Teil davon:
`IntoTrash`. Das Ergebnis enthält die Kennungen **nicht** (die Oberfläche lädt neu).

### `passwords_references_parse`

```text
args:   { text }
result: RefMark[]            // Teile mit Position, Quelle, Titel der Quelle, Art, Zustand
```

Das Zerlegen ist rein; Titel der Quelle und `status` kommen aus der Vault und gelten für den
Nutzer (eine Quelle im Papierkorb ist `ok`, sie fehlt nur, wenn es sie nicht gibt). Die
Oberfläche ruft es entprellt auf (200 ms).

### `passwords_reference_token`

```text
args:   { itemId, kind: 'username' | 'password' | 'extra', key?: string }
result: string               // der Platzhalter mit Schutzzeichen
errors: NotFound | InvalidInput{field:'key'}
```

### `passwords_item_key_names`

```text
args:   { itemId }
result: string[]             // Schlüssel der eigenen Felder, nicht deren Werte
```

### `passwords_reference_usage`

```text
args:   { itemIds: string[] }
result: ReferenceUsage[]     // { itemId, targetItems: number, passkeyLinks: number }
```

### `passwords_passkey_unlink`

```text
args:   { itemId, passkeyId }     // Ziel und Quelle
result: { removed: bool }
```

## Geändert

- **`passwords_delete_permanently`**: neuer Parameter `inlineReferences: bool` (Vorgabe `false`).
  Mit `true` setzt die Transaktion vor dem Löschen in allen Zielen den heutigen Wert für jeden
  Platzhalter auf die gelöschten Quellen ein (`references::inline_all`) und löscht danach.
  `passwords_empty_trash` bekommt denselben Parameter.
- **`passwords_get_item`**: `ItemDetail` mit `…References` (siehe Datenmodell) und `passkeys`
  einschließlich Verbindungen. Ohne Werte.
- **`passwords_reveal`, `passwords_copy_field`, `passwords_history_reveal`**: lösen Platzhalter
  auf (`references::resolve` mit den Rechten des Nutzers). Fehler:
  `ReferenceError { kind: 'missing' | 'cycle' | 'tooDeep' }`; der Platzhalter wird nie als Text
  geliefert oder kopiert. `history_reveal` löst aus dem **Stand** nur auf, wenn der Platzhalter
  noch eine lesbare Quelle hat, sonst Fehler `missing`.
- **`passwords_create_item`, `passwords_update_item`**: lehnen einen Kreis ab
  (`ReferenceCycle { sourceItemId }`).
- **`passwords_import_*`**: der Bericht bekommt `referencesConverted` und
  `referencesLeftAsText`.
- **`passwords_passkey_delete`**: löscht bei einer Verbindung nur die Verbindung (`removed: 'link'`),
  sonst den Passkey (`removed: 'passkey'`).

## Fehlerarten (neu, in `useErrorString.ts`)

`ReferenceError` (`kind`), `ReferenceCycle`, `IntoTrash`; die Texte stehen in
`passwords.errors.*` (de, en).
