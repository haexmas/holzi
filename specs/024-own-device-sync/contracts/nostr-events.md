# Contract: Nostr-Ereignisse für Präsenz, Aufnahmeanfrage und Verknüpfen

Alle Ereignisse sind flüchtig (Art 21059, NIP-01: nicht gespeichert). Aufbau wie NIP-59: außen ein
Ereignis, signiert mit einem Einmalschlüssel, `p`-Tag an den Empfänger, Inhalt mit NIP-44 v2
verschlüsselt; darin ein Siegel (Art 13), signiert mit dem Geräteschlüssel des Absenders; darin das
eigentliche Ereignis (unsigniert, Art unten). Das innere Ereignis trägt den kanonischen `ts`-Wert
für Frischeprüfungen. `created_at` (das Zeitfeld jedes Nostr-Ereignisses, NIP-01) ist bei Siegel und
Gift-Wrap die aktuelle Zeit, nicht zufällig in die Vergangenheit verschoben wie bei gespeicherten
NIP-59-Nachrichten: Ein Nostr-Relay sieht die Ankunftszeit ohnehin, flüchtige Ereignisse speichert es
nicht, und verschobene Zeiten würden von Abos mit `since` weggefiltert und von Nostr-Relays mit
`created_at_lower_limit` (NIP-11) abgewiesen (research R7). Server: die eingestellten Nostr-Relays
(FR-008).

**Abos und Größen**: Abos setzen kein `since` (den Zeitfilter eines Abos, NIP-01). holzi sendet kein
Ereignis über 16 KiB und verwirft empfangene Ereignisse über 16 KiB, bevor es sie entschlüsselt;
Präsenzmeldungen sind unter 1 KiB. Je Gerät höchstens eine Präsenzmeldung pro Minute, außer bei einer
Adressänderung. Frisch ist eine Meldung, wenn ihr `ts` höchstens 150 s alt und höchstens 30 s in der
Zukunft liegt.

## Präsenzmeldung (inneres Ereignis Art 24100)

- Empfänger (`p`-Tag): `mb_pk` aus `mb_sk = HKDF-SHA256(ikm = inhaltsschlüssel der höchsten
Generation, info = "holzi/presence/v1" ‖ tag_u32)`, `tag` = Tage seit 1970 (UTC). `tag_u32` ist
  die unsigned 32-bit Darstellung von `tag` in Big-Endian-Reihenfolge; sie wird ohne Textkodierung
  direkt an die UTF-8-Bytes des Info-Präfixes angehängt.
- Inhalt (JSON):

```json
{
  "v": 1,
  "device": "<hex device_pubkey>",
  "endpoint": "<hex endpoint_id>",
  "iroh_relay": "https://…",
  "addrs": ["203.0.113.5:4433"],
  "list_generation": 7,
  "ts": 1790000000000,
  "nonce": "<hex 16 byte>"
}
```

- Absender: nur ein Gerät, dessen Geräteliste mindestens ein weiteres Gerät nennt (FR-007). Takt:
  beim Öffnen, bei Adressänderung, alle 60 s.
- Empfänger prüft: Siegel mit einem Geräteschlüssel der geltenden Geräteliste signiert und gleich
  `device`; `ts` nicht älter als 150 s; dann Adresse in den `MemoryLookup`, Verbindung aufbauen,
  wenn noch keine besteht. Meldungen unbekannter Geräte führen zu keiner Verbindung, außer zur
  Prüfung einer neueren Geräteliste (Kopie eines Hauptgeräts, FR-007).
- Abo: `{kinds: [21059], "#p": [mb_pk(heute), mb_pk(gestern)]}`, beim Tageswechsel erneuert.

## Aufnahmeanfrage (inneres Ereignis Art 24101)

- Empfänger: `mb_pk` wie oben (die Kopie hat den Inhaltsschlüssel aus der kopierten Datei).
- Inhalt: `{v, device, endpoint, name, requested_at, sig}`; `sig` = Schnorr des Geräteschlüssels
  über `holzi-admission/v1 ‖ lp(device) ‖ lp(endpoint) ‖ lp(name) ‖ requested_at`.
- Jedes Gerät der Vault, das sie empfängt, legt sie in `admission_requests` ab (FR-045); die Kopie
  wiederholt sie im Präsenztakt, bis sie auf der Geräteliste steht.

## Treffpunkt beim Verknüpfen (inneres Ereignis Art 24102)

- Empfänger: `rv_pk` aus `rv_sk = HKDF-SHA256(ikm = code, info = "holzi/link/rendezvous/v1")`.
- Absender: die neue Installation mit ihrem frisch erzeugten Geräteschlüssel.
- Inhalt: `{v, endpoint, iroh_relay, addrs, ts}`.
- Das Hauptgerät abonniert `#p = rv_pk`, solange der Code gilt, und wählt die Installation über
  `holzi-link/1` an (contracts/sync-protocol.md). Mehr als eine Meldung für denselben Code: nur die
  erste wird verfolgt.

## Metadaten, die ein Nostr-Relay sieht (D9)

IP-Adressen und Zeiten der Verbindungen, Größe der Ereignisse, einen täglich wechselnden
Empfänger je Vault, einen zufälligen Absender je Ereignis. Nicht: Vault-Identität,
Geräteschlüssel, Adressen, Namen.
