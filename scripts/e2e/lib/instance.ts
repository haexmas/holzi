// One isolated instance of the application: its own directory tree, its own virtual screen, its own
// driver and ports, and a WebDriver session. Everything it starts carries the run's marker (FR-006,
// FR-008, FR-010).
import {
  chmodSync,
  existsSync,
  mkdirSync,
  readdirSync,
  realpathSync,
  rmSync,
  writeFileSync,
} from 'node:fs'
import net from 'node:net'
import { join } from 'node:path'
import type { ChildProcess } from 'node:child_process'
import { PortInUseError, freePort, withPortRetry } from './ports.ts'
import {
  MARKER_ENV,
  findByMarker,
  pidAlive,
  spawnMarked,
  stopGroup,
} from './processes.ts'
import type { Tools } from './preflight.ts'
import { WebDriverClient } from './webdriver.ts'

export type ColorScheme = 'light' | 'dark'

/**
 * What the application must not inherit from the maintainer's session: locations it would write to, the
 * desktop bus, and the display variables that would let it open a window on the real desktop.
 */
const NOT_INHERITED = new Set([
  'XDG_STATE_HOME',
  'XDG_SESSION_ID',
  'XDG_SESSION_TYPE',
  'XDG_SESSION_DESKTOP',
  'XDG_CURRENT_DESKTOP',
  'DISPLAY',
  'WAYLAND_DISPLAY',
  'XAUTHORITY',
  'SESSION_MANAGER',
  'GTK_THEME',
  'DBUS_SESSION_BUS_ADDRESS',
])

/**
 * The environment of the application and everything below it. Data, configuration, cache, runtime
 * directory and home are inside the instance root; the session bus is disabled (research R5: the
 * application reads the desktop's colour scheme through it, and a private bus starts host services that
 * outlive the run); GTK is pinned to X11 so a Wayland session can never receive the window.
 */
export function buildInstanceEnv(
  root: string,
  marker: string,
  parentEnv: NodeJS.ProcessEnv = process.env,
): Record<string, string> {
  const env: Record<string, string> = {}
  for (const [key, value] of Object.entries(parentEnv)) {
    if (value !== undefined && !NOT_INHERITED.has(key)) env[key] = value
  }
  return {
    ...env,
    XDG_DATA_HOME: join(root, 'data'),
    XDG_CONFIG_HOME: join(root, 'config'),
    XDG_CACHE_HOME: join(root, 'cache'),
    XDG_RUNTIME_DIR: join(root, 'run'),
    HOME: join(root, 'home'),
    DBUS_SESSION_BUS_ADDRESS: 'disabled:',
    GDK_BACKEND: 'x11',
    [MARKER_ENV]: marker,
  }
}

/**
 * Create the instance root: empty, except for the GTK settings file that fixes the colour scheme.
 * A root that already holds something is refused unless a fresh instance is deliberately reusing it.
 */
export function prepareRoot(
  root: string,
  options: { colorScheme?: ColorScheme; reuse?: boolean } = {},
): void {
  if (!options.reuse && existsSync(root) && readdirSync(root).length > 0) {
    throw new Error(`instance root ${root} is not empty`)
  }
  for (const dir of ['data', 'cache', 'home'])
    mkdirSync(join(root, dir), { recursive: true })
  mkdirSync(join(root, 'run'), { recursive: true, mode: 0o700 })
  chmodSync(join(root, 'run'), 0o700)
  mkdirSync(join(root, 'config', 'gtk-3.0'), { recursive: true })
  const dark = options.colorScheme === 'dark'
  writeFileSync(
    join(root, 'config', 'gtk-3.0', 'settings.ini'),
    `[Settings]\ngtk-application-prefer-dark-theme=${dark}\n`,
  )
}

export function removeRoot(root: string): void {
  rmSync(root, { recursive: true, force: true })
}

/**
 * The command that starts the driver on its own virtual screen. The driver, the webview driver and the
 * application inherit the screen from `xvfb-run` and it ends with them.
 * ponytail: one X server per instance costs well under a second and keeps scenarios apart; the upgrade
 * path is one shared screen if start-up time ever dominates a large suite.
 */
export function driverCommand(
  tools: Tools,
  ports: { driver: number; native: number },
) {
  return {
    command: tools.xvfbRun,
    args: [
      '-a',
      '-s',
      '-screen 0 1280x800x24',
      tools.tauriDriver,
      '--port',
      String(ports.driver),
      '--native-port',
      String(ports.native),
      '--native-driver',
      tools.webKitWebDriver,
    ],
  }
}

