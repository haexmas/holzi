// The interaction surface a scenario drives: reach a control by its stable hook (contracts/test-hooks.md)
// through the driver's own find/displayed/click/send-keys calls, never by displayed text and never by
// running a lookup script on the page. The "is element displayed" endpoint (used to tell apart two
// elements that share a hook when only one is on screen) was confirmed against a real WebKitWebDriver
// session on 2026-09-22; a real `click()` call was confirmed on the same date to return promptly (tens of
// milliseconds) even when the click ends the application, so it needs no special scheduling either
// (research: pressing Enter did not submit the unlock form; the page a closing invoke() replaces gets no
// reply, which is why `exec`/`invoke` still go through a script).
import type {
  InvokeOptions,
  InvokeResult,
  WebDriverClient,
} from './webdriver.ts'
import { SessionGoneError, WebDriverError } from './webdriver.ts'
import { findByMarker, pidAlive } from './processes.ts'
import type { MarkedProcess } from './processes.ts'

/**
 * Whether `error` is a legitimate way for a click past the first to land once the application is already
 * on its way out: the session itself may already be gone, or - observed for real, under load, where the
 * DOM was torn down before the session was - the element the earlier click found is now a stale
 * reference (or gone outright). Anything else is a real failure and still propagates.
 */
function isGoneMidPress(error: unknown): boolean {
  return (
    error instanceof SessionGoneError ||
    (error instanceof WebDriverError &&
      (error.code === 'stale element reference' ||
        error.code === 'no such element' ||
        error.code === 'element not interactable'))
  )
}

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

/** Every element id found for hook that is genuinely displayed right now (there may be several ids for
 * one hook - contracts/test-hooks.md says at most one is ever on screen). */
async function displayedNow(
  client: WebDriverClient,
  selector: string,
): Promise<string[]> {
  const ids = await client.findElements('css selector', selector)
  const displayed: string[] = []
  for (const id of ids) {
    try {
      if (await client.isDisplayed(id)) displayed.push(id)
    } catch {
      // A stale reference (the page moved on between finding and asking) is not displayed either.
    }
  }
  return displayed
}

/** Polls find+is-displayed until one element for hook is on screen, or fails naming the hook. */
async function findDisplayed(
  client: WebDriverClient,
  hook: string,
  deadlineMs: number,
): Promise<string> {
  const selector = toSelector(hook)
  const end = Date.now() + deadlineMs
  for (;;) {
    const [id] = await displayedNow(client, selector)
    if (id !== undefined) return id
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
  const end = Date.now() + deadlineMs
  for (;;) {
    const element = await findDisplayed(
      client,
      hook,
      Math.max(0, end - Date.now()),
    )
    try {
      await client.click(element)
      return
    } catch (error) {
      if (
        !(error instanceof WebDriverError) ||
        error.code !== 'element not interactable' ||
        Date.now() >= end
      ) {
        throw error
      }
      await sleep(50)
    }
  }
}

/** Waits until a stable hook is displayed without activating it. */
export async function waitForDisplayed(
  client: WebDriverClient,
  hook: string,
  deadlineMs = 5000,
): Promise<void> {
  await findDisplayed(client, hook, deadlineMs)
}

/** Types into the displayed control found by hook. Polls until the deadline; fails naming the hook. */
export async function type(
  client: WebDriverClient,
  hook: string,
  text: string,
  deadlineMs = 5000,
): Promise<void> {
  const element = await findDisplayed(client, hook, deadlineMs)
  await client.sendKeys(element, text)
}

export interface PressOptions {
  /** Click this many times in a row (the lock-twice check). Default 1. */
  times?: number
  step: StepRecorder
}

/**
 * Clicks the displayed control found by hook, `options.times` times in a row. The control must already
 * be displayed; unlike `click`, this does not poll for it, so a scenario relying on it being on screen
 * right now gets a clear failure instead of a silent wait.
 *
 * A click past the first can find the application already ending, because the first one already ended
 * it (the lock-twice check's very point): the session itself may be gone, or - seen for real, under
 * load - only the element it found is now stale, the DOM having been torn down first. Either quietly
 * stops the remaining clicks, rather than failing the call. A click that fails for any other reason
 * still throws.
 */
export async function press(
  client: WebDriverClient,
  hook: string,
  options: PressOptions,
): Promise<void> {
  const selector = toSelector(hook)
  const [element] = await displayedNow(client, selector)
  if (element === undefined) {
    throw new Error(`hook "${hook}" (selector ${selector}) is not displayed`)
  }
  const times = options.times ?? 1
  for (let i = 0; i < times; i++) {
    try {
      if (i === 0) await click(client, hook)
      else await client.click(element)
    } catch (error) {
      if (i > 0 && isGoneMidPress(error)) break
      throw error
    }
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
  waitForDisplayed(hook: string, deadlineMs?: number): Promise<void>
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
    waitForDisplayed: (hook, deadlineMs) =>
      waitForDisplayed(client, hook, deadlineMs),
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
