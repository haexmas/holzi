import { describe, it } from 'node:test'
import assert from 'node:assert/strict'
import { existsSync, mkdtempSync, readFileSync, rmSync } from 'node:fs'
import { tmpdir } from 'node:os'
import { join } from 'node:path'
import { captureFailure } from './artifacts.ts'
import type { Instance } from './instance.ts'
import type { Provider } from './provider.ts'

function withRunDir<T>(fn: (runDir: string) => Promise<T>): Promise<T> {
  const runDir = mkdtempSync(join(tmpdir(), 'e2e-artifacts-'))
  return fn(runDir).finally(() =>
    rmSync(runDir, { recursive: true, force: true }),
  )
}

function fakeInstance(overrides: Partial<Instance> = {}): Instance {
  return {
    alive: () => true,
    screenshot: async () => Buffer.from('png'),
    ...overrides,
  } as Instance
}

function fakeProvider(overrides: Partial<Provider> = {}): Provider {
  return {
    connections: () => [],
    requests: () => [],
    ...overrides,
  } as Provider
}

describe('captureFailure', () => {
  it('writes timeline.json with the steps, the failure message and the deadline reached', () =>
    withRunDir(async (runDir) => {
      await captureFailure({
        runDir,
        scenario: 'a',
        error: new Error('boom'),
        steps: [
          { name: 'instance-ready', atMs: 10, at: '2026-01-01T00:00:00.000Z' },
        ],
        deadlineMs: 4000,
        instances: [],
        providers: [],
      })
      const timeline = JSON.parse(
        readFileSync(join(runDir, 'a', 'timeline.json'), 'utf8'),
      )
      assert.deepEqual(timeline.steps, [
        { name: 'instance-ready', atMs: 10, at: '2026-01-01T00:00:00.000Z' },
      ])
      assert.equal(timeline.error, 'boom')
      assert.equal(timeline.deadlineMs, 4000)
    }))

  it('takes a screenshot of the first instance', () =>
    withRunDir(async (runDir) => {
      await captureFailure({
        runDir,
        scenario: 'a',
        error: new Error('x'),
        steps: [],
        instances: [fakeInstance()],
        providers: [],
      })
      assert.equal(
        readFileSync(join(runDir, 'a', 'screenshot.png'), 'utf8'),
        'png',
      )
    }))

  it('leaves out the screenshot and notes it in the timeline when the application had already ended', () =>
    withRunDir(async (runDir) => {
      await captureFailure({
        runDir,
        scenario: 'a',
        error: new Error('x'),
        steps: [],
        instances: [fakeInstance({ alive: () => false })],
        providers: [],
      })
      assert.equal(existsSync(join(runDir, 'a', 'screenshot.png')), false)
      const timeline = JSON.parse(
        readFileSync(join(runDir, 'a', 'timeline.json'), 'utf8'),
      )
      assert.match(timeline.screenshotNote, /already ended/)
    }))

  it('never throws when the session is gone, and still writes the other files', () =>
    withRunDir(async (runDir) => {
      await captureFailure({
        runDir,
        scenario: 'a',
        error: new Error('x'),
        steps: [],
        instances: [
          fakeInstance({
            screenshot: async () => {
              throw new Error('no such window')
            },
          }),
        ],
        providers: [],
      })
      assert.equal(existsSync(join(runDir, 'a', 'screenshot.png')), false)
      assert.ok(existsSync(join(runDir, 'a', 'timeline.json')))
      assert.ok(existsSync(join(runDir, 'a', 'provider.json')))
    }))

  it('writes provider.json with each provider’s connections and requests', () =>
    withRunDir(async (runDir) => {
      await captureFailure({
        runDir,
        scenario: 'a',
        error: new Error('x'),
        steps: [],
        instances: [],
        providers: [
          fakeProvider({
            connections: () => [
              { id: 1, openedAt: 0, method: 'POST', path: '/v1/messages' },
            ],
            requests: () => [
              { id: 1, at: 0, method: 'GET', path: '/v1/models' },
            ],
          }),
        ],
      })
      const provider = JSON.parse(
        readFileSync(join(runDir, 'a', 'provider.json'), 'utf8'),
      )
      assert.deepEqual(provider, [
        {
          connections: [
            { id: 1, openedAt: 0, method: 'POST', path: '/v1/messages' },
          ],
          requests: [{ id: 1, at: 0, method: 'GET', path: '/v1/models' }],
        },
      ])
    }))

  it('never throws even if the run directory itself cannot be created', async () => {
    await captureFailure({
      runDir: '/nonexistent/no/permission/path',
      scenario: 'a',
      error: new Error('x'),
      steps: [],
      instances: [],
      providers: [],
    })
  })
})