const sleep = (ms: number) =>
  new Promise<void>((resolve) => setTimeout(resolve, ms))

function portOpen(port: number): Promise<boolean> {
  return new Promise((resolve) => {
    const socket = net.connect(port, '127.0.0.1')
    socket.once('connect', () => {
      socket.destroy()
      resolve(true)
    })
    socket.once('error', () => resolve(false))
  })
}

function hasEnded(child: ChildProcess): boolean {
  return child.exitCode !== null || child.signalCode !== null
}

export interface StartInstanceOptions {
  /** The application under test, as given. */
  app: string
  tools: Tools
  /** The instance root. It is created here and removed by whoever owns the scenario. */
  root: string
  marker: string
  /** Driver, webview driver and application output goes here. */
  logFile: string
  colorScheme?: ColorScheme
  /** A fresh instance over the data of an earlier one. */
  reuse?: boolean
  /** How long the driver and the session may take to come up. */
  startLimitMs?: number
  parentEnv?: NodeJS.ProcessEnv
}

export interface Instance {
  root: string
  marker: string
  logFile: string
  ports: { driver: number; native: number }
  driverPid: number
  /** The pid of the application process this instance started. */
  pid: number
  client: WebDriverClient
  exec<T = unknown>(script: string, args?: unknown[]): Promise<T>
  screenshot(): Promise<Buffer>
  alive(): boolean
  /** End the session and everything the instance started. Safe to call more than once. */
  stop(): Promise<void>
}

async function launchDriver(
  options: StartInstanceOptions,
  env: Record<string, string>,
  limitMs: number,
) {
  return withPortRetry(async () => {
    const ports = { driver: await freePort(), native: await freePort() }
    const { command, args } = driverCommand(options.tools, ports)
    const started = spawnMarked(command, args, {
      env,
      logFile: options.logFile,
    })
    const end = Date.now() + limitMs
    for (;;) {
      if (hasEnded(started.child)) {
        // The driver died before it listened: most likely a port taken in the meantime.
        throw new PortInUseError(ports.driver)
      }
      if (await portOpen(ports.driver)) return { ...started, ports }
      if (Date.now() >= end) {
        await stopGroup(started.child.pid ?? 0)
        throw new Error(
          `tauri-driver did not listen on port ${ports.driver} within ${limitMs} ms (see ${options.logFile})`,
        )
      }
      await sleep(100)
    }
  })
}

export async function startInstance(
  options: StartInstanceOptions,
): Promise<Instance> {
  const limitMs = options.startLimitMs ?? 20_000
  const executable = realpathSync(options.app)
  prepareRoot(options.root, {
    colorScheme: options.colorScheme,
    reuse: options.reuse,
  })
  const env = buildInstanceEnv(options.root, options.marker, options.parentEnv)
  const { child, closed, ports } = await launchDriver(options, env, limitMs)
  const driverPid = child.pid
  if (driverPid === undefined) throw new Error('the driver process has no pid')
  const client = new WebDriverClient(`http://127.0.0.1:${ports.driver}`)

  const stop = async () => {
    try {
      await client.deleteSession()
    } catch {
      // The application may already have ended, which is what closing it does.
    }
    await stopGroup(driverPid)
    await Promise.race([closed, sleep(1000)])
  }

  let pid: number | undefined
  try {
    await client.newSession(options.app)
    // ponytail: a poll with a fixed deadline; the application appears within milliseconds of the session.
    const end = Date.now() + 5000
    while (pid === undefined && Date.now() < end) {
      pid = findByMarker(options.marker, executable).find(
        (p) => p.pid !== process.pid,
      )?.pid
      if (pid === undefined) await sleep(50)
    }
    if (pid === undefined)
      throw new Error(
        'the application process did not appear after the session started',
      )
  } catch (error) {
    await stop()
    throw new Error(
      `could not start the application: ${(error as Error).message} (see ${options.logFile})`,
      {
        cause: error,
      },
    )
  }
  const appPid = pid

  return {
    root: options.root,
    marker: options.marker,
    logFile: options.logFile,
    ports,
    driverPid,
    pid: appPid,
    client,
    exec: (script, args) => client.execute(script, args),
    screenshot: () => client.screenshot(),
    alive: () => pidAlive(appPid),
    stop,
  }
}
