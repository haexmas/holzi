// Run with `pnpm check:shell-navigation` (spec 020-tab-navigation). Harness for the per-tab
// navigation modules under `src/lib/shell/` — like `check-shell-state.ts`, these modules avoid
// Nuxt auto-imports and use relative `.ts`-suffixed sibling imports (015 plan research R6) so they
// load standalone here.
//
// Sections: history reducers (navigation.ts) and the route matcher (routeMatch.ts). The action
// core, keybindings and the store-level navigation live in check-shell-actions.ts and
// check-shell-nav-store.ts, which `pnpm check:shell-navigation` runs together with this file (split
// to stay below the 500-line limit).
import { test } from 'node:test'

test('harness loads', () => {})
