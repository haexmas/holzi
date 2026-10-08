// The Linux implementation of the driver layer: one application process per device with its own
// directory tree, virtual screen, driver and ports (spec 016), wrapped without changing `instance.ts`
// or `processes.ts`.
import {
  cpSync,
  copyFileSync,
  existsSync,
  mkdirSync,
  readdirSync,
  readFileSync,
  rmSync,
  statSync,
} from 'node:fs'
import { join } from 'node:path'
import { removeRoot, startInstance } from '../instance.ts'
import { pidAlive } from '../processes.ts'
import type { E2EEnv } from '../scenario-types.ts'
import type {
  DataHandle,
  DeviceHost,
  RunningDevice,
  StartDeviceOptions,
} from './host.ts'

/** Where the application keeps its vault files below the data root of an instance. */
export const VAULT_DIR = ['data', 'com.haex.holzi', 'instances']

const sleep = (ms: number) =>
  new Promise<void>((resolve) => setTimeout(resolve, ms))

export class LinuxData implements DataHandle {
  readonly root: string
  /** True while a device runs over this data. */
  running = false

  constructor(root: string) {
    this.root = root
  }

  async copyVaultFile(vaultName: string, to: DataHandle): Promise<void> {
    if (!(to instanceof LinuxData)) {
      throw new Error('the target of a copy must come from the same host')
    }
    if (this.running) {
      throw new Error(
        'the source device is still running; stop it before copying its vault file',
      )
    }
    const from = join(this.root, ...VAULT_DIR)
    if (!existsSync(from)) {
      throw new Error(`there is no vault file of "${vaultName}" to copy`)
    }
    // The file with what the closed application still keeps beside it (a journal of the last writes);
    // never the lock file.
    const files = readdirSync(from)
      .filter(
        (file) => file.startsWith(`${vaultName}.db`) && !file.endsWith('.lock'),
      )
      .sort()
    if (!files.includes(`${vaultName}.db`)) {
      throw new Error(`there is no vault file of "${vaultName}" to copy`)
    }
    const target = join(to.root, ...VAULT_DIR)
    mkdirSync(target, { recursive: true })
    const copied: string[] = []
    try {
      for (const file of files) {
        copyFileSync(join(from, file), join(target, file))
        copied.push(file)
        if (
          statSync(join(from, file)).size !== statSync(join(target, file)).size
        ) {
          throw new Error(`${file} was copied only in part`)
        }
        if (
          !readFileSync(join(from, file)).equals(
            readFileSync(join(target, file)),
          )
        ) {
          throw new Error(`${file} differs from its copy`)
        }
      }
    } catch (error) {
      for (const file of copied) rmSync(join(target, file), { force: true })
      throw new Error(
        `copying the vault file of "${vaultName}" failed: ${(error as Error).message}`,
        { cause: error },
      )
    }
  }

  keep(folder: string): void {
    if (existsSync(this.root)) {
      cpSync(this.root, join(folder, 'data'), { recursive: true })
    }
  }

  dispose(): void {
    removeRoot(this.root)
  }
}

export interface LinuxHostOptions {
  env: E2EEnv
  scenario: string
  /** Replaced by checks that must not start the application. */
  startInstance?: typeof startInstance
}

/** The host for a scenario: data folders and logs live below the run directory. */
export function createLinuxHost(options: LinuxHostOptions): DeviceHost {
  const { env, scenario } = options
  const launch = options.startInstance ?? startInstance
  return {
    newData(folder) {
      return new LinuxData(
        join(env.runDir, 'instances', `${scenario}-${folder}`),
      )
    },
    async start(start: StartDeviceOptions): Promise<RunningDevice> {
      const data = start.data
      if (!(data instanceof LinuxData)) {
        throw new Error('the data of a device must come from the same host')
      }
      if (data.running) {
        throw new Error(`a device already runs over ${data.root}`)
      }
      const instance = await launch({
        app: env.app,
        tools: env.tools,
        root: data.root,
        marker: env.marker,
        logFile: join(env.runDir, scenario, start.folder, 'driver.log'),
        reuse: existsSync(data.root) && readdirSync(data.root).length > 0,
        step: start.step,
      })
      data.running = true
      return {
        ...instance,
        stop: async () => {
          await instance.stop()
          data.running = false
        },
        kill: async () => {
          // The whole group: screen, driver and application, without the application's own shutdown.
          for (const target of [-instance.driverPid, instance.pid]) {
            try {
              process.kill(target, 'SIGKILL')
            } catch {
              // Already gone.
            }
          }
          const end = Date.now() + 3000
          while (pidAlive(instance.pid) && Date.now() < end) await sleep(20)
          data.running = false
        },
      }
    },
  }
}
