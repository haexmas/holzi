import assert from 'node:assert/strict'
import { scenario } from '../lib/scenario.ts'

// Spec 024, FR-008, FR-023: the link form on the start page lists the Nostr servers: the built-in
// ones, which can be switched off but not removed, and added ones, which can be switched off or
// removed. A main device with its own servers cannot tell a new installation about them, so this is
// the only place to name them there.
scenario('link-sheet-servers', {}, async (ctx) => {
  const instance = await ctx.startInstance()
  const rows = (kind: string, enabled?: boolean) =>
    instance.exec<number>(
      `return document.querySelectorAll('[data-testid="link-servers-nostr-${kind}"]${
        enabled === undefined ? '' : `[data-enabled="${enabled}"]`
      }').length`,
    )

  await instance.click('landing-link')
  await instance.waitForDisplayed('link-servers-nostr-input')
  await ctx.waitFor('the built-in Nostr servers to load', async () => {
    return (await rows('default', true)) === 3
  })
  assert.equal(await rows('added'), 0)
  ctx.step('the three built-in Nostr servers are listed, all in use')

  await instance.type('link-servers-nostr-input', 'wss://relay.example.org')
  await instance.click('link-servers-nostr-add')
  await ctx.waitFor('the added server to be listed', async () => {
    return (await rows('added', true)) === 1
  })
  assert.equal(await rows('default'), 3)
  ctx.step('an added server is listed after them')

  await instance.click('link-servers-nostr-toggle')
  await ctx.waitFor('the first server to be switched off', async () => {
    return (await rows('default', false)) === 1
  })
  assert.equal(await rows('default'), 3)
  ctx.step('switching one off keeps it listed')

  await instance.click('link-servers-nostr-remove')
  await ctx.waitFor('the added server to be deleted', async () => {
    return (await rows('added')) === 0
  })
  assert.equal(await rows('default'), 3)
  assert.equal(await rows('default', false), 1)
  ctx.step('deleting the added server leaves the built-in ones')
})
