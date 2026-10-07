// The Android implementation of the driver layer (spec 043, contract e2e-android.md): the app on one
// device (an emulator in CI), started and ended through adb, driven through chromedriver attached to
// its web view. One device holds one app's data, so a scenario has at most one Android device.
import { mkdirSync, writeFileSync } from 'node:fs'
import { join } from 'node:path'
import type { ColorScheme, Instance } from '../instance.ts'
import { newSessionWithRetry } from '../instance.ts'
import { createPage } from '../page.ts'
import type { StepRecorder } from '../page.ts'
import { WebDriverClient } from '../webdriver.ts'
import type { Adb } from './adb.ts'
import { androidCapabilities, startChromedriver } from './chromedriver.ts'
import type { Chromedriver } from './chromedriver.ts'
import type { DataHandle, DeviceHost, RunningDevice } from './host.ts'

export const PACKAGE = 'com.haex.holzi'
const ACTIVITY = `${PACKAGE}/.MainActivity`
/** Where the app keeps its vault files, relative to its data directory (`get_app_local_data`). */
const VAULT_DIR = 'instances'

const sleep = (ms: number) =>
  new Promise<void>((resolve) => setTimeout(resolve, ms))

/**
 * The app's pages on Android are served from `http://tauri.localhost` and extension frames from
 * `http://holzi-ext.localhost` (Tauri's custom protocols on Android); scenarios name the desktop URLs.
 */
export function mapAndroidUrl(url: string): string {
  return url
    .replace(/^tauri:\/\/localhost/, 'http://tauri.localhost')
    .replace(/^holzi-ext:\/\/localhost/, 'http://holzi-ext.localhost')
}

/**
 * Brings the device into the state a scenario expects before the app starts: its data cleared unless
 * the scenario reuses it, a desktop-sized display (about 1280×800 dp, like the Linux virtual screen)
 * and the requested colour scheme.
 */
export function prepareDevice(
  adb: Adb,
  options: { reuse?: boolean; colorScheme?: ColorScheme },
): void {
  if (options.reuse !== true) adb.shell('pm', 'clear', PACKAGE)
  adb.shell('wm', 'size', '2560x1600')
  adb.shell('wm', 'density', '320')
  adb.shell(
    'cmd',
    'uimode',
    'night',
    options.colorScheme === 'dark' ? 'yes' : 'no',
  )
}

/** Puts the device's display back the way it was. */
export function restoreDevice(adb: Adb): void {
  adb.shell('wm', 'size', 'reset')
  adb.shell('wm', 'density', 'reset')
}

/** Starts the app and waits until its web view accepts a debugger; resolves with its pid. */
export async function launchApp(adb: Adb, limitMs = 20_000): Promise<number> {
  adb.shell('am', 'start', '-W', '-n', ACTIVITY)
  const end = Date.now() + limitMs
  for (;;) {
    const pid = adb.pidof(PACKAGE)
    if (pid !== undefined && adb.devtoolsOpen(pid)) return pid
    if (Date.now() >= end) {
      throw new Error(
        `the app's web view did not open for debugging within ${limitMs} ms (is the APK a debug build?)`,
      )
    }
    await sleep(100)
  }
}

export interface AndroidStartOptions {
  adb: Adb
  /** The chromedriver for the device's web view version. */
  chromedriver: string
  marker: string
  logFile: string
  colorScheme?: ColorScheme
  /** The app's data stays from the instance before. */
  reuse?: boolean
  step?: StepRecorder
}

