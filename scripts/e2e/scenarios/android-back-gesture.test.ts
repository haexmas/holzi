import assert from 'node:assert/strict'
import { scenario } from '../lib/scenario.ts'
import { createAndUnlock, openLauncher } from '../lib/flows.ts'
import { isShown, runAction, waitForLocation } from '../lib/settings.ts'

const LAUNCHER_ENTRY = '[data-app-id="system.settings"]'

// Spec 043 FR-013 with spec 020 FR-019: the system's back gesture closes an open overlay, else goes
// back in the active tab, else (on a phone) opens the window overview, and it never leaves the app.
scenario('android-back-gesture', { needs: { phone: true } }, async (ctx) => {
  const instance = await ctx.startInstance()
  const phone = instance.phone!
  await createAndUnlock(instance, { name: 'phone-vault' })

  const opened = await runAction(instance, 'wm.app.open', {
    appId: 'system.settings',
    at: '/general/basic',
  })
  assert.ok(opened.ok, JSON.stringify(opened))
  await waitForLocation(instance, 'general.basic')
  await instance.click('settings-row-general.basic.password')
  await waitForLocation(instance, 'general.basic.password')
  phone.back()
  await waitForLocation(instance, 'general.basic')
  ctx.step('back goes back in the active tab')

  await openLauncher(instance)
  await ctx.waitFor('the launcher', () => isShown(instance, LAUNCHER_ENTRY))
  phone.back()
  await ctx.waitFor(
    'the launcher to close',
    async () => !(await isShown(instance, LAUNCHER_ENTRY)),
  )
  ctx.step('back closes an open overlay')

  // Further back gestures with nothing left to go back to: the app stays, on its workspace.
  phone.back()
  phone.back()
  phone.back()
  await new Promise((resolve) => setTimeout(resolve, 1_000))
  assert.ok(instance.alive(), 'the back gesture ended the app')
  assert.match(
    await instance.exec<string>('return location.pathname'),
    /^\/workspace\//,
  )
  ctx.step('back never leaves the app')
})
