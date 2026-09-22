// `pnpm test:e2e`: check the tools, remove leftovers of an earlier killed run, build the debug application
// (or take one by path), run every scenario in an isolated instance, and report (contracts/cli.md).
import { spawn } from 'node:child_process'
import {
  existsSync,
  mkdirSync,
  readFileSync,
  readdirSync,
  realpathSync,
} from 'node:fs'
import { dirname, join, resolve } from 'node:path'
import { parseArgs } from 'node:util'
import { fileURLToPath } from 'node:url'
import {
  BuildError,
  buildDebugApplication,
  resolveApplication,
  targetDirectory,
} from './lib/build.ts'
import type { ApplicationInfo } from './lib/build.ts'
import { checkPreflight } from './lib/preflight.ts'
import type { ToolFacts, ToolName } from './lib/preflight.ts'
import { newMarker, stopGroup, stopRun, sweepOrphans } from './lib/processes.ts'
import type { MarkedProcess } from './lib/processes.ts'
import {
  buildReport,
  exitCodeFor,
  formatSummary,
  readResults,
  writeReport,
} from './lib/report.ts'
import type { RunFacts } from './lib/report.ts'
import type { CloseBehavior } from './lib/scenario.ts'

const HERE = dirname(fileURLToPath(import.meta.url))
const REPO_ROOT = resolve(HERE, '..', '..')
const SCENARIOS_DIR = join(HERE, 'scenarios')

export class UsageError extends Error {
  constructor(message: string) {
    super(message)
    this.name = 'UsageError'
  }
}

export interface CliOptions {
  app?: string
  closeBehavior?: CloseBehavior
  grep?: string
  keep: boolean
  scenarioTimeoutSec: number
  runTimeoutSec: number
  timeScale: number
}

function positive(name: string, value: string): number {
  const number = Number(value)
  if (!Number.isFinite(number) || number <= 0)
    throw new UsageError(`${name} must be a positive number, got ${value}`)
  return number
}

export function parseCliOptions(
  argv: string[],
  env: NodeJS.ProcessEnv,
): CliOptions {
  let values: {
    app?: string
    'close-behavior'?: string
    grep?: string
    keep?: boolean
    'scenario-timeout'?: string
    'run-timeout'?: string
  }
  try {
    values = parseArgs({
      args: argv,
      options: {
        app: { type: 'string' },
        'close-behavior': { type: 'string' },
        grep: { type: 'string' },
        keep: { type: 'boolean' },
        'scenario-timeout': { type: 'string' },
        'run-timeout': { type: 'string' },
      },
      strict: true,
      allowPositionals: false,
    }).values
  } catch (error) {
    throw new UsageError((error as Error).message)
  }
  const closeBehavior = values['close-behavior']
  if (
    closeBehavior !== undefined &&
    closeBehavior !== 'exit' &&
    closeBehavior !== 'relaunch'
  ) {
    throw new UsageError(
      `--close-behavior must be exit or relaunch, got ${closeBehavior}`,
    )
  }
  return {
    app: values.app ?? (env.E2E_APP === '' ? undefined : env.E2E_APP),
    closeBehavior,
    grep: values.grep,
    keep: values.keep === true,
    scenarioTimeoutSec: positive(
      '--scenario-timeout',
      values['scenario-timeout'] ?? '60',
    ),
    runTimeoutSec: positive('--run-timeout', values['run-timeout'] ?? '600'),
    timeScale: positive('E2E_TIME_SCALE', env.E2E_TIME_SCALE ?? '1'),
  }
}

/** The scenarios, by the base name of their files, in the order they run. */
export function listScenarioNames(dir: string = SCENARIOS_DIR): string[] {
  return readdirSync(dir)
    .filter((file) => file.endsWith('.test.ts'))
    .map((file) => file.slice(0, -'.test.ts'.length))
    .sort()
}

/** A scenario file must declare `scenario('<its own name>'`: the report and the result files key on it. */
export function findNameProblems(dir: string = SCENARIOS_DIR): string[] {
  const problems: string[] = []
  for (const name of listScenarioNames(dir)) {
    const source = readFileSync(join(dir, `${name}.test.ts`), 'utf8')
    if (!new RegExp(`scenario\\(\\s*'${name}'`).test(source)) {
      problems.push(`${name}.test.ts does not declare scenario('${name}'`)
    }
  }
  return problems
}

