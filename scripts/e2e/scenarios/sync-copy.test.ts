import assert from 'node:assert/strict'
import { scenario } from '../lib/scenario.ts'
import type { ScenarioContext } from '../lib/scenario.ts'
import type { Device } from '../lib/group.ts'
import { addThread, expectThreads, threadTitles } from '../lib/sync-flows.ts'
import {
  admissionRequestCount,
  decideAdmission,
  federationNotice,
  openFederation,
} from '../lib/sync-ui.ts'

const START = 'Vor dem Kopieren'
const FROM_A = 'Vom Hauptgerät'
const FROM_COPY = 'Von der Kopie'
const WHILE_WAITING = 'Während die Kopie wartet'
const MADE_WAITING = 'In der Wartezeit der Kopie gemacht'
const AFTER_REFUSAL = 'Nach der Ablehnung'

/** Gives the others time to send something that must not come; absence has no event to wait for. */
const QUIET_MS = 5_000
const quiet = () => new Promise((resolve) => setTimeout(resolve, QUIET_MS))

async function expectState(
  ctx: ScenarioContext,
  device: Device,
  wanted: string[],
  description: string,
) {
  await ctx.waitFor(
    description,
    async () => wanted.includes((await device.status()).thisDevice),
    { timeoutMs: 40_000, fixed: true },
  )
}

// Spec 024, user story 7 (M5 of its quickstart, FR-044 and FR-045): the vault file of a main device,
// copied to another computer, makes that computer a main device which syncs with the first; the copy
// of a linked device waits until a main device admits it, meanwhile neither sends nor receives, and
// brings what it did meanwhile along; a copy that is refused stays outside.
scenario('sync-copy', { timeoutMs: 900_000 }, async (ctx) => {
  const g = await ctx.group({ users: { anna: ['laptop', 'phone'] } })
  const a = g.device('anna/laptop')
  const b = g.device('anna/phone')
  await addThread(a, START)
  await expectThreads(ctx, b, [START], 'the linked device to get the chat')

  // A copy of a main device is a main device.
  await a.stop()
  const c = await a.copyVaultTo('desktop')
  await a.start()
  await c.start()
  await c.page.waitForDisplayed('copy-notice')
  assert.equal((await c.status()).thisDevice, 'main')
  await c.page.click('copy-notice-dismiss')
  await ctx.waitFor(
    'the notice to go',
    async () =>
      await c.page.exec<boolean>(
        `return document.querySelector('[data-testid="copy-notice"]') === null`,
      ),
  )
  ctx.step('the copy of a main device is a main device and says so')

  await ctx.waitFor(
    'the first device to list the copy as a main device',
    async () =>
      (await a.deviceList()).filter((row) => row.role === 'main').length === 2,
    { timeoutMs: 40_000, fixed: true },
  )
  await addThread(a, FROM_A)
  await expectThreads(
    ctx,
    c,
    [START, FROM_A],
    'the copy to get a chat from the first device',
  )
  await addThread(c, FROM_COPY)
  const all = [START, FROM_A, FROM_COPY]
  await expectThreads(
    ctx,
    a,
    all,
    'the first device to get a chat from the copy',
  )
  await expectThreads(ctx, b, all, 'the linked device to get both')
  ctx.step('both main devices sync both ways')
  await c.stop()

  // A copy of a linked device waits.
  await b.stop()
  const d = await b.copyVaultTo('tablet')
  await b.start()
  await d.start()
  await expectState(
    ctx,
    d,
    ['awaiting_admission'],
    'the copy of the linked device to wait for admission',
  )
  await openFederation(d.page)
  await d.page.waitForDisplayed('settings-federation-notice')
  assert.ok((await federationNotice(d.page)) !== null)
  ctx.step('the copy of a linked device waits for admission')

  await addThread(a, WHILE_WAITING)
  await expectThreads(
    ctx,
    b,
    [...all, WHILE_WAITING],
    'the linked device to get the chat made while the copy waits',
  )
  await addThread(d, MADE_WAITING)
  await quiet()
  assert.deepEqual(
    await threadTitles(d),
    [...all, MADE_WAITING].sort(),
    'the waiting copy receives nothing',
  )
  assert.deepEqual(
    await threadTitles(a),
    [...all, WHILE_WAITING].sort(),
    'the waiting copy sends nothing',
  )
  ctx.step('the waiting copy neither sends nor receives')

  await decideAdmission(a.page, true)
  await expectState(
    ctx,
    d,
    ['linked'],
    'the copy to be admitted as a linked device',
  )
  const everything = [...all, WHILE_WAITING, MADE_WAITING]
  for (const device of [a, b, d]) {
    await expectThreads(
      ctx,
      device,
      everything,
      `${device.address} to have every chat after the admission`,
    )
  }
  await c.start()
  await expectThreads(
    ctx,
    c,
    everything,
    'the stopped main device to catch up on everything',
  )
  ctx.step(
    'after the admission the copy syncs, and what it made arrives everywhere',
  )

  // A second copy of the linked device is refused.
  await b.stop()
  const e = await b.copyVaultTo('notebook')
  await b.start()
  await e.start()
  await expectState(
    ctx,
    e,
    ['awaiting_admission'],
    'the second copy to wait for admission',
  )
  await decideAdmission(a.page, false)
  assert.equal(await admissionRequestCount(a.page), 0)
  await addThread(a, AFTER_REFUSAL)
  await expectThreads(
    ctx,
    b,
    [...everything, AFTER_REFUSAL],
    'the linked device to get the chat made after the refusal',
  )
  await quiet()
  assert.ok(
    !(await threadTitles(e)).includes(AFTER_REFUSAL),
    'the refused copy receives nothing',
  )
  assert.notEqual((await e.status()).thisDevice, 'linked')
  assert.notEqual((await e.status()).thisDevice, 'main')
  assert.equal(
    (await a.deviceList()).length,
    4,
    'the refused copy is not in the device list',
  )
  ctx.step('a refused copy stays outside')
})
