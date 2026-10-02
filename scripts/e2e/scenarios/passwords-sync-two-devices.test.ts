import assert from 'node:assert/strict'
import { scenario } from '../lib/scenario.ts'
import {
  createEntry,
  expectEntries,
  restoreEntry,
  tagCount,
  trashEntry,
} from '../lib/passwords.ts'
import { unwrap } from '../lib/flows.ts'

// Spec 034, US8 (quickstart §10): two devices of one user keep the password manager's data in step. An
// entry made on one shows on the other; a tag made on both under one name stays one tag; an entry
// with tags and a custom field that is deleted for good on one is gone on the other. The devices
// come from the group API of spec 033 and find each other through the scenario's Nostr relay. The
// attachments of 25 MiB and the equal files are covered by `tests/passwords_sync.rs` (a scenario
// cannot write a file to a device).
scenario('passwords-sync-two-devices', { timeoutMs: 300_000 }, async (ctx) => {
  const group = await ctx.group({ users: { anna: ['laptop', 'phone'] } })
  const laptop = group.device('anna/laptop').page
  const phone = group.device('anna/phone').page

  const mail = await createEntry(laptop, {
    title: 'Mail',
    username: 'anna',
    tags: ['shared'],
    keyValues: [{ key: 'PIN', value: 'SECRET-MARKER-E2E-SYNC' }],
  })
  await expectEntries(ctx, phone, ['Mail'], 'the entry to show on the phone')
  ctx.step('created on one device, shown on the other')

  // A tag made on both devices under one name is one tag on both.
  await createEntry(laptop, { title: 'On laptop', tags: ['Work'] })
  await createEntry(phone, { title: 'On phone', tags: ['work'] })
  for (const [name, device] of [
    ['laptop', laptop],
    ['phone', phone],
  ] as const) {
    await ctx.waitFor(
      `the ${name} to hold three entries and one Work tag`,
      async () =>
        (await tagCount(device, 'work')) === 1 &&
        (
          unwrap<{ headers: unknown[] }>(
            'passwords_load_overview',
            await device.invoke('passwords_load_overview'),
          ) as { headers: unknown[] }
        ).headers.length === 3,
      { timeoutMs: 40_000, fixed: true },
    )
  }
  ctx.step('a tag made twice is one tag')

  // Trash on one device shows on the other, a restore too, and a delete for good removes the entry.
  await trashEntry(laptop, mail)
  await ctx.waitFor(
    'the trash to reach the phone',
    async () => {
      const overview = unwrap<{
        headers: Array<{ id: string; groupId: string | null }>
      }>(
        'passwords_load_overview',
        await phone.invoke('passwords_load_overview'),
      )
      return overview.headers.find((h) => h.id === mail)?.groupId === 'trash'
    },
    { timeoutMs: 40_000, fixed: true },
  )
  await restoreEntry(laptop, mail)
  await ctx.waitFor(
    'the restore to reach the phone',
    async () => {
      const overview = unwrap<{
        headers: Array<{ id: string; groupId: string | null }>
      }>(
        'passwords_load_overview',
        await phone.invoke('passwords_load_overview'),
      )
      return overview.headers.find((h) => h.id === mail)?.groupId !== 'trash'
    },
    { timeoutMs: 40_000, fixed: true },
  )
  await trashEntry(laptop, mail)
  unwrap(
    'passwords_delete_permanently',
    await laptop.invoke('passwords_delete_permanently', {
      args: { targets: [{ kind: 'item', id: mail }] },
    }),
  )
  await expectEntries(
    ctx,
    phone,
    ['On laptop', 'On phone'],
    'the delete for good to reach the phone',
  )
  assert.equal(await tagCount(phone, 'shared'), 1, 'the tag outlives its entry')
  ctx.step('trash, restore and delete for good arrive')
})
