import assert from 'node:assert/strict'
import { scenario } from '../lib/scenario.ts'
import { addThread, threadTitles } from '../lib/sync-flows.ts'
import { confirmRemoveDevice, openRemoveDevice } from '../lib/sync-ui.ts'

const CHAT = 'Während das Telefon weg war'

// Spec 024, FR-010 and the content key of FR-026 to FR-028: removing a device replaces the content key,
// and presence meets in a mailbox that the key names. A device that was away while that happened only
// knows the old key; the others must still find it there, or it can never learn the new key by syncing.
// Here the phone is stopped, a main device is removed, the laptop restarts (so its old address is
// stale for the phone) and writes a chat, and then the phone comes back: it has to catch up.
scenario(
  'sync-away-through-key-change',
  { timeoutMs: 600_000 },
  async (ctx) => {
    const g = await ctx.group({
      users: { anna: ['laptop', { name: 'desktop', main: true }, 'phone'] },
    })
    const [laptop, desktop, phone] = ['laptop', 'desktop', 'phone'].map(
      (name) => g.device(`anna/${name}`),
    )
    assert.ok(laptop && desktop && phone)
    await desktop.pubkey()
    await phone.pubkey()

    await phone.stop()
    await openRemoveDevice(laptop, desktop)
    await confirmRemoveDevice(laptop.page)
    await laptop.restart()
    await addThread(laptop, CHAT)
    ctx.step('the phone is away while the content key is replaced')

    await phone.start()
    await ctx.waitFor(
      'the phone, still on the old content key, to be found and to catch up',
      async () => (await threadTitles(phone)).includes(CHAT),
      { timeoutMs: 120_000, fixed: true },
    )
    ctx.step('the phone caught up')
  },
)
