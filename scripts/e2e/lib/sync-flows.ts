// Flows for scenarios with several devices of one vault (spec 024, T078): each device is its own
// application process with its own data, all pointed at one Nostr test relay. A device is made the
// way a user does it: the first by creating a vault, the others by linking with a code (user story 5).
import type { Instance } from './instance.ts'
import type { Page } from './page.ts'
import type { ScenarioContext } from './scenario.ts'
import { unwrap, waitForPath } from './flows.ts'
import type { FlowInstance } from './flows.ts'

/** Devices on one machine find each other by address; the iroh relays would need a network, so they
 * point at a closed port, which refuses at once (as in `tests/common/sync_fixture.rs`). */
const NO_IROH_RELAY = ['https://127.0.0.1:1']

/** One device: the application process that has the vault open, and what unlocks it. */
export interface Device {
  instance: Instance
  vaultName: string
  passphrase: string
}

interface Thread {
  id: string
  title: string
}

/** Only the test relay: the built-in servers, which would need a network, are switched off. */
export async function onlyServers(page: Page, relayUrl: string) {
  const defaults = unwrap<{ nostrRelays: string[]; irohRelays: string[] }>(
    'sync_servers_defaults',
    await page.invoke('sync_servers_defaults'),
  )
  return {
    nostrRelays: [relayUrl],
    irohRelays: NO_IROH_RELAY,
    disabled: [...defaults.nostrRelays, ...defaults.irohRelays],
  }
}

/** What the steps below need of a scenario: its timeline and its waits. */
export type WaitContext = Pick<ScenarioContext, 'waitFor' | 'step'>

/** Opens the vault in a started application and shows its workspace. */
export async function openVault(
  page: FlowInstance,
  vaultName: string,
  passphrase: string,
): Promise<void> {
  unwrap(
    'open_instance',
    await page.invoke('open_instance', {
      args: { name: vaultName, passphrase },
    }),
  )
  await page.navigate(`tauri://localhost/workspace/${vaultName}`)
  await waitForPath(page, '/workspace/')
}

/** Creates the vault and points it at the test relay only (the servers apply at the next opening). */
export async function createVaultOnRelay(
  page: Page,
  relayUrl: string,
  vaultName: string,
  passphrase: string,
): Promise<void> {
  unwrap(
    'create_instance',
    await page.invoke('create_instance', {
      args: { name: vaultName, passphrase },
    }),
  )
  unwrap(
    'sync_servers_set',
    await page.invoke('sync_servers_set', {
      args: await onlyServers(page, relayUrl),
    }),
  )
}

/** Starts an instance, optionally reusing its data root, and opens the vault's workspace. */
async function openAndShow(
  ctx: ScenarioContext,
  root: string | undefined,
  vaultName: string,
  passphrase: string,
): Promise<Instance> {
  const instance = await ctx.startInstance(
    root === undefined ? {} : { reusesRoot: root },
  )
  await openVault(instance, vaultName, passphrase)
  return instance
}

/**
 * The first device of a vault, using the relay. The Nostr relays of the vault apply the next time it
 * opens, so it is created, pointed at the relay, closed and opened again.
 */
export async function startFirstDevice(
  ctx: ScenarioContext,
  relayUrl: string,
  vaultName: string,
): Promise<Device> {
  const { passphrase } = ctx.credentials()
  const first = await ctx.startInstance()
  await createVaultOnRelay(first, relayUrl, vaultName, passphrase)
  await first.stop()
  const instance = await openAndShow(ctx, first.root, vaultName, passphrase)
  return { instance, vaultName, passphrase }
}

/** Closes a device's process and opens its vault again in a new one over the same data. */
export async function restartDevice(
  ctx: ScenarioContext,
  device: Device,
): Promise<Device> {
  const { root } = device.instance
  await device.instance.stop()
  const instance = await openAndShow(
    ctx,
    root,
    device.vaultName,
    device.passphrase,
  )
  return { ...device, instance }
}

/** Ends a device's process; its data stays for [`restartDevice`]. */
export async function closeDevice(device: Device): Promise<void> {
  await device.instance.stop()
}

/**
 * The steps of linking through commands: the host shows a code, the new installation enters it with
 * the servers of the vault, the host agrees, and the new installation reports it is done. The new
 * installation still has to be reopened over its data to open the vault it was given.
 */
