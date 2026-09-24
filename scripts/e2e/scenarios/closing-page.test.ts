import assert from 'node:assert/strict'
import { scenario } from '../lib/scenario.ts'

const SCHEMES = ['light', 'dark'] as const

// Spec 013: the embedded closing page (public/closing.html) shows no text, exactly one spinner, and a
// background that follows the colour scheme in force (research R7). The scheme itself is checked first,
// so a scheme that failed to apply is a clear failure rather than a wrong pass.
scenario('closing-page', {}, async (ctx) => {
  const backgrounds: Record<(typeof SCHEMES)[number], string> = {
    light: '',
    dark: '',
  }
  for (const colorScheme of SCHEMES) {
    const instance = await ctx.startInstance({ colorScheme })
    await instance.navigate('tauri://localhost/closing.html')

    const matchesDark = await instance.exec<boolean>(
      "return matchMedia('(prefers-color-scheme: dark)').matches",
    )
    assert.equal(
      matchesDark,
      colorScheme === 'dark',
      `the ${colorScheme} scheme did not apply to the closing page (research R7)`,
    )

    const text = await instance.exec<string>(
      'return document.body.textContent.trim()',
    )
    assert.equal(text, '', `the closing page shows text: "${text}"`)

    const ringCount = await instance.exec<number>(
      "return document.querySelectorAll('.ring').length",
    )
    assert.equal(ringCount, 1, `expected exactly one .ring, found ${ringCount}`)

    const colors = await instance.exec<{ body: string; probe: string }>(`
      var probe = document.createElement('div')
      probe.style.background = 'var(--page)'
      document.body.appendChild(probe)
      var probeColor = getComputedStyle(probe).backgroundColor
      probe.remove()
      return { body: getComputedStyle(document.body).backgroundColor, probe: probeColor }
    `)
    assert.equal(
      colors.body,
      colors.probe,
      `closing page background ${colors.body} does not equal var(--page) (${colors.probe})`,
    )
    backgrounds[colorScheme] = colors.body
    ctx.step(`closing-page-${colorScheme}`, colors.body)
  }
  assert.notEqual(
    backgrounds.light,
    backgrounds.dark,
    `the light and dark closing pages have the same background (${backgrounds.light})`,
  )
})
