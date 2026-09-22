// What a failed scenario leaves behind, so a maintainer sees why without a second run (FR-019, FR-020,
// SC-004). `driver.log` needs no writing here: it already exists, written continuously for the life of
// each instance (scripts/e2e/lib/instance.ts); this only adds the three files a failure specifically
// needs.
import { mkdirSync, writeFileSync } from 'node:fs'
import { join } from 'node:path'
import type { Instance } from './instance.ts'
import type { Provider } from './provider.ts'
import type { Step } from './scenario.ts'

export interface CaptureFailureOptions {
  runDir: string
  scenario: string
  error: unknown
  steps: Step[]
  deadlineMs?: number
  instances: Instance[]
  providers: Provider[]
}

function describeError(error: unknown): string {
  return error instanceof Error ? error.message : String(error)
}

/**
 * Takes a screenshot of the first instance the scenario started, unless the application had already
 * ended (no window left to capture) or the session is gone by the time this runs; either way this never
 * throws, since a diagnostic must never hide the failure it describes.
 */
async function screenshotOrNote(
  dir: string,
  instances: Instance[],
): Promise<string | undefined> {
  const instance = instances[0]
  if (instance === undefined) return undefined
  if (!instance.alive()) {
    return 'no screenshot: the application had already ended'
  }
  try {
    writeFileSync(join(dir, 'screenshot.png'), await instance.screenshot())
    return undefined
  } catch (error) {
    return `no screenshot: ${describeError(error)}`
  }
}

/**
 * Writes `timeline.json`, `screenshot.png` and `provider.json` into the scenario's own directory under
 * the run directory. Never throws: a diagnostic that fails must not hide the failure it was meant to
 * explain.
 */
export async function captureFailure(
  options: CaptureFailureOptions,
): Promise<void> {
  const dir = join(options.runDir, options.scenario)
  try {
    mkdirSync(dir, { recursive: true })
    const screenshotNote = await screenshotOrNote(dir, options.instances)
    writeFileSync(
      join(dir, 'timeline.json'),
      JSON.stringify(
        {
          steps: options.steps,
          error: describeError(options.error),
          ...(options.deadlineMs === undefined
            ? {}
            : { deadlineMs: options.deadlineMs }),
          ...(screenshotNote === undefined ? {} : { screenshotNote }),
        },
        null,
        2,
      ),
    )
    writeFileSync(
      join(dir, 'provider.json'),
      JSON.stringify(
        options.providers.map((provider) => ({
          connections: provider.connections(),
          requests: provider.requests(),
        })),
        null,
        2,
      ),
    )
  } catch {
    // Best effort: a scenario's real failure must still be reported even if capturing it fails too.
  }
}
