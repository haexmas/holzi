import assert from 'node:assert/strict'
import { scenario } from '../lib/scenario.ts'
import {
  dialogClosed,
  reveal,
  storedAppearance,
  theme,
  themeHue,
  themeLightness,
} from '../lib/appearance.ts'
import {
  KEY,
  openSettings,
  runAction,
  textContrast,
  waitForLocation,
} from '../lib/settings.ts'

// Spec 035-appearance-and-fields, quickstart Stage 2 (US2, US4, US5; FR-012, FR-013, FR-014, FR-017,
// FR-021, SC-003, SC-006): a colour field applies at once and is saved without a save button; a custom
// colour and a half-typed hex value; the window and the container tint change only their own
// surfaces; the choice survives a restart; the reset goes back to the defaults after a confirmation
// and keeps the colour scheme; export and import give the same state and a broken file changes
// nothing. (One process holds one vault, spec 013, so a second vault never shows these colours.)
scenario('appearance-basic', { timeoutMs: 300_000 }, async (ctx) => {
  const group = await ctx.group({ users: { anna: ['laptop'] } })
  const device = group.device('anna/laptop')
  let page = device.page

  await openSettings(page)
  await page.click('settings-category-appearance')
  await waitForLocation(page, 'appearance')
  const teal = await themeHue(page, '--primary')
  assert.equal(teal, 180, 'the default accent is teal')

  // A colour field applies at once and is saved.
  await page.click('appearance-swatch-accent-blue')
  await ctx.waitFor(
    'the accent to turn blue',
    async () => (await themeHue(page, '--primary')) === 255,
  )
  assert.equal(
    await themeHue(page, '--ring'),
    255,
    'the ring follows the accent',
  )
  assert.deepEqual(
    ((await storedAppearance(page)) as { accent: unknown }).accent,
    { preset: 'blue' },
  )
  ctx.step('accent applies at once and is stored')

  // The accent follows the colour scheme: same hue, lightness derived for the scheme (US3, FR-018).
  const lightnessIn = async (scheme: string) => {
    await runAction(page, 'settings.appearance.setColorScheme', { scheme })
    await ctx.waitFor(`the ${scheme} accent`, async () => {
      const dark = await page.exec<boolean>(
        `return document.documentElement.classList.contains('dark')`,
      )
      return dark === (scheme === 'dark')
    })
    return themeLightness(page, '--primary')
  }
  const lightL = await lightnessIn('light')
  const darkL = await lightnessIn('dark')
  assert.notEqual(lightL, darkL, 'the accent is derived for each scheme')
  assert.equal(await themeHue(page, '--primary'), 255, 'the hue stays')
  await lightnessIn('system')
  ctx.step('the accent follows the colour scheme')

  // A custom accent that needs another lightness says so; a colour field never does (FR-017).
  await runAction(page, 'settings.appearance.setColorScheme', {
    scheme: 'light',
  })
  await runAction(page, 'settings.appearance.set', {
    accent: { custom: '#ffffaa' },
  })
  await page.waitForDisplayed('appearance-adjusted')
  await page.click('appearance-swatch-accent-teal')
  await ctx.waitFor('the note to go', async () =>
    page.exec<boolean>(
      `return !document.querySelector('[data-testid="appearance-adjusted"]')`,
    ),
  )
  await runAction(page, 'settings.appearance.set', {
    accent: { preset: 'blue' },
  })
  await runAction(page, 'settings.appearance.setColorScheme', {
    scheme: 'system',
  })
  ctx.step('an adjusted custom colour says so')

  // A half-typed hex value changes nothing; a whole one applies.
  await page.click('appearance-add-accent')
  await page.waitForDisplayed('appearance-hex-accent')
  await page.type('appearance-hex-accent', KEY.backspace.repeat(7) + '#12')
  assert.deepEqual(
    ((await storedAppearance(page)) as { accent: unknown }).accent,
    { preset: 'blue' },
  )
  assert.equal(await themeHue(page, '--primary'), 255)
  await page.type('appearance-hex-accent', '3456')
  await ctx.waitFor(
    'the custom accent to apply',
    async () =>
      JSON.stringify(
        ((await storedAppearance(page)) as { accent: unknown }).accent,
      ) === JSON.stringify({ custom: '#123456' }),
  )
  await page.type('appearance-hex-accent', KEY.escape)
  ctx.step('a half value writes nothing, a whole one applies')

  // The window tint changes the window background only; the container tint the cards and sidebars.
  const before = {
    background: await theme(page, '--background'),
    card: await theme(page, '--card'),
  }
  await page.click('appearance-swatch-window-warm')
  await ctx.waitFor(
    'the window background to change',
    async () => (await theme(page, '--background')) !== before.background,
  )
  assert.equal(await theme(page, '--card'), before.card)
  const afterWindow = await theme(page, '--background')
  await reveal(page, 'appearance-swatch-container-cool')
  await page.click('appearance-swatch-container-cool')
  await ctx.waitFor(
    'the container to change',
    async () => (await theme(page, '--card')) !== before.card,
  )
  assert.equal(await theme(page, '--background'), afterWindow)
  assert.equal(await theme(page, '--popover'), await theme(page, '--card'))
  ctx.step('tints change only their own surfaces')

  // Text on a primary button stays readable against it (FR-016, FR-022): the confirmation button of the
  // reset is a primary button.
  await reveal(page, 'appearance-reset')
  await page.click('appearance-reset')
  await page.waitForDisplayed('appearance-reset-confirm')
  const ratio = await textContrast(
    page,
    '[data-testid="appearance-reset-confirm"]',
  )
  assert.ok(
    ratio >= 4.5,
    `text on the accent: contrast ${ratio.toFixed(2)} < 4.5`,
  )
  await page.type('appearance-reset-confirm', KEY.escape)
  await ctx.waitFor('the dialog to close', () => dialogClosed(page))
  ctx.step('text on the accent reaches 4.5:1')

  // Export, change, import: the same state; a broken file changes nothing (SC-006).
  const exported = await runAction(page, 'settings.appearance.export')
  assert.ok(exported.ok)
  const file = (exported.result as { file: string }).file
  const state = await storedAppearance(page)
  await reveal(page, 'appearance-swatch-accent-green')
  await page.click('appearance-swatch-accent-green')
  await ctx.waitFor(
    'green',
    async () => (await themeHue(page, '--primary')) === 150,
  )
  const imported = await runAction(page, 'settings.appearance.import', { file })
  assert.ok(imported.ok, JSON.stringify(imported))
  assert.deepEqual(await storedAppearance(page), state)
  const broken = await runAction(page, 'settings.appearance.import', {
    file: '{nope',
  })
  assert.equal(broken.ok, false)
  assert.equal(
    (broken.error as { reason?: string }).reason,
    'settings.appearance.import.notJson',
  )
  assert.deepEqual(await storedAppearance(page), state)
  ctx.step('export and import round-trip, a broken file changes nothing')

  // The choice survives a restart (SC-003).
  await device.restart()
  page = device.page
  await ctx.waitFor(
    'the appearance to come back after the restart',
    async () => (await theme(page, '--background')) === afterWindow,
    { fixed: true, timeoutMs: 30_000 },
  )
  ctx.step('persists across a restart')

  // The reset goes back to the defaults after the confirmation; the colour scheme stays.
  await runAction(page, 'settings.appearance.setColorScheme', {
    scheme: 'light',
  })
  await openSettings(page)
  await page.click('settings-category-appearance')
  await reveal(page, 'appearance-reset')
  await page.click('appearance-reset')
  await page.click('appearance-reset-confirm')
  await ctx.waitFor(
    'the defaults to return',
    async () => (await themeHue(page, '--primary')) === 180,
  )
  assert.equal(
    await page.exec<boolean>(
      `return !document.documentElement.classList.contains('dark')`,
    ),
    true,
    'the colour scheme stays',
  )
  assert.equal(await theme(page, '--background'), 'oklch(0.955 0 0)')
  ctx.step('reset restores the defaults and keeps the colour scheme')
})
