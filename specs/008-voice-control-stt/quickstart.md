# Quickstart: Voice Control (Local Speech-to-Text)

## Voraussetzungen

- Holzi mit geöffneter Vault starten, Mikrofon am Gerät verfügbar.
- Für automatisierte Tests: `cargo test` und `pnpm typecheck` ausführen.
- Für den externen Adapter ausschließlich `wiremock`-Fixtures verwenden; keine echten
  Zugangsdaten nötig.

## 1. Diktieren und Senden (User Story 1)

1. Chat-Ansicht öffnen, Mic-Kontrolle drücken/halten, einen kurzen Satz sprechen, loslassen.
2. Prüfen: Transkript erscheint im Eingabefeld und wird (Default: Auto-Send an) automatisch
   abgeschickt.
3. Auto-Send in den Einstellungen deaktivieren, erneut diktieren.
4. Prüfen: Transkript bleibt editierbar im Eingabefeld stehen, wird nicht automatisch gesendet.
5. Eine Aufnahme mit reiner Stille/Hintergrundrauschen machen.
6. Prüfen: Eingabefeld bleibt unverändert, nichts wird gesendet.

## 2. Sprachlicher Interrupt (User Story 2)

1. Eine Anfrage stellen, die eine längere Antwort auslöst.
2. Während die Antwort läuft, "stop" (oder "halt"/"abbrechen") sprechen.
3. Prüfen: Die Antwort bricht innerhalb einer Sekunde ab.
4. Erneut eine längere Antwort anstoßen und stattdessen einen ganzen Satz diktieren, der eines der
   drei Wörter enthält (z. B. "bitte nicht mehr stoppen mitten im Satz").
5. Prüfen: Die Antwort läuft ungestört weiter, der Satz landet stattdessen normal im Eingabefeld.
6. Im Leerlauf (keine laufende Antwort) ein Interrupt-Wort sprechen.
7. Prüfen: Nichts passiert, nichts wird gesendet.

## 3. Transkriptionsquelle wechseln (User Story 3)

1. In der Modellverwaltung einen externen Transkriptions-Dienst mit gültigen Testdaten
   hinzufügen und aktivieren.
2. Prüfen: Mic-Kontrolle zeigt durchgehend ein sichtbares Signal, dass Audio extern verarbeitet
   wird (FR-021).
3. Diktieren; prüfen, dass die Transkription über den externen Dienst läuft (per Test-Fixture
   verifizierbar).
4. Zugangsdaten ungültig machen (Fixture auf 401 stellen) und erneut diktieren.
5. Prüfen: Klarer Fehlerhinweis, Eingabefeld bleibt unverändert.
6. Zurück auf die gebündelte lokale Quelle wechseln.
7. Prüfen: Diktieren funktioniert wieder vollständig offline, kein sichtbares Extern-Signal mehr.

## 4. Rand- und Fehlerfälle

- Mikrofon-Berechtigung verweigern: Mic-Kontrolle zeigt "Berechtigung nötig", keine Aufnahme
  möglich.
- Während einer laufenden Aufnahme die App in den Hintergrund schicken (Mobile): Aufnahme wird
  verworfen, nichts wird transkribiert.
- Aufnahme über das konfigurierte Maximum hinaus halten: Aufnahme stoppt automatisch, das bis
  dahin Aufgenommene wird transkribiert.
- Erste Nutzung nach frischer Installation, ohne jede Konfiguration: Diktieren funktioniert direkt
  nach Erteilen der Mikrofon-Berechtigung, ohne Download oder Einrichtung (SC-003).
