# Data Model: Active Tab Emphasis

This feature introduces no data model changes.

The existing `activeTabId` value on a window remains the single source of truth. The tab bar derives visual state from the existing relationship between `activeTabId` and each tab's `id`; no new persisted field or runtime state is required.
