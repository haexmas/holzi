// Keyboard shortcuts for Shell actions (spec 020-tab-navigation, T030, contracts/shell-actions.md
// §3). Pure: normalizes a key event to a chord, detects the platform and resolves a chord to the
// catalog action bound to it. The bindings are fixed defaults from the catalog; user rebinding is a
// later spec. No Nuxt auto-imports, relative sibling imports only.
import type { KeyChord, ShellActionDefinition } from '../actions/types.ts'

export type Platform = 'mac' | 'default'

type KeyLike = {
  code: string
  ctrlKey: boolean
  altKey: boolean
  shiftKey: boolean
  metaKey: boolean
}

const MODIFIER_CODES = new Set([
  'ControlLeft',
  'ControlRight',
  'AltLeft',
  'AltRight',
  'ShiftLeft',
  'ShiftRight',
  'MetaLeft',
  'MetaRight',
])

/** `Ctrl+Alt+Shift+Meta+<code>` in this order; `null` for a bare modifier press. */
export function chordFromEvent(event: KeyLike): KeyChord | null {
  if (MODIFIER_CODES.has(event.code)) return null
  const parts: string[] = []
  if (event.ctrlKey) parts.push('Ctrl')
  if (event.altKey) parts.push('Alt')
  if (event.shiftKey) parts.push('Shift')
  if (event.metaKey) parts.push('Meta')
  parts.push(event.code)
  return parts.join('+')
}

export function detectPlatform(nav: {
  platform?: string
  userAgentData?: { platform?: string }
}): Platform {
  const name = nav.userAgentData?.platform ?? nav.platform ?? ''
  return name.toLowerCase().startsWith('mac') ? 'mac' : 'default'
}

function chordsFor(
  action: ShellActionDefinition,
  platform: Platform,
): readonly KeyChord[] {
  const keys = action.defaultKeys
  if (!keys) return []
  return (platform === 'mac' ? keys.mac : undefined) ?? keys.default ?? []
}

/** The catalog action bound to `chord` on this platform, if any. */
export function resolveChord(
  catalog: readonly ShellActionDefinition[],
  chord: KeyChord,
  platform: Platform,
): ShellActionDefinition | undefined {
  return catalog.find((action) => chordsFor(action, platform).includes(chord))
}

const TEXT_INPUT_TYPES = new Set([
  '',
  'text',
  'search',
  'url',
  'email',
  'tel',
  'password',
  'number',
])

/** A focused element where arrow and modifier keys edit text. */
export function isEditableTarget(target: unknown): boolean {
  if (typeof target !== 'object' || target === null) return false
  const element = target as {
    tagName?: string
    type?: string
    isContentEditable?: boolean
  }
  if (element.isContentEditable) return true
  if (element.tagName === 'TEXTAREA') return true
  if (element.tagName === 'INPUT')
    return TEXT_INPUT_TYPES.has((element.type ?? '').toLowerCase())
  return false
}

/** Whether `chord` must be left to the focused text field instead of triggering `action`. */
export function shouldYield(
  action: ShellActionDefinition,
  platform: Platform,
  chord: KeyChord,
  target: unknown,
): boolean {
  const chords =
    (platform === 'mac'
      ? action.yieldToTextInput?.mac
      : action.yieldToTextInput?.default) ?? []
  return chords.includes(chord) && isEditableTarget(target)
}
