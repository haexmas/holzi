// Groups of vaults and devices for scenarios (contracts/group.md): users with their own vaults, each
// device an application over its own data, all pointed at one test relay. Everything a scenario asks of
// a platform goes through the driver layer (`platform/host.ts`); nothing here names a platform.
import { deviceFolder } from './device-folder.ts'
import { Device } from './device.ts'
import {
  DEFAULT_MAX_DEVICES,
  limitMessage,
  maxDevicesFrom,
  planGroup,
} from './group-plan.ts'
import type { GroupSpec, PlannedDevice } from './group-plan.ts'
import type { NostrRelay } from './nostr-relay.ts'
import type { DeviceHost } from './platform/host.ts'
import { createVaultOnRelay, runLink } from './sync-flows.ts'
import type { WaitContext } from './sync-flows.ts'

export { Device, DEFAULT_MAX_DEVICES, maxDevicesFrom, planGroup }
export type { DeviceRow, SyncServers, ThisDevice } from './device.ts'
export type { DeviceState, GroupSpec } from './group-plan.ts'

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

  /**
   * A new device of the host's user, linked the way a person does it with a code from the host (which
   * must be a main device), and started. `main: true` makes it a main device too.
   */
  async link(
    host: Device,
    name: string,
    options: { main?: boolean } = {},
  ): Promise<Device> {
    return linkDeviceInto(this, host, {
      user: host.user,
      name,
      address: `${host.user}/${name}`,
      folder: deviceFolder(host.user, name),
      first: false,
      main: options.main === true,
    })
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
): Promise<Device> {
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
  return device
}
