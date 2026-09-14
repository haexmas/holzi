# Quickstart: Chatfenster und Session-Handling

## Voraussetzungen

- Holzi mit einer geöffneten Vault starten.
- Mindestens ein lokales Modell installiert haben.
- Für Reasoning einen Adapter bzw. ein lokales Modell verwenden, das
  `reasoning`-Deltas liefert.

## 1. Neuer Chat-Einstieg

1. Einen Chat mit einer Nachricht senden.
2. Zur Workspace-Landing zurückgehen.
3. Den Chat erneut öffnen.
4. Prüfen: Eingabe ist leer, kein alter Thread ist automatisch aktiv.
5. Eine zweite Nachricht senden.
6. Prüfen: Die zweite Nachricht liegt in einem neuen Thread; der erste Thread
   bleibt in der Historie.
7. Den Chat erneut öffnen und ohne Nachricht verlassen.
8. Prüfen: Kein leerer Thread erscheint in der Historie.

## 2. Modell-Preload

1. Eine Vault schließen und erneut öffnen.
2. Im Workspace bleiben, ohne den Chat zu öffnen.
3. Prüfen: Der Modell-Load startet bereits im Hintergrund.
4. Während des Loads den Chat öffnen.
5. Prüfen: Der Chat übernimmt den bestehenden Status und startet keinen zweiten
   Load.
6. Einen Vault-Wechsel durchführen.
7. Prüfen: Das Modell der neuen Vault wird geladen; ein verspätetes Ergebnis des
   alten Loads wird nicht sichtbar aktiv.
8. Einen Load-Fehler simulieren oder ein nicht ladbares Modell auswählen.
9. Prüfen: Die Vault bleibt offen und ein alternatives Modell ist auswählbar.

## 3. Composer und Textarea

1. Den Composer auf Desktop öffnen.
2. Prüfen: Modell, Effort und Freigabe erscheinen kompakt innerhalb des
   gemeinsamen Eingabecontainers; es gibt keinen separaten Reasoning-Schalter.
3. Jedes Control öffnen, eine andere Option wählen und den Wert wieder ablesen.
4. Einen Prompt mit mehreren `Shift+Enter`-Zeilen eingeben.
5. Prüfen: Die Textarea wächst bis maximal 8 sichtbare Zeilen und scrollt
   danach intern.
6. Mit `Enter` senden und prüfen: Inhalt wird vollständig gesendet, Textarea
   leert sich und schrumpft zurück.
7. Den Vorgang in einer schmalen Fensterbreite wiederholen.

## 4. Reasoning-Accordion

1. Eine Antwort mit Reasoning erzeugen.
2. Prüfen: Das Accordion ist sichtbar, aber geschlossen.
3. Es öffnen und prüfen: Der Reasoning-Text wird vollständig angezeigt.
4. Eine zweite Antwort erzeugen.
5. Prüfen: Das zweite Accordion startet ebenfalls geschlossen und unabhängig
   vom ersten.
6. Chat verlassen und erneut öffnen.
7. Prüfen: Alle Reasoning-Accordions starten wieder geschlossen.

## 5. Automatisierte Checks

```bash
cargo test --lib
cargo test --test chat_message_idempotency
pnpm typecheck
```

Zusätzlich den i18n-Key-Baum von `de.json` und `en.json` auf identische
Schlüssel prüfen.
