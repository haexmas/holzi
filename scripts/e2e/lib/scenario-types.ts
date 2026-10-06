// The types of a scenario's shell: what the command passes in, the context a scenario body gets, and
// what a run produces. Kept apart from `scenario.ts` (the logic) so that file stays under the 500-line
// boundary; `scenario.ts` re-exports every name, so importers do not change.
import type { CaptureDevice, Group, GroupSpec } from './group.ts'
import type { ColorScheme, Instance } from './instance.ts'
import type { Tools } from './preflight.ts'
import type { NostrRelay } from './nostr-relay.ts'
import type { DeviceHost } from './platform/host.ts'
import type { Behavior, Provider, StandInModel } from './provider.ts'

export type CloseBehavior = 'exit' | 'relaunch'
export type ScenarioStatus = 'passed' | 'failed' | 'skipped'

export interface E2EEnv {
  runDir: string
  app: string
  closeBehavior: CloseBehavior
  tools: Tools
  /** Default limit of a scenario that sets none. */
  scenarioTimeoutMs: number
  /** Multiplies generic timeouts. The close promises stay fixed and a scaled run is non-conformant. */
  timeScale: number
  marker: string
  /** Keep the material of passing scenarios too. */
  keep: boolean
}

export interface Step {
  name: string
  /** Milliseconds since the scenario started, from one monotonic clock. */
  atMs: number
  at: string
  detail?: string
  /** Address of the device touched by a multi-device step. */
  device?: string
}

export interface ScenarioResult {
  name: string
  status: ScenarioStatus
  durationMs: number
  skipReason?: string
  error?: string
  steps: Step[]
  /** The last step reached before a failure, or the failing wait's own description. Failed status only. */
  failedStep?: string
  /** Where the kept material for a failure is. Failed status only. */
  material?: string
}

export interface ScenarioOptions {
  /** What the scenario needs; without it the scenario is skipped. `container`: a working `docker`
   * (spec 038: a local RustFS). */
  needs?: { closeBehavior?: CloseBehavior; container?: boolean }
  timeoutMs?: number
}

export interface StartInstanceRequest {
  scenario: string
  root: string
  logFile: string
  colorScheme?: ColorScheme
  reuse: boolean
  env: E2EEnv
  /** Records a timeline entry on the scenario that made the request. */
  step: (name: string, detail?: string, device?: string) => void
  /** Keeps the screen's current XWD image at `<framebufferDir>/Xvfb_screen0` (research R11, T068). */
  framebufferDir?: string
}

export interface FailureInfo {
  scenario: string
  env: E2EEnv
  error: unknown
  steps: Step[]
  instances: Instance[]
  providers: Provider[]
  /** The devices of the scenario's group, if it made one. */
  devices: CaptureDevice[]
  failedStep?: string
  deadlineMs?: number
}

export interface RunDeps {
  env: E2EEnv
  startInstance: (request: StartInstanceRequest) => Promise<Instance>
  /** The driver layer for groups; a run without one cannot make a group. */
  createHost?: (request: { scenario: string; env: E2EEnv }) => DeviceHost
  /** Called before teardown for body failures and after teardown for teardown failures. */
  onFailure?: (info: FailureInfo) => Promise<void>
}

export interface WaitOptions {
  timeoutMs?: number
  intervalMs?: number
  /** Do not multiply the limit by the time scale, for a wait that checks a fixed promise. */
  fixed?: boolean
}

export interface ScenarioContext {
  app: { path: string; closeBehavior: CloseBehavior }
  name: string
  /** Aborted when the scenario reaches its deadline. */
  signal: AbortSignal
  step(name: string, detail?: string, device?: string): void
  waitFor<T>(
    description: string,
    predicate: () => T | Promise<T>,
    options?: WaitOptions,
  ): Promise<Awaited<T>>
  onTeardown(action: () => Promise<void> | void): void
  startInstance(options?: {
    colorScheme?: ColorScheme
    reusesRoot?: string
    /** Keeps the screen's current XWD image at `<framebufferDir>/Xvfb_screen0` (research R11, T068). */
    framebufferDir?: string
  }): Promise<Instance>
  /** Starts a stand-in model provider ([stand-in-provider.md](stand-in-provider.md)); ended with the context. */
  provider(
    behavior?: Behavior,
    options?: { models?: StandInModel[] },
  ): Promise<Provider>
  /** A Nostr relay (the binary built with `--features e2e`) the instances of a scenario can share; ended with the context. */
  nostrRelay(): Promise<NostrRelay>
  /**
   * Users with vaults and devices over one test relay (contracts/group.md); the devices start with
   * their own data and all of it ends with the scenario.
   */
  group(spec: GroupSpec): Promise<Group>
  /** A passphrase and a provider key generated for this run; no credential is ever committed. */
  credentials(): { passphrase: string; providerKey: string }
}
