import assert from 'node:assert/strict'
import { scenario } from '../lib/scenario.ts'
import { createAndUnlock, unwrap } from '../lib/flows.ts'
import { runAction, waitForLocation } from '../lib/settings.ts'

// Spec 043 FR-016, FR-026 (the counter case of `extension-files` and `extension-dev-mode`): what a
// phone does not have is not offered and says so. The capability table names it, developer mode
// (a project folder) is left out of the extension settings, and the delegate settings say "not
// available on this device".
scenario('android-not-available', { needs: { phone: true } }, async (ctx) => {
  const instance = await ctx.startInstance()
  await createAndUnlock(instance, { name: 'phone-vault' })
  const table = unwrap<Record<string, unknown>>(
    'platform_capabilities',
    await instance.invoke('platform_capabilities'),
  )
  for (const facility of [
    'cliDelegates',
    'commandTool',
    'terminal',
    'folderWatch',
    'freePaths',
    'folderPick',
  ]) {
    assert.equal(table[facility], false, facility)
  }
  ctx.step('the capability table names what is missing')

  const extensions = await runAction(instance, 'wm.app.open', {
    appId: 'system.settings',
    at: '/extensions',
  })
  assert.ok(extensions.ok, JSON.stringify(extensions))
  await waitForLocation(instance, 'extensions')
  assert.equal(
    await instance.exec<boolean>(
      `return document.querySelector('[data-testid="extension-dev-mode"]') !== null`,
    ),
    false,
    'developer mode is offered',
  )
  ctx.step('developer mode is not offered')

  const agents = await runAction(instance, 'wm.app.open', {
    appId: 'system.settings',
    at: '/agents/providers',
  })
  assert.ok(agents.ok, JSON.stringify(agents))
  await waitForLocation(instance, 'agents.providers')
  await instance.waitForDisplayed('settings-not-on-this-device')
  ctx.step('the delegate settings say they are not available here')
})
