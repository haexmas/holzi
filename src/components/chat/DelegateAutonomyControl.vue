<script setup lang="ts">
const { t } = useI18n()

const props = defineProps<{
  mode: 'standard' | 'ungated' | 'gated_permissive'
  disabled?: boolean
}>()

const autonomyModeLabel = computed(() => t(`chat.autonomy.${props.mode}`))

const emit = defineEmits<{
  'update:mode': [mode: 'standard' | 'ungated' | 'gated_permissive']
}>()
</script>

<template>
  <ChatComposerControl
    :label="t('chat.autonomy.modeLabel')"
    :value="mode"
    :display-value="autonomyModeLabel"
    icon="lucide:zap"
    control-id="autonomy-mode"
    :options="[
      { value: 'standard', label: t('chat.autonomy.standard') },
      { value: 'ungated', label: t('chat.autonomy.ungated') },
      {
        value: 'gated_permissive',
        label: t('chat.autonomy.gated_permissive'),
      },
    ]"
    :disabled="disabled"
    @update:value="
      emit('update:mode', $event as 'standard' | 'ungated' | 'gated_permissive')
    "
  />
</template>
