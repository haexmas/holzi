import assert from 'node:assert/strict'
import { scenario } from '../lib/scenario.ts'
import { createAndUnlock, openChat } from '../lib/flows.ts'
import { isShown, runAction, wmSnapshot } from '../lib/settings.ts'

// Spec 015-workspace-shell, FR-031/036/037: below the compact threshold the tab bar collapses to the
// active tab's title, dropping the per-tab close buttons. With more than one tab that left no way to
// close a single tab short of closing the whole window. The collapsed title now carries its own close
// button whenever there is more than one tab.
scenario('compact-tab-close', {}, async (ctx) => {
  const instance = await ctx.startInstance()
  await createAndUnlock(instance, { name: 'e2e-compact-close' })
  await openChat(instance)

  const [chatWindow] = (await wmSnapshot(instance)).windows
  if (!chatWindow) throw new Error('no window after opening chat')
  const opened = await runAction(instance, 'wm.tab.new', {
    windowId: chatWindow.id,
    appId: 'system.settings',
  })
  if (!opened.ok) {
    throw new Error(`wm.tab.new failed: ${JSON.stringify(opened)}`)
  }
  ctx.step('two tabs in one window')

  const wide = await wmSnapshot(instance)
  assert.equal(
    wide.windows.find((w) => w.id === chatWindow.id)?.tabs.length,
    2,
    'settings did not join the chat window as a second tab',
  )

  // `compact` follows the app's own viewport (`wm/Desktop.vue`'s `useWindowSize`), not an individual
  // window's geometry (T049, `layoutState.ts`) — drive it the same way that watcher does.
  await instance.exec(
    "document.querySelector('#__nuxt').__vue_app__.config.globalProperties.$pinia._s.get('windowManager').updateArea({ width: 600, height: 700 }); return true",
  )
  await ctx.waitFor('the compact close button', () =>
    isShown(instance, '[data-testid="tab-close"]'),
  )
  ctx.step('compact: close button visible with two tabs')

  await instance.click('tab-close')
  await ctx.waitFor('one tab left in the window', async () => {
    const snapshot = await wmSnapshot(instance)
    return (
      snapshot.windows.find((w) => w.id === chatWindow.id)?.tabs.length === 1
    )
  })
  const afterClose = await wmSnapshot(instance)
  const remaining = afterClose.windows.find((w) => w.id === chatWindow.id)
  assert.ok(remaining, 'the window closed along with the tab')
  assert.equal(
    remaining?.tabs[0]?.appId,
    'system.chat',
    'the wrong tab was closed',
  )
  ctx.step('closing the tab kept the window open')

  assert.ok(
    !(await isShown(instance, '[data-testid="tab-close"]')),
    'the close button stayed with a single tab left',
  )
  ctx.step('single tab: no close button, same as the non-compact case')
})
