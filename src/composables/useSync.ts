import { invoke } from '@tauri-apps/api/core'
import type { SyncServers } from '@bindings/SyncServers'
import type { SyncStatus } from '@bindings/SyncStatus'
import type { VaultPublicIdentity } from '@bindings/VaultPublicIdentity'

/**
 * The sync state calls of the open vault session (spec 024). Changes to the vault's data reach
 * the views through `useVaultData`, not through here.
 */
export function useSync() {
  /** Who this device is in the vault, open requests, and a link in progress. */
  async function syncStatusAsync(): Promise<SyncStatus> {
    return await invoke<SyncStatus>('sync_status')
  }

  /** The public key of the vault identity, as `npub` and hex. */
  async function vaultPublicIdentityAsync(): Promise<VaultPublicIdentity> {
    return await invoke<VaultPublicIdentity>('vault_public_identity')
  }

  /** The servers devices find each other through; an empty list means the defaults. */
  async function syncServersGetAsync(): Promise<SyncServers> {
    return await invoke<SyncServers>('sync_servers_get')
  }

  async function syncServersSetAsync(args: SyncServers): Promise<void> {
    await invoke('sync_servers_set', { args })
  }

  /** Removes a device from the vault (main device only, never this one). */
  async function deviceRemoveAsync(devicePubkey: string): Promise<void> {
    await invoke('device_remove', { args: { devicePubkey } })
  }

  /** Admits (`admit`) or refuses the request of a copy of the vault file (main device only). */
  async function admissionDecideAsync(
    devicePubkey: string,
    admit: boolean,
  ): Promise<void> {
    await invoke('admission_decide', { args: { devicePubkey, admit } })
  }

  /** The user has read that this copy enrolled itself as a main device. */
  async function copyNoticeDismissAsync(): Promise<void> {
    await invoke('sync_copy_notice_dismiss')
  }

  return {
    syncStatusAsync,
    vaultPublicIdentityAsync,
    syncServersGetAsync,
    syncServersSetAsync,
    deviceRemoveAsync,
    admissionDecideAsync,
    copyNoticeDismissAsync,
  }
}
