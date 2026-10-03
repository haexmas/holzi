<script setup lang="ts">
/**
 * The category "Erweiterungen" (spec 017, US1, T049): the installed extensions with icon, name,
 * version and their state on this device, and installing one from a file. The list follows
 * `extensions-changed` through the extensions store.
 */
const { t } = useI18n()
const store = useExtensionsStore()
const installing = ref(false)

function description(version: string | undefined, status: string | undefined) {
  const parts = [version ? t('settings.extensions.version', { version }) : null]
  if (status) parts.push(t(`extensions.status.${status}`))
  return parts.filter((part) => part !== null).join(' · ')
}
</script>

<template>
  <section class="flex flex-col gap-3">
    <SettingsGroup>
      <SettingsRow
        navigates
        icon="lucide:file-plus"
        :title="t('settings.extensions.installFromFile')"
        :description="t('settings.extensions.installFromFileHint')"
        data-testid="extensions-install-from-file"
        @select="installing = true"
      />
    </SettingsGroup>

    <SettingsGroup
      v-if="store.list.length > 0"
      :label="t('settings.extensions.installed')"
    >
      <SettingsRow
        v-for="extension in store.list"
        :key="extension.id"
        :title="extension.title"
        :description="description(extension.version, extension.statusHere)"
        :data-extension-id="extension.id"
      >
        <template #title>
          <span class="flex items-center gap-3">
            <img
              v-if="store.icons[extension.id]"
              :src="store.icons[extension.id]"
              alt=""
              class="size-5 object-contain"
            />
            <Icon v-else name="lucide:puzzle" class="size-5" />
            <span>{{ extension.title }}</span>
          </span>
        </template>
        <span v-if="!extension.enabled" class="text-xs text-muted-foreground">{{
          t('extensions.status.disabled')
        }}</span>
      </SettingsRow>
    </SettingsGroup>
    <p v-else class="px-1 text-sm text-muted-foreground">
      {{ t('settings.extensions.none') }}
    </p>

    <ExtensionsInstallDialog v-model:open="installing" />
  </section>
</template>
