// The interaction surface a scenario drives: reach a control by its stable hook (contracts/test-hooks.md),
// never by displayed text, and press a control that may close or replace the page before a normal call
// could return (research: pressing Enter did not submit the unlock form; the page a closing invoke()
// replaces gets no reply).
import type {
  InvokeOptions,
  InvokeResult,
  WebDriverClient,
} from './webdriver.ts'
import { SessionGoneError } from './webdriver.ts'
import { findByMarker, pidAlive } from './processes.ts'
import type { MarkedProcess } from './processes.ts'

const sleep = (ms: number) =>
  new Promise<void>((resolve) => setTimeout(resolve, ms))

/** A record of something the scenario did, on the runner's one clock (contracts/helpers.md). */
export interface StepRecorder {
  (name: string, detail?: string): void
}

/** A value starting with `#`, `.` or `[` is a selector as is; anything else is a `data-testid`. */
export function toSelector(hook: string): string {
  const first = hook[0]
  return first === '#' || first === '.' || first === '['
    ? hook
    : `[data-testid="${hook}"]`
}

function displayedElementExpr(selector: string): string {
  return `(function () {
    var els = document.querySelectorAll(${JSON.stringify(selector)})
    for (var i = 0; i < els.length; i++) {
      if (els[i].offsetWidth > 0 && els[i].offsetHeight > 0) return els[i]
    }
    return null
  })()`
}

function clickScript(selector: string): string {
  return `var el = ${displayedElementExpr(selector)}
    if (!el) return false
    el.click()
    return true`
}

function typeScript(selector: string, text: string): string {
  return `var el = ${displayedElementExpr(selector)}
    if (!el) return false
    el.value = ${JSON.stringify(text)}
    el.dispatchEvent(new Event('input', { bubbles: true }))
    return true`
}

function pressScript(selector: string, times: number): string {
  return `var el = ${displayedElementExpr(selector)}
    if (!el) return false
    setTimeout(function () {
      el.click()
      ${times >= 2 ? 'el.click()' : ''}
    }, 0)
    return true`
}

async function pollAction(
  client: WebDriverClient,
  hook: string,
  selector: string,
  script: string,
  deadlineMs: number,
): Promise<void> {
  const end = Date.now() + deadlineMs
  for (;;) {
    if (await client.execute<boolean>(script)) return
    if (Date.now() >= end) {
      throw new Error(
        `hook "${hook}" (selector ${selector}) was not displayed within ${deadlineMs} ms`,
      )
    }
    await sleep(50)
  }
}

/** Clicks the displayed control found by hook. Polls until the deadline; fails naming the hook. */
export async function click(
  client: WebDriverClient,
  hook: string,
  deadlineMs = 5000,
): Promise<void> {
  const selector = toSelector(hook)
  await pollAction(client, hook, selector, clickScript(selector), deadlineMs)
}

/** Types into the displayed control found by hook. Polls until the deadline; fails naming the hook. */
export async function type(
  client: WebDriverClient,
  hook: string,
  text: string,
  deadlineMs = 5000,
): Promise<void> {
  const selector = toSelector(hook)
  await pollAction(
    client,
    hook,
    selector,
    typeScript(selector, text),
    deadlineMs,
  )
}

export interface PressOptions {
  /** Dispatch this many clicks in the same script tick (the lock-twice check). Default 1. */
  times?: number
  step: StepRecorder
}

/**
 * Clicks the displayed control found by hook, scheduled with `setTimeout(…, 0)` so the call returns
 * before the click's effect (a navigation, a close) can replace the page mid-response. The control must
 * already be displayed; unlike `click`, this does not poll for it.
 */
export async function press(
  client: WebDriverClient,
  hook: string,
  options: PressOptions,
): Promise<void> {
  const selector = toSelector(hook)
  const times = options.times ?? 1
  const found = await client.execute<boolean>(pressScript(selector, times))
  if (!found) {
    throw new Error(`hook "${hook}" (selector ${selector}) is not displayed`)
  }
  options.step('press', hook)
}

/**
 * Resolves with the time from the call to the end of `pid`, and records `process-ended`. Fails at the
 * deadline instead of waiting forever for a process that will not end.
 */
export async function waitForEnd(
  pid: number,
  deadlineMs: number,
  step: StepRecorder,
): Promise<number> {
  const startedAt = Date.now()
  const end = startedAt + deadlineMs
  while (pidAlive(pid)) {
    if (Date.now() >= end) {
      throw new Error(`process ${pid} did not end within ${deadlineMs} ms`)
    }
    await sleep(50)
  }
  const elapsedMs = Date.now() - startedAt
  step('process-ended', `${elapsedMs} ms`)
  return elapsedMs
}

/** The run's marked processes running the application, for the relaunch and lock-twice checks. */
export function markedProcesses(
  marker: string,
  executable: string,
): MarkedProcess[] {
  return findByMarker(marker, executable)
}

/**
 * Runs `script` on the page every `intervalMs` and collects the results, until the session (and so the
 * application) ends. Any other failure of the script still throws.
 */
export async function sampleUntilEnd<T = unknown>(
  client: WebDriverClient,
  script: string,
  intervalMs: number,
): Promise<T[]> {
  const samples: T[] = []
  for (;;) {
    try {
      samples.push(await client.execute<T>(script))
    } catch (error) {
      if (error instanceof SessionGoneError) return samples
      throw error
    }
    await sleep(intervalMs)
  }
}

/** The interaction surface exposed on an instance (contracts/helpers.md). */
export interface Page {
  invoke(
    command: string,
    args?: unknown,
    options?: InvokeOptions,
  ): Promise<InvokeResult>
  click(hook: string, deadlineMs?: number): Promise<void>
  type(hook: string, text: string, deadlineMs?: number): Promise<void>
  press(hook: string, options?: { times?: number }): Promise<void>
  closeWindow(): Promise<void>
  navigate(url: string): Promise<void>
  exec<T = unknown>(script: string, args?: unknown[]): Promise<T>
  waitForEnd(deadlineMs: number): Promise<number>
  markedProcesses(): MarkedProcess[]
  sampleUntilEnd<T = unknown>(script: string, intervalMs: number): Promise<T[]>
}

export interface PageOptions {
  client: WebDriverClient
  /** The application process this page's session started. */
  appPid: number
  marker: string
  /** The application's real, resolved path (what `markedProcesses` matches on). */
  executable: string
  step: StepRecorder
}

/** Binds the functions above to one instance's client, process and marker. */
export function createPage(options: PageOptions): Page {
  const { client, appPid, marker, executable, step } = options
  return {
    invoke: (command, args, invokeOptions) =>
      client.invoke(command, args, invokeOptions),
    click: (hook, deadlineMs) => click(client, hook, deadlineMs),
    type: (hook, text, deadlineMs) => type(client, hook, text, deadlineMs),
    press: (hook, pressOptions) =>
      press(client, hook, { ...pressOptions, step }),
    closeWindow: () => client.closeWindow(),
    navigate: (url) => client.navigate(url),
    exec: (script, args) => client.execute(script, args),
    waitForEnd: (deadlineMs) => waitForEnd(appPid, deadlineMs, step),
    markedProcesses: () => markedProcesses(marker, executable),
    sampleUntilEnd: (script, intervalMs) =>
      sampleUntilEnd(client, script, intervalMs),
  }
}
