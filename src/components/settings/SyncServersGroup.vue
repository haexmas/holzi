<script setup lang="ts">
/**
 * The servers devices find each other through (spec 024, FR-008): Nostr-Relays for the presence
 * meetings and iroh-Relays for the connections. Each change is saved at once, without a save button
 * (spec 023 FR-021); with none listed, holzi uses its built-in public servers, which are always
 * shown. The iroh-Relays apply at once, the Nostr-Relays when the vault is opened next.
 */
import type { SyncServers } from '@bindings/SyncServers'

const { t } = useI18n()
const { errString } = useErrorString()
const { syncServersGetAsync, syncServersDefaultsAsync } = useSync()
const setServers = useActionOrThrow('settings.sync.servers.set')

const servers = ref<SyncServers>({ nostrRelays: [], irohRelays: [] })
const defaults = ref<SyncServers>({ nostrRelays: [], irohRelays: [] })
const loaded = ref(false)
const error = ref<string | null>(null)
const busy = ref(false)

async function loadAsync() {
  try {
    ;[servers.value, defaults.value] = await Promise.all([
      syncServersGetAsync(),
      syncServersDefaultsAsync(),
    ])
    loaded.value = true
  } catch (e) {
    error.value = errString(e)
  }
}

onMounted(loadAsync)
// The servers are vault preferences: a change made on another device shows here at once.
onVaultTablesChanged(['preferences'], () => {
  if (!busy.value) return loadAsync()
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

function addAsync(kind: Kind, url: string): Promise<boolean> {
  return save({
    ...servers.value,
    [kind]: [...servers.value[kind], url],
  })
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
    <SettingsServerList
      v-for="group in groups"
      :key="group.kind"
      :label="group.label"
      :description="group.description"
      :servers="servers[group.kind]"
      :defaults="defaults[group.kind]"
      :placeholder="group.placeholder"
      :test-id="`settings-servers-${group.kind}`"
      :busy="busy"
      :add-async="(url: string) => addAsync(group.kind, url)"
      @remove="onRemove(group.kind, $event)"
    />
    <p v-if="error" class="px-1 text-sm text-destructive" role="alert">
      {{ error }}
    </p>
  </section>
  <p v-else-if="error" class="text-sm text-destructive" role="alert">
    {{ error }}
  </p>
</template>
