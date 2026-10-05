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
enum Scope {                                  // Tag-Namen nach fold; "*" in einer Freigabe ergibt All
  All, Tags(BTreeSet<String>),
  AllExcept { denied: BTreeSet<String>, granted: BTreeSet<String> }, // 017 FR-017: verweigerte Tags
}
struct Grant { action: GrantAction, scope: Scope }
```

`Scope` eines Aufrufers = Vereinigung der Bereiche seiner passenden Freigaben; ein `All` macht
den ganzen Bereich `All`. `AllExcept` deckt alle Einträge außer denen mit einem Tag aus `denied`;
trägt ein solcher Eintrag auch ein Tag aus `granted`, ist er gedeckt (ein erteiltes Tag schlägt ein
verweigertes). Mit `Tags` vereinigt, kommen deren Tags zu `granted`. `create_item` nimmt dort kein
Tag aus `denied`, ein Eintrag ohne Tags liegt im Bereich, und nur `All` und `AllExcept` decken Z9. Es zählt, was eine Freigabe **zum Zeitpunkt der Anfrage** deckt; es
gibt keine Zwischenspeicherung im Dienst.

## Dienst (`passwords/service/`)

Der **einzige Weg zu den Daten**, auch für die Oberfläche (FR-024). Jede Methode nimmt `&Caller`
und die Freigaben des Aufrufers (`&[Grant]`; für `User` und `BuiltinAgent` ignoriert) und prüft
zuerst den Aufrufer:

```text
load_overview(caller = User)                       -> { headers, groups, tags } // Nutzer-Übersicht, inkl. Papierkorb
list_headers(caller, grants)                       -> Vec<ItemHeader> | Vec<AgentHeader>
read_secret_item(caller, grants, item_id)          -> SecretItem        // Geheimnisse einer Einzelabfrage
create_item(caller, grants, input)                 -> item_id
update_item(caller, grants, item_id, patch)        -> ()
delete_item(caller, grants, item_id)               -> ()                // verschiebt in den Papierkorb (FR-015)
```

Daneben hat der Dienst je eine Methode für jede Funktion der Oberfläche (`service/organize.rs`:
Ordner, Verschieben, Reihenfolge, Tags; `trash.rs`; `history.rs`; `attachments.rs`; `passkeys.rs`;
`presets.rs`; `import.rs`), die alle `Caller::User` verlangen (Z11). Es gibt keine zweite Lese-
oder Schreibschicht für Tabellen, an der die Prüfung vorbeiliefe; die Commands rufen nur den
Dienst. `Internal`-Aufrufer legen ihre Freigabe fest im
Code ab (zum Beispiel `Grant { ReadWrite, Tags({"s3"}) }`); es gibt keine für holzi reservierten
Tags. Welche Einträge eine holzi-Funktion nutzt, meldet sie über `EntryUsage` (`usage.rs`,
Hinweis vor dem Löschen, FR-034): ein Trait, das die Funktion beim Start anmeldet und das zu einer
Eintragskennung die Namen der nutzenden Funktionen liefert.

## Regeln

| Nr. | Regel                                                                                                                                                                                                                                                                                                                                                                                                                               | Spec           |
| --- | ----------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------- | -------------- |
| Z1  | `User` darf alles ohne Freigabe.                                                                                                                                                                                                                                                                                                                                                                                                    | FR-031         |
| Z2  | `BuiltinAgent` bekommt höchstens `AgentHeader` über `list_headers`; jede andere Methode ist `Forbidden`, Freigaben haben keine Wirkung.                                                                                                                                                                                                                                                                                             | FR-027         |
| Z3  | Für alle anderen Aufrufer: ohne passende Freigabe der verlangten Art ist die Anfrage `Forbidden`.                                                                                                                                                                                                                                                                                                                                   | FR-029         |
| Z4  | Für Aufrufer außerhalb von `User` liefert `list_headers` nur Einträge im Bereich, nur `ItemHeader`-Felder (nie Passwort, TOTP-Secret, Passkey-Schlüssel, eigene Felder, Notiz, Anhänge); Einträge im Papierkorb nie. Die `User`-Übersicht `load_overview` darf den Papierkorb für die Nutzeroberfläche enthalten.                                                                                                                   | FR-026         |
| Z5  | `read_secret_item` für einen Eintrag **außerhalb** des Bereichs ist `NotFound`, nicht von „nicht vorhanden“ zu unterscheiden.                                                                                                                                                                                                                                                                                                       | FR-029         |
| Z6  | `create_item` mit Bereich `Tags`: die gesendete Tagliste darf nur Tags des Bereichs enthalten und muss mindestens eines enthalten, sonst `Forbidden`.                                                                                                                                                                                                                                                                               | FR-028         |
| Z7  | `update_item` mit Bereich `Tags`: der Eintrag muss **vor** der Änderung im Bereich liegen (sonst `NotFound`) und **danach** mindestens ein Tag im Bereich tragen (sonst `Forbidden`, nichts ändert sich). Tags außerhalb des Bereichs bleiben unverändert (Z12).                                                                                                                                                                    | FR-028         |
| Z8  | `delete_item` verlangt `ReadWrite` und einen Eintrag im Bereich (sonst `NotFound`) und **verschiebt ihn in den Papierkorb**; endgültiges Löschen, Wiederherstellen und Papierkorb leeren gehören zu Z11.                                                                                                                                                                                                                            | FR-015, FR-028 |
| Z9  | Passkeys ohne `item_id` gehören zu keinem Tag-Bereich; nur `All` und `AllExcept` decken sie.                                                                                                                                                                                                                                                                                                                                        | Datenmodell    |
| Z10 | Keine Antwort, kein Fehler, keine Protokollzeile enthält einen Wert eines Geheimnisses.                                                                                                                                                                                                                                                                                                                                             | FR-040         |
| Z11 | Jede Methode außer `list_headers`, `read_secret_item`, `create_item`, `update_item` und `delete_item` ist für andere Aufrufer als `User` `Forbidden` (Ordner, Verschieben, Reihenfolge, Tags, Papierkorb, Verlauf, Anhänge, Passkeys, Voreinstellungen, Import), bis eine spätere Spec dafür eine Regel schreibt.                                                                                                                   | FR-024         |
| Z12 | Ein Aufrufer sieht alle Tags eines Eintrags im Bereich (`list_headers`, `read_secret_item`), aber ein Tag außerhalb seines Bereichs bleibt bei seiner Änderung unverändert: er kann es nicht entfernen (ein Weglassen in der gesendeten Liste ändert nichts), nicht hinzufügen (ein neues Tag außerhalb des Bereichs in der gesendeten Liste ist `Forbidden`) und nicht umbenennen oder löschen (Z11).                              | FR-028         |
| Z13 | Für `read_secret_item`, `update_item` und `delete_item` prüft Z3 zuerst die verlangte Freigabe: fehlt sie, ist das Ergebnis `Forbidden`. Erst bei passender Freigabe sind Einträge im Papierkorb (`group_id` ist `trash` oder ein Nachfahre) für jeden Aufrufer außer `User` nicht vorhanden und das Ergebnis `NotFound`; nichts ändert sich. Grund: `trash` auf ein Ziel im Papierkorb ist `delete_permanently` und gehört zu Z11. | FR-015, FR-028 |

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
deckt die Regeln Z1–Z9 und Z11–Z13 ab (mindestens: fehlende Freigabe liefert auch für einen
Eintrag im Papierkorb zuerst `Forbidden`; eine Freigabe `Read` für Tag `s3` liest ein
`s3`-Eintrag, wird bei einem Eintrag ohne `s3` mit `NotFound` abgewiesen, wird beim
Schreiben mit `Forbidden` abgewiesen; zwei Tags vereinigen sich; `All` deckt alles;
`ReadWrite` deckt Lesen; `BuiltinAgent` mit einer beliebigen Freigabe bekommt nichts
außer Kopfdaten; ein Eintrag im Papierkorb mit Tag im Bereich ist für `Read`, `ReadWrite` und `All`
beim Lesen, Ändern und Löschen `NotFound` und bleibt unverändert, Z13). Die Dienst-Tests (`tests/passwords_access.rs`) prüfen dasselbe gegen die
echte Datenbank, einschließlich „kein Geheimnis in der Liste“ durch Suche nach einem
markierten Wert in der serialisierten Antwort.
