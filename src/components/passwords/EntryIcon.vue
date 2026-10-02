<script setup lang="ts">
/**
 * The picture of an entry or folder (spec 034, T088): a name of the lists renders as the literal
 * `lucide:` icon, `binary:<hash>` renders the picture the import stored (loaded lazily through the
 * icon cache), anything unknown renders the default of the place.
 */
import { resolveIcon } from '~/lib/passwords/icons'

const props = defineProps<{
  /** The stored `icon` value. */
  value: string | null | undefined
  /** The icon of the place when the value is empty or unknown. */
  fallback: string
}>()

const icons = usePasswordsIconsStore()
const resolved = computed(() => resolveIcon(props.value))
const url = computed(() =>
  resolved.value.kind === 'binary' ? icons.urlFor(resolved.value.hash) : null,
)

watchEffect(() => {
  if (resolved.value.kind === 'binary')
    void icons.loadAsync(resolved.value.hash)
})
</script>

<template>
  <img
    v-if="url"
    :src="url"
    alt=""
    class="size-full rounded-[inherit] object-contain"
    aria-hidden="true"
  />
  <Icon
    v-else
    :name="resolved.kind === 'name' ? resolved.name : fallback"
    class="size-full"
  />
</template>
