# Quickstart: Kompakter Passwortverlauf

## Manuelle Prüfung

1. Passwortmanager öffnen und einen Eintrag mit mindestens zwei Änderungen auswählen.
2. Den Tab „Verlauf“ öffnen.
3. Prüfen, dass zuerst ein Dropdown und darunter der ausgewählte Stand in einer Spalte
   erscheinen; es darf keine Timeline-Spalte daneben geben.
4. Einen anderen Stand über das Dropdown auswählen und prüfen, dass dessen Zeit und Werte
   geladen werden.
5. Prüfen, dass „Gespeichert am …“ sichtbar, klein und grau/dezent dargestellt ist.
6. Einen Stand wiederherstellen und prüfen, dass der bestehende Bestätigungsdialog sowie der
   erfolgreiche Restore unverändert funktionieren.
7. Einen Eintrag mit genau einem Stand öffnen und prüfen, dass der Hinweis „nichts Älteres“
   weiterhin erscheint.
8. Im Eintragskopf „Löschen“ wählen, bestätigen und prüfen, dass der Eintrag im Papierkorb liegt.
9. Ein langes Passwort mit der Maus gedrückt halten: es bleibt sichtbar, bis die Taste losgelassen
   wird.
10. Bei offenem Vault die Oberfläche neu laden (Strg+R): sie kehrt in den Arbeitsbereich zurück.

## Automatische Checks

```sh
pnpm check:templates
pnpm check:passwords
pnpm check:vault-lifecycle
pnpm typecheck
pnpm lint
pnpm test:e2e --grep passwords-tabs
```
