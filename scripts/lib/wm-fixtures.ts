// Shared fixtures for the window manager check scripts (spec 015-workspace-shell): the test area and two
// singleton test apps with distinct minSizes, used across the reducer, tabs, geometry and
// hydration checks instead of the shipped WM_APPS.
import type { AppDefinition } from '../../src/lib/wm/apps.ts'
import { hydrate } from '../../src/lib/wm/layoutState.ts'
import type { WmState, Size } from '../../src/lib/wm/types.ts'

export const AREA: Size = { width: 1920, height: 1080 }

// Two singletons, distinct minSizes, for the general reducer/geometry/tabs/hydration sections.
export const ALPHA: AppDefinition = {
  id: 'test.alpha',
  titleKey: 'test.alpha',
  icon: 'lucide:circle',
  defaultSize: { width: 600, height: 400 },
  minSize: { width: 300, height: 200 },
  multiInstance: false,
}
export const BETA: AppDefinition = {
  id: 'test.beta',
  titleKey: 'test.beta',
  icon: 'lucide:triangle',
  defaultSize: { width: 500, height: 350 },
  minSize: { width: 250, height: 180 },
  multiInstance: false,
}
export const APPS: readonly AppDefinition[] = [ALPHA, BETA]

export function emptyState(): WmState {
  return hydrate(
    { workspaces: [], windows: [], activeWorkspaceId: '' },
    APPS,
    AREA,
  )
}
