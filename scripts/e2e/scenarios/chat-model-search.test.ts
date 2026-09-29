import assert from 'node:assert/strict'
import { scenario } from '../lib/scenario.ts'
import { createAndUnlock, openChat, connectProvider } from '../lib/flows.ts'
import { KEY, isShown } from '../lib/settings.ts'

const MODELS = [
  { id: 'gpt-4o', displayName: 'GPT-4o' },
  { id: 'gpt-4o-mini', displayName: 'GPT-4o mini' },
  { id: 'claude', displayName: 'Claude' },
  { id: 'qwen', displayName: 'Qwen2.5' },
]

const optionShown = (
  instance: Parameters<typeof isShown>[0],
  modelId: string,
) => isShown(instance, `[role="option"][data-value$=":${modelId}"]`)

const triggerTitle = (instance: { exec<T>(script: string): Promise<T> }) =>
  instance.exec<string | null>(
    "return document.querySelector('[data-testid=\"chat-settings-trigger\"]')?.getAttribute('title') ?? null",
  )

// Spec 031-chat-model-search: a search field in the chat composer's model picker filters the list in
// real time, fuzzily, grouped by provider, with a no-results hint and full keyboard control.
scenario('chat-model-search', {}, async (ctx) => {
  const instance = await ctx.startInstance()
  const provider = await ctx.provider(undefined, { models: MODELS })

  await createAndUnlock(instance, { name: 'e2e-model-search' })
  await openChat(instance)
  await connectProvider(instance, provider)

  await instance.click('chat-settings-trigger')
  await instance.click('#chat-model-popover')
  await instance.waitForDisplayed('chat-model-search')
  // `add_provider`'s model-list refresh lands asynchronously; `connectProvider` only guarantees
  // `load_model` (the active model) resolved, not that `modelGroups` already reflects the fetch.
  for (const model of MODELS) {
    await ctx.waitFor(`${model.displayName} to be listed`, () =>
      optionShown(instance, model.id),
    )
  }
  ctx.step('all models shown before searching')

  await instance.type('chat-model-search', 'gpt4o')
  assert.ok(await optionShown(instance, 'gpt-4o'), 'GPT-4o was filtered out')
  assert.ok(
    await optionShown(instance, 'gpt-4o-mini'),
    'GPT-4o mini was filtered out',
  )
  assert.ok(
    !(await optionShown(instance, 'claude')),
    'Claude stayed visible for a non-matching query',
  )
  assert.ok(
    !(await optionShown(instance, 'qwen')),
    'Qwen2.5 stayed visible for a non-matching query',
  )
  ctx.step('non-contiguous query filters to matching models')

  for (const key of Array(5).fill(KEY.backspace))
    await instance.type('chat-model-search', key)
  for (const model of MODELS) {
    assert.ok(
      await optionShown(instance, model.id),
      `${model.displayName} did not come back after clearing the search`,
    )
  }
  ctx.step('clearing the search restores the full list')

  await instance.type('chat-model-search', 'zzzzzzzz')
  await instance.waitForDisplayed('chat-model-search-empty')
  for (const model of MODELS) {
    assert.ok(
      !(await optionShown(instance, model.id)),
      `${model.displayName} stayed visible with no matching query`,
    )
  }
  ctx.step('a query with no match shows the empty hint')

  for (const key of Array(8).fill(KEY.backspace))
    await instance.type('chat-model-search', key)
  await instance.type('chat-model-search', 'claude')
  await instance.type('chat-model-search', KEY.arrowDown)
  await instance.typeToFocused(KEY.enter)
  await ctx.waitFor(
    'Claude to become the active model',
    async () => (await triggerTitle(instance))?.includes('Claude') ?? false,
  )
  ctx.step('keyboard select-and-close picks the highlighted match')

  // Picking a model closes only the model select, not the whole settings popover — it is still
  // open, so reopening the model select alone is enough.
  await instance.click('#chat-model-popover')
  await ctx.waitFor('the search field to be empty on reopen', async () => {
    const value = await instance.exec<string>(
      'return document.querySelector(\'[data-testid="chat-model-search"]\')?.value ?? ""',
    )
    return value === ''
  })
  for (const model of MODELS) {
    assert.ok(
      await optionShown(instance, model.id),
      `${model.displayName} was not shown again after reopening`,
    )
  }
  ctx.step('reopening resets the search and shows every model again')
})
