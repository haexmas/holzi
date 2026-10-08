import assert from 'node:assert/strict'
import { scenario } from '../lib/scenario.ts'
import { createAndUnlock, openLauncher, unwrap } from '../lib/flows.ts'
import { PROCESS_END_LIMIT_MS } from '../lib/close-promises.ts'
import { runAction, waitForLocation } from '../lib/settings.ts'

// Spec 043 FR-011a: while a vault is open, the window is kept out of screenshots and recordings by
// default; the switch in Grundeinstellung turns that off at once and for this device, and the choice
// holds when the vault is opened again.
scenario('android-screen-capture', { needs: { phone: true } }, async (ctx) => {
  const { passphrase } = ctx.credentials()
  const first = await ctx.startInstance()
  const phone = first.phone!
  assert.equal(
    phone.screenProtected(),
    false,
    'protected before a vault is open',
  )
  await createAndUnlock(first, { name: 'phone-vault', passphrase })
  await ctx.waitFor('the protection to be on', () => phone.screenProtected())
  ctx.step('an open vault is protected by default')

  const opened = await runAction(first, 'wm.app.open', {
    appId: 'system.settings',
    at: '/general/basic',
  })
  assert.ok(opened.ok, JSON.stringify(opened))
  await waitForLocation(first, 'general.basic')
  await first.click('screen-capture-switch')
  await ctx.waitFor('the protection to go', () => !phone.screenProtected())
  ctx.step('the switch turns it off at once')

  await openLauncher(first)
  await first.press('lock-instance')
  await first.waitForEnd(PROCESS_END_LIMIT_MS)
  await first.stop()
  const next = await ctx.startInstance({ reusesRoot: first.root })
  unwrap(
    'open_instance',
    await next.invoke('open_instance', {
      args: { name: 'phone-vault', passphrase },
    }),
  )
  assert.equal(
    unwrap<boolean>(
      'screen_capture_protection_get',
      await next.invoke('screen_capture_protection_get'),
    ),
    false,
  )
  assert.equal(next.phone!.screenProtected(), false)
  ctx.step('the choice holds for this device')
})
