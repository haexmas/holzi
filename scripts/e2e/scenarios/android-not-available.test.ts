import assert from 'node:assert/strict'
import { scenario } from '../lib/scenario.ts'
import { createAndUnlock, unwrap } from '../lib/flows.ts'
import {
  fixture,
  inFrame,
  install,
  openFromLauncher,
  probeRequest,
} from '../lib/extensions.ts'
import { runAction, waitForLocation } from '../lib/settings.ts'

/** The bridge's answer for what this device does not have (spec 017, FR-066). */
const NOT_AVAILABLE = 8001

// Spec 043 FR-016, FR-026 (the counter case of `extension-files` and `extension-dev-mode`): what a
// phone does not have is not offered and says so. The capability table names it; an extension that
// reads a free path, picks a folder or watches one hears "not available"; developer mode (a project
// folder) is left out of the extension settings, and the delegate settings say "not available on
// this device".
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

  const probe = await install(instance, fixture('e2e', 'probe'))
  await openFromLauncher(instance, probe)
  await ctx.waitFor(
    'the probe to be ready',
    () =>
      inFrame<boolean>(
        instance,
        probe,
        `return document.documentElement.dataset.probeReady === '1'`,
      ).catch(() => false),
    { timeoutMs: 20_000 },
  )
  const answers = {
    'a free path': await probeRequest(
      instance,
      probe,
      'extension_filesystem_read_file',
      { path: '/system/etc/hosts' },
    ),
    'a folder dialog': await probeRequest(
      instance,
      probe,
      'extension_filesystem_select_folder',
      {},
    ),
    'a watched folder': await probeRequest(
      instance,
      probe,
      'extension_filesystem_watch',
      { ruleId: 'notes', path: '/system/etc' },
    ),
  }
  for (const [what, answer] of Object.entries(answers)) {
    assert.equal(answer.error?.code, NOT_AVAILABLE, what)
  }
  ctx.step('an extension hears "not available" for free paths and folders')

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
