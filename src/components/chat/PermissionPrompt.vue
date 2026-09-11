<script setup lang="ts">
import type { RiskClass } from '~/composables/useChat'

export interface PendingApproval {
  requestId: string
  toolName: string
  toolInput: unknown
  riskClass: RiskClass
}

const { t } = useI18n()

defineProps<{
  mode: 'manual' | 'auto' | 'plan'
  /** Oldest-first queue — only the first is shown; more than one can be
   * pending at once (spec.md Edge Case: independent tool calls each get
   * their own request). */
  pendingApprovals: PendingApproval[]
}>()

const emit = defineEmits<{
  'update:mode': [mode: 'manual' | 'auto' | 'plan']
  allow: [requestId: string]
  deny: [requestId: string]
}>()
</script>

<template>
  <div class="flex items-center gap-2">
    <label class="text-xs text-muted-foreground">{{ t('chat.permission.modeLabel') }}</label>
    <select
      class="text-xs bg-background border border-border rounded px-2 py-1"
      :value="mode"
      @change="emit('update:mode', ($event.target as HTMLSelectElement).value as 'manual' | 'auto' | 'plan')"
    >
      <option value="manual">
        {{ t('chat.permission.manual') }}
      </option>
      <option value="auto">
        {{ t('chat.permission.auto') }}
      </option>
      <option value="plan">
        {{ t('chat.permission.plan') }}
      </option>
    </select>
  </div>

  <div
    v-if="pendingApprovals[0]"
    class="fixed inset-0 bg-black/40 flex items-center justify-center z-50"
  >
    <div class="bg-background border border-border rounded-lg p-4 max-w-md w-full space-y-3">
      <div class="text-sm font-semibold">
        {{ t('chat.permission.requestTitle', { name: pendingApprovals[0].toolName }) }}
      </div>
      <div
        class="text-xs"
        :class="pendingApprovals[0].riskClass === 'risky' ? 'text-destructive' : 'text-muted-foreground'"
      >
        {{ pendingApprovals[0].riskClass === 'risky' ? t('chat.permission.risky') : t('chat.permission.safe') }}
      </div>
      <pre class="text-xs bg-muted/30 rounded p-2 overflow-x-auto whitespace-pre-wrap">{{ JSON.stringify(pendingApprovals[0].toolInput, null, 2) }}</pre>
      <div class="flex justify-end gap-2">
        <UiButton size="sm" variant="outline" @click="emit('deny', pendingApprovals[0].requestId)">
          {{ t('chat.permission.deny') }}
        </UiButton>
        <UiButton size="sm" @click="emit('allow', pendingApprovals[0].requestId)">
          {{ t('chat.permission.allow') }}
        </UiButton>
      </div>
    </div>
  </div>
</template>