/** The app on the device as an `Instance` (the same surface a Linux instance offers). */
export async function startAndroidInstance(
  options: AndroidStartOptions,
): Promise<Instance> {
  const { adb } = options
  prepareDevice(adb, options)
  const pid = await launchApp(adb)
  const forceStop = () => {
    try {
      adb.shell('am', 'force-stop', PACKAGE)
    } catch {
      // The device may be gone; there is nothing left to end then.
    }
  }
  let driver: Chromedriver
  try {
    driver = await startChromedriver({
      binary: options.chromedriver,
      marker: options.marker,
      logFile: options.logFile,
    })
  } catch (error) {
    forceStop()
    throw error
  }
  const client = new WebDriverClient(`http://127.0.0.1:${driver.port}`)
  const stop = async () => {
    try {
      // Frees the device for the next session; chromedriver refuses a second one otherwise.
      await client.deleteSession()
    } catch {
      // The app may already have ended.
    }
    try {
      await driver.stop()
    } finally {
      forceStop()
      restoreDevice(adb)
    }
  }
  try {
    await newSessionWithRetry(client, androidCapabilities(PACKAGE, adb.serial))
  } catch (error) {
    await stop()
    throw new Error(
      `could not attach to the app on ${adb.serial}: ${(error as Error).message} (see ${options.logFile})`,
      { cause: error },
    )
  }
  const alive = () => adb.pidof(PACKAGE) === pid
  const step: StepRecorder = options.step ?? (() => {})
  const page = createPage({
    client,
    step,
    process: {
      alive,
      marked: () => {
        const running = adb.pidof(PACKAGE)
        return running === undefined
          ? []
          : [{ pid: running, marker: options.marker, exe: PACKAGE }]
      },
    },
    mapUrl: mapAndroidUrl,
    closeWindow: () =>
      Promise.reject(
        new Error('closing the window has no Android mapping yet (stage 1c)'),
      ),
  })
  return {
    ...page,
    root: `android:${adb.serial}`,
    marker: options.marker,
    logFile: options.logFile,
    ports: { driver: driver.port, native: 0 },
    driverPid: driver.pid,
    pid,
    screenshot: (callLimitMs) => client.screenshot(callLimitMs),
    alive,
    stop,
    step,
  }
}

/** The app's data on the device; the device holds one at a time. */
export class AndroidData implements DataHandle {
  readonly adb: Adb
  /** True while the app runs over this data. */
  running = false
  /** True once the app has run over it (the next start keeps the data). */
  used = false

  constructor(adb: Adb) {
    this.adb = adb
  }

  copyVaultFile(): Promise<void> {
    return Promise.reject(
      new Error(
        'copying a vault file between devices needs a second host (stage 2: mixed groups)',
      ),
    )
  }

  keep(folder: string): void {
    try {
      const tar = this.adb.runAs(PACKAGE, `tar c ${VAULT_DIR}`)
      mkdirSync(folder, { recursive: true })
      writeFileSync(join(folder, 'android-data.tar'), tar)
    } catch {
      // Diagnostics must never hide the failure they describe.
    }
  }

  dispose(): void {
    try {
      restoreDevice(this.adb)
    } finally {
      this.adb.shell('pm', 'clear', PACKAGE)
    }
  }
}

export interface AndroidHostOptions {
  adb: Adb
  chromedriver: string
  marker: string
  logDir: string
}

/** One Android device as a host; a second device in the same scenario is refused. */
export function createAndroidHost(options: AndroidHostOptions): DeviceHost {
  let data: AndroidData | undefined
  return {
    newData() {
      if (data !== undefined) {
        throw new Error(
          'a scenario can have one Android device; the others run on Linux (stage 2: mixed groups)',
        )
      }
      data = new AndroidData(options.adb)
      return data
    },
    async start(start): Promise<RunningDevice> {
      const own = start.data
      if (!(own instanceof AndroidData)) {
        throw new Error('the data of a device must come from the same host')
      }
      if (own.running) throw new Error('the app already runs on the device')
      const instance = await startAndroidInstance({
        adb: options.adb,
        chromedriver: options.chromedriver,
        marker: options.marker,
        logFile: join(options.logDir, start.folder, 'driver.log'),
        reuse: own.used,
        step: start.step,
      })
      own.running = true
      own.used = true
      const end = async () => {
        await instance.stop()
        own.running = false
      }
      return { ...instance, stop: end, kill: end }
    },
  }
}
