# Datenmodell: Passwortmanager-Redesign

**Spec**: [spec.md](./spec.md) | **Plan**: [plan.md](./plan.md) | **Research**: [research.md](./research.md)

Aufbauend auf dem Datenmodell von [034](../034-password-manager/data-model.md): Es gibt **keine
Änderung an vorhandenen Tabellen** (keine neue Spalte, kein neuer Index auf alten Tabellen).
Verweise sind Text in vorhandenen Spalten (R3); neu sind zwei CRDT-Tabellen. Beide entstehen
in Migration `0024_passwords_refs` (SQL in `src-tauri/src/identity/migrations_passwords_refs.rs`,
weil `identity/migrations.rs` schon über 500 Zeilen steht), sind CRDT-synchronisiert (kein
`_no_sync`-Suffix), gehören der Vault, haben **keine UNIQUE-Constraints** (ein Konflikt hält
den Sync an; Eindeutigkeit über abgeleitete Kennungen, wie in 034 R2) und werden beim
endgültigen Löschen eines Eintrags oder Passkeys **ausdrücklich zuerst** gelöscht
(`trash::purge_item`; der Sync wendet entfernte Löschungen ohne Fremdschlüssel an).
`HOLZI_TRIGGER_VERSION` 15 → 16 (wie bei 0022 und 0023, damit die CRDT-Trigger der neuen
Tabellen angelegt werden); eine Zeile im Versionsverlauf der Konstante.

## Neue Tabellen

### `haex_passwords_passkey_counters` — Zähler je Passkey und Gerät (R5)

| Spalte       | Typ                        | Regel                                                                                  |
| ------------ | -------------------------- | -------------------------------------------------------------------------------------- |
| `id`         | TEXT PK                    | UUIDv5 (`NS_PASSKEY_COUNTER`) aus `passkey_id` + `:` + `device_id` (kein UNIQUE nötig) |
| `passkey_id` | TEXT NOT NULL              | Passkey (`haex_passwords_passkeys.id`); FK `ON DELETE CASCADE`                         |
| `device_id`  | TEXT NOT NULL              | `vault_device_uuid` des schreibenden Geräts (Gerät, nicht Installation)                |
| `count`      | INTEGER NOT NULL DEFAULT 0 | Letzter von diesem Gerät gesendeter Zähler; wächst nur                                 |
| `updated_at` | TEXT                       | `CURRENT_TIMESTAMP`                                                                    |

Index: `idx_haex_passwords_passkey_counters_passkey_id (passkey_id)`.

**Wirksamer Zähler** eines Passkeys: `max(passkeys.sign_count, max(count))` über seine Zeilen.
Beim Bestätigen schreibt das Gerät `wirksamer Zähler + 1` in **seine** Zeile (Upsert über die
abgeleitete Kennung) — jede Zelle hat genau einen Schreiber, LWW kann nichts überschreiben,
und das Maximum sinkt nie. Die Spalte `passkeys.sign_count` ist nach dieser Spec der
Ausgangswert (Import, haex-vault) und wird nicht mehr geschrieben.

### `haex_passwords_passkey_links` — Passkey per Verweis (R6)

| Spalte       | Typ           | Regel                                                                          |
| ------------ | ------------- | ------------------------------------------------------------------------------ |
| `id`         | TEXT PK       | UUIDv5 (`NS_PASSKEY_LINK`) aus `item_id` + `:` + `passkey_id`                  |
| `item_id`    | TEXT NOT NULL | **Ziel**: Eintrag, der den Passkey eines anderen zeigt; FK `ON DELETE CASCADE` |
| `passkey_id` | TEXT NOT NULL | **Quelle**: der verwiesene Passkey; FK `ON DELETE CASCADE`                     |
| `created_at` | TEXT          | `CURRENT_TIMESTAMP`                                                            |

Indizes: `idx_haex_passwords_passkey_links_item_id (item_id)`,
`idx_haex_passwords_passkey_links_passkey_id (passkey_id)`.

