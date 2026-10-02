// Groups of vaults and devices for scenarios (contracts/group.md): users with their own vaults, each
// device an application over its own data, all pointed at one test relay. Everything a scenario asks of
// a platform goes through the driver layer (`platform/host.ts`); nothing here names a platform.
import { unwrap } from './flows.ts'
import { deviceFolder } from './device-folder.ts'
import {
  DEFAULT_MAX_DEVICES,
  limitMessage,
  maxDevicesFrom,
  nextState,
  planGroup,
} from './group-plan.ts'
import type { DeviceState, GroupSpec, PlannedDevice } from './group-plan.ts'
import type { NostrRelay } from './nostr-relay.ts'
import type { DataHandle, DeviceHost, RunningDevice } from './platform/host.ts'
import {
  createVaultOnRelay,
  onlyServers,
  openVault,
  runLink,
} from './sync-flows.ts'
import type { WaitContext } from './sync-flows.ts'

export { DEFAULT_MAX_DEVICES, maxDevicesFrom, planGroup }
export type { DeviceState, GroupSpec }

/** What the failure material needs of a device (contracts/failure-material.md). */
export interface CaptureDevice {
  folder: string
  alive(): boolean
  screenshot(callLimitMs?: number): Promise<Uint8Array>
  keepData(folder: string): void
}

export interface GroupDeps extends WaitContext {
  host: DeviceHost
  relay: NostrRelay
  credentials(): { passphrase: string }
  onTeardown(action: () => Promise<void> | void): void
  /** Keep the data of passing scenarios too. */
  keep: boolean
  maxDevices: number
  /** Called once with a function that lists the devices for the failure material. */
  captureWith?: (devices: () => CaptureDevice[]) => void
}

/** A row of the device list the application shows (`list_vault_devices`). */
export interface DeviceRow {
  vaultDeviceUuid: string
  devicePubkey: string
  alias: string | null
  role: 'main' | 'linked'
  isCurrent: boolean
  online: boolean
  lastSeen: number | null
  problem: string | null
}

export interface SyncServers {
  nostrRelays: string[]
  irohRelays: string[]
  disabled: string[]
}

export type ThisDevice = 'main' | 'linked' | 'awaiting_admission' | 'removed'

export class Device {
  readonly user: string
  readonly name: string
  readonly address: string
  readonly folder: string
  readonly vaultName: string
  readonly passphrase: string
  /** `main` or `linked`, as the device was created or linked. */
  readonly role: 'main' | 'linked'
  state: DeviceState = 'stopped'
  readonly data: DataHandle
  private process: RunningDevice | undefined
  private offlineMode = false
  private cachedPubkey: string | undefined
  private readonly group: Group

  constructor(
    group: Group,
    planned: PlannedDevice,
    vault: {
      vaultName: string
      passphrase: string
      data: DataHandle
      role: 'main' | 'linked'
    },
  ) {
    this.group = group
    this.user = planned.user
    this.name = planned.name
    this.address = planned.address
    this.folder = planned.folder
    this.vaultName = vault.vaultName
    this.passphrase = vault.passphrase
    this.data = vault.data
    this.role = vault.role
  }

  /** The running application; an error while the device is stopped or killed. */
  get page(): RunningDevice {
    if (this.process === undefined) {
      throw new Error(`device ${this.address} is ${this.state}, it has no page`)
    }
    return this.process
  }

  /** The same, for the helpers of `sync-flows.ts` that take a device with an `instance`. */
  get instance(): RunningDevice {
    return this.page
  }

  /** Starts the application over the data and opens the vault. */
  async start(): Promise<void> {
    const next = nextState(this.state, 'start', this.offlineMode)
    await this.launch()
    this.state = next
    this.group.deps.step('device-started', this.address, this.address)
  }

  /**
   * Starts the application over this device's data without opening a vault: for a device that does not
   * have one yet because the scenario links it through the start page.
   */
  async startUnopened(): Promise<void> {
    const next = nextState(this.state, 'start', this.offlineMode)
    this.process = await this.group.deps.host.start({
      data: this.data,
      folder: this.folder,
      step: (name, detail) =>
        this.group.deps.step(name, detail ?? this.address),
    })
    this.state = next
    this.group.deps.step('device-started', `${this.address} (no vault yet)`)
  }

  private async launch(): Promise<void> {
    const { host } = this.group.deps
    const process = await host.start({
      data: this.data,
      folder: this.folder,
      step: (name, detail) =>
        this.group.deps.step(name, detail ?? this.address, this.address),
    })
    try {
      await openVault(process, this.vaultName, this.passphrase)
    } catch (error) {
      try {
        await process.stop()
      } catch {
        // Preserve the vault-opening failure; teardown can report a separate stop failure.
      }
      throw error
    }
    this.process = process
  }

