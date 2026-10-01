import assert from 'node:assert/strict'
import { scenario } from '../lib/scenario.ts'
import { unwrap } from '../lib/flows.ts'
import { openSettings, runAction } from '../lib/settings.ts'
import { startFirstDevice } from '../lib/sync-flows.ts'

const SERVERS = (relayUrl: string) => ({
  nostrRelays: [relayUrl],
  irohRelays: ['https://127.0.0.1:1'],
})

// Spec 024, user story 5 (SC-009, SC-011): linking a new device, with the main device driven through
// its own window and the new installation a second application process. The code shown in the window
// brings the new device up, the main device shows its name and asks, "Verknüpfen" gives it the vault;
// a second new device that is refused gets nothing and no vault is left on it. (The new installation
// is driven by command: its form has no field for servers of its own, so it could not find a relay
// that is not one of the built-in ones.)
scenario('sync-link', { timeoutMs: 360_000 }, async (ctx) => {
  const relay = await ctx.nostrRelay()
  const main = await startFirstDevice(ctx, relay.url, 'e2e-link')
  const passphrase = ctx.credentials().passphrase

  const showCode = async (): Promise<string> => {
    await ctx.waitFor('the link view to open', async () => {
      await runAction(main.instance, 'settings.devices.link')
      return main.instance.exec<boolean>(
        `return document.querySelector('[data-testid="link-device-view"]') !== null`,
      )
    })
    // After a link the view offers to start again before it shows the button for a code.
    const again = await main.instance.exec<boolean>(
      `return document.querySelector('[data-testid="link-again"]') !== null`,
    )
    if (again) await main.instance.click('link-again')
    await main.instance.click('link-show-code')
    await main.instance.waitForDisplayed('link-code-text')
    return main.instance.exec<string>(
      `return document.querySelector('[data-testid="link-code-text"]').textContent.trim()`,
    )
  }
  const join = async (code: string, deviceName: string) => {
    const fresh = await ctx.startInstance()
    unwrap(
      'link_join_start',
      await fresh.invoke('link_join_start', {
        args: {
          code,
          vaultName: 'e2e-link',
          deviceName,
          passphrase,
          servers: SERVERS(relay.url),
        },
      }),
    )
    return fresh
  }
  const stateOf = async (fresh: Awaited<ReturnType<typeof join>>) =>
    unwrap<{ state: string; reason?: string }>(
      'link_join_status',
      await fresh.invoke('link_join_status'),
    )
  const vaults = async (fresh: Awaited<ReturnType<typeof join>>) =>
    unwrap<Array<{ name: string }>>(
      'list_instances',
      await fresh.invoke('list_instances'),
    ).map((vault) => vault.name)

  await openSettings(main.instance)

  // A new device that is agreed to.
  const first = await join(await showCode(), 'Neues Gerät')
  await main.instance.waitForDisplayed('link-confirm', 40_000)
  const asked = await main.instance.exec<boolean>(
    `return document.body.textContent.includes('Neues Gerät')`,
  )
  assert.ok(asked, 'the main device shows the name of the new device')
  await main.instance.click('link-confirm')
  await ctx.waitFor(
    'the new device to get the vault',
    async () => (await stateOf(first)).state === 'done',
    { timeoutMs: 40_000, fixed: true },
  )
  assert.deepEqual(await vaults(first), ['e2e-link'])
  ctx.step('agreed to, the vault arrived')

  // A new device that is refused.
  const second = await join(await showCode(), 'Fremdes Gerät')
  await main.instance.waitForDisplayed('link-reject', 40_000)
  await main.instance.click('link-reject')
  await ctx.waitFor(
    'the refused device to hear it was refused',
    async () => {
      const state = await stateOf(second)
      return state.state === 'failed' && state.reason === 'rejected'
    },
    { timeoutMs: 40_000, fixed: true },
  )
  assert.deepEqual(await vaults(second), [], 'a refused device keeps no vault')
  ctx.step('refused, nothing left on the new device')
})
