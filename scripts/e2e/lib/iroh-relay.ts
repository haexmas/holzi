// A local iroh relay for runs with a phone (spec 043, contract e2e-android.md): the phone sits behind
// the emulator's own network, so the devices on Linux cannot reach it directly. Every device of the
// group uses this relay instead; the phone reaches it through `adb reverse` at the same address. The
// program is the `iroh-relay` of the iroh release holzi uses, in its development mode (plain HTTP).
// The relay never outlives its scenario.
import { mkdtempSync, rmSync, writeFileSync } from 'node:fs'
import { tmpdir } from 'node:os'
import { join } from 'node:path'
import { freePort } from './ports.ts'
import { spawnMarked, stopGroup } from './processes.ts'
import { reachFromDevice } from './platform/reach.ts'

/** The environment variable that names the relay program; without it `iroh-relay` on the PATH. */
export const IROH_RELAY_VARIABLE = 'E2E_IROH_RELAY'

export interface IrohRelay {
  /** `http://127.0.0.1:<port>`, the same for every device. */
  url: string
  stop(): Promise<void>
}

/** The relay's settings: the port, and no metrics server (it would take a fixed port of its own). */
export function relayConfig(port: number): string {
  return `http_bind_addr = "127.0.0.1:${port}"\nenable_metrics = false\n`
}

/** Starts the relay and waits until it answers its health check. */
export async function startIrohRelay(options: {
  logFile: string
  env?: NodeJS.ProcessEnv
  limitMs?: number
}): Promise<IrohRelay> {
  const env = options.env ?? process.env
  const binary = env[IROH_RELAY_VARIABLE] || 'iroh-relay'
  const port = await freePort()
  const folder = mkdtempSync(join(tmpdir(), 'holzi-e2e-iroh-relay-'))
  const config = join(folder, 'relay.toml')
  writeFileSync(config, relayConfig(port))
  const { child } = spawnMarked(binary, ['--dev', '--config-path', config], {
    env,
    logFile: options.logFile,
  })
  const stop = async () => {
    if (child.pid !== undefined) await stopGroup(child.pid)
    rmSync(folder, { recursive: true, force: true })
  }
  const url = `http://127.0.0.1:${port}`
  const end = Date.now() + (options.limitMs ?? 15_000)
  for (;;) {
    if (child.exitCode !== null) {
      await stop()
      throw new Error(`the iroh relay ended; see ${options.logFile}`)
    }
    try {
      if ((await fetch(`${url}/healthz`)).ok) break
    } catch {
      // Not listening yet.
    }
    if (Date.now() >= end) {
      await stop()
      throw new Error(`the iroh relay did not come up; see ${options.logFile}`)
    }
    await new Promise((resolveWait) => setTimeout(resolveWait, 100))
  }
  reachFromDevice(port, env)
  return { url, stop }
}
