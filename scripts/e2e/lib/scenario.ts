// The shell around a scenario: its context (an isolated instance on request, a timeline, waits), its
// deadline, its teardown and its result file. `runScenario` holds the logic and takes what it needs from
// outside; `scenario` registers it with Node's test runner.
import { test } from 'node:test'
import { randomBytes } from 'node:crypto'
import { mkdirSync, rmSync, writeFileSync } from 'node:fs'
import { join } from 'node:path'
import { removeRoot, startInstance } from './instance.ts'
import type { ColorScheme, Instance } from './instance.ts'
import { toTools } from './preflight.ts'
import type { Tools } from './preflight.ts'
import { startProvider } from './provider.ts'
import type { Behavior, Provider } from './provider.ts'
import { captureFailure } from './artifacts.ts'

export type CloseBehavior = 'exit' | 'relaunch'
export type ScenarioStatus = 'passed' | 'failed' | 'skipped'

export interface E2EEnv {
  runDir: string
  app: string
  closeBehavior: CloseBehavior
  tools: Tools
  /** Default limit of a scenario that sets none. */
  scenarioTimeoutMs: number
  /** Multiplies generic timeouts. The close promises stay fixed and a scaled run is non-conformant. */
  timeScale: number
  marker: string
  /** Keep the material of passing scenarios too. */
  keep: boolean
}

export interface Step {
  name: string
  /** Milliseconds since the scenario started, from one monotonic clock. */
  atMs: number
  at: string
  detail?: string
}

export interface ScenarioResult {
  name: string
  status: ScenarioStatus
  durationMs: number
  skipReason?: string
  error?: string
  steps: Step[]
  /** The last step reached before a failure, or the failing wait's own description. Failed status only. */
  failedStep?: string
  /** Where the kept material for a failure is. Failed status only. */
  material?: string
}

export interface ScenarioOptions {
  needs?: { closeBehavior: CloseBehavior }
  timeoutMs?: number
}

export interface StartInstanceRequest {
  scenario: string
  root: string
  logFile: string
  colorScheme?: ColorScheme
  reuse: boolean
  env: E2EEnv
  /** Records a timeline entry on the scenario that made the request. */
  step: (name: string, detail?: string) => void
}

export interface FailureInfo {
  scenario: string
  env: E2EEnv
  error: unknown
  steps: Step[]
  instances: Instance[]
  providers: Provider[]
  deadlineMs?: number
}

export interface RunDeps {
  env: E2EEnv
  startInstance: (request: StartInstanceRequest) => Promise<Instance>
  /** Called before teardown when a scenario fails or reaches its deadline. */
  onFailure?: (info: FailureInfo) => Promise<void>
}

export interface WaitOptions {
  timeoutMs?: number
  intervalMs?: number
  /** Do not multiply the limit by the time scale, for a wait that checks a fixed promise. */
  fixed?: boolean
}

export interface ScenarioContext {
  app: { path: string; closeBehavior: CloseBehavior }
  name: string
  /** Aborted when the scenario reaches its deadline. */
  signal: AbortSignal
  step(name: string, detail?: string): void
  waitFor<T>(
    description: string,
    predicate: () => T | Promise<T>,
    options?: WaitOptions,
  ): Promise<Awaited<T>>
  onTeardown(action: () => Promise<void> | void): void
  startInstance(options?: {
    colorScheme?: ColorScheme
    reusesRoot?: string
  }): Promise<Instance>
  /** Starts a stand-in model provider ([stand-in-provider.md](stand-in-provider.md)); ended with the context. */
  provider(behavior?: Behavior): Promise<Provider>
  /** A passphrase and a provider key generated for this run; no credential is ever committed. */
  credentials(): { passphrase: string; providerKey: string }
}

class WaitTimeoutError extends Error {
  description: string

  constructor(message: string, description: string) {
    super(message)
    this.description = description
  }
}
class ScenarioDeadlineError extends Error {}

const sleep = (ms: number) =>
  new Promise<void>((resolve) => setTimeout(resolve, ms))

const names = new Set<string>()

/** Scenario names are unique because the name filter, the result file and the report all key on them. */
export function registerName(name: string): void {
  if (names.has(name)) throw new Error(`duplicate scenario name: ${name}`)
  names.add(name)
}

export function effectiveTimeoutMs(
  options: ScenarioOptions,
  env: E2EEnv,
): number {
  return Math.round(
    (options.timeoutMs ?? env.scenarioTimeoutMs) * env.timeScale,
  )
}

