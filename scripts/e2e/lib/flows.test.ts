import { after, before, beforeEach, describe, it } from 'node:test'
import assert from 'node:assert/strict'
import {
  createAndUnlock,
  connectProvider,
  openChat,
  startReply,
} from './flows.ts'
import type { FlowInstance } from './flows.ts'
import { createPage } from './page.ts'
import { WebDriverClient } from './webdriver.ts'
import { startFakeDriver } from './fake-driver.testlib.ts'
import type { FakeDriver } from './fake-driver.testlib.ts'
import { startProvider } from './provider.ts'

let driver: FakeDriver
let client: WebDriverClient
let steps: Array<[string, string | undefined]>
let instance: FlowInstance

before(async () => {
  driver = await startFakeDriver()
  client = new WebDriverClient(driver.url)
  await client.newSession('/some/app')
})

after(async () => {
  await driver.close()
})

beforeEach(() => {
  steps = []
  const step = (name: string, detail?: string) => steps.push([name, detail])
  instance = {
    ...createPage({
      client,
      appPid: process.pid,
      marker: 'unused:0:unused',
      executable: '/bin/true',
      step,
    }),
    step,
  }
})

describe('createAndUnlock', () => {
  it('creates the instance, unlocks it and waits for the workspace address, recording unlocked', async () => {
    let path = '/'
    const calls: string[] = []
    driver.onExecute((kind, script) => {
      if (kind === 'async') {
        calls.push('create_instance')
        assert.match(script, /create_instance/)
        assert.match(script, /"name":"test"/)
        return { value: { ok: true, data: { info: { name: 'test' } } } }
      }
      if (script === 'return location.pathname') return { value: path }
      calls.push('click-or-type')
      path = '/workspace/test'
      return { value: true }
    })
    await createAndUnlock(instance, { name: 'test' })
    assert.deepEqual(steps, [['unlocked', undefined]])
    assert.deepEqual(calls, [
      'create_instance',
      'click-or-type',
      'click-or-type',
      'click-or-type',
    ])
  })

  it('escapes a quote in the name for the data-instance-name selector', async () => {
    let sawEntryScript = ''
    driver.onExecute((kind, script) => {
      if (kind === 'async') return { value: { ok: true, data: {} } }
      if (script === 'return location.pathname')
        return { value: '/workspace/x' }
      if (script.includes('data-instance-name')) sawEntryScript = script
      return { value: true }
    })
    await createAndUnlock(instance, { name: 'a"b' })
    const expectedSelector =
      '[data-testid="instance-entry"][data-instance-name="a\\"b"]'
    assert.ok(
      sawEntryScript.includes(JSON.stringify(expectedSelector)),
      sawEntryScript,
    )
  })

  it('fails naming the command when create_instance is rejected', async () => {
    driver.onExecute(() => ({ value: { ok: false, error: 'name taken' } }))
    await assert.rejects(
      createAndUnlock(instance, { name: 'test' }),
      /create_instance failed.*name taken/,
    )
  })
})

describe('openChat', () => {
  it('clicks open-chat and waits for the chat address', async () => {
    let path = '/workspace/test'
    driver.onExecute((kind, script) => {
      if (script === 'return location.pathname') return { value: path }
      path = '/chat/test'
      return { value: true }
    })
    await openChat(instance)
  })
})

describe('connectProvider', () => {
  it('adds the stand-in and loads its model, returning the composite model id', async () => {
    driver.onExecute((kind, script) => {
      if (script.includes('add_provider')) {
        assert.match(script, /"kind":"api_key"/)
        assert.match(script, /"adapter":"anthropic"/)
        return { value: { ok: true, data: { provider: { id: 'prov-1' } } } }
      }
      if (script.includes('load_model')) {
        assert.match(script, /prov-1:stand-in-model/)
        return { value: { ok: true, data: null } }
      }
      return { value: true }
    })
    const provider = await startProvider()
    try {
      const modelId = await connectProvider(instance, provider)
      assert.equal(modelId, 'prov-1:stand-in-model')
    } finally {
      await provider.close()
    }
  })

  it('fails naming add_provider when it is rejected', async () => {
    driver.onExecute(() => ({ value: { ok: false, error: 'bad key' } }))
    const provider = await startProvider()
    try {
      await assert.rejects(
        connectProvider(instance, provider),
        /add_provider failed.*bad key/,
      )
    } finally {
      await provider.close()
    }
  })
})

describe('startReply', () => {
  it('sends send_message and records reply-streaming once the provider has an open connection', async () => {
    driver.onExecute(() => ({ value: { ok: true, data: {} } }))
    const provider = await startProvider({
      kind: 'stream-forever',
      intervalMs: 20,
    })
    const controller = new AbortController()
    try {
      fetch(`${provider.baseUrl}/v1/messages`, {
        method: 'POST',
        body: '{}',
        signal: controller.signal,
      }).catch(() => {})
      await startReply(instance, provider, 'hello')
      assert.deepEqual(steps, [['reply-streaming', undefined]])
    } finally {
      controller.abort()
      await provider.close()
    }
  })

  it('fails naming send_message when it is rejected', async () => {
    driver.onExecute(() => ({ value: { ok: false, error: 'no model loaded' } }))
    const provider = await startProvider()
    try {
      await assert.rejects(
        startReply(instance, provider, 'hello'),
        /send_message failed.*no model loaded/,
      )
    } finally {
      await provider.close()
    }
  })
})
