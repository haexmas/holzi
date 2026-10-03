# Vertrag: Brücke zwischen Erweiterung und holzi

Richtung B aus ADR-0004. Protokoll des vault-sdk v3.7.0 (`src/messages.ts`, `src/client/transport.ts`,
`src/commands/*.ts` @ `502593e84b8d289b0986a2777754d6bd8f52da5e`). Begründung:
[research.md](../research.md) R12–R14, R17.

## Rahmen

- `<iframe sandbox="allow-scripts" src="holzi-ext://localhost/<extId>/<entry>?hf=<token>#<ort>">`
  (Windows: `http://holzi-ext.localhost/…`), kein `allow-same-origin`, `allow-popups`,
  `allow-top-navigation`, `allow-forms`.
- Das Frontend legt die Rahmensitzung mit `extension_frame_open(extId, tabId)` an und bekommt `frame`
  und die URL mit dem Start-Token (`?hf=<token>`). `frame` erreicht die Erweiterung nie.
- Der Protokoll-Handler liefert HTML nur bei gültigem `token` für die `extId` des Pfads, setzt die CSP
  aus R12 als Header und fügt den Rahmen-Shim (unten) ein.

## Kanal (SDK)

1. Bei jedem `load` des Rahmens: alter Port zu, neuer `MessageChannel`, alle 200 ms
   `contentWindow.postMessage({type: "haexspace:port:init"}, "*", [port2])` bis `{type: "haexspace:port:ready"}`
   auf `port1` kommt (höchstens 10 s, dann Fehleransicht mit „Neu laden“).
2. Danach Anfragen `{id, method, params, timestamp}` auf dem Port; Antwort `{id, result}` oder
   `{id, error: {code, message, details?}}`.
3. Meldungen holzi → Erweiterung: `{type, data?, timestamp, …}` auf dem Port; vor `ready` gepuffert.
4. Konsolenausgabe und `haexspace:debug` kommen über `window.parent.postMessage` und zählen nur, wenn
   `event.source === iframe.contentWindow`.
5. SDK-Änderung (L0): `waitForHostPortAsync` nimmt `port:init` nur an, wenn `event.source === window.parent`.

## Rahmen-Shim (holzi, zweiter Port)

Inline-Skript, das holzi in jedes ausgelieferte HTML-Dokument der Erweiterung einfügt (CSP-Hash). holzi
schickt `{type: "holzi:frame:init", shortcuts: [...]}` mit einem eigenen Port; der Shim nimmt es nur von
`window.parent` an.

Der Shim läuft im JavaScript der Erweiterung; jede Nachricht auf seinem Port ist eine Eingabe der Erweiterung
und kann gefälscht sein. `nav`, `title`, `closeGuard` und `close` wirken nur auf den eigenen Tab. `shortcut`
führt holzi nur aus, wenn der Rahmen gerade den Fokus hat (`document.activeElement` ist das iframe) und das
Fenster von holzi aktiv ist; sonst wird es verworfen. Mehr als ein Tastendruck des Nutzers in diesem Moment
kann die Erweiterung damit nicht auslösen.

| Shim → holzi                 | Auslöser                                    | Wirkung in holzi                         |
| ---------------------------- | ------------------------------------------- | ---------------------------------------- |
| `nav {path, query, replace}` | `hashchange`, `popstate`                    | `useTabRouter().push/replace` (Spec 020) |
| `title {text}`               | Änderung von `document.title`               | `useWmTab().setTitle`                    |
| `closeGuard {active}`        | `beforeunload`-Handler registriert/entfernt | `registerCloseGuard`                     |
| `close`                      | `window.close()`                            | `closeSelf`                              |
| `shortcut {id}`              | `keydown` passend zu `shortcuts`            | Aktion des Kürzels                       |

| holzi → Shim             | Wirkung                               |
| ------------------------ | ------------------------------------- |
| `navigate {path, query}` | setzt den Hash (Zurück/Vor in holzi)  |
| `shortcuts {list}`       | aktualisiert die abzufangenden Kürzel |

## Weiterleitung an Rust

Das Frontend ruft für jede Anfrage `extension_bridge_call({frame, id, method, params})` und gibt die Antwort
unverändert zurück. Ereignisse von Rust: `extension-frame-event {frame, type, data}` (bereits gefiltert),
`extension-permission-request` (nur für die Oberfläche, siehe [permissions.md](./permissions.md)).
`Uint8Array`/`ArrayBuffer` in `params` werden zu `{"$bytes": "<base64>"}`.

## Fehlercodes

| Code                             | Bedeutung                                                                          | Herkunft                             |
| -------------------------------- | ---------------------------------------------------------------------------------- | ------------------------------------ |
| 1000                             | Sicherheitsverstoß (Form nicht zuordenbar, gesperrter Pfad, …)                     | HV                                   |
| 1001                             | nicht gefunden                                                                     | HV                                   |
| 1002                             | verweigert                                                                         | HV, SDK `PermissionErrorCode.DENIED` |
| 1004                             | Anfrage nötig (SDK wartet auf `extension:permission-resolved` und wiederholt)      | HV, SDK                              |
| 2000 / 2001 / 2002 / 2003 / 2005 | Datenbank / Dateisystem / HTTP / Shell / Web                                       | HV                                   |
| 3000 / 3001                      | Manifest / Eingabe ungültig                                                        | HV                                   |
| 7000                             | Grenze überschritten (Zeilen, Laufzeit, Größe, gleichzeitig)                       | HV                                   |
| **8000**                         | nicht unterstützt (Methode unbekannt oder bewusst nicht angeboten, FR-060/061)     | holzi                                |
| **8001**                         | nicht verfügbar (Funktion fehlt auf diesem Gerät oder in dieser Fassung von holzi) | holzi                                |
| **8002**                         | Erweiterung deaktiviert                                                            | holzi                                |