  async stop(): Promise<void> {
    const next = nextState(this.state, 'stop', this.offlineMode)
    await this.process?.stop()
    this.process = undefined
    this.state = next
    this.group.deps.step('device-stopped', this.address, this.address)
  }

  /** Ends the device without the application's shutdown. */
  async kill(): Promise<void> {
    const next = nextState(this.state, 'kill', this.offlineMode)
    await this.process?.kill()
    this.process = undefined
    this.state = next
    this.group.deps.step('device-killed', this.address, this.address)
  }

  /** Stop, then start again; a device that is offline comes back offline. */
  async restart(): Promise<void> {
    await this.stop()
    await this.start()
  }

  /**
   * The device keeps running and working locally but cannot reach or be reached by any other device:
   * its servers are set to none and it restarts (research R2).
   */
  async goOffline(): Promise<void> {
    nextState(this.state, 'goOffline', this.offlineMode)
    await this.setServers({
      ...(await onlyServers(this.page, this.group.deps.relay.url)),
      nostrRelays: [],
      irohRelays: [],
    })
    this.offlineMode = true
    await this.restart()
    this.group.deps.step('device-offline', this.address, this.address)
  }

  /** The group's servers back, and a restart to apply them. */
  async goOnline(): Promise<void> {
    nextState(this.state, 'goOnline', this.offlineMode)
    await this.setServers(
      await onlyServers(this.page, this.group.deps.relay.url),
    )
    this.offlineMode = false
    await this.restart()
    this.group.deps.step('device-online', this.address, this.address)
  }

  /** The server lists of the vault (they apply at the next opening). */
  async setServers(servers: SyncServers): Promise<void> {
    unwrap(
      'sync_servers_set',
      await this.page.invoke('sync_servers_set', { args: servers }),
    )
  }

  async deviceList(): Promise<DeviceRow[]> {
    return unwrap<DeviceRow[]>(
      'list_vault_devices',
      await this.page.invoke('list_vault_devices'),
    )
  }

  async status(): Promise<{ thisDevice: ThisDevice }> {
    return unwrap<{ thisDevice: ThisDevice }>(
      'sync_status',
      await this.page.invoke('sync_status'),
    )
  }

  async identity(): Promise<{ npub: string; hex: string }> {
    return unwrap<{ npub: string; hex: string }>(
      'vault_public_identity',
      await this.page.invoke('vault_public_identity'),
    )
  }

  /** The public key of this device, as other devices list it. */
  async pubkey(): Promise<string> {
    if (this.cachedPubkey !== undefined) return this.cachedPubkey
    const own = (await this.deviceList()).find((row) => row.isCurrent)
    if (own === undefined) {
      throw new Error(`device ${this.address} does not list itself`)
    }
    this.cachedPubkey = own.devicePubkey
    return own.devicePubkey
  }

  /**
   * Copies this device's vault file to a new device `name` of the group, which is not started. The
   * device must be stopped. The copy has the same passphrase and, as a copy of a main device, is a
   * main device; the copy of a linked device waits for admission (spec 024, user story 7).
   */
  async copyVaultTo(
    name: string,
    options: { user?: string } = {},
  ): Promise<Device> {
    if (this.state !== 'stopped' && this.state !== 'killed') {
      throw new Error(
        `stop device ${this.address} before copying its vault file`,
      )
    }
    const user = options.user ?? this.user
    const planned: PlannedDevice = {
      user,
      name,
      address: `${user}/${name}`,
      folder: deviceFolder(user, name),
      first: false,
      main: this.role === 'main',
    }
    const copy = this.group.add(planned, {
      vaultName: this.vaultName,
      passphrase: this.passphrase,
      role: this.role,
    })
    try {
      await this.data.copyVaultFile(this.vaultName, copy.data)
    } catch (error) {
      this.group.devices.delete(copy.address)
      try {
        copy.data.dispose()
      } catch {
        // Preserve the copy failure; the failed target is no longer registered.
      }
      throw error
    }
    this.group.deps.step(
      'vault-file-copied',
      `${this.address} -> ${copy.address}`,
      this.address,
    )
    return copy
  }
}

export class Group {
  readonly deps: GroupDeps
  /** Every device by address (`<user>/<device>`). */
  readonly devices = new Map<string, Device>()
  /** The vault of each user. */
  readonly users = new Map<string, { vaultName: string }>()

