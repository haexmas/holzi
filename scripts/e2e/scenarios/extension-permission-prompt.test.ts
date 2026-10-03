import assert from 'node:assert/strict'
import { scenario } from '../lib/scenario.ts'
import { unwrap } from '../lib/flows.ts'
import {
  fixture,
  install,
  openFromLauncher,
  probeRequest,
  start,
} from '../lib/extensions.ts'

const NOTES_KEY =
  '3614253f84ba66a8faa168d317a6979992979a3e7ad15ae83ca2823f5ca97d34'

// Spec 017, US3, T072 (quickstart §4): reading another extension's table asks the user; "Erlauben"
// without "Merken" holds for this run, with "Merken" it survives a restart, "Verweigern" answers
// 1002, and revoking in the settings asks again.
scenario('extension-permission-prompt', { timeoutMs: 300_000 }, async (ctx) => {
  const group = await ctx.group({ users: { anna: ['laptop'] } })
  const device = group.device('anna/laptop')
  let page = device.page
  const notesExtension = await install(
    page,
    fixture('vectors', 'good-notes-like'),
  )
  const probe = await install(page, fixture('e2e', 'probe'))
  // The notes tables exist once the notes extension has started (its migrations ran).
  await start(page, notesExtension)
  await openFromLauncher(page, probe)

  const notes = `${NOTES_KEY}__notes-like__notes`
  const tags = `${NOTES_KEY}__notes-like__tags`
  const read = (table: string) =>
    probeRequest(page, probe, 'extension_database_query', {
      sql: `SELECT * FROM "${table}"`,
      params: [],
    })

  assert.equal((await read(notes)).error?.code, 1004)
  await page.waitForDisplayed('extension-permission-request', 10_000)
  await page.click(
    '[data-testid="extension-permission-request"] [role="checkbox"]',
  )
  await page.click('extension-permission-allow')
  assert.equal((await read(notes)).error, undefined, 'allowed once')
  ctx.step('allowed without remembering')

  assert.equal((await read(tags)).error?.code, 1004)
  await page.click('extension-permission-allow')
  assert.equal((await read(tags)).error, undefined)
  ctx.step('allowed and remembered')

  await device.restart()
  page = device.page
  await openFromLauncher(page, probe)
  assert.equal(
    (await read(tags)).error,
    undefined,
    'remembered across a restart',
  )
  assert.equal(
    (await read(notes)).error?.code,
    1004,
    'held only for the last run',
  )
  await page.click('extension-permission-deny')
  assert.equal((await read(notes)).error?.code, 1002)
  ctx.step('denied')

  const views = unwrap<Array<{ id?: string; target: string }>>(
    'extension_permissions_list',
    await page.invoke('extension_permissions_list', { extensionId: probe.id }),
  )
  const remembered = views.find((v) => v.target === tags)
  assert.ok(remembered?.id, JSON.stringify(views))
  unwrap(
    'extension_permission_remove',
    await page.invoke('extension_permission_remove', {
      args: { extensionId: probe.id, permissionId: remembered.id },
    }),
  )
  assert.equal((await read(tags)).error?.code, 1004, 'revoked asks again')
  ctx.step('revoked')
})
