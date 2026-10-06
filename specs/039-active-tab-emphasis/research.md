# Research: Active Tab Emphasis

## Decision: Extend the existing active-tab class branch

The multi-tab markup in `src/components/wm/TabBar.vue` already compares each tab id with `activeTabId`, sets `aria-selected`, and applies a separate active class string. The smallest correct change is to strengthen that existing class string instead of adding state, a selector, or a new component.

## Decision: Reuse theme tokens

The stylesheet exposes `background`, `foreground`, `muted`, `accent`, `primary`, and `border` theme tokens in both light and dark schemes. Existing utility classes can therefore provide a consistent surface and accent without introducing hard-coded colors or a new dependency.

## Alternatives considered

- Adding an active-tab indicator to the window border was rejected because it would identify the window rather than the selected tab.
- Adding a new tab state or composable was rejected because `activeTabId` already drives selection, ARIA state, scrolling, and rendering.
- Changing the compact title branch was rejected because the feature targets the visible multi-tab strip and the specification preserves compact behavior.
