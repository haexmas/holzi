<script setup lang="ts">
/**
 * One extension in the settings (spec 017, US3, T070): what it is and its state on this device,
 * then its permissions. Reached from its row in "Erweiterungen".
 */
const { t } = useI18n()
const router = useTabRouter()
const store = useExtensionsStore()

const extensionId = computed(() => router.route.params.extensionId ?? '')
const extension = computed(() =>
  store.list.find((e) => e.id === extensionId.value),
)
</script>

<template>
  <section class="flex flex-col gap-3">
    <p v-if="!extension" class="px-1 text-sm text-muted-foreground">
      {{ t('errors.extensions.notFound') }}
    </p>
    <template v-else>
      <SettingsGroup>
        <SettingsRow
          :title="extension.title"
          :description="extension.description"
        >
          <template #title>
            <span class="flex items-center gap-3">
              <img
                v-if="store.icons[extension.id]"
                :src="store.icons[extension.id]"
                alt=""
                class="size-6 object-contain"
              />
              <Icon v-else name="lucide:puzzle" class="size-6" />
              <span class="font-semibold">{{ extension.title }}</span>
            </span>
          </template>
        </SettingsRow>
        <SettingsRow
          :title="t('settings.extensions.versionLabel')"
          :description="extension.version ?? '—'"
        />
        <SettingsRow
          :title="t('settings.extensions.publisher')"
          :description="extension.publisherFingerprint"
        />
        <SettingsRow
          :title="t('settings.extensions.stateHere')"
          :description="
            extension.statusHere
              ? t(`extensions.status.${extension.statusHere}`)
              : t('settings.extensions.notStartedHere')
          "
        />
      </SettingsGroup>
      <SettingsExtensionsExtensionPermissionsView
        :extension-id="extension.id"
      />
    </template>
  </section>
</template>
