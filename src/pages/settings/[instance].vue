<script setup lang="ts">
// Legacy route (spec 002), kept as a redirect into the Shell (spec
// 015-workspace-shell, T024, FR-004, contracts/shell-app-contract.md §3).
// The redirect resolves before any navigation guard, so /workspace's own
// `onboarded` middleware still runs against the final URL.
definePageMeta({
  redirect: (to) =>
    `/workspace/${encodeURIComponent(String(to.params.instance ?? ''))}?open=system.settings`,
})
</script>

<template><div /></template>
