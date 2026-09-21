import { describe, it } from 'node:test'
import assert from 'node:assert/strict'
import { mkdtempSync, readFileSync, rmSync } from 'node:fs'
import { tmpdir } from 'node:os'
import { join } from 'node:path'
import { effectiveTimeoutMs, registerName, runScenario } from './scenario.ts'
import type { E2EEnv, RunDeps } from './scenario.ts'
import type { Instance } from './instance.ts'

function withRunDir<T>(fn: (runDir: string) => Promise<T>): Promise<T> {
  const runDir = mkdtempSync(join(tmpdir(), 'e2e-scenario-'))
  return fn(runDir).finally(() =>
    rmSync(runDir, { recursive: true, force: true }),
  )
}

function envFor(runDir: string, extra: Partial<E2EEnv> = {}): E2EEnv {
  return {
    runDir,
    app: '/app/holzi',
    closeBehavior: 'exit',
    tools: {
      tauriDriver: '/t/tauri-driver',
      webKitWebDriver: '/t/WebKitWebDriver',
      xvfbRun: '/t/xvfb-run',
    },
    scenarioTimeoutMs: 60_000,
    timeScale: 1,
    marker: 'test-marker',
    keep: false,
    ...extra,
  }
}

function depsFor(
  runDir: string,
  extra: Partial<RunDeps> = {},
  env: Partial<E2EEnv> = {},
): RunDeps {
  return {
    env: envFor(runDir, env),
    startInstance: async () =>
      ({ stop: async () => {} }) as unknown as Instance,
    ...extra,
  }
}

describe('runScenario', () => {
  it('skips a scenario that needs the other close behavior and never runs its body', () =>
    withRunDir(async (runDir) => {
      let ran = false
      const result = await runScenario(
        'needs-relaunch',
        { needs: { closeBehavior: 'relaunch' } },
        async () => {
          ran = true
        },
        depsFor(runDir),
      )
      assert.equal(ran, false)
      assert.equal(result.status, 'skipped')
      assert.equal(
        result.skipReason,
        'the build exits on close; this scenario needs one that relaunches',
      )

      const reverse = await runScenario(
        'needs-exit',
        { needs: { closeBehavior: 'exit' } },
        async () => {},
        depsFor(runDir, {}, { closeBehavior: 'relaunch' }),
      )
      assert.equal(reverse.status, 'skipped')
      assert.equal(
        reverse.skipReason,
        'the build relaunches on close; this scenario needs one that exits',
      )
    }))

  it('runs a scenario whose need matches the application', () =>
    withRunDir(async (runDir) => {
      const result = await runScenario(
        'matching-need',
        { needs: { closeBehavior: 'exit' } },
        async () => {},
        depsFor(runDir),
      )
      assert.equal(result.status, 'passed')
    }))

  it('fails a throwing body and runs every teardown once, in reverse order', () =>
    withRunDir(async (runDir) => {
      const order: string[] = []
      const result = await runScenario(
        'throws',
        {},
        async (ctx) => {
          ctx.onTeardown(async () => void order.push('a'))
          ctx.onTeardown(async () => void order.push('b'))
          throw new Error('the body broke')
        },
        depsFor(runDir),
      )
      assert.equal(result.status, 'failed')
      assert.match(result.error ?? '', /the body broke/)
      assert.deepEqual(order, ['b', 'a'])
    }))

  it('fails a body that reaches its deadline, tears down, and lets the next scenario run', () =>
    withRunDir(async (runDir) => {
      let tornDown = false
      const result = await runScenario(
        'hangs',
        { timeoutMs: 100 },
        async (ctx) => {
          ctx.onTeardown(async () => {
            tornDown = true
          })
          await new Promise(() => {})
        },
        depsFor(runDir),
      )
      assert.equal(result.status, 'failed')
      assert.match(result.error ?? '', /deadline of 100 ms/)
      assert.equal(tornDown, true)

      const next = await runScenario(
        'after-hang',
        {},
        async () => {},
        depsFor(runDir),
      )
      assert.equal(next.status, 'passed')
    }))

  it('marks a scenario failed when its teardown fails even though the body passed', () =>
    withRunDir(async (runDir) => {
      const result = await runScenario(
        'teardown-fails',
        {},
        async (ctx) => {
          ctx.onTeardown(async () => {
            throw new Error('could not stop it')
          })
        },
        depsFor(runDir),
      )
      assert.equal(result.status, 'failed')
      assert.match(result.error ?? '', /teardown failed: could not stop it/)
    }))

  it('calls the failure hook before teardown', () =>
    withRunDir(async (runDir) => {
      const order: string[] = []
      await runScenario(
        'hook-order',
        {},
        async (ctx) => {
          ctx.onTeardown(async () => void order.push('teardown'))
          throw new Error('x')
        },
        depsFor(runDir, {
          onFailure: async () => void order.push('failure-hook'),
        }),
      )
      assert.deepEqual(order, ['failure-hook', 'teardown'])
    }))

  it('starts an instance through the context, records instance-ready and stops it in teardown', () =>
    withRunDir(async (runDir) => {
      const events: string[] = []
      const result = await runScenario(
        'starts-instance',
        {},
        async (ctx) => {
          await ctx.startInstance()
        },
        depsFor(runDir, {
          startInstance: async () => {
            events.push('start')
            return {
              stop: async () => void events.push('stop'),
            } as unknown as Instance
          },
        }),
      )
      assert.equal(result.status, 'passed')
      assert.deepEqual(events, ['start', 'stop'])
      assert.ok(result.steps.some((s) => s.name === 'instance-ready'))
    }))

  it('records steps from one monotonic clock', () =>
    withRunDir(async (runDir) => {
      const result = await runScenario(
        'steps',
        {},
        async (ctx) => {
          ctx.step('first')
          ctx.step('second', 'with detail')
        },
        depsFor(runDir),
      )
      const [a, b] = result.steps
      assert.deepEqual([a.name, b.name], ['first', 'second'])
      assert.equal(b.detail, 'with detail')
      assert.ok(Number.isInteger(a.atMs) && a.atMs >= 0 && b.atMs >= a.atMs)
    }))

  it('writes the result to <run dir>/results/<name>.json', () =>
    withRunDir(async (runDir) => {
      await runScenario('written', {}, async () => {}, depsFor(runDir))
      const written = JSON.parse(
        readFileSync(join(runDir, 'results', 'written.json'), 'utf8'),
      )
      assert.equal(written.name, 'written')
      assert.equal(written.status, 'passed')
      assert.equal(typeof written.durationMs, 'number')
      assert.ok(Array.isArray(written.steps))
    }))
})

