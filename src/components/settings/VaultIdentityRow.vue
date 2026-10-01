<script setup lang="ts">
/**
 * The vault's public identity (spec 024, FR-046): the key that names the vault in member lists and
 * rights of spaces and shares, as `npub`; not an address to invite the vault at. It can be copied
 * and not changed; the private key is shown nowhere (FR-002).
 */
import type { VaultPublicIdentity } from '@bindings/VaultPublicIdentity'

defineProps<{
  identity: VaultPublicIdentity
}>()

const { t } = useI18n()
const copied = ref(false)

async function onCopy(npub: string) {
  try {
    await navigator.clipboard.writeText(npub)
    copied.value = true
  } catch {
    copied.value = false
  }
}
</script>

<template>
  <SettingsGroup :label="t('settings.federation.identityLabel')">
    <SettingsRow
      icon="lucide:key-round"
      :title="t('settings.federation.identityTitle')"
      :description="t('settings.federation.identityDescription')"
    >
      <template #default>
        <UiButton
          variant="outline"
          data-testid="settings-identity-copy"
          @click="onCopy(identity.npub)"
        >
          {{
            copied
              ? t('settings.federation.identityCopied')
              : t('settings.federation.identityCopy')
          }}
        </UiButton>
      </template>
    </SettingsRow>
    <li class="px-4 py-3">
      <output
        class="block font-mono text-xs break-all select-all"
        data-testid="settings-identity-npub"
      >
        {{ identity.npub }}
      </output>
    </li>
  </SettingsGroup>
</template>
