# Implementation Plan: Active Tab Emphasis

**Branch**: `feat/active-tab-emphasis` | **Date**: 2026-10-06 | **Spec**: [spec.md](spec.md)

**Input**: Feature specification from `specs/040-active-tab-emphasis/spec.md`

## Summary

Make the selected tab in the existing window-manager tab strip immediately recognizable in both color schemes. Reuse the current theme tokens and Tailwind utility classes in `src/components/wm/TabBar.vue`, preserving the existing ARIA state, keyboard handling, close controls, and compact/single-tab layouts.

## Technical Context

**Language/Version**: TypeScript with Vue 3 single-file components

**Primary Dependencies**: Nuxt, Tailwind CSS, existing window-manager components and theme tokens

**Storage**: N/A

**Testing**: Existing `pnpm check:templates`, `pnpm check:wm-navigation`, `pnpm lint`, and `pnpm typecheck`

**Target Platform**: Desktop application UI rendered by the Nuxt frontend

**Project Type**: Desktop application frontend

**Performance Goals**: No additional runtime work; tab state changes should continue to use the existing reactive class binding.

**Constraints**: Keep the change local to the existing tab-bar presentation, use existing theme colors, preserve responsive truncation and control hit areas, and do not change tab behavior.

**Scale/Scope**: One Vue component and its existing visual state classes.

## Constitution Check

_GATE: Must pass before design and implementation._

- No secrets, credentials, or machine-local paths are introduced.
- No external references, dependencies, or configuration changes are needed.
- The existing active-tab class binding is extended rather than duplicating tab state or authoring a new component.
- The change remains in the existing worktree/topic-branch workflow and preserves test code separation.

**Gate status**: PASS

## Research

See [research.md](research.md). The existing `TabBar.vue` already exposes the correct active state through `aria-selected` and a conditional class binding, so a presentation-only class adjustment is sufficient.

## Project Structure

### Documentation (this feature)

```text
specs/040-active-tab-emphasis/
├── checklists/requirements.md
├── spec.md
├── plan.md
├── research.md
├── data-model.md
├── quickstart.md
└── tasks.md
```

### Source Code

```text
src/components/wm/TabBar.vue  # Existing multi-tab presentation and active-state classes
scripts/check-vue-templates.ts # Existing template validation
```

**Structure Decision**: Extend the existing window-manager tab component. No new module, state, dependency, or contract is required because active-tab state and all interaction behavior already exist.

## Design Details

The active multi-tab state will use a raised surface against the title-bar background (`bg-card`, and `bg-secondary` in the dark scheme where the card equals the window background), raise the title from `font-medium` to `font-semibold`, and add a 2px bottom accent in the primary token. Weight and the accent bar are non-colour cues, so the active tab is not told apart by colour alone; the primary token keeps at least 3:1 against every surface (spec 035 control pairs). Keyboard focus gets an inset `ring-ring` focus ring, because the scrolling tablist clips an outline drawn outside the tab. Inactive tabs keep their muted text and hover treatment. The conditional branch remains keyed by `row.tab.id === activeTabId`, so selection changes automatically update the presentation while ARIA and keyboard semantics remain untouched.

The compact and single-tab branches are intentionally outside this change and retain their current classes.

## Complexity Tracking

No constitution violations or complexity exceptions.
