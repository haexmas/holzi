# Other platforms for the end-to-end tests

Today the scenarios run on Linux only: `tauri-driver` with `WebKitWebDriver`, a virtual screen per device
and one process per device ([`lib/platform/linux.ts`](lib/platform/linux.ts)). Holzi is also built for
Windows, macOS, Android and iOS, and its multi-device behavior has to be tested there as well.

**Nothing below is implemented.** It states what each platform would need so that the interface fits it,
and every fact is to be confirmed by the follow-up spec of that platform
([`specs/033-multi-device-e2e/research.md`](../../specs/033-multi-device-e2e/research.md), R9). A platform
is added by a file beside `lib/platform/linux.ts`; no scenario changes.

## What a platform provides

The whole of it is `DeviceHost` in [`lib/platform/host.ts`](lib/platform/host.ts):

- `newData(folder)`: a place for the data of one device, with `copyVaultFile` (copy the vault file of a
  stopped device into another device's place), `keep` (copy it out for the failure material) and
  `dispose`.
- `start({ data, folder })`: start the application over the data and return a running device: the page
  operations (`click`, `type`, `invoke`, `exec`, `waitForDisplayed`, `navigate`), `stop`, `kill`, `alive`
  and `screenshot`.

Everything else (`goOffline`, `goOnline`, `lock`, `restart`, groups, users, waiting) is built on these in
`lib/device.ts` and `lib/group.ts`. `pnpm check:e2e-lib` fails when a scenario or a scenario-facing helper
names a platform, so the scenarios stay as they are.

## The four platforms

| Platform | Likely driver                                                                                     | Runner                       | Several devices                                           | Known limits                                                          |
| -------- | ------------------------------------------------------------------------------------------------- | ---------------------------- | --------------------------------------------------------- | --------------------------------------------------------------------- |
| Windows  | Tauri WebDriver bridge (`tauri-driver`) with Microsoft Edge WebDriver                             | Windows runner               | several processes with separate data folders, as on Linux | no virtual screen; the screenshot and process operations differ       |
| macOS    | no official driver for the system web view; third-party driver or a bridge inside the application | macOS runner                 | separate data folders                                     | the biggest unknown; the bridge would be test-only code               |
| Android  | Appium (UiAutomator2) in the web view context                                                     | emulators or devices         | several emulators must reach each other on a network      | start and stop are app lifecycle, not processes; data is per emulator |
| iOS      | Appium (XCUITest)                                                                                 | macOS runner with simulators | several simulators on one host                            | simulators are limited in count; data is per simulator                |

On all four, `start`, `stop` and `kill` are app lifecycle calls, `goOffline` and `goOnline` stay built
from start, stop and the servers setting, and `copyVaultFile` is a file transfer into the sandbox of the
other device. The test relay has to be reachable from every device: on one host that is a local address,
on emulators or simulators on separate hosts it needs a network between them.
