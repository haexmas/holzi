import assert from 'node:assert/strict'
import { scenario } from '../lib/scenario.ts'
import { createAndUnlock } from '../lib/flows.ts'
import {
  activeTab,
  createEntry,
  openPasswords,
  selectTab,
} from '../lib/passwords.ts'
import { KEY, isShown, resizeAppWindow } from '../lib/settings.ts'

const APP = 'system.passwords'
const sidebarShown = (instance: Parameters<typeof isShown>[0]) =>
  isShown(instance, '#passwords-sidebar')

// Spec 034, quickstart §11 (FR-041, SC-012): below the same width as the settings the sidebar is gone
// and the toolbar keeps its button and the search; the button opens the sidebar over the content, a
// choice or Escape closes it; at 360 px no place scrolls sideways and every action stays reachable.
scenario('passwords-narrow-window', {}, async (ctx) => {
  const instance = await ctx.startInstance()
  await createAndUnlock(instance, { name: 'e2e-passwords-narrow' })
  const id = await createEntry(instance, {
    title: 'A rather long title of an entry that has to fit a narrow window',
    username: 'someone-with-a-long-address@example.invalid',
    tags: ['narrow'],
  })
  await openPasswords(instance)

  await resizeAppWindow(instance, APP, 1000)
  await ctx.waitFor('the sidebar in the wide window', () =>
    sidebarShown(instance),
  )
  await instance.click('passwords-sidebar-toggle')
  await ctx.waitFor(
    'the sidebar to hide',
    async () => !(await sidebarShown(instance)),
  )
  await instance.click('passwords-sidebar-toggle')
  await ctx.waitFor('the sidebar to come back', () => sidebarShown(instance))
  ctx.step('wide: hide and show')

  await resizeAppWindow(instance, APP, 600)
  await ctx.waitFor(
    'the sidebar to leave the narrow window',
    async () => !(await sidebarShown(instance)),
  )
  await instance.waitForDisplayed('passwords-sidebar-toggle')
  await instance.waitForDisplayed('passwords-search')
  await instance.waitForDisplayed('passwords-new')
  await instance.click('passwords-sidebar-toggle')
  await ctx.waitFor('the sidebar over the content', () =>
    sidebarShown(instance),
  )
  await instance.click('passwords-trash')
  await ctx.waitFor(
    'a choice to close the sidebar',
    async () => !(await sidebarShown(instance)),
  )
  await instance.click('passwords-sidebar-toggle')
  await ctx.waitFor('the sidebar to open again', () => sidebarShown(instance))
  await instance.type('#passwords-sidebar button', KEY.escape)
  await ctx.waitFor(
    'Escape to close the sidebar',
    async () => !(await sidebarShown(instance)),
  )
  ctx.step('narrow: open over the content, a choice and Escape close')

  await resizeAppWindow(instance, APP, 360)
  const overflow = () =>
    instance.exec<number>(
      `const frame = document.querySelector('[data-testid="passwords-search"]').closest('[data-wm-window-id]')
       const right = frame.getBoundingClientRect().right
       const clipped = (el) => {
         for (let node = el.parentElement; node && node !== frame; node = node.parentElement) {
           if (getComputedStyle(node).overflowX !== 'visible') return true
         }
         return false
       }
       let worst = 0
       for (const el of frame.querySelectorAll('*')) {
         const style = getComputedStyle(el)
         if (style.visibility === 'hidden') continue
         if (style.overflowX === 'auto' || style.overflowX === 'scroll') {
           worst = Math.max(worst, el.scrollWidth - el.clientWidth)
         }
         if (!clipped(el)) worst = Math.max(worst, el.getBoundingClientRect().right - right)
       }
       return worst`,
    )
  // The list, the entry and the editor at 360 px; the sidebar's own choice leads back to the list.
  await instance.click('passwords-sidebar-toggle')
  await ctx.waitFor('the sidebar to open at 360 px', () =>
    sidebarShown(instance),
  )
  await instance.click('passwords-all')
  await ctx.waitFor(
    'the sidebar to close at 360 px',
    async () => !(await sidebarShown(instance)),
  )
  await instance.waitForDisplayed(`passwords-entry-${id}`)
  assert.ok((await overflow()) <= 1, 'the list scrolls sideways at 360 px')
  await instance.click(`passwords-entry-${id}`)
  await instance.waitForDisplayed('passwords-edit')
  assert.ok((await overflow()) <= 1, 'the entry scrolls sideways at 360 px')
  for (const hook of [
    'passwords-edit',
    'entry-tab-details',
    'entry-tab-extra',
    'entry-tab-history',
    'passwords-delete',
  ]) {
    await instance.waitForDisplayed(hook)
  }
  // Spec 036 (FR-042, SC-003): every tab of the entry fits 360 px.
  for (const tab of ['extra', 'history', 'details'] as const) {
    await selectTab(instance, tab)
    await ctx.waitFor(
      `the ${tab} tab`,
      async () => (await activeTab(instance)) === tab,
    )
    assert.ok(
      (await overflow()) <= 1,
      `the ${tab} tab scrolls sideways at 360 px`,
    )
  }
  await instance.click('passwords-edit')
  await instance.waitForDisplayed('passwords-editor')
  assert.ok((await overflow()) <= 1, 'the editor scrolls sideways at 360 px')
  await selectTab(instance, 'extra')
  await ctx.waitFor(
    'the editor on Extra',
    async () => (await activeTab(instance)) === 'extra',
  )
  assert.ok(
    (await overflow()) <= 1,
    'the editor Extra scrolls sideways at 360 px',
  )
  await instance.waitForDisplayed('passwords-editor-save')
  ctx.step('360 px without sideways scrolling, actions reachable')
})
