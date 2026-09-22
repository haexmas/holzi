import { after, before, describe, it } from 'node:test'
import assert from 'node:assert/strict'
import { spawn } from 'node:child_process'
import type { ChildProcess } from 'node:child_process'
import { readlinkSync } from 'node:fs'
import {
  click,
  createPage,
  markedProcesses,
  press,
  sampleUntilEnd,
  toSelector,
  type,
  waitForEnd,
} from './page.ts'
import { WebDriverClient } from './webdriver.ts'
import { startFakeDriver } from './fake-driver.testlib.ts'
import type { FakeDriver } from './fake-driver.testlib.ts'
import { newMarker } from './processes.ts'

const spawned: ChildProcess[] = []

/** A long sleep that carries the given marker, or none (mirrors processes.test.ts's `sleeper`). */
function sleeper(marker?: string): ChildProcess & { pid: number } {
  const env = { ...process.env }
  delete env.HOLZI_E2E_RUN
  if (marker) env.HOLZI_E2E_RUN = marker
  const child = spawn('sleep', ['60'], { env, stdio: 'ignore' })
  spawned.push(child)
  assert.ok(child.pid !== undefined)
  return child as ChildProcess & { pid: number }
}

let driver: FakeDriver
let client: WebDriverClient

before(async () => {
  driver = await startFakeDriver()
  client = new WebDriverClient(driver.url)
  await client.newSession('/some/app')
})

after(async () => {
  await driver.close()
  for (const child of spawned) {
    try {
      child.kill('SIGKILL')
    } catch {
      // already gone
    }
  }
})

describe('toSelector', () => {
  it('uses a value starting with #, . or [ as a selector as is, and wraps anything else as a data-testid', () => {
    assert.equal(toSelector('#unlock-passphrase'), '#unlock-passphrase')
    assert.equal(toSelector('.ring'), '.ring')
    assert.equal(toSelector('[form="unlock-form"]'), '[form="unlock-form"]')
    assert.equal(toSelector('lock-instance'), '[data-testid="lock-instance"]')
  })
})

describe('click', () => {
  it('polls until the script reports a displayed element, then stops', async () => {
    let calls = 0
    driver.onExecute(() => {
      calls += 1
      return { value: calls >= 3 }
    })
    await click(client, 'open-chat', 200)
    assert.equal(calls, 3)
  })

  it('fails naming the hook and the selector when nothing is displayed by the deadline', async () => {
    driver.onExecute(() => ({ value: false }))
    await assert.rejects(
      click(client, 'open-chat', 80),
      /hook "open-chat" \(selector \[data-testid="open-chat"\]\) was not displayed within 80 ms/,
    )
  })
})

describe('type', () => {
  it('sends the selector and text embedded in the script', async () => {
    let seenScript = ''
    driver.onExecute((kind, script) => {
      seenScript = script
      return { value: true }
    })
    await type(client, '#unlock-passphrase', 'sekret')
    assert.match(seenScript, /#unlock-passphrase/)
    assert.match(seenScript, /sekret/)
  })
})

describe('press', () => {
  it('schedules the click and returns without waiting for it, and records the step', async () => {
    let seenScript = ''
    driver.onExecute((kind, script) => {
      seenScript = script
      return { value: true }
    })
    const steps: Array<[string, string | undefined]> = []
    await press(client, 'lock-instance', {
      step: (name, detail) => steps.push([name, detail]),
    })
    assert.match(seenScript, /setTimeout/)
    assert.deepEqual(steps, [['press', 'lock-instance']])
  })

  it('with times: 2 dispatches both clicks in the same script tick', async () => {
    let seenScript = ''
    driver.onExecute((kind, script) => {
      seenScript = script
      return { value: true }
    })
    await press(client, 'lock-instance', { times: 2, step: () => {} })
    const timeoutBody = seenScript.split('setTimeout(function () {')[1]
    assert.equal(timeoutBody?.match(/el\.click\(\)/g)?.length, 2)
  })

  it('fails at once, without polling, when the control is not displayed', async () => {
    let calls = 0
    driver.onExecute(() => {
      calls += 1
      return { value: false }
    })
    await assert.rejects(
      press(client, 'lock-instance', { step: () => {} }),
      /hook "lock-instance" \(selector \[data-testid="lock-instance"\]\) is not displayed/,
    )
    assert.equal(calls, 1)
  })
})

describe(
  'waitForEnd',
  { skip: process.platform === 'linux' ? false : 'needs /proc (Linux only)' },
  () => {
    it('resolves with the elapsed time and records process-ended once the process is gone', async () => {
      const child = spawn('sh', ['-c', 'sleep 0.1'])
      spawned.push(child)
      assert.ok(child.pid !== undefined)
      const steps: Array<[string, string | undefined]> = []
      const elapsed = await waitForEnd(child.pid, 2000, (n, d) =>
        steps.push([n, d]),
      )
      assert.ok(elapsed >= 0)
      assert.equal(steps[0]?.[0], 'process-ended')
    })

    it('fails at the deadline for a process that keeps running', async () => {
      const child = sleeper()
      await assert.rejects(
        waitForEnd(child.pid, 100, () => {}),
        /did not end within 100 ms/,
      )
    })
  },
)

describe(
  'markedProcesses',
  { skip: process.platform === 'linux' ? false : 'needs /proc (Linux only)' },
  () => {
    it('finds only the marked processes and only those running the given executable', () => {
      const marker = newMarker()
      const a = sleeper(marker)
      const b = sleeper(marker)
      sleeper() // unmarked, must not be found
      const exe = readlinkSync(`/proc/${a.pid}/exe`)
      const found = markedProcesses(marker, exe)
      assert.deepEqual(found.map((p) => p.pid).sort(), [a.pid, b.pid].sort())
      assert.deepEqual(markedProcesses(marker, '/nonexistent/binary'), [])
    })
  },
)

describe('sampleUntilEnd', () => {
  it('collects every sample until the session ends, and stops there', async () => {
    let n = 0
    driver.onExecute(() => {
      n += 1
      return n <= 3 ? { value: n } : 'drop'
    })
    const samples = await sampleUntilEnd<number>(client, 'return 1', 1)
    assert.deepEqual(samples, [1, 2, 3])
  })

  it('still throws a real script error instead of treating it as the end', async () => {
    driver.onExecute(() => ({
      status: 400,
      body: { value: { error: 'javascript error', message: 'boom' } },
    }))
    await assert.rejects(
      sampleUntilEnd(client, 'return 1', 1),
      /javascript error/,
    )
  })
})

describe('createPage', () => {
  it('binds every helper to the given client, pid, marker and executable', async () => {
    driver.onExecute(() => ({ value: { ok: true, data: 'hi' } }))
    const page = createPage({
      client,
      appPid: process.pid,
      marker: 'unused:0:unused',
      executable: '/bin/true',
      step: () => {},
    })
    assert.deepEqual(await page.invoke('list_instances'), {
      ok: true,
      data: 'hi',
    })
    assert.deepEqual(page.markedProcesses(), [])
    await page.navigate('tauri://localhost/closing.html')
    const nav = driver.requests.filter((r) => r.path.endsWith('/url')).pop()
    assert.deepEqual(nav?.body, { url: 'tauri://localhost/closing.html' })
  })
})
