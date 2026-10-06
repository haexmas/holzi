import assert from 'node:assert/strict'
import { scenario } from '../lib/scenario.ts'
import { storedAppearance, theme, themeHue } from '../lib/appearance.ts'
import { runAction } from '../lib/settings.ts'

// Spec 035-appearance-and-fields, SC-005 and the edge case "two devices change the appearance at the
// same time" (research R1): the appearance is one vault preference, so it reaches the other device of
// the user through the sync (spec 024) without a restart, and when two devices change different parts
// at once the younger write wins as a whole and both devices end with the same appearance.
scenario('appearance-sync-two-devices', { timeoutMs: 300_000 }, async (ctx) => {
  const group = await ctx.group({ users: { anna: ['laptop', 'phone'] } })
  const laptop = group.device('anna/laptop').page
  const phone = group.device('anna/phone').page

  for (const [name, device] of [
    ['laptop', laptop],
    ['phone', phone],
  ] as const) {
    await ctx.waitFor(
      `the ${name} to apply the default appearance`,
      async () => (await themeHue(device, '--primary')) === 237,
      { timeoutMs: 20_000, fixed: true },
    )
  }
  const set = await runAction(laptop, 'settings.appearance.set', {
    accent: { preset: 'violet' },
  })
  assert.ok(set.ok, JSON.stringify(set))
  await ctx.waitFor(
    'the accent to reach the phone',
    async () => (await themeHue(phone, '--primary')) === 300,
    { timeoutMs: 40_000, fixed: true },
  )
  ctx.step('a change on one device shows on the other without a restart')

  // Both devices change a different part; afterwards both hold the same, whole appearance.
  await Promise.all([
    runAction(laptop, 'settings.appearance.set', {
      accent: { preset: 'orange' },
    }),
    runAction(phone, 'settings.appearance.set', { window: { preset: 'warm' } }),
  ])
  await ctx.waitFor(
    'both devices to hold the same appearance',
    async () => {
      const [a, b] = await Promise.all([
        storedAppearance(laptop),
        storedAppearance(phone),
      ])
      if (JSON.stringify(a) !== JSON.stringify(b)) return false
      const [
        [laptopPrimary, phonePrimary],
        [laptopBackground, phoneBackground],
      ] = await Promise.all([
        Promise.all([theme(laptop, '--primary'), theme(phone, '--primary')]),
        Promise.all([
          theme(laptop, '--background'),
          theme(phone, '--background'),
        ]),
      ])
      return (
        laptopPrimary === phonePrimary && laptopBackground === phoneBackground
      )
    },
    { timeoutMs: 60_000, fixed: true },
  )
  assert.equal(
    await theme(laptop, '--primary'),
    await theme(phone, '--primary'),
  )
  assert.equal(
    await theme(laptop, '--background'),
    await theme(phone, '--background'),
  )
  ctx.step('concurrent changes end in one appearance on both devices')
})