Regeln: Ein Passkey hängt nie über eine Verbindung an dem Eintrag, dem er ohnehin gehört
(`passkeys.item_id == item_id`: abgelehnt). Verbindungen auf Verbindungen gibt es nicht
(die Quelle ist immer ein Passkey mit `item_id`). Doppelte Verbindungen (Sync-Wettlauf)
zeigen im Ziel nur einmal (Gruppierung nach `passkey_id`).

## Verweise im Text (keine Tabelle)

Platzhalter stehen in `item_details.username`, `.password`, `.url`, `.note` und
`item_key_values.value`; die Grammatik in [contracts/references.md](./contracts/references.md).
Kein Wert wird aufgelöst gespeichert, auch nicht in `item_snapshots.data` (FR-049).
Die Zahl der Verweise auf einen Eintrag ist abgeleitet (R12), nicht gespeichert.

## Nicht geändert, aber betroffen

- `haex_passwords_passkeys`: `item_id` bleibt nullbar (alte Daten); ein Passkey ohne Eintrag
  zählt für alle Aufrufer außer dem Nutzer als nicht vorhanden (Spec, Randfall) und wird in
  der Oberfläche nicht angezeigt. holzi legt keinen an; der Import legt immer einen Eintrag an
  (034 FR-023).
- `trash::purge_item` löscht zusätzlich `passkey_counters` und `passkey_links` (als Quelle und
  als Ziel) **vor** den Passkeys.
- `restore` aus dem Papierkorb hebt nichts auf: Verbindungen und Platzhalter sind nicht an den
  Ort gebunden.

## Zustand im Frontend (nichts gespeichert)

| Zustand        | Ort                            | Inhalt                                                                                                      |
| -------------- | ------------------------------ | ----------------------------------------------------------------------------------------------------------- |
| Ablage         | `stores/passwordsClipboard.ts` | `{ ids: Target[], mode: 'cut' \| 'copy' }`; nur Kennungen; leer bei `reset`, letztem Fenster, Vault-Wechsel |
| Auswahl        | `stores/passwordsSelection.ts` | unverändert (034); endet beim Ordnerwechsel und Öffnen eines Eintrags                                       |
| Tab            | Ort des Fenster-Tabs (`?tab=`) | `details` \| `extra`; `entry/:id/history` = Verlauf                                                         |
| Kopier-Dialog  | `CopyDialog.vue` (lokal)       | Titel/Zusatz, `history`, `usernameAsReference`, `passwordAsReference`, `passkeysAsLinks`                    |
| Vorschaubilder | `lib/passwords/thumbnails.ts`  | LRU 200, Blob-URLs nach Prüfsumme; beim Schließen des Fensters freigegeben                                  |

## Typänderungen (ts-rs nach `src/types/bindings/`)

- `PasskeyView` bekommt `itemId`, `isDiscoverable`, `signCount` (wirksamer Zähler),
  `linkedFrom: { itemId, title } | null` (gesetzt bei Verbindung, dann ohne Umbenennen und
  Löschen, nur „Verweis lösen“).
- `ItemDetail` bekommt `passwordReferences`, `usernameReferences`, `urlReferences`,
  `noteReferences` (je `RefMark[]`) und `keyValues[].references` — nur Zustand und Quellen,
  keine Werte; `passkeys` enthält zusätzlich die per Verbindung verwiesenen.
- `RefMark { start, end, sourceItemId, sourceTitle: string | null, kind: 'username' | 'password' | 'extra', key: string | null, status: 'ok' | 'missing' | 'cycle' | 'tooDeep' }`
  (Positionen in Zeichen des **Rohtexts**).
- `ItemHeader.hasPassword` zählt einen Verweis im Passwort als Passwort.
- `ImportReport` bekommt `referencesConverted` und `referencesLeftAsText`.
- Neue Typen: `CopyOptions`, `CopyReport`, `ReferenceUsage`, `PasskeyHeader`,
  `PasskeyCreateRequest/Response`, `PasskeyConfirmRequest/Response` (die letzten vier nur
  im Dienst; ts-rs nur, wenn ein Command sie braucht — heute keiner).
