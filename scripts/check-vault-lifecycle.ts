// Run with `node scripts/check-vault-lifecycle.ts`. Frontend cases for spec 013 (vault lifecycle
// isolation, specs/013-vault-lifecycle-isolation): the lock flows, the passphrase lifetime in the
// unlock and create sheets, and the reads that must never fail. They run on the shared replay
// harness in `scripts/lib/chat-state-harness.ts`. New frontend cases for this feature go here,
// never into the oversized `scripts/check-chat-state.ts`.
import assert from 'node:assert/strict'
import { test } from 'node:test'
import {
  composablePath,
  createChatState,
  runComposable,
} from './lib/chat-state-harness.ts'
import { loadScriptSetup } from './lib/script-setup-sandbox.ts'

test('the shared harness boots the chat page against the IPC double', () => {
  const state = createChatState()
  assert.equal(state.busy.value, false)
  assert.equal(state.input.value, '')
})

// The lock flows (US1, FR-002): the lock control only asks the backend to close. The backend
// replaces the page with a spinner and ends the process, so `useVaultLock` keeps no closing state,
// navigates nowhere and does not clear the active instance itself. It provides neither
// `navigateTo` nor the instances store, so touching them fails the case.

/** Runs the real `useVaultLock` against doubles that record what it does, in order. */
function loadVaultLock(closeAsync: () => Promise<void>) {
  const calls: string[] = []
  const composable = runComposable(
    composablePath('useVaultLock'),
    (specifier) => {
      throw new Error(`useVaultLock sandbox: unexpected import '${specifier}'`)
    },
    {
      useWindowManagerStore: () => ({
        flushAsync: async () => {
          calls.push('flush')
        },
      }),
      useInstance: () => ({
        closeAsync: async () => {
          calls.push('close')
          await closeAsync()
        },
      }),
    },
  ) as { useVaultLock: () => { lock: () => Promise<void> } }
  return { calls, ...composable.useVaultLock() }
}

test('useVaultLock flushes the layout, then asks for the close once and does nothing else', async () => {
  const { calls, lock } = loadVaultLock(async () => {})

  await lock()

  assert.deepEqual(calls, ['flush', 'close'])
})

test('useVaultLock swallows a rejected close and shows no error', async () => {
  const { calls, lock } = loadVaultLock(async () => {
    throw new Error('the page is already gone')
  })

  await lock()

  assert.deepEqual(calls, ['flush', 'close'])
})

// US4 frontend errors (T066/T074, contracts/frontend-surface.md): `useErrorString` maps the three
// new fieldless kinds to their own `errors.*` keys instead of falling through to a raw
// `JSON.stringify(e)`.

function loadErrString(): { errString: (e: unknown) => string } {
  const req = (specifier: string) => {
    throw new Error(`useErrorString sandbox: unexpected import '${specifier}'`)
  }
  const mod = runComposable(composablePath('useErrorString'), req, {
    hfErrorKey: () => 'errors.hf.generic',
  }) as { useErrorString: () => { errString: (e: unknown) => string } }
  return mod.useErrorString()
}

test('useErrorString maps VaultAlreadyOpenElsewhere, VaultAlreadyActive and VaultClosed to their errors.* keys', () => {
  const { errString } = loadErrString()
  assert.equal(
    errString({ kind: 'VaultAlreadyOpenElsewhere' }),
    'errors.vaultAlreadyOpenElsewhere',
  )
  assert.equal(
    errString({ kind: 'VaultAlreadyActive' }),
    'errors.vaultAlreadyActive',
  )
  assert.equal(errString({ kind: 'VaultClosed' }), 'errors.vaultClosed')
})

// UnlockSheet (T074): `VaultAlreadyOpenElsewhere` gets its own message; `WrongPassphrase` and
// `NotFound` both still fall back to the generic `errors.openFailed` (spec 001 FR-021 — the typed
// discriminator must not leak which of the two it was).

