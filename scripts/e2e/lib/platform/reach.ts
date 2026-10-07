// Makes a service of the run (Nostr test relay, stand-in provider, RustFS, extension dev server)
// reachable from the device under test at the same address, `127.0.0.1:<port>` (spec 043, research
// R14): on Android through `adb reverse`, elsewhere the address is already the same machine. Every
// device then uses the same URLs, which matters for settings a vault syncs between devices.
import { createAdb, realAdbRunner } from './adb.ts'
import type { AdbRunner } from './adb.ts'

export function reachFromDevice(
  port: number,
  env: NodeJS.ProcessEnv = process.env,
  run: AdbRunner = realAdbRunner,
): void {
  const serial = env.ANDROID_SERIAL
  if (env.E2E_PLATFORM !== 'android' || serial === undefined || serial === '')
    return
  createAdb(serial, run).reverse(port)
}
