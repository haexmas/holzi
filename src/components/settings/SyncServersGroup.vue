<script setup lang="ts">
/**
 * The servers devices find each other through (spec 024, FR-008): Nostr-Relays for the presence
 * meetings and iroh-Relays for the connections. Each change is saved at once, without a save button
 * (spec 023 FR-021); with none listed, holzi uses its built-in public servers. The iroh-Relays apply
 * at once, the Nostr-Relays when the vault is opened next.
 */
import type { SyncServers } from '@bindings/SyncServers'

const { t } = useI18n()
const { errString } = useErrorString()
const { syncServersGetAsync } = useSync()
const setServers = useActionOrThrow('settings.sync.servers.set')

const servers = ref<SyncServers>({ nostrRelays: [], irohRelays: [] })
const loaded = ref(false)
const error = ref<string | null>(null)
const drafts = reactive<Record<'nostrRelays' | 'irohRelays', string>>({
  nostrRelays: '',
  irohRelays: '',
})
const busy = ref(false)

onMounted(async () => {
  try {
    servers.value = await syncServersGetAsync()
    loaded.value = true
  } catch (e) {
    error.value = errString(e)
  }
})

type Kind = 'nostrRelays' | 'irohRelays'

async function save(next: SyncServers): Promise<boolean> {
  busy.value = true
  error.value = null
  try {
    await setServers({ ...next })
    servers.value = next
    return true
  } catch (e) {
    const reason = (e as { reason?: string })?.reason
    error.value = reason ?? errString(e)
    return false
  } finally {
    busy.value = false
  }
}

async function onAdd(kind: Kind) {
  const url = drafts[kind].trim()
  if (!url || busy.value || servers.value[kind].includes(url)) return
  const saved = await save({
    ...servers.value,
    [kind]: [...servers.value[kind], url],
  })
  if (saved) drafts[kind] = ''
}

async function onRemove(kind: Kind, url: string) {
  if (busy.value) return
  await save({
    ...servers.value,
    [kind]: servers.value[kind].filter((entry) => entry !== url),
  })
}

const groups = computed(() => [
  {
    kind: 'nostrRelays' as const,
    label: t('settings.federation.servers.nostr'),
    description: t('settings.federation.servers.nostrDescription'),
    placeholder: 'wss://',
  },
  {
    kind: 'irohRelays' as const,
    label: t('settings.federation.servers.iroh'),
    description: t('settings.federation.servers.irohDescription'),
    placeholder: 'https://',
  },
])
</script>

<template>
  <section v-if="loaded" class="flex flex-col gap-3">
    <SettingsGroup
      v-for="group in groups"
      :key="group.kind"
      :label="group.label"
    >
      <li class="px-4 py-3 text-sm text-muted-foreground">
        {{ group.description }}
      </li>
      <li
        v-if="servers[group.kind].length === 0"
        class="px-4 py-3 text-sm"
        :data-testid="`settings-servers-${group.kind}-default`"
      >
        {{ t('settings.federation.servers.defaults') }}
      </li>
      <SettingsRow
        v-for="url in servers[group.kind]"
        :key="url"
        :title="url"
        icon="lucide:server"
        :data-testid="`settings-servers-${group.kind}`"
      >
        <UiButton
          variant="outline"
          :disabled="busy"
          :aria-label="t('settings.federation.servers.remove', { url })"
          @click="onRemove(group.kind, url)"
        >
          <Icon name="lucide:trash-2" class="size-4" />
        </UiButton>
      </SettingsRow>
      <li class="flex items-center gap-2 px-4 py-3">
        <ShadcnInput
          v-model="drafts[group.kind]"
          :placeholder="group.placeholder"
          :aria-label="group.label"
          autocomplete="off"
          spellcheck="false"
          :data-testid="`settings-servers-${group.kind}-input`"
          @keydown.enter.prevent="onAdd(group.kind)"
        />
        <UiButton
          :disabled="busy || drafts[group.kind].trim() === ''"
          :data-testid="`settings-servers-${group.kind}-add`"
          @click="onAdd(group.kind)"
        >
          {{ t('settings.federation.servers.add') }}
        </UiButton>
      </li>
    </SettingsGroup>
    <p v-if="error" class="px-1 text-sm text-destructive" role="alert">
      {{ error }}
    </p>
  </section>
  <p v-else-if="error" class="text-sm text-destructive" role="alert">
    {{ error }}
  </p>
</template>
