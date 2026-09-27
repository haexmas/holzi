<script setup lang="ts">
// Legacy route, kept as a redirect into the window manager (spec 015-workspace-shell,
// T024, FR-004, contracts/shell-app-contract.md §3); the federation app became the settings
// category "Föderation" (spec 023-settings-app, research R11). The redirect resolves
// before any navigation guard, so /workspace's own `onboarded` middleware
// still runs against the final URL.
definePageMeta({
  redirect: (to) =>
    `/workspace/${encodeURIComponent(String(to.params.instance ?? ''))}?open=system.settings&at=/federation`,
})
</script>

<template><div /></template>