export interface PreflightOutcome {
  ok: boolean
  found: Partial<Record<ToolName, string>>
  tools: ToolFacts[]
  versions: { driver?: string; webview?: string }
  messages: string[]
}

export interface SignalSource {
  on(signal: NodeJS.Signals, handler: () => void): unknown
  off(signal: NodeJS.Signals, handler: () => void): unknown
}

export interface CliDeps {
  repoRoot: string
  /** The run directory is `<artifactsRoot>/<run id>`. */
  artifactsRoot: string
  preflight(): Promise<PreflightOutcome>
  scenarioNames(): string[]
  scenarioNameProblems(): string[]
  newMarker(): string
  newRunId(): string
  sweepOrphans(): Promise<MarkedProcess[]>
  build(app: ApplicationInfo, runDir: string, marker: string): Promise<void>
  runScenarios(input: {
    names: string[]
    grep?: string
    runDir: string
    env: Record<string, string>
    signal: AbortSignal
  }): Promise<void>
  stopRun(marker: string): Promise<unknown>
  signals: SignalSource
  print(line: string): void
}

const PLACEHOLDER_TOOLS: ToolFacts[] = []

export async function runCli(
  argv: string[],
  env: NodeJS.ProcessEnv,
  deps: CliDeps,
): Promise<number> {
  let options: CliOptions
  try {
    options = parseCliOptions(argv, env)
  } catch (error) {
    deps.print(`${(error as Error).message}`)
    return 2
  }

  const marker = deps.newMarker()
  const runId = deps.newRunId()
  const runDir = join(deps.artifactsRoot, runId)
  mkdirSync(runDir, { recursive: true })
  const names = deps
    .scenarioNames()
    .filter((name) => options.grep === undefined || name.includes(options.grep))

  const base = {
    runId,
    marker,
    tools: PLACEHOLDER_TOOLS,
    versions: {},
    timeScale: options.timeScale,
  }
  const finish = (
    facts: Partial<RunFacts> &
      Pick<RunFacts, 'outcome' | 'application' | 'expected'>,
  ): number => {
    const report = buildReport({ ...base, ...facts }, readResults(runDir))
    for (const line of formatSummary(report)) deps.print(line)
    writeReport(runDir, report)
    return exitCodeFor(report)
  }

  // 1. Preflight: nothing has been started yet.
  const problems = deps.scenarioNameProblems()
  if (problems.length > 0) {
    for (const problem of problems) deps.print(problem)
    return finish({
      outcome: 'preflight-failed',
      application: null,
      expected: [],
    })
  }
  const preflight = await deps.preflight()
  base.tools = preflight.tools
  base.versions = preflight.versions
  let application: ApplicationInfo | null = null
  const messages = [...preflight.messages]
  try {
    application = resolveApplication({
      repoRoot: deps.repoRoot,
      env,
      appPath: options.app,
      closeBehaviorFlag: options.closeBehavior,
    })
  } catch (error) {
    messages.push((error as Error).message)
  }
  if (!preflight.ok || application === null || messages.length > 0) {
    for (const message of messages) deps.print(message)
    return finish({ outcome: 'preflight-failed', application, expected: [] })
  }

  // The signal handlers stay until the end, so a second Ctrl-C during cleanup cannot leave leftovers.
  const controller = new AbortController()
  let interruptedBy: 'SIGINT' | 'SIGTERM' | undefined
  let runLimitReached = false
  const onInt = () => {
    interruptedBy ??= 'SIGINT'
    controller.abort()
  }
  const onTerm = () => {
    interruptedBy ??= 'SIGTERM'
    controller.abort()
  }
  deps.signals.on('SIGINT', onInt)
  deps.signals.on('SIGTERM', onTerm)
  try {
    // 2. Leftovers of a killed run.
    const swept = await deps.sweepOrphans()
    if (swept.length > 0) {
      deps.print(
        `removed ${swept.length} leftover process(es) of an earlier killed run (pid ${swept.map((p) => p.pid).join(', ')})`,
      )
    }

    // 3. Build.
    if (application.source === 'built') {
      try {
        await deps.build(application, runDir, marker)
      } catch (error) {
        deps.print((error as Error).message)
        await deps.stopRun(marker)
        return finish({ outcome: 'build-failed', application, expected: [] })
      }
    }

    // 4. Scenarios.
    const timer = setTimeout(() => {
      runLimitReached = true
      controller.abort()
    }, options.runTimeoutSec * 1000)
    try {
      await deps.runScenarios({
        names,
        grep: options.grep,
        runDir,
        signal: controller.signal,
        env: {
          E2E_RUN_DIR: runDir,
          E2E_APP: application.path,
          E2E_CLOSE_BEHAVIOR: application.closeBehavior,
          E2E_TOOLS: JSON.stringify(preflight.found),
          E2E_SCENARIO_TIMEOUT_MS: String(options.scenarioTimeoutSec * 1000),
          E2E_TIME_SCALE: String(options.timeScale),
          E2E_KEEP: options.keep ? '1' : '0',
          HOLZI_E2E_RUN: marker,
        },
      })
    } catch (error) {
      deps.print(`the scenario runner failed: ${(error as Error).message}`)
    } finally {
      clearTimeout(timer)
    }

    // 5. Sweep by this run's marker, then report.
    await deps.stopRun(marker)
    return finish({
      outcome: interruptedBy === undefined ? 'completed' : 'interrupted',
      interruptedBy,
      runLimitReached: runLimitReached && interruptedBy === undefined,
      application,
      expected: names,
    })
  } finally {
    deps.signals.off('SIGINT', onInt)
    deps.signals.off('SIGTERM', onTerm)
  }
}

