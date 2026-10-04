import assert from 'node:assert/strict'
import { scenario } from '../lib/scenario.ts'
import { createAndUnlock, unwrap, type FlowInstance } from '../lib/flows.ts'
import {
  createEntry,
  openPasswords,
  trashEntry,
  updateEntry,
} from '../lib/passwords.ts'

// Spec 036, quickstart M4 (US7): a reference inserted through the picker resolves to the source's
// value, also after the source changes; a reference that would loop is refused on save; a mark is
// removed in the editor; deleting the source for good with "in eigene Werte umwandeln" keeps the
// value in the other entry, without it the field reports "Quelle nicht verfügbar" and copying is an
// error instead of the placeholder.

const MARKER = 'SECRET-MARKER-E2E-REFERENCES'

async function reveal(instance: FlowInstance, id: string) {
  return instance.invoke('passwords_reveal', {
    args: { itemId: id, field: { kind: 'password' } },
  })
}

async function password(instance: FlowInstance, id: string): Promise<string> {
  return unwrap<{ value: string }>(
    'passwords_reveal',
    await reveal(instance, id),
  ).value
}

async function detail(instance: FlowInstance, id: string) {
  return unwrap<{
    username: string | null
    references: {
      username: Array<{ status: string }>
      password: Array<{ status: string; sourceItemId: string }>
    }
  }>(
    'passwords_get_item',
    await instance.invoke('passwords_get_item', { args: { itemId: id } }),
  )
}

async function token(
  instance: FlowInstance,
  id: string,
  kind: 'username' | 'password',
) {
  return unwrap<string>(
    'passwords_reference_token',
    await instance.invoke('passwords_reference_token', {
      args: { itemId: id, kind },
    }),
  )
}

/** Opens the trash, deletes an entry for good, with or without turning references into values. */
async function deleteForGood(
  instance: FlowInstance,
  id: string,
  inline: boolean,
) {
  await instance.click('passwords-trash')
  await instance.click(`passwords-trash-delete-${id}`)
  await instance.waitForDisplayed('passwords-reference-usage')
  if (!inline) await instance.click('passwords-reference-inline')
  await instance.click('passwords-trash-delete-confirm')
}

scenario('passwords-references', { timeoutMs: 240_000 }, async (ctx) => {
  const instance = await ctx.startInstance()
  await createAndUnlock(instance, { name: 'e2e-passwords-references' })
  await openPasswords(instance)
  const konto = await createEntry(instance, {
    title: 'Konto',
    username: 'anna',
    password: `${MARKER}-eins`,
  })
  const zweit = await createEntry(instance, { title: 'Zweit' })

  // Insert a password reference through the picker and save.
  await instance.click(`passwords-entry-${zweit}`)
  await instance.click('passwords-edit')
  await instance.waitForDisplayed('passwords-editor')
  await instance.click('passwords-reference-insert-password')
  await instance.type('passwords-reference-search', 'Konto')
  await instance.click(`passwords-reference-source-${konto}`)
  await instance.click('passwords-reference-value-password')
  await ctx.waitFor(
    'the mark under the password field',
    async () =>
      (await instance.exec<number>(
        `return document.querySelectorAll('[data-testid="passwords-reference-field-password"] [data-testid="passwords-reference-mark-ok"]').length`,
      )) === 1,
  )
  await instance.click('passwords-editor-save')
  await instance.waitForDisplayed('passwords-edit')
  assert.equal(await password(instance, zweit), `${MARKER}-eins`)
  await instance.waitForDisplayed('passwords-references-password')
  ctx.step('a reference from the picker resolves to the source')

  await updateEntry(instance, konto, { password: `${MARKER}-zwei` })
  assert.equal(await password(instance, zweit), `${MARKER}-zwei`)
  ctx.step('a change of the source shows in the reference')

  // Konto's password pointing back at Zweit's would loop: refused.
  const back = await token(instance, zweit, 'password')
  const item = unwrap<{ updatedAt: string }>(
    'passwords_get_item',
    await instance.invoke('passwords_get_item', { args: { itemId: konto } }),
  )
  const refused = await instance.invoke('passwords_update_item', {
    args: {
      itemId: konto,
      expectedUpdatedAt: item.updatedAt,
      patch: { password: back },
    },
  })
  assert.ok(
    !('ended' in refused) &&
      !refused.ok &&
      (refused.error as { kind?: string }).kind === 'PasswordsReferenceCycle',
    JSON.stringify(refused),
  )
  ctx.step('a loop is refused on save')

  // A username reference removed in the editor.
  await updateEntry(instance, zweit, {
    username: await token(instance, konto, 'username'),
  })
  await instance.click('passwords-all')
  await instance.click(`passwords-entry-${zweit}`)
  await ctx.waitFor(
    'the username mark in the view',
    async () =>
      (await instance.exec<number>(
        `return document.querySelectorAll('[data-testid="passwords-references-username"]').length`,
      )) === 1,
  )
  await instance.click('passwords-edit')
  await instance.waitForDisplayed('passwords-editor')
  // The marks come a moment after the editor (the backend reads them from the text).
  await instance.click('passwords-reference-remove-username', 10_000)
  await ctx.waitFor(
    'the username mark to go',
    async () =>
      (await instance.exec<number>(
        `return document.querySelectorAll('[data-testid="passwords-reference-remove-username"]').length`,
      )) === 0,
  )
  await instance.click('passwords-editor-save')
  await instance.waitForDisplayed('passwords-edit')
  await ctx.waitFor(
    'the username to be empty',
    async () => ((await detail(instance, zweit)).username ?? '') === '',
  )
  ctx.step('a mark is removed in the editor')

  // Deleted for good with the references turned into values: Zweit keeps the value.
  await trashEntry(instance, konto)
  await deleteForGood(instance, konto, true)
  await ctx.waitFor(
    'Zweit to hold its own value',
    async () =>
      (await detail(instance, zweit)).references.password.length === 0,
  )
  assert.equal(await password(instance, zweit), `${MARKER}-zwei`)
  ctx.step('deleting with "in eigene Werte umwandeln" keeps the value')

  // Without turning them into values the reference reports the missing source.
  const quelle = await createEntry(instance, {
    title: 'Quelle',
    password: `${MARKER}-drei`,
  })
  const ziel = await createEntry(instance, {
    title: 'Ziel',
    password: await token(instance, quelle, 'password'),
  })
  await trashEntry(instance, quelle)
  await deleteForGood(instance, quelle, false)
  await ctx.waitFor(
    'the reference to show the missing source',
    async () =>
      (await detail(instance, ziel)).references.password[0]?.status ===
      'missing',
  )
  const missing = await reveal(instance, ziel)
  assert.ok(
    !('ended' in missing) &&
      !missing.ok &&
      (missing.error as { kind?: string; reason?: string }).reason ===
        'missing',
    JSON.stringify(missing),
  )
  const copied = await instance.invoke('passwords_copy_field', {
    args: { itemId: ziel, field: { kind: 'password' } },
  })
  assert.ok(!('ended' in copied) && !copied.ok, 'copying is an error')
  await instance.click('passwords-all')
  await instance.click(`passwords-entry-${ziel}`)
  await instance.waitForDisplayed('passwords-reference-mark-missing')
  ctx.step('without it the field reports the missing source')
})
