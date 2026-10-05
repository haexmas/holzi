import assert from 'node:assert/strict'
import { scenario } from '../lib/scenario.ts'
import { createAndUnlock, unwrap } from '../lib/flows.ts'
import { exists } from '../lib/sync-ui.ts'
import {
  activeTab,
  openPasswords,
  overview,
  selectTab,
} from '../lib/passwords.ts'
import type { FlowInstance } from '../lib/flows.ts'

const PIN = 'SECRET-MARKER-E2E-FIELDS'

type KeyValueView = { id: string; key: string | null; hasValue: boolean }

async function keyValuesOf(
  instance: FlowInstance,
  id: string,
): Promise<KeyValueView[]> {
  return unwrap<{ keyValues: KeyValueView[] }>(
    'passwords_get_item',
    await instance.invoke('passwords_get_item', { args: { itemId: id } }),
  ).keyValues
}

async function revealKeyValue(
  instance: FlowInstance,
  itemId: string,
  fieldId: string,
): Promise<string> {
  return unwrap<{ value: string }>(
    'passwords_reveal',
    await instance.invoke('passwords_reveal', {
      args: { itemId, field: { kind: 'keyValue', id: fieldId } },
    }),
  ).value
}

/** The buttons of the editor's Extra tab and of the masked values sit inside the editor's form; a
 * button that submitted it would save and leave the editor instead of doing its own thing. */
async function expectStillEditing(instance: FlowInstance, what: string) {
  assert.ok(
    await exists(instance, 'passwords-editor'),
    `${what} left the editor`,
  )
}

// Spec 034 FR-002, spec 036 US1: custom fields in the editor. A new entry gets two fields on the
// Extra tab and keeps them after the save; editing adds a third, removes one and leaves the stored
// value of the untouched one as it was. Revealing the password while editing stays in the editor.
scenario('passwords-custom-fields', {}, async (ctx) => {
  const instance = await ctx.startInstance()
  await createAndUnlock(instance, { name: 'e2e-passwords-fields' })
  await openPasswords(instance)

  await instance.click('passwords-new')
  await instance.click('passwords-new-entry')
  await instance.waitForDisplayed('passwords-editor')
  await instance.type('passwords-field-title', 'Bank')
  await instance.type('passwords-field-password', 'SECRET-MARKER-E2E-PW')
  await selectTab(instance, 'extra')
  await ctx.waitFor(
    'the Extra tab',
    async () => (await activeTab(instance)) === 'extra',
  )
  await instance.click('passwords-kv-add')
  await instance.waitForDisplayed('passwords-kv-key-0')
  await expectStillEditing(instance, 'adding a field')
  await instance.type('passwords-kv-key-0', 'PIN')
  await instance.type('passwords-kv-value-0', PIN)
  await instance.click('passwords-kv-add')
  await instance.waitForDisplayed('passwords-kv-key-1')
  await instance.type('passwords-kv-key-1', 'Kundennummer')
  await instance.type('passwords-kv-value-1', '4711')
  await instance.click('passwords-editor-save')
  await instance.waitForDisplayed('passwords-edit')
  const id = (await overview(instance)).headers[0]?.id
  assert.ok(id, 'the entry exists in the overview')
  const created = await keyValuesOf(instance, id)
  assert.deepEqual(
    created.map((field) => [field.key, field.hasValue]),
    [
      ['PIN', true],
      ['Kundennummer', true],
    ],
  )
  const pinId = created[0]!.id
  assert.equal(await revealKeyValue(instance, id, pinId), PIN)
  ctx.step('fields saved with a new entry')

  // The editor holds the stored password and the stored custom values in plain fields.
  await instance.click('passwords-edit')
  await instance.waitForDisplayed('passwords-editor')
  await selectTab(instance, 'details')
  await ctx.waitFor('the stored password in its field', async () =>
    instance.exec<boolean>(
      `return document.querySelector('[data-testid="passwords-field-password"]').value.includes('SECRET-MARKER-E2E-PW')`,
    ),
  )
  ctx.step('the editor shows the stored password')

  // Editing: add a field, remove the second, leave PIN untouched.
  await selectTab(instance, 'extra')
  await ctx.waitFor(
    'the Extra tab',
    async () => (await activeTab(instance)) === 'extra',
  )
  assert.equal(
    await instance.exec<string>(
      `return document.querySelector('[data-testid="passwords-kv-value-0"]').value`,
    ),
    PIN,
    'the stored custom value is in its field',
  )
  await instance.click('passwords-kv-add')
  await instance.waitForDisplayed('passwords-kv-key-2')
  await expectStillEditing(instance, 'adding a field while editing')
  await instance.type('passwords-kv-key-2', 'Online-ID')
  await instance.type('passwords-kv-value-2', 'anna-42')
  await instance.click('passwords-kv-remove-1')
  await ctx.waitFor(
    'the removed row to go',
    async () => !(await exists(instance, 'passwords-kv-key-2')),
  )
  await expectStillEditing(instance, 'removing a field')
  await instance.click('passwords-editor-save')
  await instance.waitForDisplayed('passwords-edit')
  const edited = await keyValuesOf(instance, id)
  assert.deepEqual(edited.map((field) => field.key).sort(), [
    'Online-ID',
    'PIN',
  ])
  assert.equal(
    await revealKeyValue(instance, id, pinId),
    PIN,
    'the untouched field keeps its stored value',
  )
  const added = edited.find((field) => field.key === 'Online-ID')!
  assert.equal(await revealKeyValue(instance, id, added.id), 'anna-42')
  ctx.step('fields added and removed while editing')
})
