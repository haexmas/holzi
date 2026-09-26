import assert from 'node:assert/strict'
import { scenario } from '../lib/scenario.ts'
import { createAndUnlock } from '../lib/flows.ts'
import {
  KEY,
  openSettings,
  runAction,
  waitForLocation,
} from '../lib/settings.ts'

type Row = { title: string; current: boolean; mark: boolean }
const rows = (instance: { exec<T>(s: string): Promise<T> }) =>
  instance.exec<Row[]>(
    `return [...document.querySelectorAll('[data-testid="settings-device"]')].map((row) => ({
       title: row.querySelector('span, label').textContent.trim(),
       current: row.dataset.current === 'true',
       mark: row.querySelector('[data-testid="settings-device-current"]') !== null,
     }))`,
  )

// Spec 023-settings-app, quickstart S13a without the second installation (FR-022): the category
// "Föderation" lists this device once, first and marked; after a rename in "Allgemein" it shows the new
// name. The same list is readable as the action `settings.devices.list`. Ordering with other and
// unnamed devices is covered by the Rust tests (`device/commands_tests.rs`, `known_devices_tests.rs`).
scenario('settings-federation', {}, async (ctx) => {
  const instance = await ctx.startInstance()
  await createAndUnlock(instance, { name: 'e2e-settings-federation' })
  await openSettings(instance)

  const renamed = 'E2E Föderation'
  await instance.waitForDisplayed('settings-alias')
  const length = await instance.exec<number>(
    'return document.querySelector(\'[data-testid="settings-alias"]\').value.length',
  )
  await instance.type('settings-alias', KEY.backspace.repeat(length))
  await instance.type('settings-alias', renamed)
  await instance.type('settings-alias', KEY.enter)
  await ctx.waitFor('the name to be saved', () =>
    instance.exec<boolean>(
      'return document.querySelector(\'ul:has([data-testid="settings-alias"]) [role="status"]\') !== null',
    ),
  )

  await instance.click('settings-category-federation')
  await waitForLocation(instance, 'federation')
  await instance.waitForDisplayed('settings-device')
  const listed = await rows(instance)
  assert.deepEqual(listed, [{ title: renamed, current: true, mark: true }])
  ctx.step('this device, marked, with its new name')

  const outcome = await runAction(instance, 'settings.devices.list')
  assert.ok(outcome.ok, `settings.devices.list: ${JSON.stringify(outcome)}`)
  const devices = (
    outcome.result as {
      devices: Array<{ alias?: string; isCurrent: boolean }>
    }
  ).devices
  assert.equal(devices.length, 1)
  assert.equal(devices[0]?.alias, renamed)
  assert.equal(devices[0]?.isCurrent, true)
  ctx.step('settings.devices.list')
})
