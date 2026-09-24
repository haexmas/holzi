// The close promises of spec 013, as fixed deadlines a scenario checks against (contracts/report.md;
// tasks.md T060/T061). They never move with `E2E_TIME_SCALE`: a scenario passes `fixed: true` (or uses
// `instance.waitForEnd`, which is fixed by construction) with these values, so a diagnostic run that
// scales generic timeouts still holds them - the run reports itself non-conformant instead (FR-020,
// scenario.ts/report.ts), rather than these numbers ever being relaxed.
//
// Source of the smaller two: src-tauri/src/vault_gate/drain.rs's ladder - a 1 s cooperative window, a
// 3 s total drain limit, then a 0.5 s grace before a plain thread forces the process to end. Worst case
// that is 3.5 s from the close request to the process actually ending; PROCESS_END_LIMIT_MS adds
// headroom for the press-to-close-request hop itself. PROVIDER_CLOSE_LIMIT_MS is the drain's own
// cooperative window: the streaming reply is tracked work, so it is cancelled within it in the ordinary
// case. RELAUNCH_LIMIT_MS is not part of the drain ladder at all - a fresh process starting the
// application again - and is the spike's own observed ceiling with margin.

/** The application process ends within this long of the lock control or the window close. */
export const PROCESS_END_LIMIT_MS = 4_000

/** The stand-in provider's connection closes within this long of the press. */
export const PROVIDER_CLOSE_LIMIT_MS = 1_000

/** A relaunching build's new process appears within this long of the original one ending. */
export const RELAUNCH_LIMIT_MS = 10_000
