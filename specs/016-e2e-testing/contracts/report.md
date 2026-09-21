# Contract: report, timeline and kept material

**Feature**: [spec.md](../spec.md) | **Plan**: [plan.md](../plan.md)

## Where

`<target>/e2e/<run-id>/`, where `<target>` is `CARGO_TARGET_DIR` if set, else `src-tauri/target`. The
root can be moved with `E2E_ARTIFACTS_DIR`. Version control already ignores `src-tauri/target/`.

A passing run leaves only `report.json` (and `build.log` if the suite built the application). A failed
or timed-out scenario also leaves its own directory (FR-019). `--keep` keeps the directory of passing
scenarios too.

## `report.json`

```jsonc
{
  "runId": "20260921-231500-ab12cd",
  "status": "passed | failed | preflight-failed | build-failed | interrupted",
  "application": {
    "path": "…",
    "source": "built | given",
    "closeBehavior": "exit | relaunch",
    "closeBehaviorFrom": "flag | path",
  },
  "tools": [{ "name": "tauri-driver", "path": "…", "ok": true }],
  "versions": { "driver": "2.52.6", "webview": "2.52.6" },
  "scenarios": [
    {
      "name": "lock-while-streaming",
      "status": "passed | failed | skipped",
      "skipReason": "the build exits on close; this scenario needs one that relaunches",
      "durationMs": 9120,
      "failedStep": "process-ended",
      "steps": [
        { "name": "press", "atMs": 6100 },
        { "name": "provider-closed", "atMs": 6101 },
      ],
      "material": "<run directory>/lock-while-streaming",
    },
  ],
}
```

Paths in the report are for reading on the machine that produced it; they are not committed.

`failedStep` names the last step the scenario reached before failing, or the failing wait's description,
so that a maintainer needs no second run to find the step (SC-004).

## Key steps (FR-020)

`instance-ready`, `unlocked`, `reply-streaming`, `press`, `provider-closed`, `process-ended`,
`relaunch-seen`. A scenario may add its own. `atMs` counts from the scenario's start. "Press to process
end" is `process-ended.atMs - press.atMs`, and the summary prints it, so a slowdown is visible while the
scenario still passes.

## Material kept for a failure

| File             | Content                                                                                                                                |
| ---------------- | -------------------------------------------------------------------------------------------------------------------------------------- |
| `timeline.json`  | All steps with `atMs` and an ISO time, plus the failure message and the deadline that was reached, if any.                             |
| `screenshot.png` | The window at the moment of failure, taken before teardown. Absent, with a note in the timeline, if the application had already ended. |
| `driver.log`     | Output of the driver, the webview driver and the application, in the order it arrived, each line with its time.                        |
| `provider.json`  | The stand-in provider's connections and requests.                                                                                      |

## Printed summary

```text
passed  lock-while-streaming            9.1 s   press to process end 0.6 s
failed  window-close-while-streaming   14.0 s   failed at: process-ended (limit 4 s)
                                                material: <run directory>/window-close-while-streaming
skipped relaunch-after-lock                     the build exits on close; this scenario needs one that relaunches
…
Application: <path> (closes by exit, from path)   Result: FAILED (1 failed, 1 skipped, 5 passed)
```
