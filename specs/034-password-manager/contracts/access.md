# Vertrag: Zugriff und Freigaben (Rust)

**Spec**: [../spec.md](../spec.md) (FR-024 bis FR-035) | **Commands**: [tauri-commands.md](./tauri-commands.md)

Dieser Vertrag legt fest, wie jeder Zugriff auf Passwortmanager-Daten geprüft wird. Er ist
die Schnittstelle, die Spec 029 (holzi-Funktion), 017–019 (haextensions) und 021 (externe
Agenten, External Bridge später) benutzen. Wo Freigaben **gespeichert, erteilt, angezeigt und
widerrufen** werden, regeln diese Specs, nicht diese (FR-030).

## Typen (`src-tauri/src/passwords/access.rs`, rein, ohne Datenbank)

```text
enum Caller {
  User,                               // Oberfläche des Nutzers
  BuiltinAgent,                       // Modell im Chat (Spec 032)
  Extension { id },                   // haextension (017–019)
  ExternalAgent { id },               // MCP-Client (021)
  Internal { feature: &'static str }, // holzi-Funktion, z. B. "s3-storage" (029)
}
enum GrantAction { Read, ReadWrite }  // ReadWrite deckt Read
enum Scope { All, Tags(BTreeSet<String>) }   // Tag-Namen nach fold; "*" in einer Freigabe ergibt All
struct Grant { action: GrantAction, scope: Scope }
```

`Scope` eines Aufrufers = Vereinigung der Bereiche seiner passenden Freigaben; ein `All` macht
den ganzen Bereich `All`. Es zählt, was eine Freigabe **zum Zeitpunkt der Anfrage** deckt; es
gibt keine Zwischenspeicherung im Dienst.

## Dienst (`passwords/service.rs`)

Der einzige Weg zu den Daten außer der Oberfläche. Jede Methode nimmt `&Caller` und die
Freigaben des Aufrufers (`&[Grant]`; für `User` und `BuiltinAgent` ignoriert):

```text
list_headers(caller, grants)                       -> Vec<ItemHeader> | Vec<AgentHeader>
read_secret_item(caller, grants, item_id)          -> SecretItem        // Geheimnisse einer Einzelabfrage
create_item(caller, grants, input)                 -> item_id
update_item(caller, grants, item_id, patch)        -> ()
delete_item(caller, grants, item_id)               -> ()                // endgültig, nicht Papierkorb
```

Die Oberfläche geht über dieselben Funktionen (`Caller::User`); daneben gibt es keine zweite
Lese- oder Schreibschicht für Tabellen (FR-024). `Internal`-Aufrufer legen ihre Freigabe fest im
Code ab (zum Beispiel `Grant { ReadWrite, Tags({"holzi:s3"}) }`), mit dem reservierten
Tag-Präfix `holzi:` (Hinweis vor dem Löschen, FR-034).

## Regeln

| Nr. | Regel                                                                                                                                                                                                     | Spec        |
| --- | --------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------- | ----------- |
| Z1  | `User` darf alles ohne Freigabe.                                                                                                                                                                          | FR-031      |
| Z2  | `BuiltinAgent` bekommt höchstens `AgentHeader` über `list_headers`; jede andere Methode ist `Forbidden`, Freigaben haben keine Wirkung.                                                                   | FR-027      |
| Z3  | Für alle anderen Aufrufer: ohne passende Freigabe der verlangten Art ist die Anfrage `Forbidden`.                                                                                                         | FR-029      |
| Z4  | `list_headers` liefert nur Einträge im Bereich, nur `ItemHeader`-Felder (nie Passwort, TOTP-Secret, Passkey-Schlüssel, eigene Felder, Notiz, Anhänge); Einträge im Papierkorb nie.                        | FR-026      |
| Z5  | `read_secret_item` für einen Eintrag **außerhalb** des Bereichs ist `NotFound`, nicht von „nicht vorhanden“ zu unterscheiden.                                                                             | FR-029      |
| Z6  | `create_item` mit Bereich `Tags`: die gesendete Tagliste muss mindestens ein Tag im Bereich enthalten, sonst `Forbidden`.                                                                                 | FR-028      |
| Z7  | `update_item` mit Bereich `Tags`: der Eintrag muss **vor** der Änderung im Bereich liegen (sonst `NotFound`) und **danach** mindestens ein Tag im Bereich tragen (sonst `Forbidden`, nichts ändert sich). | FR-028      |
| Z8  | `delete_item` verlangt `ReadWrite` und einen Eintrag im Bereich (sonst `NotFound`).                                                                                                                       | FR-028      |
| Z9  | Passkeys ohne `item_id` gehören zu keinem Tag-Bereich; nur `All` deckt sie.                                                                                                                               | Datenmodell |
| Z10 | Keine Antwort, kein Fehler, keine Protokollzeile enthält einen Wert eines Geheimnisses.                                                                                                                   | FR-040      |

## Aufrufer und Eingang

| Eingang                                                    | Aufrufer                     | Freigaben                 | Stand in dieser Spec                                    |
| ---------------------------------------------------------- | ---------------------------- | ------------------------- | ------------------------------------------------------- |
| Commands der Oberfläche (`passwords_*`)                    | `User`                       | —                         | gebaut                                                  |
| `passwords_agent_search` (Aktion `passwords.items.search`) | `BuiltinAgent`               | —                         | gebaut                                                  |
| Rust-Aufruf einer holzi-Funktion                           | `Internal{…}`                | fest im Code              | Schnittstelle gebaut, erster Nutzer ist Spec 029        |
| haextension                                                | `Extension{id}`              | aus 017–019               | nicht gebaut                                            |
| MCP-Client                                                 | `ExternalAgent{id}`          | aus 021                   | nicht gebaut                                            |
| External Bridge (Autofill, Passkeys)                       | `Extension` oder eigener Typ | aus 017–019 / eigene Spec | nicht gebaut; Schnittstelle bleibt unverändert (FR-032) |

Der Aufrufer ist **nie** ein Argument eines Commands: ein Command legt ihn fest, weil er der
Eingang ist.

## Prüfbarkeit (FR-030)

`access.rs` ist rein und wird in `access_tests.rs` ohne Datenbank und ohne Freigabespeicher
geprüft: eine Tabelle aus Aufrufer, Freigaben, Methode, Eintrag-Tags und erwartetem Ergebnis
deckt die Regeln Z1–Z9 ab (mindestens: eine Freigabe `Read` für Tag `s3` liest ein
`s3`-Eintrag, wird bei einem Eintrag ohne `s3` mit `NotFound` abgewiesen, wird beim
Schreiben mit `Forbidden` abgewiesen; zwei Tags vereinigen sich; `All` deckt alles;
`ReadWrite` deckt Lesen; `BuiltinAgent` mit einer beliebigen Freigabe bekommt nichts
außer Kopfdaten). Die Dienst-Tests (`tests/passwords_access.rs`) prüfen dasselbe gegen die
echte Datenbank, einschließlich „kein Geheimnis in der Liste“ durch Suche nach einem
markierten Wert in der serialisierten Antwort.
