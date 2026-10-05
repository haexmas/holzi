import assert from 'node:assert/strict'
import { scenario, type ScenarioContext } from '../lib/scenario.ts'
import { createAndUnlock, type FlowInstance } from '../lib/flows.ts'
import {
  addAttachments,
  createEntry,
  horizontalOverflow,
  openPasswords,
  selectTab,
  slideSettled,
} from '../lib/passwords.ts'
import { solidPng } from '../lib/png.ts'
import { KEY, resizeAppWindow } from '../lib/settings.ts'

// Spec 036, quickstart M6 part 1 (US6, FR-037 to FR-042): the attachments of an entry are cards with
// name, size and type; images get a thumbnail, a corrupt image keeps a card with the type icon; a
// tap on an image opens the lightbox, the arrow key walks the images in card order and never
// reaches the PDF, Escape closes it and gives the focus back to the card; renaming works; at 360 px
// nothing scrolls sideways; a tap on the PDF opens no lightbox.

async function openExtra(ctx: ScenarioContext, instance: FlowInstance) {
  await selectTab(instance, 'extra')
  await ctx.waitFor('the slide Extra to rest', () =>
    slideSettled(instance, 'extra'),
  )
}

const lightboxOpen = (instance: FlowInstance) =>
  instance.exec<boolean>(
    `return Boolean(document.querySelector('.pswp.pswp--open'))`,
  )

/** The name a card shows, `null` while it is being renamed. */
const cardTitle = (instance: FlowInstance, id: string) =>
  instance.exec<string | null>(
    `const el = document.querySelector('[data-testid="passwords-attachment-title-' + arguments[0] + '"]')
     return el ? el.textContent : null`,
    [id],
  )

const lightboxName = (instance: FlowInstance) =>
  instance.exec<string>(
    `const el = document.querySelector('[data-testid="passwords-lightbox-name"]')
     return el ? el.textContent : ''`,
  )

const lightboxCounter = (instance: FlowInstance) =>
  instance.exec<string>(
    `const el = document.querySelector('.pswp__counter')
     return el ? el.textContent.trim() : ''`,
  )