function skipReasonFor(
  options: ScenarioOptions,
  env: E2EEnv,
): string | undefined {
  const needed = options.needs?.closeBehavior
  if (needed === undefined || needed === env.closeBehavior) return undefined
  return needed === 'relaunch'
    ? 'the build exits on close; this scenario needs one that relaunches'
    : 'the build relaunches on close; this scenario needs one that exits'
}

function describeObserved(value: unknown): string {
  try {
    return JSON.stringify(value) ?? String(value)
  } catch {
    return String(value)
  }
}

/** The last step reached before the failure, or the failing wait's own description (contracts/report.md). */
function failedStepFor(failure: unknown, steps: Step[]): string | undefined {
  if (failure instanceof WaitTimeoutError) return failure.description
  return steps.length > 0 ? steps[steps.length - 1]?.name : undefined
}

function writeResult(env: E2EEnv, result: ScenarioResult) {
  mkdirSync(join(env.runDir, 'results'), { recursive: true })
  writeFileSync(
    join(env.runDir, 'results', `${result.name}.json`),
    JSON.stringify(result, null, 2),
  )
}

export async function runScenario(
  name: string,
  options: ScenarioOptions,
  body: (ctx: ScenarioContext) => Promise<void>,
  deps: RunDeps,
): Promise<ScenarioResult> {
  const { env } = deps
  const startedAt = performance.now()
  const steps: Step[] = []
  const finish = (
    result: Omit<ScenarioResult, 'name' | 'durationMs' | 'steps'>,
  ): ScenarioResult => {
    const full: ScenarioResult = {
      name,
      durationMs: Math.round(performance.now() - startedAt),
      steps,
      ...result,
    }
    writeResult(env, full)
    return full
  }

  const skipReason = skipReasonFor(options, env)
  if (skipReason !== undefined) return finish({ status: 'skipped', skipReason })

  const controller = new AbortController()
  const teardowns: Array<() => Promise<void> | void> = []
  const instances: Instance[] = []
  const providers: Provider[] = []
  let instanceCount = 0

  const step = (stepName: string, detail?: string) => {
    const entry: Step = {
      name: stepName,
      atMs: Math.round(performance.now() - startedAt),
      at: new Date().toISOString(),
    }
    if (detail !== undefined) entry.detail = detail
    steps.push(entry)
  }
  const scaled = (ms: number) => Math.round(ms * env.timeScale)

  const ctx: ScenarioContext = {
    app: { path: env.app, closeBehavior: env.closeBehavior },
    name,
    signal: controller.signal,
    step,
    onTeardown: (action) => void teardowns.push(action),
    credentials: () => ({
      passphrase: randomBytes(18).toString('base64url'),
      providerKey: randomBytes(18).toString('base64url'),
    }),
    async waitFor(description, predicate, waitOptions = {}) {
      const limit =
        waitOptions.fixed === true
          ? (waitOptions.timeoutMs ?? 10_000)
          : scaled(waitOptions.timeoutMs ?? 10_000)
      // Never poll slower than every 50 ms: the promises under test are a second or a few.
      const interval = Math.min(waitOptions.intervalMs ?? 50, 50)
      const end = performance.now() + limit
      for (;;) {
        if (controller.signal.aborted) {
          throw new WaitTimeoutError(
            `stopped while waiting for ${description}`,
            description,
          )
        }
        const observed = await predicate()
        if (observed) return observed
        if (performance.now() >= end) {
          throw new WaitTimeoutError(
            `timed out after ${limit} ms waiting for ${description} (last observed: ${describeObserved(observed)})`,
            description,
          )
        }
        await sleep(interval)
      }
    },
    async startInstance(instanceOptions = {}) {
      instanceCount += 1
      const root =
        instanceOptions.reusesRoot ??
        join(env.runDir, 'instances', `${name}-${instanceCount}`)
      const instance = await deps.startInstance({
        scenario: name,
        root,
        logFile: join(env.runDir, name, 'driver.log'),
        colorScheme: instanceOptions.colorScheme,
        reuse: instanceOptions.reusesRoot !== undefined,
        env,
        step,
      })
      instances.push(instance)
      // The root is removed with the scenario, not when the instance stops, so a fresh instance can reuse it.
      teardowns.push(async () => {
        await instance.stop()
        removeRoot(root)
      })
      step('instance-ready')
      return instance
    },
    async provider(behavior) {
      const started = await startProvider(behavior)
      providers.push(started)
      teardowns.push(() => started.close())
      return started
    },
  }

  const limit = effectiveTimeoutMs(options, env)
  let failure: unknown
  let timer: NodeJS.Timeout | undefined
  const deadline = new Promise<never>((_, reject) => {
    timer = setTimeout(() => {
      controller.abort()
      reject(
        new ScenarioDeadlineError(
          `scenario ${name} reached its deadline of ${limit} ms`,
        ),
      )
    }, limit)
  })
  try {
    await Promise.race([body(ctx), deadline])
  } catch (error) {
    failure = error
  } finally {
    clearTimeout(timer)
  }

  if (failure !== undefined && deps.onFailure !== undefined) {
    try {
      await deps.onFailure({
        scenario: name,
        env,
        error: failure,
        steps,
        instances,
        providers,
        deadlineMs:
          failure instanceof ScenarioDeadlineError ? limit : undefined,
      })
    } catch {
      // Diagnostics must never hide the failure they describe.
    }
  }

  const teardownErrors: string[] = []
  for (const action of teardowns.reverse()) {
    try {
      await action()
    } catch (error) {
      teardownErrors.push((error as Error).message)
    }
  }

  if (failure !== undefined) {
    return finish({
      status: 'failed',
      error: failure instanceof Error ? failure.message : String(failure),
      failedStep: failedStepFor(failure, steps),
      material: join(env.runDir, name),
    })
  }
  if (teardownErrors.length > 0) {
    return finish({
      status: 'failed',
      error: `teardown failed: ${teardownErrors.join('; ')}`,
      material: join(env.runDir, name),
    })
  }
  // Nothing to show for a pass: the material (the instance's own continuous driver.log, and anything a
  // failure would have added) is removed unless the maintainer asked to keep it too (contracts/report.md).
  if (!env.keep)
    rmSync(join(env.runDir, name), { recursive: true, force: true })
  return finish({ status: 'passed' })
}

