// Waits for the things scenarios with several devices check, with fixed deadlines that the time scale
// of the suite does not stretch (they check promises, not generic timeouts).
import type { Device, DeviceRow, ThisDevice } from './group.ts'
import type { WaitContext } from './sync-flows.ts'

export { expectThreads } from './sync-flows.ts'

/** The most a sync-dependent wait takes, as in the existing sync helpers. */
const SYNC_WAIT_MS = 40_000

type Observer = Pick<Device, 'address' | 'deviceList'>
type Target = Pick<Device, 'address' | 'pubkey'>

async function rowOf(
  observer: Observer,
  target: Target,
): Promise<DeviceRow | undefined> {
  const pubkey = await target.pubkey()
  return (await observer.deviceList()).find(
    (row) => row.devicePubkey === pubkey,
  )
}

/** Waits until `observer`'s device list shows `target` online or not online. */
export async function expectOnline(
  ctx: WaitContext,
  observer: Observer,
  target: Target,
  online: boolean,
  timeoutMs: number = SYNC_WAIT_MS,
): Promise<DeviceRow> {
  // `waitFor` only returns what the predicate found, never the `false` it keeps polling on.
  return (await ctx.waitFor(
    `${observer.address} to list ${target.address} as ${online ? 'online' : 'not online'}`,
    async () => {
      const row = await rowOf(observer, target)
      return row?.online === online ? row : false
    },
    { timeoutMs, fixed: true },
  )) as DeviceRow
}

/**
 * Waits until `observer` lists `target` as not online with a last-seen time no older than `withinMs`
 * and not in the future (the promise of the online state, spec 024 user story 4).
 */
export async function expectLastSeen(
  ctx: WaitContext,
  observer: Observer,
  target: Target,
  options: { withinMs: number; timeoutMs?: number },
): Promise<DeviceRow> {
  return (await ctx.waitFor(
    `${observer.address} to show ${target.address} as last seen within ${options.withinMs} ms`,
    async () => {
      const row = await rowOf(observer, target)
      if (row === undefined || row.online || row.lastSeen === null) return false
      const now = Date.now()
      return row.lastSeen >= now - options.withinMs &&
        row.lastSeen <= now + 30_000
        ? row
        : false
    },
    { timeoutMs: options.timeoutMs ?? SYNC_WAIT_MS, fixed: true },
  )) as DeviceRow
}

/** Waits until the device reports this role or state of itself (`sync_status.thisDevice`). */
export async function expectRole(
  ctx: WaitContext,
  device: Pick<Device, 'address' | 'status'>,
  role: ThisDevice,
  timeoutMs: number = SYNC_WAIT_MS,
): Promise<void> {
  await ctx.waitFor(
    `${device.address} to report ${role}`,
    async () => (await device.status()).thisDevice === role,
    { timeoutMs, fixed: true },
  )
}