function loadUnlockSheet(openAsync: () => Promise<{ name: string }>) {
  return loadScriptSetup<{
    onSubmit: () => Promise<void>
    error: { value: string | null }
    passphrase: { value: string }
  }>(
    'src/components/onboarding/UnlockSheet.vue',
    ['onSubmit', 'error', 'passphrase'],
    {
      useInstance: () => ({ openAsync }),
      useFieldLabels: () => ({ password: { value: {} } }),
      props: { open: true, name: 'vault' },
    },
  )
}

test('UnlockSheet shows the dedicated message for VaultAlreadyOpenElsewhere', async () => {
  const { onSubmit, error, passphrase } = loadUnlockSheet(() => {
    throw { kind: 'VaultAlreadyOpenElsewhere' }
  })
  passphrase.value = 'some-passphrase'

  await onSubmit()

  assert.equal(error.value, 'errors.vaultAlreadyOpenElsewhere')
})

for (const kind of ['WrongPassphrase', 'NotFound']) {
  test(`UnlockSheet still shows the generic errors.openFailed for ${kind}`, async () => {
    const { onSubmit, error, passphrase } = loadUnlockSheet(() => {
      throw { kind }
    })
    passphrase.value = 'some-passphrase'

    await onSubmit()

    assert.equal(error.value, 'errors.openFailed')
  })
}

// pages/index.vue (T075): re-syncs the vault list on window focus, on document visibilitychange
// (only while actually visible), and when an instance is selected (opening the unlock sheet) —
// and removes both listeners on unmount.

/** A `window`/`document` double that records registered listeners by event type and lets a case
 * fire them, plus a settable `visibilityState`. */
function fakeDom() {
  const listeners = new Map<string, Set<() => void>>()
  const on = (type: string, fn: () => void) => {
    if (!listeners.has(type)) listeners.set(type, new Set())
    listeners.get(type)!.add(fn)
  }
  const off = (type: string, fn: () => void) => listeners.get(type)?.delete(fn)
  const fire = (type: string) => {
    for (const fn of listeners.get(type) ?? []) fn()
  }
  const documentDouble = {
    visibilityState: 'visible' as 'visible' | 'hidden',
    addEventListener: on,
    removeEventListener: off,
  }
  return {
    window: { addEventListener: on, removeEventListener: off },
    document: documentDouble,
    fire,
    setVisibility: (state: 'visible' | 'hidden') => {
      documentDouble.visibilityState = state
    },
    listenerCount: (type: string) => listeners.get(type)?.size ?? 0,
  }
}

function loadIndexPage(dom: ReturnType<typeof fakeDom>) {
  let synced = 0
  const store = {
    instances: [] as unknown[],
    lastError: null as string | null,
    async startListening() {},
    stopListening() {},
    async syncAsync() {
      synced += 1
    },
    setActiveInstance() {},
  }
  const page = loadScriptSetup<{ onSelect: (name: string) => void }>(
    'src/pages/index.vue',
    ['onSelect'],
    {
      useInstancesStore: () => store,
      window: dom.window,
      document: dom.document,
    },
  )
  return { ...page, syncCount: () => synced }
}

test('pages/index.vue re-syncs on window focus and removes the listener on unmount', async () => {
  const dom = fakeDom()
  const page = loadIndexPage(dom)
  await page.mount()
  const before = page.syncCount() // the mount-time sync itself

  dom.fire('focus')
  assert.equal(page.syncCount(), before + 1)

  await page.unmount()
  dom.fire('focus')
  assert.equal(page.syncCount(), before + 1, 'no re-sync after unmount')
})

test('pages/index.vue re-syncs on visibilitychange only while actually visible', async () => {
  const dom = fakeDom()
  const page = loadIndexPage(dom)
  await page.mount()
  const before = page.syncCount()

  dom.setVisibility('hidden')
  dom.fire('visibilitychange')
  assert.equal(page.syncCount(), before, 'no re-sync while hidden')

  dom.setVisibility('visible')
  dom.fire('visibilitychange')
  assert.equal(page.syncCount(), before + 1)
})

test('pages/index.vue re-syncs when an instance is selected (opening the unlock sheet)', async () => {
  const dom = fakeDom()
  const page = loadIndexPage(dom)
  await page.mount()
  const before = page.syncCount()

  page.onSelect('vault-a')

  assert.equal(page.syncCount(), before + 1)
})
