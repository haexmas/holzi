import { scenario } from '../lib/scenario.ts'
import { createAndUnlock, openChat } from '../lib/flows.ts'
import { KEY, isShown, resizeAppWindow } from '../lib/settings.ts'

const sidebarShown = (instance: Parameters<typeof isShown>[0]) =>
  isShown(instance, '#chat-sidebar')

// The chat's thread sidebar follows the window's width like the settings sidebar does: below 672 px
// it is gone and its toolbar button opens it over the content; the button closes it again. In a wide window the button hides and shows it.
scenario('chat-narrow-window', {}, async (ctx) => {
  const instance = await ctx.startInstance()
  await createAndUnlock(instance, { name: 'e2e-chat-narrow' })
  await openChat(instance)

  await resizeAppWindow(instance, 'system.chat', 1000)
  await ctx.waitFor('the sidebar in the wide window', () =>
    sidebarShown(instance),
  )
  await instance.click('chat-sidebar-toggle')
  await ctx.waitFor(
    'the sidebar to hide',
    async () => !(await sidebarShown(instance)),
  )
  await instance.click('chat-sidebar-toggle')
  await ctx.waitFor('the sidebar to come back', () => sidebarShown(instance))
  ctx.step('wide: hide and show')

  await resizeAppWindow(instance, 'system.chat', 600)
  await ctx.waitFor(
    'the sidebar to leave the narrow window',
    async () => !(await sidebarShown(instance)),
  )
  await instance.waitForDisplayed('chat-sidebar-toggle')
  await instance.click('chat-sidebar-toggle')
  await ctx.waitFor('the sidebar over the content', () =>
    sidebarShown(instance),
  )
  await instance.waitForDisplayed('lock-instance-sidebar')
  await instance.type('chat-sidebar-toggle', KEY.escape)
  await ctx.waitFor(
    'Escape to close the sidebar',
    async () => !(await sidebarShown(instance)),
  )
  await instance.click('chat-sidebar-toggle')
  await ctx.waitFor('the sidebar to reopen', () => sidebarShown(instance))
  await instance.click('chat-sidebar-toggle')
  await ctx.waitFor(
    'the toggle to close the sidebar',
    async () => !(await sidebarShown(instance)),
  )
  ctx.step('narrow: open over the content, lock reachable, toggle closes')
})
