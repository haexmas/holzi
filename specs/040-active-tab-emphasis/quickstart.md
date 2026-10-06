# Quickstart: Active Tab Emphasis

## Prerequisites

- Node dependencies are installed with pnpm.
- The application can be started with the repository's normal desktop/frontend development command.

## Validation

1. Open a window with at least two tabs.
2. Confirm the selected tab has a clearly contrasting surface, stronger title weight, and a visible accent at its lower edge.
3. Switch tabs and confirm the visual treatment moves to the newly selected tab.
4. Repeat in light and dark color schemes.
5. Move keyboard focus into the tab strip and use the arrow keys; confirm the focused tab shows a visible focus ring in both schemes.
6. Confirm long tab titles remain truncated and close/new-tab controls remain usable.
7. Confirm a single-tab window and compact mode retain their existing title-only presentation.

## Automated checks

```sh
pnpm check:templates
pnpm check:wm-navigation
pnpm lint
pnpm typecheck
```

The checks cover template validity, unchanged tab behavior, linting, and Vue/TypeScript correctness. The visual contrast and light/dark appearance require the manual steps above.