describe('waitFor', () => {
  it('polls at most every 50 ms until the value is truthy', () =>
    withRunDir(async (runDir) => {
      let calls = 0
      const result = await runScenario(
        'polls',
        {},
        async (ctx) => {
          const value = await ctx.waitFor(
            'the third call',
            () => (++calls >= 3 ? 'done' : 0),
            { timeoutMs: 2000 },
          )
          assert.equal(value, 'done')
        },
        depsFor(runDir),
      )
      assert.equal(result.status, 'passed', result.error ?? '')
      assert.equal(calls, 3)
    }))

  it('names the description and the last observed value when it times out', () =>
    withRunDir(async (runDir) => {
      let calls = 0
      const result = await runScenario(
        'times-out',
        {},
        async (ctx) => {
          await ctx.waitFor('the widget to show', () => (calls++, 0), {
            timeoutMs: 300,
          })
        },
        depsFor(runDir),
      )
      assert.equal(result.status, 'failed')
      assert.match(result.error ?? '', /the widget to show/)
      assert.match(result.error ?? '', /last observed: 0/)
      assert.ok(calls >= 3, `polled ${calls} times`)
    }))
})

describe('timeouts and names', () => {
  it('defaults to the run limit and multiplies generic timeouts by the time scale', () => {
    const env = envFor('/run')
    assert.equal(effectiveTimeoutMs({}, env), 60_000)
    assert.equal(effectiveTimeoutMs({ timeoutMs: 5000 }, env), 5000)
    const scaled = envFor('/run', { timeScale: 2 })
    assert.equal(effectiveTimeoutMs({}, scaled), 120_000)
    assert.equal(effectiveTimeoutMs({ timeoutMs: 5000 }, scaled), 10_000)
  })

  it('rejects a scenario name registered twice', () => {
    registerName('unique-name-for-the-test')
    assert.throws(
      () => registerName('unique-name-for-the-test'),
      /duplicate scenario name/,
    )
  })
})
