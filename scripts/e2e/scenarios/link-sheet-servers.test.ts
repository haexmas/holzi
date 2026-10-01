import assert from 'node:assert/strict'
import { scenario } from '../lib/scenario.ts'

// Spec 024, FR-008, FR-023: the link form on the start page has a field for the Nostr servers, and
// the built-in servers are shown while none is entered. A main device with its own servers cannot tell
// a new installation about them, so this is the only place to name them there.
scenario('link-sheet-servers', {}, async (ctx) => {
  const instance = await ctx.startInstance()
  const count = (hook: string) =>
    instance.exec<number>(
      `return document.querySelectorAll('[data-testid="${hook}"]').length`,
    )
  const text = (hook: string) =>
    instance.exec<string>(
      `return document.querySelector('[data-testid="${hook}"]')?.textContent ?? ''`,
    )

  await instance.click('landing-link')
  await instance.waitForDisplayed('link-servers-nostr-input')
  assert.equal(await count('link-servers-nostr-default'), 3)
  assert.equal(await count('link-servers-nostr'), 0)
  ctx.step('the three built-in Nostr servers are shown')

  await instance.type('link-servers-nostr-input', 'wss://relay.example.org')
  await instance.click('link-servers-nostr-add')
  await ctx.waitFor('the entered server to be listed', async () => {
    return (await count('link-servers-nostr')) === 1
  })
  assert.equal(await count('link-servers-nostr-default'), 0)
  assert.match(await text('link-servers-nostr'), /relay\.example\.org/)
  assert.match(await text('link-servers-nostr-replaced'), /relay\.damus\.io/)
  ctx.step('the entered server replaces them, and they are still named')
})
