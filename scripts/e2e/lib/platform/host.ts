// The driver layer (contracts/driver-layer.md): everything a scenario and the group helpers may ask of
// a platform. Nothing below this interface is named outside `platform/`; one implementation exists
// (`linux.ts`) and later platforms add a file beside it, each by its own spec.
import type { Page } from '../page.ts'

/** The shapes of a backend call, so nothing outside `platform/` has to name the driver's own module. */
export type { InvokeOptions, InvokeResult } from '../webdriver.ts'

/** Where one device keeps its data. Opaque to scenarios; only the host that made it can use it. */
export interface DataHandle {
  /**
   * Copies the vault file and what it needs to be consistent into `to`. The source device must be
   * stopped; a failure leaves nothing partial in `to`.
   */
  copyVaultFile(vaultName: string, to: DataHandle): Promise<void>
  /** Copies the data into `folder`, for the failure material of a scenario. */
  keep(folder: string): void
  /** Removes the data. */
  dispose(): void
}

/** A running device: the interaction surface of spec 016 plus its lifecycle. */
export interface RunningDevice extends Page {
  /** Graceful end; the data stays. Safe to call twice. */
  stop(): Promise<void>
  /** End without the application's own shutdown; the data stays. */
  kill(): Promise<void>
  alive(): boolean
  /** Records a timeline entry of the scenario that started the device. */
  step(name: string, detail?: string): void
  /** The current window as PNG bytes. */
  screenshot(callLimitMs?: number): Promise<Uint8Array>
}

export interface StartDeviceOptions {
  data: DataHandle
  /** Folder name of the device's material (see `device-folder.ts`). */
  folder: string
  step?: (name: string, detail?: string) => void
}

export interface DeviceHost {
  /** Empty data for a device that does not exist yet. */
  newData(folder: string): DataHandle
  /** Starts the application over the data (a fresh start when the data is empty). */
  start(options: StartDeviceOptions): Promise<RunningDevice>
}
