// chromedriver for one Android device (spec 043, research R14): it attaches to the web view of the
// app that is already running, so it never starts, clears or ends the app itself.
import { portOpen } from '../instance.ts'
import { freePort, withPortRetry } from '../ports.ts'
import { MARKER_ENV, spawnMarked, stopGroup } from '../processes.ts'

export interface Chromedriver {
  port: number
  pid: number
  stop(): Promise<void>
}

const sleep = (ms: number) =>
  new Promise<void>((resolve) => setTimeout(resolve, ms))

/** Starts chromedriver on a free port and waits until it listens. */
export async function startChromedriver(options: {
  binary: string
  marker: string
  logFile: string
  env?: NodeJS.ProcessEnv
  limitMs?: number
}): Promise<Chromedriver> {
  return withPortRetry(async () => {
    const port = await freePort()
    const started = spawnMarked(options.binary, [`--port=${port}`], {
      env: {
        ...Object.fromEntries(
          Object.entries(options.env ?? process.env).filter(
            (entry): entry is [string, string] => entry[1] !== undefined,
          ),
        ),
        [MARKER_ENV]: options.marker,
      },
      logFile: options.logFile,
    })
    const pid = started.child.pid
    if (pid === undefined) throw new Error('chromedriver has no pid')
    const stop = async () => {
      await stopGroup(pid)
      await Promise.race([started.closed, sleep(1000)])
    }
    const end = Date.now() + (options.limitMs ?? 10_000)
    while (!(await portOpen(port))) {
      if (started.child.exitCode !== null || Date.now() >= end) {
        await stop()
        throw new Error(
          `chromedriver did not listen on port ${port} (see ${options.logFile})`,
        )
      }
      await sleep(100)
    }
    return { port, pid, stop }
  })
}

/** The capabilities that attach a session to the app's running web view without clearing its data. */
export function androidCapabilities(
  pkg: string,
  serial: string,
): Record<string, unknown> {
  return {
    'goog:chromeOptions': {
      androidPackage: pkg,
      androidDeviceSerial: serial,
      // Without it chromedriver starts the app "with a clear data directory": the vault would be gone.
      androidUseRunningApp: true,
    },
  }
}
