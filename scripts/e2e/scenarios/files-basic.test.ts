import assert from 'node:assert/strict'

import { deviceFiles } from '../lib/extension-files.ts'
import { createAndUnlock, type FlowInstance } from '../lib/flows.ts'
import { solidPng } from '../lib/png.ts'
import { scenario } from '../lib/scenario.ts'
import { runAction } from '../lib/settings.ts'

// Spec 044, US1, quickstart §1 (T035 part 1): the file browser lists a folder, opens a text and an
// image, switches to the grid with a thumbnail, shows a file another program creates, goes up one
// folder and back into the folder through the list.

const textOf = (instance: FlowInstance, hook: string) =>
  instance.exec<string>(
    `const el = document.querySelector('[data-testid="' + arguments[0] + '"]')
     return el ? el.textContent : ''`,
    [hook],
  )

const entryNames = (instance: FlowInstance) =>
  instance.exec<string[]>(
    `return [...document.querySelectorAll('[data-testid^="files-entry-"]')]
       .map((el) => el.getAttribute('data-testid').slice('files-entry-'.length))`,
  )

scenario('files-basic', { timeoutMs: 180_000 }, async (ctx) => {
  const files = deviceFiles('holzi-files-')
  try {
    const instance = await ctx.startInstance()
    await createAndUnlock(instance, { name: 'e2e-files-basic' })

    // After the start: on Android it clears the app's data, where the folder lives.
    files.write('notiz.txt', 'Hallo aus der Datei')
    files.write('foto.png', solidPng(64, 48, [220, 40, 40]))
    const folderName = files.folder.split('/').at(-1) ?? ''
    await runAction(instance, 'wm.app.open', {
      appId: 'system.files',
      at: `/device?p=${encodeURIComponent(files.folder)}`,
    })

    await instance.waitForDisplayed('files-entry-notiz.txt')
    assert.deepEqual(await entryNames(instance), ['foto.png', 'notiz.txt'])

    // A text opens in the viewer.
    await instance.click('files-entry-notiz.txt')
    await instance.waitForDisplayed('files-viewer-text')
    assert.match(
      await textOf(instance, 'files-viewer-text'),
      /Hallo aus der Datei/,
    )
    await instance.click('files-viewer-close')

    // An image opens too.
    await instance.click('files-entry-foto.png')
    await instance.waitForDisplayed('files-viewer-image')
    await instance.click('files-viewer-close')

    // The grid shows a thumbnail for the image.
    await instance.click('files-view-toggle')
    await ctx.waitFor('the thumbnail of foto.png', () =>
      instance.exec<boolean>(
        `const img = document.querySelector('[data-testid="files-entry-foto.png"] img')
         return Boolean(img && img.complete && img.naturalWidth > 0)`,
      ),
    )
    await instance.click('files-view-toggle')

    // A file another program creates appears without reloading.
    files.write('neu.txt', 'neu')
    await instance.waitForDisplayed('files-entry-neu.txt', 15_000)

    // Up one folder and back into the folder through the list.
    await instance.click('files-up')
    await instance.waitForDisplayed(`files-entry-${folderName}`)
    await instance.click(`files-entry-${folderName}`)
    await instance.waitForDisplayed('files-entry-notiz.txt')
  } finally {
    files.remove()
  }
})
