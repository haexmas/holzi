<script setup lang="ts">
/**
 * The category "Darstellung" (spec 023-settings-app FR-013, spec 035-appearance-and-fields): the
 * colour scheme, then the accent colour and the backgrounds like the COSMIC dialog "Aussehen", with
 * import, export and reset. Every choice is saved as soon as it is made (no save button).
 */
const { t } = useI18n()

const status = ref<{ kind: 'saved' | 'failed'; text: string } | null>(null)
const saved = () =>
  (status.value = { kind: 'saved', text: t('settings.appearance.saved') })
const done = (text: string) => (status.value = { kind: 'saved', text })
const failed = (text: string) => (status.value = { kind: 'failed', text })
</script>

<template>
  <div class="flex flex-col gap-4">
    <div class="flex flex-wrap items-center justify-end gap-2">
      <SettingsAppearanceFileButtons @done="done" @failed="failed" />
    </div>

    <SettingsColorSchemeSetting />

    <SettingsGroup>
      <SettingsAppearanceColorRow
        control="accent"
        title-key="accent"
        @saved="saved"
        @failed="failed"
      />
      <SettingsAppearanceColorRow
        control="window"
        title-key="window"
        @saved="saved"
        @failed="failed"
      />
      <SettingsAppearanceColorRow
        control="container"
        title-key="container"
        hint-key="containerHint"
        @saved="saved"
        @failed="failed"
      />
    </SettingsGroup>

    <div class="flex flex-wrap items-center gap-2">
      <SettingsAppearanceResetButton
        @done="done(t('settings.appearance.saved'))"
        @failed="failed"
      />
    </div>

    <p
      v-if="status?.kind === 'saved'"
      class="px-1 text-xs text-success"
      role="status"
    >
      {{ status.text }}
    </p>
    <p
      v-if="status?.kind === 'failed'"
      class="px-1 text-xs text-destructive"
      role="alert"
      data-testid="appearance-error"
    >
      {{ status.text }}
    </p>
  </div>
</template>
