<script setup lang="ts">
/**
 * Category sidebar of the settings (spec 023-settings-app, FR-001, FR-004, FR-010). A flat list
 * in registry order; the category of the current location is highlighted. A click always pushes
 * the category's own location, so a category entered from the sidebar starts at its overview.
 * Below the `@2xl` container width it shrinks to icons with the name as tooltip. The tooltip
 * content is portalled out of the container, so whether it is needed comes from the measured
 * width of the sidebar rather than a container query.
 */
import { categoryOf, SETTINGS_CATEGORIES } from '~/lib/settings/registry'

const { t } = useI18n()
const router = useTabRouter()

const active = computed(() => categoryOf(router.route.path))

const nav = useTemplateRef<HTMLElement>('nav')
const { width } = useElementSize(nav)
/** Icons only: the rail is 3.5rem wide, the full sidebar 16rem. */
const iconsOnly = computed(() => width.value > 0 && width.value < 128)
</script>

<template>
  <ShadcnTooltipProvider :delay-duration="300">
    <nav
      ref="nav"
      class="flex w-14 shrink-0 flex-col gap-1 border-r border-border p-2 @2xl:w-64"
      :aria-label="t('wm.apps.settings')"
    >
      <ShadcnTooltip
        v-for="category in SETTINGS_CATEGORIES"
        :key="category.id"
        :disabled="!iconsOnly"
      >
        <ShadcnTooltipTrigger as-child>
          <button
            type="button"
            class="flex items-center justify-center gap-3 rounded-md px-2 py-2 text-sm hover:bg-accent hover:text-accent-foreground @2xl:justify-start"
            :class="
              active === category.id
                ? 'bg-primary text-primary-foreground hover:bg-primary hover:text-primary-foreground'
                : ''
            "
            :aria-current="active === category.id ? 'page' : undefined"
            :aria-label="t(category.titleKey)"
            :data-testid="`settings-category-${category.id}`"
            @click="router.push(category.path)"
          >
            <Icon :name="category.icon" class="size-5 shrink-0" />
            <span class="hidden truncate @2xl:inline">
              {{ t(category.titleKey) }}
            </span>
          </button>
        </ShadcnTooltipTrigger>
        <ShadcnTooltipContent side="right">
          {{ t(category.titleKey) }}
        </ShadcnTooltipContent>
      </ShadcnTooltip>
    </nav>
  </ShadcnTooltipProvider>
</template>
