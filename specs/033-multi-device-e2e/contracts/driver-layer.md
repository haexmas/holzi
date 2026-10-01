# Contract: driver layer (`DeviceHost`)

The only place that knows the platform. One implementation now (`scripts/e2e/lib/platform/linux.ts`, wrapping `instance.ts`, `processes.ts`, `webdriver.ts`); later platforms add a file next to it, each by its own spec. The group helpers and all scenarios use this interface and nothing below it.

```ts
interface DeviceHost {
  // lifecycle
  start(opts: { dataRoot: DataHandle; app: string }): Promise<RunningDevice>
  // operations of a running device are the Page of spec 016 plus:
}
interface RunningDevice extends Page {
  stop(): Promise<void> // graceful
  kill(): Promise<void> // no cleanup
  alive(): boolean
  screenshot(name: string): Promise<void>
}
interface DataHandle {
  // opaque to scenarios
  copyVaultFile(vaultName: string, to: DataHandle): Promise<void>
}
```

Operations of FR-021 and where each lives: start and stop a device (`start`, `stop`, `kill`), click, type, read, call a backend command, wait for a condition (`Page`), take a screenshot (`screenshot`), control a device's network (`goOffline` and `goOnline` in the group helper, built on start, stop and the servers action, so no extra platform operation), and copy a vault file (`DataHandle.copyVaultFile`). Waiting uses fixed real-time deadlines; the driver layer exposes no clock-control operation.

## Rules

- A `DataHandle` is created by the host and passed back to it; scenarios never build paths.
- `copyVaultFile` copies the vault file and everything it needs to be consistent (the main file and its companion files) after the source has been stopped, or reports an error. It refuses to run against a running source.
- `kill` must not run the application's shutdown (on Linux SIGKILL of the process group).
- Errors carry the device name, added by the group helper.

## Per-platform notes

See `scripts/e2e/PLATFORMS.md`.
