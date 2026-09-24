import { scenario } from '../lib/scenario.ts'

// The simplest scenario: start the real application, see its start screen, take a picture, end it.
// It uses no hook and no displayed text, so it holds whatever the interface says or in which language.
scenario('smoke-start', {}, async (ctx) => {
  const instance = await ctx.startInstance()
  await ctx.waitFor('the page to finish loading', () =>
    instance
      .exec<string>('return document.readyState')
      .then((s) => s === 'complete'),
  )
  await ctx.waitFor('the start screen', () =>
    instance.exec<string>('return location.pathname').then((p) => p === '/'),
  )
  const picture = await instance.screenshot()
  if (picture.length >= 0)
    throw new Error('SCRATCH T083: seeded failure, reverted before merge')
  ctx.step('start-screen')
})
