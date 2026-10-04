import { detectPlatform } from '~/lib/wm/keybindings'
import { shortcutLabel, type ShortcutHint } from '~/lib/passwords/shortcuts'

/** The shortcut next to a menu action (spec 036, FR-016, FR-018), with the key names of the
 * interface language on Linux and Windows and the symbols on macOS. */
export function usePasswordsMenuText() {
  const { t } = useI18n()
  const platform = detectPlatform(
    navigator as Navigator & { userAgentData?: { platform?: string } },
  )

  function shortcut(hint: ShortcutHint | undefined): string {
    if (!hint) return ''
    return shortcutLabel(hint, platform, {
      mod: t('passwords.menu.keys.mod'),
      shift: t('passwords.menu.keys.shift'),
      Delete: t('passwords.menu.keys.delete'),
      Enter: t('passwords.menu.keys.enter'),
    })
  }

  return { platform, shortcut }
}
