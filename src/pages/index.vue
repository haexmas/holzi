<script setup lang="ts">
const { t } = useI18n()
</script>

<template>
  <main class="min-h-screen flex flex-col items-center justify-center gap-4 p-8">
    <h1 class="text-3xl font-semibold">
      {{ t('landing.welcome') }}
    </h1>
    <p class="text-sm text-muted-foreground">
      {{ t('landing.lastUsed') }}
    </p>
    <UiButton>
      {{ t('onboarding.create.submit') }}
    </UiButton>
  </main>
</template>
