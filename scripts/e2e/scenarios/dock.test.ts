import assert from 'node:assert/strict'
import { scenario } from '../lib/scenario.ts'
import { createAndUnlock } from '../lib/flows.ts'
import { contextMenu } from '../lib/passwords.ts'
import {
  choose,
  isShown,
  openSettings,
  runAction,
  waitForLocation,
  WM,
  wmSnapshot,
} from '../lib/settings.ts'

const PASSWORDS = 'system.passwords'

async function passwordsTabs(
  instance: Parameters<typeof wmSnapshot>[0],
): Promise<number> {
  return (await wmSnapshot(instance)).windows
    .flatMap((w) => w.tabs)
    .filter((tab) => tab.appId === PASSWORDS).length
}

// Spec 045-dock, quickstart 1–4 and 6–8 (US1–US5; FR-001, FR-003, FR-005, FR-007, FR-010–FR-012,
// FR-015, FR-017, FR-020, FR-022): the dock replaces the floating buttons, an app pinned from the
// launcher opens from the dock and unpins from its menu, a click brings its one instance back from
// another workspace instead of opening a second, an app with two instances offers them to choose,
// the settings move the dock and remove an entry, but never the launcher, and the wheel fans out.
scenario('dock', {}, async (ctx) => {
  const instance = await ctx.startInstance()
  await createAndUnlock(instance, { name: 'e2e-dock' })

  await instance.waitForDisplayed('dock')
  await instance.waitForDisplayed('open-launcher')
  await instance.waitForDisplayed('dock-control-workspaces')
  await instance.waitForDisplayed('dock-control-windows')
  ctx.step('default dock: launcher, workspaces, windows')

  async function pinFromLauncher() {
    await instance.click('open-launcher')
    await contextMenu(instance, `[data-app-id="${PASSWORDS}"]`)
    await instance.click('launcher-menu-pin')
    await instance.exec(`${WM}.overlays.launcher = false; return true`)
    await instance.waitForDisplayed(`dock-item-${PASSWORDS}`)
  }

  await pinFromLauncher()
  ctx.step('pinned from the launcher')

  // Unpinned while not running, so it leaves the dock (a running app would stay, FR-005).
  await contextMenu(instance, `dock-item-${PASSWORDS}`)
  await instance.click('dock-menu-pin')
  await ctx.waitFor(
    'the passwords entry to leave the dock',
    async () =>
      !(await isShown(instance, `[data-testid="dock-item-${PASSWORDS}"]`)),
  )
  ctx.step('unpinned from the dock')

  await pinFromLauncher()
  await instance.click(`dock-item-${PASSWORDS}`)
  await ctx.waitFor(
    'a passwords tab',
    async () => (await passwordsTabs(instance)) === 1,
  )
  ctx.step('opened from the dock')

  const activeWorkspace = () =>
    instance.exec<string>(`return ${WM}.activeWorkspaceId`)
  const first = await activeWorkspace()
  const created = await runAction(instance, 'wm.workspace.create')
  if (!created.ok)
    throw new Error(`wm.workspace.create failed: ${JSON.stringify(created)}`)
  await ctx.waitFor(
    'the new workspace',
    async () => (await activeWorkspace()) !== first,
  )
  await instance.click(`dock-item-${PASSWORDS}`)
  await ctx.waitFor(
    'back in the first workspace',
    async () => (await activeWorkspace()) === first,
  )
  assert.equal(
    await passwordsTabs(instance),
    1,
    'the dock opened a second passwords tab',
  )
  ctx.step('focused across workspaces')

  for (let i = 0; i < 2; i += 1) {
    await instance.click('open-launcher')
    await instance.click('open-chat')
    await instance.exec(`${WM}.overlays.launcher = false; return true`)
  }
  await ctx.waitFor('two chat instances in the dock', async () =>
    instance.exec<boolean>(
      `return document.querySelector('[data-testid="dock-item-system.chat"]')?.dataset.count === '2'`,
    ),
  )
  await instance.click('dock-item-system.chat')
  await instance.waitForDisplayed('dock-instances')
  ctx.step('two instances offered to choose')

  await instance.exec(
    "document.dispatchEvent(new KeyboardEvent('keydown', { key: 'Escape', bubbles: true })); return true",
  )
  await openSettings(instance)
  await instance.click('settings-row-general.dock')
  await waitForLocation(instance, 'general.dock')
  await choose(instance, 'settings-dock-edge', 'left')
  await ctx.waitFor('the dock at the left edge', async () =>
    instance.exec<boolean>(
      `return document.querySelector('[data-testid="dock"]')?.dataset.edge === 'left'`,
    ),
  )
  ctx.step('placed at the left edge from the settings')

  await instance.click(
    '[data-testid="settings-dock-item-control:windows"] [data-testid="settings-dock-remove"]',
  )
  await ctx.waitFor(
    'the windows entry to leave the dock',
    async () =>
      !(await isShown(instance, '[data-testid="dock-control-windows"]')),
  )
  assert.equal(
    await isShown(
      instance,
      '[data-testid="settings-dock-item-control:launcher"] [data-testid="settings-dock-remove"]',
    ),
    false,
    'the launcher can be removed',
  )
  ctx.step('windows entry removed, launcher not removable')

  // FR-033: a dock reserving space at the left narrows the area windows get (in the 800 px app
  // window below the compact threshold), but compact mode follows the app window, so the dock stays.
  const layout = await instance.exec<{
    area: number
    viewport: number
    compact: boolean
  }>(
    `const wm = ${WM}; return { area: wm.area.width, viewport: window.innerWidth, compact: wm.compact }`,
  )
  assert.ok(
    layout.area < layout.viewport,
    `the left dock reserves no space: area ${layout.area}, window ${layout.viewport}`,
  )
  assert.equal(
    layout.compact,
    false,
    'a narrow window area switched to compact mode',
  )
  ctx.step('the dock narrows the area without switching to compact mode')

  // US5 (FR-027, FR-029): the wheel fans out on a click and folds on Escape.
  await choose(instance, 'settings-dock-style', 'wheel')
  await instance.waitForDisplayed('dock-wheel-toggle')
  const expanded = () =>
    instance.exec<string | null>(
      `return document.querySelector('[data-testid="dock-wheel-toggle"]')?.getAttribute('aria-expanded')`,
    )
  await instance.click('dock-wheel-toggle')
  await ctx.waitFor(
    'the wheel to open',
    async () => (await expanded()) === 'true',
  )
  await instance.exec(
    `document.querySelector('[data-testid="dock-wheel-toggle"]').dispatchEvent(new KeyboardEvent('keydown', { key: 'Escape', bubbles: true })); return true`,
  )
  await ctx.waitFor(
    'the wheel to fold',
    async () => (await expanded()) === 'false',
  )
  ctx.step('wheel opens and folds')
})
