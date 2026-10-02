// A fake driver layer for the checks of the group helpers: no application, no process, no display. It
// records what the group asks of it and answers the backend commands the flows use.
import type { NostrRelay } from './nostr-relay.ts'
import type {
  DataHandle,
  DeviceHost,
  RunningDevice,
  StartDeviceOptions,
} from './platform/host.ts'

export class FakeData implements DataHandle {
  readonly folder: string
  running = false
  disposed = false
  kept: string[] = []
  copiedTo: string[] = []
  private readonly calls: string[]

  constructor(folder: string, calls: string[]) {
    this.folder = folder
    this.calls = calls
  }

  async copyVaultFile(vaultName: string, to: DataHandle): Promise<void> {
    if (this.running) throw new Error('the source device is still running')
    this.copiedTo.push((to as FakeData).folder)
    this.calls.push(
      `copy ${this.folder} -> ${(to as FakeData).folder} (${vaultName})`,
    )
  }

  keep(folder: string): void {
    this.kept.push(folder)
  }

  dispose(): void {
    this.disposed = true
    this.calls.push(`dispose ${this.folder}`)
  }
}

export class FakeHost implements DeviceHost {
  /** Every start, stop, kill, dispose and command, in order. */
  readonly calls: string[] = []
  readonly data = new Map<string, FakeData>()
  /** The arguments of every `sync_servers_set`, by device folder. */
  readonly servers: Array<{ folder: string; args: unknown }> = []
  /** The arguments of every `link_join_start`. */
  readonly joins: unknown[] = []
  private joining = ''

  newData(folder: string): DataHandle {
    const data = new FakeData(folder, this.calls)
    this.data.set(folder, data)
    return data
  }

  async start(options: StartDeviceOptions): Promise<RunningDevice> {
    const data = options.data as FakeData
    const { folder } = options
    data.running = true
    this.calls.push(`start ${folder}`)
    const invoke = async (command: string, args?: unknown) => {
      this.calls.push(`${folder}: ${command}`)
      const reply = (value: unknown) => ({ ok: true as const, data: value })
      switch (command) {
        case 'sync_servers_defaults':
          return reply({
            nostrRelays: ['wss://default-nostr'],
            irohRelays: ['https://default-iroh'],
            disabled: [],
          })
        case 'sync_servers_set':
          this.servers.push({ folder, args: (args as { args: unknown }).args })
          return reply(null)
        case 'link_code_create':
          return reply({ code: 'CODE' })
        case 'link_join_start': {
          const joined = (args as { args: { deviceName: string } }).args
          this.joining = joined.deviceName
          this.joins.push(joined)
          return reply(null)
        }
        case 'sync_status':
          return reply({
            thisDevice: 'main',
            linking: {
              stage: 'awaiting_confirmation',
              newDeviceName: this.joining,
            },
          })
        case 'link_join_status':
          return reply({ state: 'done' })
        case 'list_vault_devices':
          return reply([
            {
              vaultDeviceUuid: `uuid-${folder}`,
              devicePubkey: `pk-${folder}`,
              alias: null,
              role: 'main',
              isCurrent: true,
              online: true,
              lastSeen: null,
              problem: null,
            },
          ])
        case 'vault_public_identity':
          return reply({ npub: 'npub1fake', hex: 'ab' })
        default:
          return reply(null)
      }
    }
    const stopped = { value: false }
    const device = {
      invoke,
      exec: async () => '/workspace/fake',
      navigate: async () => {},
      step: () => {},
      alive: () => !stopped.value,
      screenshot: async () => new Uint8Array([1]),
      stop: async () => {
        if (stopped.value) return
        stopped.value = true
        data.running = false
        this.calls.push(`stop ${folder}`)
      },
      kill: async () => {
        stopped.value = true
        data.running = false
        this.calls.push(`kill ${folder}`)
      },
    }
    return device as unknown as RunningDevice
  }
}

/** Returns a relay stub with a fixed local URL and no-op lifecycle methods. */
export function fakeRelay(): NostrRelay {
  return {
    url: 'ws://127.0.0.1:1',
    state: 'up',
    stop: async () => {},
    start: async () => {},
  }
}

/** A `waitFor` that polls a few times and gives up. */
export async function pollingWaitFor<T>(
  description: string,
  predicate: () => T | Promise<T>,
): Promise<Awaited<T>> {
  for (let attempt = 0; attempt < 20; attempt++) {
    const value = await predicate()
    if (value) return value
  }
  throw new Error(`timed out waiting for ${description}`)
}
