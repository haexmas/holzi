# Quickstart: Allgemein mit Grundeinstellung und Erscheinungsbild

Automatisch:

```sh
pnpm check:settings            # Registry, Suche, i18n-Schlüssel
pnpm typecheck && pnpm lint && pnpm format:check
cd src-tauri && cargo test --test vault_passphrase_change && cargo test instances::passphrase
pnpm test:e2e                  # nur auf Arch (Host-Bridge), Settings-/Appearance-Szenarien
```

Manuell in der laufenden App:

1. **Struktur (US1)**: Einstellungen → Sidebar ohne „Darstellung“; „Allgemein“ zeigt zwei Karten; beide
   öffnen; Suche „Akzent“ → Erscheinungsbild, „Sprache“ → Grundeinstellung.
2. **Sprache (US2)**: App mit `LANG=de_DE.UTF-8` starten → Startbildschirm Deutsch; mit `LANG=fr_FR.UTF-8`
   → Englisch. Oben rechts auf Deutsch umschalten, App neu starten → wieder Englisch. Vault ohne Sprache
   entsperren → Grundeinstellung zeigt die aktive Sprache. In Grundeinstellung auf English → sofort
   Englisch; App neu starten, entsperren → Englisch, auch wenn der Startbildschirm Deutsch war.
3. **Passwort (US3)**: Grundeinstellung → „Vaultpasswort ändern“; falsches aktuelles → Fehlermeldung;
   7 Zeichen → nicht absendbar; gültig → Erfolg. App neu starten: altes Passwort scheitert, neues öffnet,
   Passwörter und Chats vorhanden. Agent bitten „öffne die Passwortänderung“ → Ansicht öffnet sich; „ändere
   mein Vaultpasswort auf X“ → Agent kann es nicht.
4. **Hintergrund (US4)**: Erscheinungsbild → Bild wählen (ein 6000×4000-JPEG) → Hintergrund hinter allen
   Workspaces, Wert < 1 MB; „Entfernen“ → Standard. Eine Textdatei mit `.png`-Endung → Fehlermeldung.
5. **Sync (SC-003, SC-005)**: zweites, verknüpftes Gerät: Sprache und Hintergrund kommen nach dem Sync an;
   dessen Passwort bleibt das alte.
