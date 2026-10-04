import assert from 'node:assert/strict'
import { scenario } from '../lib/scenario.ts'
import { unwrap } from '../lib/flows.ts'
import type { Page } from '../lib/page.ts'
import {
  fixture,
  inFrame,
  install,
  openFromLauncher,
  probeRequest,
  type InstalledExtension,
} from '../lib/extensions.ts'

// Spec 017, US4, T081 (quickstart §6): one extension on two devices of one user. Installed on the
// laptop, the phone verifies it itself and opens it; both write to its table and see each other's
// rows; an update on the laptop reaches the phone, which migrates and reloads its open tab; a
// removal with "delete data" on the laptop removes it on the phone too. The devices come from the
// group API of spec 033 and find each other through the scenario's Nostr relay.

interface Listed {
  id: string
  state: string
  version?: string
  statusHere?: string
}

async function listed(
  page: Page,
  extension: InstalledExtension,
): Promise<Listed | undefined> {
  return unwrap<Listed[]>(
    'extension_list',
    await page.invoke('extension_list'),
  ).find((e) => e.id === extension.id)
}

async function rows(
  page: Page,
  extension: InstalledExtension,
  sql: string,
): Promise<unknown[][] | null> {
  const answer = await probeRequest(
    page,
    extension,
    'extension_database_query',
    { query: sql, params: [] },
  )
  return (answer.result as { rows?: unknown[][] } | undefined)?.rows ?? null
}

scenario('extension-two-devices', { timeoutMs: 300_000 }, async (ctx) => {
  const group = await ctx.group({ users: { anna: ['laptop', 'phone'] } })
  const laptop = group.device('anna/laptop').page
  const phone = group.device('anna/phone').page

  const probe = await install(laptop, fixture('e2e', 'probe'))
  await ctx.waitFor(
    'the phone to verify the probe and start it',
    async () => (await listed(phone, probe))?.statusHere === 'ready',
    { timeoutMs: 60_000, fixed: true },
  )
  await openFromLauncher(laptop, probe)
  await openFromLauncher(phone, probe)
  ctx.step('installed on the laptop, verified and opened on the phone')

  const info = await probeRequest(laptop, probe, 'extension_get_info', {})
  const table = `${(info.result as { publicKey: string }).publicKey}__probe__items`
  for (const [page, id] of [
    [laptop, 'from-laptop'],
    [phone, 'from-phone'],
  ] as const) {
    const written = await probeRequest(
      page,
      probe,
      'extension_database_execute',
      {
        query: `INSERT INTO "${table}" (id, label) VALUES (?, ?)`,
        params: [id, id],
      },
    )
    assert.equal(written.error, undefined, JSON.stringify(written))
  }
  for (const [name, page] of [
    ['laptop', laptop],
    ['phone', phone],
  ] as const) {
    await ctx.waitFor(
      `the ${name} to hold the rows of both devices`,
      async () =>
        (await rows(page, probe, `SELECT id FROM "${table}"`))?.length === 2,
      { timeoutMs: 60_000, fixed: true },
    )
  }
  ctx.step('written on both devices, seen on both')

  // The phone's open tab is a document from before the update; after it, a new one.
  await inFrame(phone, probe, `window.beforeUpdate = true; return true`)
  await install(laptop, fixture('e2e', 'probe-v2'))
  await ctx.waitFor(
    'the phone to switch to 1.1.0',
    async () => {
      const here = await listed(phone, probe)
      return here?.version === '1.1.0' && here.statusHere === 'ready'
    },
    { timeoutMs: 60_000, fixed: true },
  )
  // The frame is replaced while it reloads; a call that lands in the old one counts as not yet.
  await ctx.waitFor(
    'the open tab on the phone to reload',
    () =>
      inFrame<boolean>(
        phone,
        probe,
        `return window.beforeUpdate === undefined && document.documentElement.dataset.probeReady === '1'`,
      ).catch(() => false),
    { timeoutMs: 30_000 },
  )
  assert.notEqual(
    await rows(phone, probe, `SELECT tag FROM "${table}"`),
    null,
    'the migration of 1.1.0 ran on the phone',
  )
  ctx.step('an update on the laptop migrates the phone and reloads its tab')

  unwrap(
    'extension_remove',
    await laptop.invoke('extension_remove', {
      extensionId: probe.id,
      deleteData: true,
    }),
  )
  // Removed with its data, nothing of it is listed any more.
  await ctx.waitFor(
    'the removal to reach the phone',
    async () => (await listed(phone, probe)) === undefined,
    { timeoutMs: 60_000, fixed: true },
  )
  await phone.click('open-launcher')
  await ctx.waitFor(
    'the launcher entry to go on the phone',
    async () =>
      (await phone.exec<number>(
        `return document.querySelectorAll('[data-app-id="extension.${probe.id}"]').length`,
      )) === 0,
    { timeoutMs: 30_000 },
  )
  ctx.step('removed with "delete data" on the laptop, gone on the phone')
})
