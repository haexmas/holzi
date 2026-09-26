<script setup lang="ts">
/**
 * Overview of a category with several areas (spec 023-settings-app, FR-003): one card per area
 * with icon, title, one-line description and chevron, like the COSMIC settings; a card opens its
 * sub-view as a new tab location.
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
  <div class="flex flex-col gap-3">
    <SettingsGroup v-for="row in rows" :key="row.id">
      <SettingsRow
        :to="locationPath(row)"
        :icon="row.icon"
        :title="t(row.titleKey)"
        :description="row.descriptionKey ? t(row.descriptionKey) : undefined"
        :data-testid="`settings-row-${row.id}`"
      />
    </SettingsGroup>
  </div>
</template>
