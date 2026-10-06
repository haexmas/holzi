import assert from 'node:assert/strict'
import { scenario } from '../lib/scenario.ts'
import { createAndUnlock } from '../lib/flows.ts'
import { openPasswords } from '../lib/passwords.ts'
import { borderIsPrimary, fieldError, labelFloated } from '../lib/fields.ts'

// Spec 035-appearance-and-fields, quickstart Stage 1 (US1, FR-002 to FR-004): the entry editor's fields
// are haex-ui fields. A label rests inside an empty field and floats onto the border when the field has
// focus; the focused field's border has the colour of --primary; an invalid value shows its error text
// under the field, tied to it for assistive technology.
scenario('fields-basic', {}, async (ctx) => {
  const instance = await ctx.startInstance()
  await createAndUnlock(instance, { name: 'e2e-fields' })
  await openPasswords(instance)
  await instance.click('passwords-new')
  await instance.click('passwords-new-entry')
  await instance.waitForDisplayed('passwords-editor')

  assert.equal(await labelFloated(instance, 'pw-username'), false, 'rests')
  await instance.click('passwords-field-username')
  await ctx.waitFor('label floats on focus', () =>
    labelFloated(instance, 'pw-username'),
  )
  assert.ok(await borderIsPrimary(instance, 'pw-username'), 'focus ring colour')
  ctx.step('label floats and the border is primary on focus')

  await instance.type('passwords-field-username', 'octo')
  await instance.click('passwords-field-title')
  assert.ok(
    await labelFloated(instance, 'pw-username'),
    'a filled field keeps its label on the border',
  )
  assert.equal(await borderIsPrimary(instance, 'pw-username'), false)
  ctx.step('a filled field keeps the label up')

  await instance.type('passwords-field-title', 'Fields')
  await instance.type('passwords-field-otp', 'not a secret!')
  await instance.click('passwords-editor-save')
  await ctx.waitFor('error under the secret field', async () =>
    Boolean(await fieldError(instance, 'pw-otp')),
  )
  assert.ok((await fieldError(instance, 'pw-otp'))?.length)
  ctx.step('an invalid value shows its error tied to the field')
})
