import { ALL_ACTIONS } from '~/lib/actions/catalog'
import {
  chordFromEvent,
  detectPlatform,
  resolveChord,
  shouldYield,
} from '~/lib/shell/keybindings'

/**
 * The one global shortcut listener of the Shell (spec 020-tab-navigation,
 * T031, contracts/shell-actions.md §3): capture phase in the top document, so
 * a bound chord reaches its catalog action before any component handler.
 * Chords a focused text field needs (Alt+Arrow on macOS) are left alone
 * (FR-017); component-local keys (tab-bar arrows, Escape, Enter) are never
 * bound here (FR-026). Keys typed inside an embedded document never reach
 * this listener (research R17).
 */
export function useShellKeyboard() {
  const shell = useShellStore()
  const platform = detectPlatform(
    navigator as Navigator & { userAgentData?: { platform?: string } },
  )

  useEventListener(
    document,
    'keydown',
    (event: KeyboardEvent) => {
      const chord = chordFromEvent(event)
      if (!chord) return
      const action = resolveChord(ALL_ACTIONS, chord, platform)
      if (!action) return
      if (shouldYield(action, platform, chord, event.target)) return
      event.preventDefault()
      void shell.runAction(action.id, {}, { kind: 'user' })
    },
    { capture: true },
  )
}
