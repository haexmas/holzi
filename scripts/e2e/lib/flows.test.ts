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
    driver.onFind(() => ['el-1'])
    driver.onDisplayed(() => true)
    driver.onExecute((kind, script) => {
      if (kind === 'async') {
        assert.match(script, /create_instance/)
        assert.match(script, /"name":"test"/)
        return { value: { ok: true, data: { info: { name: 'test' } } } }
      }
      return { value: '/workspace/test' }
    })
    await createAndUnlock(instance, { name: 'test' })
    assert.deepEqual(steps, [['unlocked', undefined]])
    const clicks = driver.requests.filter((r) => r.path.endsWith('/click'))
    assert.equal(clicks.length, 2) // the instance entry, then the unlock submit
    const typed = driver.requests.find((r) => r.path.endsWith('/value'))
    assert.equal(typeof (typed?.body as { text?: string })?.text, 'string')
    assert.ok((typed?.body as { text: string }).text.length > 0)
  })

  it('escapes a quote in the name for the data-instance-name selector', async () => {
    let seenValue = ''
    driver.onFind((_using, value) => {
      if (value.includes('data-instance-name')) seenValue = value
      return ['el-1']
    })
    driver.onDisplayed(() => true)
    driver.onExecute((kind) =>
      kind === 'async'
        ? { value: { ok: true, data: {} } }
        : { value: '/workspace/x' },
    )
    await createAndUnlock(instance, { name: 'a"b' })
    assert.equal(
      seenValue,
      '[data-testid="instance-entry"][data-instance-name="a\\"b"]',
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
    driver.onFind(() => ['el-1'])
    driver.onDisplayed(() => true)
    driver.onExecute(() => ({ value: '/chat/test' }))
    await openChat(instance)
    assert.ok(
      driver.requests.some(
        (r) => r.method === 'POST' && r.path.endsWith('/element/el-1/click'),
      ),
    )
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
