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

## Automatische Checks

```sh
pnpm check:templates
pnpm typecheck
pnpm lint
```
