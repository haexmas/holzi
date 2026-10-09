/**
 * The file browser's commands (spec 044, contracts/tauri-commands.md). Every call acts as the
 * user; paths are checked in Rust. Errors come back as `FilesError` (`{ code, message }`).
 */
import { Channel, invoke } from '@tauri-apps/api/core'
import type { ConflictChoice } from '@bindings/ConflictChoice'
import type { Entry } from '@bindings/Entry'
import type { FilesError } from '@bindings/FilesError'
import type { FolderChanged } from '@bindings/FolderChanged'
import type { Opened } from '@bindings/Opened'
import type { SourceRef } from '@bindings/SourceRef'
import type { Sources } from '@bindings/Sources'
import type { TextContent } from '@bindings/TextContent'
import type { TransferEvent } from '@bindings/TransferEvent'
import type { TransferOp } from '@bindings/TransferOp'
import type { TransferTarget } from '@bindings/TransferTarget'

/** A `FilesError` from a rejected command, or `undefined` for anything else. */
export function asFilesError(error: unknown): FilesError | undefined {
  if (
    error &&
    typeof error === 'object' &&
    'code' in error &&
    'message' in error
  ) {
    return error as FilesError
  }
  return undefined
}

export function useFiles() {
  async function sourcesAsync(): Promise<Sources> {
    return await invoke<Sources>('files_sources')
  }

  async function listAsync(source: SourceRef, path: string): Promise<Entry[]> {
    return await invoke<Entry[]>('files_list', { source, path })
  }

  async function statAsync(source: SourceRef, path: string): Promise<Entry> {
    return await invoke<Entry>('files_stat', { source, path })
  }

  async function readTextAsync(
    source: SourceRef,
    path: string,
  ): Promise<TextContent> {
    return await invoke<TextContent>('files_read_text', { source, path })
  }

  /** Opens a file for the viewer of `tabId`; images, video, audio and PDF get a media server URL
   * that lives until it is released or the tab goes away. */
  async function openAsync(
    source: SourceRef,
    path: string,
    tabId: string,
  ): Promise<Opened> {
    return await invoke<Opened>('files_open', { source, path, tabId })
  }

  async function releaseAsync(url: string): Promise<void> {
    await invoke('files_release', { url })
  }

  async function releaseTabAsync(tabId: string): Promise<void> {
    await invoke('files_release_tab', { tabId })
  }

  async function thumbnailAsync(
    source: SourceRef,
    entry: Entry,
  ): Promise<ArrayBuffer> {
    return await invoke<ArrayBuffer>('files_thumbnail', {
      source,
      path: entry.path,
      size: entry.size ?? 0,
      modifiedMs: entry.modifiedMs ?? 0,
    })
  }

  /** Watches `path`; `onChange` runs for every debounced batch. Resolves to the watch id. */
  async function watchAsync(
    path: string,
    onChange: (change: FolderChanged) => void,
  ): Promise<number> {
    const channel = new Channel<FolderChanged>()
    channel.onmessage = onChange
    return await invoke<number>('files_watch', { path, channel })
  }

  async function unwatchAsync(id: number): Promise<void> {
    await invoke('files_unwatch', { id })
  }

  async function openSystemAsync(
    source: SourceRef,
    path: string,
  ): Promise<void> {
    await invoke('files_open_system', { source, path })
  }

  async function createFolderAsync(
    source: SourceRef,
    path: string,
    name: string,
  ): Promise<Entry> {
    return await invoke<Entry>('files_create_folder', { source, path, name })
  }

  async function renameAsync(
    source: SourceRef,
    path: string,
    newName: string,
  ): Promise<Entry> {
    return await invoke<Entry>('files_rename', { source, path, newName })
  }

  /** Starts a copy, move or delete; `onEvent` hears its progress, conflicts and end. Refusals
   * before the start (`intoItself`, `noSpace`, …) reject. Resolves to the transfer id. */
  async function transferStartAsync(
    op: TransferOp,
    from: SourceRef,
    paths: string[],
    to: TransferTarget | null,
    onEvent: (event: TransferEvent) => void,
  ): Promise<string> {
    const channel = new Channel<TransferEvent>()
    channel.onmessage = onEvent
    return await invoke<string>('files_transfer_start', {
      op,
      from,
      paths,
      to,
      channel,
    })
  }

  /** Copies paths dropped from the system into a folder (FR-024). */
  async function importDroppedAsync(
    paths: string[],
    to: TransferTarget,
    onEvent: (event: TransferEvent) => void,
  ): Promise<string> {
    const channel = new Channel<TransferEvent>()
    channel.onmessage = onEvent
    return await invoke<string>('files_import_dropped', { paths, to, channel })
  }

  async function transferAnswerAsync(
    transferId: string,
    choice: ConflictChoice,
    forAll: boolean,
  ): Promise<void> {
    await invoke('files_transfer_answer', { transferId, choice, forAll })
  }

  async function transferCancelAsync(transferId: string): Promise<void> {
    await invoke('files_transfer_cancel', { transferId })
  }

  async function transferRetryAsync(transferId: string): Promise<void> {
    await invoke('files_transfer_retry', { transferId })
  }

  return {
    sourcesAsync,
    listAsync,
    statAsync,
    readTextAsync,
    openAsync,
    releaseAsync,
    releaseTabAsync,
    thumbnailAsync,
    watchAsync,
    unwatchAsync,
    openSystemAsync,
    createFolderAsync,
    renameAsync,
    transferStartAsync,
    importDroppedAsync,
    transferAnswerAsync,
    transferCancelAsync,
    transferRetryAsync,
  }
}
