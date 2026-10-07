// The calls the Android platform makes to one device through adb (spec 043, contract e2e-android.md).
// One function runs every command, so a check can replace it and see the exact argument lists.
import { execFileSync } from 'node:child_process'

/** Runs `adb <args>` and returns its standard output; throws when adb fails. */
export type AdbRunner = (args: string[], input?: Buffer) => Buffer

export const realAdbRunner: AdbRunner = (args, input) =>
  execFileSync('adb', args, {
    input,
    maxBuffer: 1024 * 1024 * 1024,
    stdio: ['pipe', 'pipe', 'pipe'],
  })

export interface Adb {
  readonly serial: string
  /** `adb shell <args>`, trimmed text. */
  shell(...args: string[]): string
  /** The app's process id, or undefined when it does not run. Synchronous: `alive()` needs it. */
  pidof(pkg: string): number | undefined
  /** Whether the app's web view accepts a debugger (its DevTools socket is open). */
  devtoolsOpen(pid: number): boolean
  /** Makes `port` on this machine reachable from the device as `127.0.0.1:port`. */
  reverse(port: number): void
  /** Runs `sh -c script` as the app (debug builds only) and returns its raw output. */
  runAs(pkg: string, script: string, input?: Buffer): Buffer
}

export function createAdb(serial: string, run: AdbRunner = realAdbRunner): Adb {
  const adb = (args: string[], input?: Buffer) =>
    run(['-s', serial, ...args], input)
  return {
    serial,
    shell: (...args) =>
      adb(['shell', ...args])
        .toString()
        .trim(),
    pidof(pkg) {
      let out: string
      try {
        out = adb(['shell', 'pidof', pkg]).toString().trim()
      } catch {
        // pidof exits with 1 when nothing matches.
        return undefined
      }
      const pid = Number(out.split(/\s+/)[0])
      return Number.isInteger(pid) && pid > 0 ? pid : undefined
    },
    devtoolsOpen(pid) {
      return adb(['shell', 'cat', '/proc/net/unix'])
        .toString()
        .includes(`webview_devtools_remote_${pid}`)
    },
    reverse(port) {
      adb(['reverse', `tcp:${port}`, `tcp:${port}`])
    },
    runAs(pkg, script, input) {
      return adb(
        input === undefined
          ? ['exec-out', 'run-as', pkg, 'sh', '-c', script]
          : ['exec-in', 'run-as', pkg, 'sh', '-c', script],
        input,
      )
    },
  }
}

/** The serial of the one device adb sees, or the one `ANDROID_SERIAL` names. */
export function deviceSerial(
  env: NodeJS.ProcessEnv,
  run: AdbRunner = realAdbRunner,
): string {
  if (env.ANDROID_SERIAL !== undefined && env.ANDROID_SERIAL !== '')
    return env.ANDROID_SERIAL
  const devices = run(['devices'])
    .toString()
    .split('\n')
    .slice(1)
    .map((line) => line.trim().split(/\s+/))
    .filter(([serial, state]) => serial !== undefined && state === 'device')
    .map(([serial]) => serial!)
  if (devices.length !== 1) {
    throw new Error(
      devices.length === 0
        ? 'adb sees no device; start an emulator or connect a phone'
        : `adb sees ${devices.length} devices; name one with ANDROID_SERIAL`,
    )
  }
  return devices[0]!
}
