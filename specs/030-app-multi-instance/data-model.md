# Data Model: Mehrfachinstanzen für Apps

Keine neue Entität. Diese Spec ändert einen Attributwert einer bereits
bestehenden Entität.

## Geändert

- **App-Definition** (`AppDefinition`, `src/lib/wm/apps.ts`), Eintrag
  `system.chat`: `multiInstance` von `false` auf `true`. Alle anderen
  Attribute (Icon, Größen, `titleKey`) bleiben unverändert. Der Eintrag
  `system.settings` bleibt vollständig unverändert (`multiInstance: false`).

## Unverändert, zur Bestätigung

- **Tab**, **Fenster**: Identität bereits unabhängig von der App (Spec 015
  Key Entities) — trägt die Mehrfachinstanz bereits ohne Modelländerung.
- **Persistierte Sitzung** (`PersistedLayout`, Spec 015/022): speichert Tabs
  bereits generisch nach `appId`, ohne Eindeutigkeit je App vorauszusetzen
  (siehe `scripts/check-wm-state.ts`s Hydrate-Test mit zwei Fenstern
  derselben Mehrfachinstanz-App-Id).