/** Read what the command passes to the scenario files. */
export function readEnv(source: NodeJS.ProcessEnv = process.env): E2EEnv {
  const need = (key: string): string => {
    const value = source[key]
    if (value === undefined || value === '') {
      throw new Error(
        `${key} is not set: scenarios are started by "pnpm test:e2e", not run on their own`,
      )
    }
    return value
  }
  const closeBehavior = need('E2E_CLOSE_BEHAVIOR')
  if (closeBehavior !== 'exit' && closeBehavior !== 'relaunch') {
    throw new Error(
      `E2E_CLOSE_BEHAVIOR must be exit or relaunch, got ${closeBehavior}`,
    )
  }
  const timeScale = Number(source.E2E_TIME_SCALE ?? '1')
  if (!Number.isFinite(timeScale) || timeScale <= 0)
    throw new Error('E2E_TIME_SCALE must be a positive number')
  return {
    runDir: need('E2E_RUN_DIR'),
    app: need('E2E_APP'),
    closeBehavior,
    tools: toTools(JSON.parse(need('E2E_TOOLS'))),
    scenarioTimeoutMs: Number(source.E2E_SCENARIO_TIMEOUT_MS ?? '60000'),
    timeScale,
    marker: need('HOLZI_E2E_RUN'),
    keep: source.E2E_KEEP === '1',
  }
}

/** Declare a scenario. The name is the file's base name; the command checks that they agree. */
export function scenario(
  name: string,
  options: ScenarioOptions,
  body: (ctx: ScenarioContext) => Promise<void>,
): void {
  registerName(name)
  test(name, async (t) => {
    const result = await runScenario(name, options, body, {
      env: readEnv(),
      startInstance: (request) =>
        startInstance({
          app: request.env.app,
          tools: request.env.tools,
          root: request.root,
          marker: request.env.marker,
          logFile: request.logFile,
          colorScheme: request.colorScheme,
          reuse: request.reuse,
          step: request.step,
        }),
      onFailure: (info) =>
        captureFailure({
          runDir: info.env.runDir,
          scenario: info.scenario,
          error: info.error,
          steps: info.steps,
          deadlineMs: info.deadlineMs,
          instances: info.instances,
          providers: info.providers,
        }),
    })
    if (result.status === 'skipped') t.skip(result.skipReason)
    if (result.status === 'failed') throw new Error(result.error)
  })
}
