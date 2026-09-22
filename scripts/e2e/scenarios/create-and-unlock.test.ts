import assert from 'node:assert/strict'
import { scenario } from '../lib/scenario.ts'
import { createAndUnlock } from '../lib/flows.ts'

// The model scenario for the README (SC-005): create a vault, unlock it, call one backend command and
// check its result. Uses only the helpers, no direct WebDriver calls.
scenario('create-and-unlock', {}, async (ctx) => {
  const instance = await ctx.startInstance()
  const name = 'e2e-test'

  await createAndUnlock(instance, { name })

  const result = await instance.invoke('list_instances')
  if (!('ok' in result) || !result.ok) {
    throw new Error(`list_instances failed: ${JSON.stringify(result)}`)
  }
  const names = (result.data as Array<{ name: string }>).map((i) => i.name)
  assert.ok(
    names.includes(name),
    `expected "${name}" among ${JSON.stringify(names)}`,
  )
  ctx.step('checked')
})
