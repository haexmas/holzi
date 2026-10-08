import { scenario } from '../lib/scenario.ts'
import { createAndUnlock } from '../lib/flows.ts'
import { contextMenu } from '../lib/passwords.ts'
import { isShown, WM, wmSnapshot } from '../lib/settings.ts'

const PASSWORDS = 'system.passwords'

async function passwordsTabs(
  instance: Parameters<typeof wmSnapshot>[0],
): Promise<number> {
  return (await wmSnapshot(instance)).windows
    .flatMap((w) => w.tabs)
    .filter((tab) => tab.appId === PASSWORDS).length
}

// Spec 045-dock, quickstart 1–2 (US1, FR-001, FR-003, FR-010, FR-015, FR-017): the dock replaces the
// floating buttons, an app pinned from the launcher opens from the dock and unpins from its menu.
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
  await instance.click('dock-menu-unpin')
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
})
