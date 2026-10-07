// The checks before an Android run (spec 043, contract e2e-android.md): one device, the debug app on
// it (installed from `--apk` when given), and a chromedriver whose major version is the device's web
// view's. A mismatched chromedriver fails in ways that look like application bugs, so it stops the run.
import { execFileSync } from 'node:child_process'
import { delimiter, join } from 'node:path'
import { existsSync } from 'node:fs'
import { createAdb, deviceSerial, realAdbRunner } from './adb.ts'
import type { AdbRunner } from './adb.ts'
import { PACKAGE } from './android.ts'

export interface AndroidPreflight {
  ok: boolean
  messages: string[]
  versions: { driver?: string; webview?: string }
  /** What the scenarios need to reach the device: ANDROID_SERIAL, E2E_CHROMEDRIVER. */
  env: Record<string, string>
}

export interface AndroidPreflightDeps {
  run: AdbRunner
  /** `<binary> --version`, or undefined when it cannot run. */
  version(binary: string): string | undefined
  exists(path: string): boolean
}

export const realAndroidPreflightDeps: AndroidPreflightDeps = {
  run: realAdbRunner,
  version(binary) {
    try {
      return execFileSync(binary, ['--version']).toString().trim()
    } catch {
      return undefined
    }
  },
  exists: existsSync,
}

/** The first number of a dotted version anywhere in `text` (`ChromeDriver 124.0.6367.207 (…)` → 124). */
export function majorOf(text: string | undefined): number | undefined {
  const match = text === undefined ? null : /(\d+)\.\d+\.\d+/.exec(text)
  return match === null ? undefined : Number(match[1])
}

function findOnPath(
  name: string,
  pathEnv: string | undefined,
): string | undefined {
  for (const dir of (pathEnv ?? '').split(delimiter)) {
    if (dir !== '' && existsSync(join(dir, name))) return join(dir, name)
  }
  return undefined
}

export function checkAndroidPreflight(
  options: { env: NodeJS.ProcessEnv; apk?: string },
  deps: AndroidPreflightDeps = realAndroidPreflightDeps,
): AndroidPreflight {
  const messages: string[] = []
  const versions: AndroidPreflight['versions'] = {}
  let serial: string
  try {
    serial = deviceSerial(options.env, deps.run)
  } catch (error) {
    return {
      ok: false,
      messages: [(error as Error).message],
      versions,
      env: {},
    }
  }
  const adb = createAdb(serial, deps.run)

  if (options.apk !== undefined) {
    if (!deps.exists(options.apk)) {
      messages.push(`--apk ${options.apk} does not exist`)
    } else {
      try {
        try {
          deps.run(['-s', serial, 'install', '-r', options.apk])
        } catch (error) {
          // An app signed with another debug key (a CI build over a local one) cannot be updated;
          // the device only ever holds test data, so it is replaced.
          if (
            !String((error as Error).message).includes(
              'INSTALL_FAILED_UPDATE_INCOMPATIBLE',
            )
          )
            throw error
          deps.run(['-s', serial, 'uninstall', PACKAGE])
          deps.run(['-s', serial, 'install', options.apk])
        }
      } catch (error) {
        messages.push(
          `installing ${options.apk} failed: ${(error as Error).message}`,
        )
      }
    }
  }
  let installed: string
  try {
    installed = adb.shell('pm', 'path', PACKAGE)
  } catch {
    installed = ''
  }
  if (!installed.includes('package:')) {
    messages.push(
      `${PACKAGE} is not installed on ${serial}; pass --apk with a debug APK (pnpm tauri android build --debug --apk --target x86_64)`,
    )
  } else if (!adb.shell('dumpsys', 'package', PACKAGE).includes('DEBUGGABLE')) {
    messages.push(
      `${PACKAGE} on ${serial} is not a debug build; its web view cannot be driven`,
    )
  }

  const chromedriver =
    options.env.E2E_CHROMEDRIVER !== undefined &&
    options.env.E2E_CHROMEDRIVER !== ''
      ? options.env.E2E_CHROMEDRIVER
      : findOnPath('chromedriver', options.env.PATH)
  if (chromedriver === undefined) {
    messages.push(
      'no chromedriver: set E2E_CHROMEDRIVER or put chromedriver on PATH (Chrome for Testing, same major version as the device web view)',
    )
  } else {
    versions.driver = deps.version(chromedriver)
    const webview = /versionName=(\S+)/.exec(
      adb.shell('dumpsys', 'package', 'com.google.android.webview'),
    )?.[1]
    versions.webview = webview
    const driverMajor = majorOf(versions.driver)
    const webviewMajor = majorOf(webview)
    if (driverMajor === undefined) {
      messages.push(`${chromedriver} --version did not answer`)
    } else if (webviewMajor !== undefined && driverMajor !== webviewMajor) {
      messages.push(
        `chromedriver ${driverMajor} does not match the device web view ${webviewMajor}; get chromedriver ${webviewMajor} from Chrome for Testing`,
      )
    }
  }

  return {
    ok: messages.length === 0,
    messages,
    versions,
    env:
      chromedriver === undefined
        ? { ANDROID_SERIAL: serial }
        : { ANDROID_SERIAL: serial, E2E_CHROMEDRIVER: chromedriver },
  }
}
