<script setup lang="ts">
/**
 * One kind of server (spec 024, FR-008) as a list with an input to add one: in the settings, where
 * each change is saved at once, and in the link form of the landing page, where it only applies to
 * the link. With none listed holzi uses its built-in servers, so those are always shown: as the
 * entries in use while the list is empty, and named underneath while own servers replace them.
 */
interface Props {
  label: string
  description: string
  /** What the user listed; empty means the built-in servers are in use. */
  servers: string[]
  /** The built-in servers. */
  defaults: string[]
  placeholder: string
  /** Prefix of the test ids, e.g. `settings-servers-nostrRelays`. */
  testId: string
  busy?: boolean
  /** Takes an entered URL; the input is cleared when it returns `true`. */
  addAsync: (url: string) => Promise<boolean>
}

const props = defineProps<Props>()
const emit = defineEmits<{ remove: [url: string] }>()

const { t } = useI18n()
const draft = ref('')

async function onAdd() {
  const url = draft.value.trim()
  if (!url || props.busy || props.servers.includes(url)) return
  if (await props.addAsync(url)) draft.value = ''
}
</script>

<template>
  <SettingsGroup :label="props.label">
    <li class="px-4 py-3 text-sm text-muted-foreground">
      {{ props.description }}
    </li>
    <template v-if="props.servers.length === 0">
      <SettingsRow
        v-for="url in props.defaults"
        :key="url"
        :title="url"
        :description="t('settings.federation.servers.default')"
        icon="lucide:server"
        :data-testid="`${props.testId}-default`"
      />
    </template>
    <template v-else>
      <SettingsRow
        v-for="url in props.servers"
        :key="url"
        :title="url"
        icon="lucide:server"
        :data-testid="props.testId"
      >
        <UiButton
          variant="outline"
          type="button"
          :disabled="props.busy"
          :data-testid="`${props.testId}-remove`"
          :aria-label="t('settings.federation.servers.remove', { url })"
          @click="emit('remove', url)"
        >
          <Icon name="lucide:trash-2" class="size-4" />
        </UiButton>
      </SettingsRow>
      <li
        class="px-4 py-3 text-sm text-muted-foreground"
        :data-testid="`${props.testId}-replaced`"
      >
        {{
          t('settings.federation.servers.replaces', {
            urls: props.defaults.join(', '),
          })
        }}
      </li>
    </template>
    <li class="flex items-center gap-2 px-4 py-3">
      <ShadcnInput
        v-model="draft"
        :placeholder="props.placeholder"
        :aria-label="props.label"
        autocomplete="off"
        spellcheck="false"
        :data-testid="`${props.testId}-input`"
        @keydown.enter.prevent="onAdd"
      />
      <UiButton
        type="button"
        :disabled="props.busy || draft.trim() === ''"
        :data-testid="`${props.testId}-add`"
        @click="onAdd"
      >
        {{ t('settings.federation.servers.add') }}
      </UiButton>
    </li>
  </SettingsGroup>
</template>