  constructor(deps: GroupDeps) {
    this.deps = deps
  }

  get relay(): NostrRelay {
    return this.deps.relay
  }

  device(address: string): Device {
    const device = this.devices.get(address)
    if (device === undefined) {
      throw new Error(
        `no device "${address}" in the group (${[...this.devices.keys()].join(', ')})`,
      )
    }
    return device
  }

  /**
   * A new device of an existing user that has no vault yet and does not run: the scenario links it,
   * for instance through the start page. It has a passphrase of its own, as a linked device does.
   */
  addDevice(user: string, name: string): Device {
    const entry = this.users.get(user)
    if (entry === undefined) {
      throw new Error(
        `no user "${user}" in the group (${[...this.users.keys()].join(', ')})`,
      )
    }
    return this.add(
      {
        user,
        name,
        address: `${user}/${name}`,
        folder: deviceFolder(user, name),
        first: false,
        main: false,
      },
      {
        vaultName: entry.vaultName,
        passphrase: this.deps.credentials().passphrase,
        role: 'linked',
      },
    )
  }

  /** Registers a device that does not run yet; refuses one more than the limit allows. */
  add(
    planned: PlannedDevice,
    vault: { vaultName: string; passphrase: string; role: 'main' | 'linked' },
  ): Device {
    if (this.devices.has(planned.address)) {
      throw new Error(`device "${planned.address}" already exists in the group`)
    }
    if (this.devices.size + 1 > this.deps.maxDevices) {
      throw new Error(limitMessage(this.devices.size + 1, this.deps.maxDevices))
    }
    const device = new Device(this, planned, {
      ...vault,
      data: this.deps.host.newData(planned.folder),
    })
    this.devices.set(planned.address, device)
    return device
  }
}

/** Makes the group the spec describes and ends all of it with the scenario. */
export async function createGroup(
  deps: GroupDeps,
  spec: GroupSpec,
): Promise<Group> {
  const plan = planGroup(spec, deps.maxDevices)
  const group = new Group(deps)
  deps.onTeardown(async () => {
    for (const device of [...group.devices.values()].reverse()) {
      try {
        if (device.state === 'running' || device.state === 'offline') {
          await device.stop()
        }
      } finally {
        if (!deps.keep) device.data.dispose()
      }
    }
  })
  deps.captureWith?.(() =>
    [...group.devices.values()].map((device) => ({
      folder: device.folder,
      alive: () => device.state === 'running' || device.state === 'offline',
      screenshot: (callLimitMs) => device.page.screenshot(callLimitMs),
      keepData: (folder: string) => device.data.keep(folder),
    })),
  )
  for (const user of plan) {
    group.users.set(user.name, { vaultName: user.vaultName })
    const { passphrase } = deps.credentials()
    const [first, ...others] = user.devices
    if (first === undefined) continue
    const origin = group.add(first, {
      vaultName: user.vaultName,
      passphrase,
      role: 'main',
    })
    await createFirstDevice(group, origin)
    for (const planned of others) {
      await linkDeviceInto(group, origin, planned)
    }
  }
  return group
}

/** The first device creates the vault, points it at the relay, and reopens it so the servers apply. */
async function createFirstDevice(group: Group, device: Device): Promise<void> {
  const { host, relay, step } = group.deps
  const fresh = await host.start({
    data: device.data,
    folder: device.folder,
    step: (name, detail) => step(name, detail, device.address),
  })
  try {
    await createVaultOnRelay(
      fresh,
      relay.url,
      device.vaultName,
      device.passphrase,
    )
  } finally {
    await fresh.stop()
  }
  await device.start()
}

/** A further device of a user is linked the way a person does it, with a code from a main device. */
async function linkDeviceInto(
  group: Group,
  origin: Device,
  planned: PlannedDevice,
): Promise<void> {
  const { host, relay, step, credentials } = group.deps
  const passphrase = credentials().passphrase
  const device = group.add(planned, {
    vaultName: origin.vaultName,
    passphrase,
    role: planned.main ? 'main' : 'linked',
  })
  const fresh = await host.start({
    data: device.data,
    folder: device.folder,
    step: (name, detail) => step(name, detail, device.address),
  })
  try {
    await runLink(group.deps, origin.page, fresh, {
      vaultName: origin.vaultName,
      deviceName: planned.name,
      passphrase,
      relayUrl: relay.url,
      asMainDevice: planned.main,
      device: device.address,
    })
  } finally {
    await fresh.stop()
  }
  await device.start()
}
