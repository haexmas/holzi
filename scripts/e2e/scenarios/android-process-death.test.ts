import assert from 'node:assert/strict'
import { scenario } from '../lib/scenario.ts'
import { createAndUnlock, unwrap } from '../lib/flows.ts'

// Spec 043 FR-007: the system may end the app's process in the background at any time. The next
// start finds the vault closed and opens it again with its passphrase; nothing says it is open
// elsewhere (the lock ended with the process).
scenario('android-process-death', { needs: { phone: true } }, async (ctx) => {
  const { passphrase } = ctx.credentials()
  const first = await ctx.startInstance()
  await createAndUnlock(first, { name: 'phone-vault', passphrase })
  first.phone!.killInBackground()
  await ctx.waitFor('the process to end', () => !first.alive(), {
    timeoutMs: 15_000,
    fixed: true,
  })
  await first.stop()
  ctx.step('the system ended the app in the background')

  const next = await ctx.startInstance({ reusesRoot: first.root })
  await next.waitForDisplayed('landing-import')
  const opened = await next.invoke('open_instance', {
    args: { name: 'phone-vault', passphrase },
  })
  unwrap('open_instance', opened)
  ctx.step('the vault opens again after the restart')
  assert.ok(next.alive())
})
