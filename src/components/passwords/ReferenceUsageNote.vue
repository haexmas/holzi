<script setup lang="ts">
/**
 * Inside the confirmation before deleting for good (spec 036, US7, FR-048, research R12): how many
 * other entries point at the ones deleted, and the choice to turn those references into own values
 * first (on by default). Shows nothing when no entry points at them. Passkey links (stage 4) are
 * named as dropping.
 */
const props = defineProps<{
  /** The entries that go, including those inside deleted folders. */
  itemIds: readonly string[]
}>()

const inline = defineModel<boolean>('inline', { required: true })

const { t } = useI18n()
const { referenceUsageAsync } = usePasswords()

const targets = ref(0)
const passkeyLinks = ref(0)

// The ids can change while the dialog is open (a sync reload); only the latest answer counts, and
// the user's choice stays (the caller resets it when it asks).
watch(
  () => props.itemIds.join(','),
  async (_ids, _old, onCleanup) => {
    let stale = false
    onCleanup(() => (stale = true))
    if (props.itemIds.length === 0) {
      targets.value = 0
      passkeyLinks.value = 0
      return
    }
    try {
      const usage = await referenceUsageAsync([...props.itemIds])
      if (stale) return
      targets.value = usage.reduce((sum, entry) => sum + entry.targetItems, 0)
      passkeyLinks.value = usage.reduce(
        (sum, entry) => sum + entry.passkeyLinks,
        0,
      )
    } catch {
      // Without the count there is no choice; the delete stays possible.
      if (stale) return
      targets.value = 0
      passkeyLinks.value = 0
    }
  },
  { immediate: true },
)
</script>

<template>
  <div
    v-if="targets > 0 || passkeyLinks > 0"
    class="flex flex-col gap-2 rounded-lg border border-dashed p-3 text-sm"
    data-testid="passwords-reference-usage"
  >
    <p v-if="targets > 0">
      {{ t('passwords.references.usage', { count: targets }, targets) }}
    </p>
    <label v-if="targets > 0" class="flex items-start gap-2">
      <ShadcnCheckbox
        v-model="inline"
        class="mt-0.5"
        data-testid="passwords-reference-inline"
      />
      <span>
        {{ t('passwords.references.inline') }}
        <span class="block text-muted-foreground">{{
          inline
            ? t('passwords.references.inlineOn')
            : t('passwords.references.inlineOff')
        }}</span>
      </span>
    </label>
    <p v-if="passkeyLinks > 0" class="text-muted-foreground">
      {{
        t(
          'passwords.references.passkeyLinks',
          { count: passkeyLinks },
          passkeyLinks,
        )
      }}
    </p>
  </div>
</template>
