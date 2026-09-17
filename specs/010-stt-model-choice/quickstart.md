# Quickstart: STT Model Choice

## Voraussetzungen

- Frisches Gerät/frische Vault für den Onboarding-Teil (oder: Device-Zeile in der DB löschen bzw.
  einen neuen Test-Vault anlegen), Netzwerkzugriff für den Download.
- Für automatisierte Tests: `cargo test --features llm-cpu` und `pnpm typecheck`.

## 1. Erstwahl beim Onboarding (User Story 1)

1. Vault auf einem neuen Gerät öffnen. Onboarding startet.
2. Alias-Schritt abschließen.
3. Im Modell-Schritt ein Chat/Agent-Modell wählen (unverändertes bestehendes Verhalten).
4. Prüfen: Direkt danach erscheint ein neuer Schritt mit drei STT-Größen-Empfehlungen
   (Easy/Sweet/Max), je mit Namen, Größenangabe und Hardware-Fit-Hinweis.
5. Eine der drei Größen wählen.
6. Prüfen: Download läuft, nach Abschluss landet man im Workspace; die gewählte Größe ist jetzt
   die aktive STT-Preference dieses Geräts.
7. Erneut von vorn onboarden (neues Gerät/neue Vault): im STT-Schritt stattdessen "später
   entscheiden" wählen.
8. Prüfen: Onboarding schließt ohne zusätzlichen Download ab; die erste Diktier-Nutzung im
   Workspace funktioniert trotzdem (lädt lazy das kleinste Tier), ohne dass der Nutzer noch etwas
   tun muss (SC-003).

## 2. Nachträglich wechseln (User Story 2)

1. Settings-Seite öffnen, zum STT-Modell-Abschnitt navigieren.
2. Prüfen: Aktuell aktives Modell wird angezeigt; weitere Tiers (installiert oder nicht) sind
   auswählbar.
3. Ein noch nicht heruntergeladenes Tier wählen.
4. Prüfen: Download startet, nach Abschluss ist es das aktive Modell.
5. Direkt danach diktieren (keine App-Neustart).
6. Prüfen: Transkription läuft mit dem neu gewählten Modell (z. B. per Log/Timing-Unterschied
   zwischen `tiny` und `small` grob verifizierbar), ohne Neustart (SC-002/SC-004).
7. Auf ein bereits vorher heruntergeladenes Tier zurückwechseln, ab dem Öffnen der Settings-Seite
   stoppen.
8. Prüfen: Sofort aktiv, kein erneuter Download, **Gesamtzeit unter zwei Minuten** (SC-002 ist ein
   Stoppuhr-Kriterium, nicht nur "funktioniert").

## 3. Rand- und Fehlerfälle

- Download während des Onboarding-Schritts abbrechen (Netzwerk trennen): Onboarding darf nicht
  hängen bleiben — Fehleranzeige mit Möglichkeit, erneut zu versuchen oder zu überspringen.
- Onboarding-STT-Schritt ohne Netzwerk durchlaufen (überspringen): Diktieren funktioniert beim
  ersten echten Gebrauch trotzdem, per Lazy-Download des Default-Tiers.
- Dateien des aktiven Tiers manuell vom Datenträger löschen, dann diktieren: Modell wird
  automatisch neu heruntergeladen statt mit einem unklaren Fehler zu scheitern.
- Modell während einer laufenden Aufnahme/Transkription wechseln: laufende Transkription
  schließt mit dem zu Beginn aktiven Modell ab; der Wechsel gilt erst ab der nächsten Aufnahme.
- Erste Nutzung nach frischer Installation ohne jede Interaktion mit dieser Funktion: Verhalten
  identisch zu vor dieser Spec (`whisper-tiny`, Lazy-Download, kein Unterschied spürbar) (FR-007).