function realDeps(env: NodeJS.ProcessEnv): CliDeps {
  return {
    repoRoot: REPO_ROOT,
    artifactsRoot:
      env.E2E_ARTIFACTS_DIR ?? join(targetDirectory(REPO_ROOT, env), 'e2e'),
    async preflight() {
      const check = await checkPreflight({ pathEnv: env.PATH })
      return {
        ok: check.result === 'ok',
        found: check.found,
        tools: check.tools,
        versions: {
          driver: check.driverVersion,
          webview: check.webviewVersion,
        },
        messages: check.messages,
      }
    },
    scenarioNames: () => listScenarioNames(),
    scenarioNameProblems: () => findNameProblems(),
    newMarker: () => newMarker(),
    newRunId() {
      const stamp = new Date()
        .toISOString()
        .replace(/[-:]/g, '')
        .replace(/\..*/, '')
        .replace('T', '-')
      return `${stamp}-${Math.random().toString(16).slice(2, 8)}`
    },
    sweepOrphans: () => sweepOrphans(),
    async build(app, runDir, marker) {
      await buildDebugApplication({
        repoRoot: REPO_ROOT,
        logFile: join(runDir, 'build.log'),
        marker,
      })
      if (!existsSync(app.path))
        throw new BuildError(
          `the build finished but ${app.path} does not exist`,
        )
    },
    async runScenarios({ names, runDir: _runDir, env: scenarioEnv, signal }) {
      if (names.length === 0) return
      const files = names.map((name) => join(SCENARIOS_DIR, `${name}.test.ts`))
      // Own process group, so the terminal's Ctrl-C reaches only this command, which cleans up in order.
      const child = spawn(
        process.execPath,
        ['--test', '--test-concurrency=1', ...files],
        {
          cwd: REPO_ROOT,
          env: { ...process.env, ...scenarioEnv },
          stdio: 'inherit',
          detached: true,
        },
      )
      const exited = new Promise<void>((resolveExit) =>
        child.once('close', () => resolveExit()),
      )
      const onAbort = () => {
        if (child.pid !== undefined)
          void stopGroup(child.pid, { graceMs: 2000 })
      }
      if (signal.aborted) onAbort()
      signal.addEventListener('abort', onAbort, { once: true })
      await exited
      signal.removeEventListener('abort', onAbort)
    },
    stopRun: (marker) => stopRun(marker),
    signals: process,
    print: (line) => console.log(line),
  }
}

const invokedDirectly =
  process.argv[1] !== undefined &&
  realpathSync(process.argv[1]) === fileURLToPath(import.meta.url)
if (invokedDirectly) {
  process.exitCode = await runCli(
    process.argv.slice(2),
    process.env,
    realDeps(process.env),
  )
}
