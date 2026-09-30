import assert from 'node:assert/strict'
import { copyFileSync, mkdirSync, readdirSync } from 'node:fs'
import { join } from 'node:path'
import { scenario } from '../lib/scenario.ts'
import { openSettings, waitForLocation } from '../lib/settings.ts'

const NAME = 'e2e-copy'
const VAULT_DIR = ['data', 'com.haex.holzi', 'instances']

// Spec 024, user story 7, scenarios 1 and 2 (FR-044): the vault file of a main device, copied to another
// computer and opened there with the same passphrase, makes that computer a main device on its own, says
// so once, and lists both devices in the category "Föderation". The second installation is a second
// instance of the real application over a copy of the first one's file; no other device is online, so
// nothing syncs here (the two-device behavior is covered by `tests/sync_copy.rs`).
scenario('sync-copy-notice', {}, async (ctx) => {
  const { passphrase } = ctx.credentials()
  const source = await ctx.startInstance()
  const created = await source.invoke('create_instance', {
    args: { name: NAME, passphrase },
  })
  assert.ok('ok' in created && created.ok, JSON.stringify(created))
  await source.stop()
  ctx.step('vault created on the source and closed')

  const copy = await ctx.startInstance()
  mkdirSync(join(copy.root, ...VAULT_DIR), { recursive: true })
  // The file with what the closed application still keeps beside it (a journal of the last writes).
  const copied = readdirSync(join(source.root, ...VAULT_DIR)).filter(
    (file) => file.startsWith(`${NAME}.db`) && !file.endsWith('.lock'),
  )
  for (const file of copied) {
    copyFileSync(
      join(source.root, ...VAULT_DIR, file),
      join(copy.root, ...VAULT_DIR, file),
    )
  }
  ctx.step('vault file copied', copied.join(', '))
  const opened = await copy.invoke('open_instance', {
    args: { name: NAME, passphrase },
  })
  assert.ok('ok' in opened && opened.ok, JSON.stringify(opened))
  await copy.navigate(`tauri://localhost/workspace/${NAME}`)
  await copy.waitForDisplayed('copy-notice')
  ctx.step('the notice shows')

  await openSettings(copy)
  await copy.click('settings-category-federation')
  await waitForLocation(copy, 'federation')
  await copy.waitForDisplayed('settings-device')
  const devices = await copy.exec<
    Array<{ role: string | undefined; current: boolean }>
  >(
    `return [...document.querySelectorAll('[data-testid="settings-device"]')].map((row) => ({
       role: row.dataset.role,
       current: row.dataset.current === 'true',
     }))`,
  )
  assert.deepEqual(
    devices,
    [
      { role: 'main', current: true },
      { role: 'main', current: false },
    ],
    'the copy lists itself first and its source, both as main devices',
  )
  await copy.waitForDisplayed('settings-link-device')
  ctx.step('both devices are listed as main devices')
  // Requests from other copies cannot be made here without a second device online, so the list is shown
  // with two requests put into the page's device store (the store is what `sync-devices-changed` feeds).
  const STORE = `document.querySelector('#__nuxt').__vue_app__.config.globalProperties.$pinia._s.get('syncDevices')`
  await copy.exec(
    `const store = ${STORE}
     store.status = { ...store.status, openAdmissions: [
       { devicePubkey: 'ab'.repeat(32), name: 'Zweitrechner', requestedAt: Date.now() },
       { devicePubkey: 'cd'.repeat(32), name: 'Laptop von Anna', requestedAt: Date.now() },
     ] }
     return true`,
  )
  await copy.waitForDisplayed('admission-request')
  const requests = await copy.exec<Array<{ name: string; buttons: string[] }>>(
    `return [...document.querySelectorAll('[data-testid="admission-request"]')].map((row) => ({
       name: row.querySelector('span, label').textContent.trim(),
       buttons: [...row.querySelectorAll('button')].map((b) => b.dataset.testid),
     }))`,
  )
  assert.deepEqual(requests, [
    { name: 'Zweitrechner', buttons: ['admission-reject', 'admission-admit'] },
    {
      name: 'Laptop von Anna',
      buttons: ['admission-reject', 'admission-admit'],
    },
  ])
  ctx.step('open requests show with "Ablehnen" and "Aufnehmen"')

  // A device that waits for admission says so (FR-044) and offers no management.
  await copy.exec(
    `const store = ${STORE}
     store.status = { ...store.status, thisDevice: 'awaiting_admission', openAdmissions: [] }
     return true`,
  )
  await copy.waitForDisplayed('settings-federation-notice')
  ctx.step('a device waiting for admission says so')
  await copy.exec(`await ${STORE}.loadAsync(); return true`)

  await copy.click('copy-notice-dismiss')
  await ctx.waitFor('the notice to go', () =>
    copy.exec<boolean>(
      `return document.querySelector('[data-testid="copy-notice"]') === null`,
    ),
  )
  const status = await copy.invoke('sync_status')
  assert.ok('ok' in status && status.ok, JSON.stringify(status))
  assert.equal(
    (status.data as { copyEnrolledAsMain: boolean }).copyEnrolledAsMain,
    false,
    'the backend remembers the notice was read',
  )
  ctx.step('the notice is dismissed for good')
})
