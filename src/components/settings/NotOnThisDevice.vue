<script setup lang="ts">
/**
 * "Auf diesem Gerät nicht verfügbar" (spec 043 FR-016, contract platform-capabilities.md): shown
 * above the settings of a facility this device does not have, which stay visible.
 */
const { t } = useI18n()
</script>

<template>
  <p
    class="rounded-md border border-border/70 bg-muted/30 px-3 py-2 text-sm text-muted-foreground"
    role="status"
    data-testid="settings-not-on-this-device"
  >
    {{ t('errors.notAvailable') }}
  </p>
</template>
