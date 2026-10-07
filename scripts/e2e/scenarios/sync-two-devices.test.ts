import { scenario } from '../lib/scenario.ts'
import { connectProvider, unwrap } from '../lib/flows.ts'
import { waitForLocation } from '../lib/settings.ts'
import { expectThreads } from '../lib/sync-flows.ts'
import { reveal } from '../lib/appearance.ts'
import type { Device } from '../lib/group.ts'
import {
  chatTitles,
  sendInNewChat,
  sessionRestoreOn,
  showApp,
} from '../lib/sync-ui.ts'

const FIRST = 'Hallo vom Laptop'
const SECOND = 'Noch ein Chat, während das Telefon aus war'

async function showGeneralSettings(device: Device) {
  await showApp(device.page, 'system.settings')
  await waitForLocation(device.page, 'general')
  await device.page.click('settings-row-general.basic')
  await waitForLocation(device.page, 'general.basic')
  await reveal(device.page, 'session-restore-switch')
  await device.page.waitForDisplayed('session-restore-switch')
}

async function showChat(device: Device) {
  await showApp(device.page, 'system.chat')
  await device.page.waitForDisplayed('chat-input')
}

// Spec 024, user story 1 (M2 of its quickstart, SC-011): two devices of one vault keep what the person
// does in step, seen through the interface. A chat started on one device shows in the chat list of the
// other; a setting changed on one applies on the other; a device that was stopped catches up on what
// the other did while it was away. (The reply of the model is a stand-in; the chat is the person's.)
scenario('sync-two-devices', { timeoutMs: 420_000 }, async (ctx) => {
  const provider = await ctx.provider({ kind: 'stream-then-finish', chunks: 2 })
  const g = await ctx.group({ users: { anna: ['laptop', 'phone'] } })
  const laptop = g.device('anna/laptop')
  const phone = g.device('anna/phone')
  await connectProvider(laptop.page, provider)

  await showChat(laptop)
  await sendInNewChat(laptop.page, FIRST)
  await showChat(phone)
  await ctx.waitFor(
    'the chat from the laptop to show in the chat list of the phone',
    async () => (await chatTitles(phone.page)).includes(FIRST),
    { timeoutMs: 40_000, fixed: true },
  )
  ctx.step('a chat started on one device shows on the other')

  await showGeneralSettings(laptop)
  await showGeneralSettings(phone)
  const before = await sessionRestoreOn(phone.page)
  await phone.page.click('session-restore-switch')
  await ctx.waitFor(
    'the setting changed on the phone to apply on the laptop',
    async () => (await sessionRestoreOn(laptop.page)) === !before,
    { timeoutMs: 40_000, fixed: true },
  )
  if (!before) {
    // Spec 023 FR-024: turned on on the phone, the laptop saves its own session from then on.
    await ctx.waitFor(
      'the laptop to save its session once the setting is on',
      async () =>
        unwrap<{ session: unknown }>(
          'wm_session_load',
          await laptop.page.invoke('wm_session_load'),
        ).session != null,
      { timeoutMs: 40_000, fixed: true },
    )
  }
  ctx.step('a setting changed on the other applies')

  await phone.stop()
  await showChat(laptop)
  await sendInNewChat(laptop.page, SECOND)
  await showGeneralSettings(laptop)
  await laptop.page.click('session-restore-switch')
  await ctx.waitFor(
    'the laptop to show its own change of the setting',
    async () => (await sessionRestoreOn(laptop.page)) === before,
  )
  await phone.start()
  await expectThreads(
    ctx,
    phone,
    [FIRST, SECOND],
    'the phone to catch up on the chat from while it was stopped',
  )
  await showGeneralSettings(phone)
  await ctx.waitFor(
    'the phone to catch up on the setting from while it was stopped',
    async () => (await sessionRestoreOn(phone.page)) === before,
    { timeoutMs: 40_000, fixed: true },
  )
  ctx.step('a device that was stopped catches up on chat and setting')
})
