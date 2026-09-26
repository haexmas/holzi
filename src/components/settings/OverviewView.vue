<script setup lang="ts">
/**
 * Overview of a category with several areas (spec 023-settings-app, FR-003): one row per area
 * with icon, title, one-line description and chevron; a row opens its sub-view as a new tab
 * location.
 */
import { categoryOf, locationPath, overviewRows } from '~/lib/settings/registry'

const { t } = useI18n()
const router = useTabRouter()

const rows = computed(() => {
  const category = categoryOf(router.route.path)
  return category ? overviewRows(category) : []
})
</script>

<template>
  <ul
    class="flex flex-col divide-y divide-border overflow-hidden rounded-lg border border-border"
  >
    <li v-for="row in rows" :key="row.id">
      <SettingsOverviewRow
        :to="locationPath(row)"
        :icon="row.icon ?? ''"
        :title="t(row.titleKey)"
        :description="t(row.descriptionKey)"
        :data-testid="`settings-row-${row.id}`"
      />
    </li>
  </ul>
</template>