Fehlertexte nennen keine Tabelle, Datei oder Eintrag außerhalb der Berechtigungen (FR-062).

## Methoden (Erlaubtliste)

Jede Methode, die hier nicht steht, antwortet 8000. Spalte „L“ = Lieferung (research R1). Berechtigung siehe
[permissions.md](./permissions.md).

| Methode (SDK)                                                                                                       | L     | Berechtigung                            | Bemerkung                                                                                                                                     |
| ------------------------------------------------------------------------------------------------------------------- | ----- | --------------------------------------- | --------------------------------------------------------------------------------------------------------------------------------------------- |
| `extension_context_get`                                                                                             | L1    | –                                       | `{theme, locale, platform, deviceId}`                                                                                                         |
| `extension_get_info`                                                                                                | L1    | –                                       | nur der eigenen Erweiterung                                                                                                                   |
| `extension_tab_attention`                                                                                           | L1    | –                                       | `{active}`; nur für den eigenen Tab (SDK-Ergänzung aus L0)                                                                                    |
| `extension_dialog_confirm`                                                                                          | L1    | –                                       | `{message, title?, confirmLabel?, cancelLabel?, destructive?}` → `true`/`false`; Dialog über dem eigenen Tab, ersetzt `confirm()` (R12, T118) |
| `extension_database_query`, `extension_database_execute`                                                            | L1    | eigene Tabellen frei; fremde `database` | Parameter `sql` oder `query`; [sql-policy.md](./sql-policy.md)                                                                                |
| `extension_database_transaction`                                                                                    | L1    | wie oben                                | `{statements: [[sql, params], …]}`, ganz oder gar nicht                                                                                       |
| `extension_database_register_migrations`                                                                            | L1    | –                                       | nur Migrationen der installierten Fassung                                                                                                     |
| `extension_permissions_check_database` / `_web` / `_filesystem`                                                     | L1/L4 | –                                       | liefert Zustand, erteilt nichts                                                                                                               |
| `extension_web_storage_get_item` / `_set_item` / `_remove_item` / `_clear` / `_keys`                                | L3    | –                                       | eigener Speicher, gerätebezogen                                                                                                               |
| `extension_logging_write` / `_read`                                                                                 | L3    | –                                       | Namen von HV, eigene Einträge                                                                                                                 |
| `extension_web_fetch`, `extension_web_open`                                                                         | L4    | `web`                                   | Weiterleitungen geprüft                                                                                                                       |
| `extension_notifications_show` / `_dismiss`                                                                         | L4    | `notifications`                         |                                                                                                                                               |
| `extension_filesystem_*` (18 Methoden)                                                                              | L4    | `filesystem`                            | Dialog-Auswahl ohne Rückfrage                                                                                                                 |
| `extension_password_list` / `_read` / `_create` / `_update` / `_delete`                                             | L5    | `passwords`                             | über 034, sonst 8001                                                                                                                          |
| `extension_remote_storage_*` (9 Methoden)                                                                           | L5    | `remoteStorage`                         | über 029, sonst 8001; Verwalten nur mit Dialog                                                                                                |
| `extension_mail_*` (11 Methoden)                                                                                    | L5    | `mail`                                  | Host und Port                                                                                                                                 |
| `extension_shell_*` (5 Methoden)                                                                                    | L5    | `shell`                                 | nur Desktop, sonst 8001                                                                                                                       |
| `extension_space_*`, `set_auth_token`                                                                               | –     | –                                       | immer 8000 (FR-061)                                                                                                                           |
| `extension_context_set`, `extension_signal_ready`, alles für Berechtigungen erteilen, Grenzwerte, Bridge-Verwaltung | –     | –                                       | gibt es nicht (FR-021), 8000                                                                                                                  |

## Meldungen holzi → Erweiterung

| Typ                               | Daten                                                                                                        | L   |
| --------------------------------- | ------------------------------------------------------------------------------------------------------------ | --- |
| `haextension:context:changed`     | `{context}`                                                                                                  | L3  |
| `haextension:sync:tables-updated` | `{tables: string[]}` (gefiltert, R9)                                                                         | L3  |
| `extension:permission-resolved`   | `{resourceType, action, target, decision}` (`decision` = `granted` \| `denied`, `target` wie in der Anfrage) | L1  |
| `filesync:file-changed`           | flach: `ruleId, changeType, path`                                                                            | L4  |
| `haextension:notification:click`  | `{notificationId, actionId?, path?}`                                                                         | L4  |
| `mail:new-messages`               | `{accountId, mailboxName, newCount}`                                                                         | L5  |
| `shell:output`, `shell:exit`      | flach: `sessionId, data` / `sessionId, exitCode`                                                             | L5  |

`haextension:action:request` und `haextension:external:request` sendet holzi nicht (Specs 018 und 034).

## Vertragstest (FR-009)

`src-tauri/tests/extension_bridge_contract.rs` zählt die Erlaubtliste auf und scheitert, wenn eine Methode
Code unter `chat/`, `llm/`, `adapters/` oder `providers/` erreicht oder eine Methode ohne Eintrag in dieser
Tabelle existiert.