export async function runLink(
  ctx: WaitContext,
  host: Page,
  fresh: Page,
  link: {
    vaultName: string
    deviceName: string
    passphrase: string
    relayUrl: string
    asMainDevice?: boolean
  },
): Promise<void> {
  const { code } = unwrap<{ code: string }>(
    'link_code_create',
    await host.invoke('link_code_create'),
  )
  unwrap(
    'link_join_start',
    await fresh.invoke('link_join_start', {
      args: {
        code,
        vaultName: link.vaultName,
        deviceName: link.deviceName,
        passphrase: link.passphrase,
        servers: await onlyServers(fresh, link.relayUrl),
      },
    }),
  )
  await ctx.waitFor(
    `the main device to ask about "${link.deviceName}"`,
    async () => {
      const status = unwrap<{
        linking: { stage: string; newDeviceName?: string } | null
      }>('sync_status', await host.invoke('sync_status'))
      return (
        status.linking?.stage === 'awaiting_confirmation' &&
        status.linking.newDeviceName === link.deviceName
      )
    },
    { timeoutMs: 40_000, fixed: true },
  )
  unwrap(
    'link_confirm',
    await host.invoke('link_confirm', {
      args: { asMainDevice: link.asMainDevice === true },
    }),
  )
  await ctx.waitFor(
    `the link of "${link.deviceName}" to finish`,
    async () => {
      const state = unwrap<{ state: string; reason?: unknown }>(
        'link_join_status',
        await fresh.invoke('link_join_status'),
      )
      if (state.state === 'failed') {
        throw new Error(`the link failed: ${JSON.stringify(state.reason)}`)
      }
      return state.state === 'done'
    },
    { timeoutMs: 40_000, fixed: true },
  )
  ctx.step('linked', link.deviceName)
}

/**
 * Links a new device to `host` with a code: the main device shows the code, the new installation
 * enters it with the servers of the vault, the main device agrees, and the new device opens the
 * vault it was given.
 */
export async function linkDevice(
  ctx: ScenarioContext,
  host: Device,
  relayUrl: string,
  options: { deviceName: string; asMainDevice?: boolean },
): Promise<Device> {
  const { passphrase } = ctx.credentials()
  const fresh = await ctx.startInstance()
  await runLink(ctx, host.instance, fresh, {
    vaultName: host.vaultName,
    deviceName: options.deviceName,
    passphrase,
    relayUrl,
    asMainDevice: options.asMainDevice,
  })
  // The linked vault is a file of the new installation like any other: it is opened with the
  // passphrase chosen for it, in a process over the same data.
  await fresh.stop()
  const instance = await openAndShow(
    ctx,
    fresh.root,
    host.vaultName,
    passphrase,
  )
  return { instance, vaultName: host.vaultName, passphrase }
}

/** The titles of a device's threads, sorted. */
export async function threadTitles(device: Device): Promise<string[]> {
  const threads = unwrap<Thread[]>(
    'list_threads',
    await device.instance.invoke('list_threads'),
  )
  return threads.map((thread) => thread.title).sort()
}

/** A new thread with the title; returns its id. */
export async function addThread(
  device: Device,
  title: string,
): Promise<string> {
  return unwrap<Thread>(
    'create_thread',
    await device.instance.invoke('create_thread', { args: { title } }),
  ).id
}

export async function renameThread(
  device: Device,
  id: string,
  title: string,
): Promise<void> {
  unwrap(
    'rename_thread',
    await device.instance.invoke('rename_thread', {
      args: { threadId: id, title },
    }),
  )
}

export async function removeThread(device: Device, id: string): Promise<void> {
  unwrap(
    'delete_thread',
    await device.instance.invoke('delete_thread', { args: { threadId: id } }),
  )
}

/** Waits until the device's threads have exactly these titles. */
export async function expectThreads(
  ctx: WaitContext,
  device: Device,
  titles: string[],
  description: string,
): Promise<void> {
  const wanted = [...titles].sort()
  await ctx.waitFor(
    description,
    async () => {
      const have = await threadTitles(device)
      return JSON.stringify(have) === JSON.stringify(wanted) ? have : false
    },
    { timeoutMs: 40_000, fixed: true },
  )
}
