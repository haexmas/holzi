import { computed, readonly, shallowRef } from 'vue'
import {
  onColorSchemeApplied,
  useColorScheme,
} from '~/composables/useColorScheme'
import { usePreferences } from '~/composables/usePreferences'
import { derive, type Adjustment } from '~/lib/appearance/derive'
import {
  APPEARANCE_KEY,
  CONTROLS,
  DEFAULT_APPEARANCE,
  exportFile,
  parseAppearance,
  parseAppearanceFile,
  serializeAppearance,
  validChoice,
  type Appearance,
} from '~/lib/appearance/schema'
import {
  TOKEN_NAMES,
  cssVar,
  type Scheme,
  type TokenName,
} from '~/lib/appearance/tokens'

/** What an action throws; `useErrorString` turns `reason` (a language key) and `field` into text. */
export interface AppearanceError {
  kind: 'AppearanceError'
  reason: string
  field?: string
}
const appearanceError = (reason: string, field?: string): AppearanceError => ({
  kind: 'AppearanceError',
  reason,
  ...(field === undefined ? {} : { field }),
})

/** What `setAsync` takes: any of the colour choices and the window hint. */
export type AppearancePatch = Partial<Omit<Appearance, 'v'>>

/** One state per process: a process holds one vault (spec 013). */
const appearance = shallowRef<Appearance>(DEFAULT_APPEARANCE)
const tokens = shallowRef<Record<TokenName, string> | null>(null)
const adjustments = shallowRef<Adjustment[]>([])
/** Whether a vault's appearance applies; before that `tailwind.css` has the defaults. */
let active = false
let started = false
let writeQueue: Promise<void> = Promise.resolve()
let generation = 0

const resolvedScheme = (): Scheme =>
  document.documentElement.classList.contains('dark') ? 'dark' : 'light'

function clear() {
  const root = document.documentElement
  for (const name of TOKEN_NAMES) root.style.removeProperty(cssVar(name))
  tokens.value = null
  adjustments.value = []
}

function apply() {
  if (!active) return
  const derived = derive(appearance.value, resolvedScheme())
  const root = document.documentElement
  for (const name of TOKEN_NAMES) {
    root.style.setProperty(cssVar(name), derived.tokens[name])
  }
  tokens.value = derived.tokens
  adjustments.value = derived.adjustments
}

/**
 * The appearance of the app (spec 035-appearance-and-fields): the colour fields, tints and the
 * window hint of the vault, one value for the whole vault like the colour scheme. It sets the
 * theme's CSS variables on `<html>` (an inline style beats `tailwind.css`, whose values are the
 * defaults); all windows share one webview, so that is the whole app. Before the vault is open, and
 * after `clear`, the defaults of `tailwind.css` apply.
 */
export function useAppearance() {
  const { getPrefAsync, setPrefAsync } = usePreferences()
  const colorScheme = useColorScheme()

  /** Re-derives on every change of the colour scheme, the system's included. */
  function start() {
    if (started) return
    started = true
    onColorSchemeApplied(apply)
  }

  async function writeAsync(next: Appearance): Promise<void> {
    await setPrefAsync(
      { kind: 'vault' },
      APPEARANCE_KEY,
      serializeAppearance(next),
    )
    generation += 1
    appearance.value = next
    apply()
  }

  function enqueueWrite<T>(operation: () => Promise<T>): Promise<T> {
    const queued = writeQueue.then(operation)
    writeQueue = queued.then(
      () => undefined,
      () => undefined,
    )
    return queued
  }

  /** Reads the vault's appearance once it is open; a read error leaves the defaults. */
  async function loadAsync(): Promise<void> {
    start()
    // Do not display the previous value while this one is loading.
    appearance.value = DEFAULT_APPEARANCE
    active = true
    apply()
    const at = generation
    const stored = await getPrefAsync({ kind: 'vault' }, APPEARANCE_KEY)
    if (at !== generation) return
    appearance.value = parseAppearance(stored)
    apply()
  }

  /** Re-reads after a synced change; unlike `loadAsync` it keeps what is shown while reading. */
  async function refreshAsync(): Promise<void> {
    const at = generation
    const stored = await getPrefAsync({ kind: 'vault' }, APPEARANCE_KEY)
    if (at !== generation) return
    const next = parseAppearance(stored)
    generation += 1
    if (serializeAppearance(next) === serializeAppearance(appearance.value))
      return
    appearance.value = next
    apply()
  }

  /** Writes the changed choices and applies them at once; a write error leaves the old state. */
  async function setAsync(patch: AppearancePatch): Promise<Appearance> {
    const changes: Partial<Omit<Appearance, 'v'>> = {}
    for (const control of CONTROLS) {
      if (!(control in patch)) continue
      const choice = validChoice(control, patch[control])
      if (choice === null)
        throw appearanceError('settings.appearance.invalid', control)
      changes[control] = choice
    }
    if ('windowHint' in patch) {
      if (typeof patch.windowHint !== 'boolean') {
        throw appearanceError('settings.appearance.invalid', 'windowHint')
      }
      changes.windowHint = patch.windowHint
    }
    return await enqueueWrite(async () => {
      const next: Appearance = { ...appearance.value, ...changes }
      await writeAsync(next)
      return next
    })
  }

  /** The defaults for every value; the colour scheme stays. */
  async function resetAsync(): Promise<Appearance> {
    return await enqueueWrite(async () => {
      await writeAsync(DEFAULT_APPEARANCE)
      return DEFAULT_APPEARANCE
    })
  }

  /** The appearance file as text (FR-021); the stored scheme choice goes with it. */
  function exportText(): string {
    return exportFile(appearance.value, colorScheme.scheme.value)
  }

  /**
   * Checks the whole file, then writes the colour scheme and the appearance; if the second write
   * fails the first is taken back. Nothing changes when the file is refused.
   */
  async function importAsync(text: string): Promise<Appearance> {
    const parsed = parseAppearanceFile(text)
    if (!parsed.ok) {
      throw appearanceError(
        `settings.appearance.import.${parsed.reason}`,
        parsed.field,
      )
    }
    return await enqueueWrite(async () => {
      const previousScheme = colorScheme.scheme.value
      try {
        await colorScheme.setAsync(parsed.colorScheme)
      } catch {
        throw appearanceError('settings.appearance.import.failed')
      }
      try {
        await writeAsync(parsed.appearance)
      } catch {
        await colorScheme.setAsync(previousScheme).catch(() => undefined)
        throw appearanceError('settings.appearance.import.failed')
      }
      return parsed.appearance
    })
  }

  return {
    appearance: readonly(appearance),
    tokens: readonly(tokens),
    adjustments: readonly(adjustments),
    windowHint: computed(() => appearance.value.windowHint),
    start,
    loadAsync,
    refreshAsync,
    setAsync,
    resetAsync,
    exportText,
    importAsync,
    clear: () => {
      active = false
      appearance.value = DEFAULT_APPEARANCE
      clear()
    },
  }
}