scenario('passwords-attachments', { timeoutMs: 240_000 }, async (ctx) => {
  const instance = await ctx.startInstance()
  await createAndUnlock(instance, { name: 'e2e-passwords-attachments' })
  await openPasswords(instance)
  const entry = await createEntry(instance, { title: 'Ausweis' })
  const [red, green, pdf, blue, broken] = await addAttachments(
    instance,
    entry,
    [
      { name: 'rot.png', content: solidPng(64, 48, [220, 40, 40]) },
      { name: 'gruen.png', content: solidPng(48, 64, [40, 180, 60]) },
      { name: 'vertrag.pdf', content: '%PDF-1.4\n% e2e\n%%EOF\n' },
      { name: 'blau.png', content: solidPng(32, 32, [40, 60, 220]) },
      { name: 'kaputt.png', content: 'this is no image' },
    ],
  )
  assert.ok(red && green && pdf && blue && broken)

  await instance.click(`passwords-entry-${entry}`)
  await openExtra(ctx, instance)
  await instance.waitForDisplayed('passwords-attachment-grid')
  const cards = await instance.exec<string[]>(
    `return [...document.querySelectorAll('[data-testid^="passwords-attachment-card-"]')]
       .map((card) => card.getAttribute('data-kind'))`,
  )
  assert.deepEqual(cards, ['image', 'image', 'pdf', 'image', 'image'])
  const meta = await instance.exec<string>(
    `return document.querySelector('[data-testid="passwords-attachment-meta-' + arguments[0] + '"]').textContent`,
    [pdf],
  )
  assert.match(meta, /Bytes · PDF$/)
  for (const id of [red, green, blue]) {
    await instance.waitForDisplayed(`passwords-attachment-thumbnail-${id}`)
  }
  // FR-041: a card out of view has not rendered its thumbnail yet.
  assert.equal(
    await instance.exec<boolean>(
      `return Boolean(document.querySelector('[data-testid="passwords-attachment-unreadable-' + arguments[0] + '"]'))`,
      [broken],
    ),
    false,
    'the card below the fold waits until it is in view',
  )
  await instance.exec(
    `document.querySelector('[data-testid="passwords-attachment-card-' + arguments[0] + '"]').scrollIntoView({ block: 'center' })
     return true`,
    [broken],
  )
  await instance.waitForDisplayed(`passwords-attachment-unreadable-${broken}`)
  await instance.waitForDisplayed(`passwords-attachment-icon-${pdf}`)
  ctx.step('the cards show thumbnails, the PDF icon and a corrupt image')

  // The lightbox walks the images in card order and never reaches the PDF.
  await instance.click(`passwords-attachment-open-${red}`)
  await ctx.waitFor('the lightbox', () => lightboxOpen(instance))
  await ctx.waitFor(
    'the first image to show',
    async () =>
      (await instance.exec<number>(
        `return document.querySelectorAll('.pswp__item img.pswp__img').length`,
      )) > 0,
  )
  const seen: string[] = []
  for (let step = 0; step < 4; step += 1) {
    seen.push(
      `${await lightboxName(instance)} ${await lightboxCounter(instance)}`,
    )
    await instance.exec(
      `document.dispatchEvent(new KeyboardEvent('keydown', { key: 'ArrowRight', keyCode: 39, bubbles: true }))
       return true`,
    )
    await ctx.waitFor(
      'the next slide',
      async () => !seen.at(-1)?.startsWith(await lightboxName(instance)),
    )
  }
  assert.deepEqual(seen, [
    'rot.png 1 von 4',
    'gruen.png 2 von 4',
    'blau.png 3 von 4',
    'kaputt.png 4 von 4',
  ])
  assert.equal(await lightboxName(instance), 'rot.png', 'it loops back')
  ctx.step('the lightbox walks the images in card order, never the PDF')

  // A corrupt image shows its message and keeps the lightbox open.
  await instance.exec(
    `document.dispatchEvent(new KeyboardEvent('keydown', { key: 'ArrowLeft', keyCode: 37, bubbles: true }))
     return true`,
  )
  await instance.waitForDisplayed(`passwords-lightbox-error-${broken}`)
  assert.ok(await lightboxOpen(instance))

  await instance.exec(
    `document.dispatchEvent(new KeyboardEvent('keydown', { key: 'Escape', keyCode: 27, bubbles: true }))
     return true`,
  )
  await ctx.waitFor(
    'the lightbox to close',
    async () => !(await lightboxOpen(instance)),
  )
  await ctx.waitFor(
    'the focus back on the card',
    async () =>
      (await instance.exec<string | null>(
        `return document.activeElement?.getAttribute('data-testid') ?? null`,
      )) === `passwords-attachment-open-${red}`,
  )
  ctx.step('Escape closes the lightbox and the focus returns to the card')

  // Renaming: Enter confirms, Escape cancels.
  // In the middle of the frame, away from the floating buttons of the window manager.
  await instance.exec(
    `document.querySelector('[data-testid="passwords-attachment-card-' + arguments[0] + '"]').scrollIntoView({ block: 'center' })
     return true`,
    [blue],
  )
  await instance.click(`passwords-attachment-rename-${blue}`)
  await instance.type(`passwords-attachment-name-${blue}`, `-alt${KEY.escape}`)
  await ctx.waitFor(
    'Escape to keep the old name',
    async () => (await cardTitle(instance, blue)) === 'blau.png',
  )
  await instance.click(`passwords-attachment-rename-${blue}`)
  // Backspaces instead of Ctrl+A: the driver keeps Ctrl held for the keys after the chord.
  await instance.type(
    `passwords-attachment-name-${blue}`,
    `${KEY.backspace.repeat('blau.png'.length)}meer.png${KEY.enter}`,
  )
  await ctx.waitFor(
    'the new name',
    async () => (await cardTitle(instance, blue)) === 'meer.png',
  )
  ctx.step('an attachment is renamed')

  await resizeAppWindow(instance, 'system.passwords', 360)
  await ctx.waitFor(
    'nothing to scroll sideways at 360 px',
    async () => (await horizontalOverflow(instance)) <= 1,
  )
  ctx.step('at 360 px nothing scrolls sideways')

  // Last, as it opens the native save dialog, which WebDriver cannot operate (and Tauri's IPC
  // cannot be stubbed): a tap on the PDF opens no lightbox. That the dialog appears is checked by
  // hand (quickstart M6).
  await instance.click(`passwords-attachment-open-${pdf}`)
  await new Promise((resolve) => setTimeout(resolve, 500))
  assert.equal(await lightboxOpen(instance), false)
  ctx.step('a tap on the PDF opens no lightbox')
})
