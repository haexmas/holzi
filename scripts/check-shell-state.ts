// Run with `node scripts/check-shell-state.ts`. Placeholder harness (spec 015, T004); the real
// checks (pure reducers, geometry, tabs, hydration, write queue, unknown-app handling) land in
// T053 once `src/lib/shell/` exists. Kept here so `pnpm check:shell-state` and the CI step are
// wired from the start of the feature branch.
import assert from 'node:assert/strict'
import { test } from 'node:test'

test('placeholder: shell-state harness is wired', () => {
  assert.ok(true)
})
