// One device of a group (contracts/group.md): the application over its own data, started and ended by a
// scenario through the driver layer, and what a scenario reads of it. Nothing here names a platform.
import { PROCESS_END_LIMIT_MS } from './close-promises.ts'
import { openLauncher, unwrap } from './flows.ts'
import { deviceFolder } from './device-folder.ts'
import { nextState } from './group-plan.ts'
import type { DeviceState, PlannedDevice } from './group-plan.ts'
import type { Group } from './group.ts'
import type { DataHandle, RunningDevice } from './platform/host.ts'
import { onlyServers, openVault } from './sync-flows.ts'

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

  /**
   * Locks the vault through the lock control, as a person does; the application ends with it. The
   * launcher is opened first unless it is already open. Returns the time from the press to the end of
   * the process, which the close promise of spec 013 bounds.
   */
  async lock(limitMs: number = PROCESS_END_LIMIT_MS): Promise<number> {
    const next = nextState(this.state, 'stop', this.offlineMode)
    const running = this.page
    try {
      await running.waitForDisplayed('lock-instance', 0)
    } catch {
      await openLauncher(running)
    }
    const pressedAt = await running.press('lock-instance')
    await running.waitForEnd(limitMs)
    const elapsedMs = Date.now() - pressedAt
    // The application is gone; this releases what the host kept for it.
    await running.stop()
    this.process = undefined
    this.state = next
    this.group.deps.step('device-locked', `${this.address} ${elapsedMs} ms`)
    return elapsedMs
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
