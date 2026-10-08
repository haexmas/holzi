<script setup lang="ts">
/**
 * The sidebar of the file browser (spec 044 FR-002): the known places of the device, then its
 * drives with free space where known. Storages (US5) join later.
 */
import type { Sources } from '@bindings/Sources'
import { formatFileSize } from '~/lib/passwords/format'

defineProps<{
  sources: Sources | null
  current: string | null
}>()

const emit = defineEmits<{ pick: [path: string] }>()

const { t } = useI18n()

const PLACE_ICONS: Record<string, string> = {
  home: 'lucide:house',
  pictures: 'lucide:image',
  downloads: 'lucide:download',
  documents: 'lucide:file-text',
  desktop: 'lucide:monitor',
  videos: 'lucide:film',
}
</script>

<template>
  <div class="flex h-full flex-col gap-4 overflow-y-auto p-2">
    <section v-if="sources?.known.length">
      <h2 class="px-2 pb-1 text-xs font-medium text-muted-foreground">
        {{ t('files.sidebar.places') }}
      </h2>
      <ul>
        <li v-for="place in sources.known" :key="place.path">
          <button
            type="button"
            class="flex w-full items-center gap-2 rounded px-2 py-1.5 text-left text-sm hover:bg-accent"
            :class="current === place.path ? 'bg-accent font-medium' : ''"
            :data-testid="`files-place-${place.name}`"
            @click="emit('pick', place.path)"
          >
            <Icon
              :name="PLACE_ICONS[place.name] ?? 'lucide:folder'"
              class="size-4 shrink-0"
            />
            <span class="truncate">{{ t(`files.places.${place.name}`) }}</span>
          </button>
        </li>
      </ul>
    </section>
    <section v-if="sources?.drives.length">
      <h2 class="px-2 pb-1 text-xs font-medium text-muted-foreground">
        {{ t('files.sidebar.drives') }}
      </h2>
      <ul>
        <li v-for="drive in sources.drives" :key="drive.path">
          <button
            type="button"
            class="flex w-full items-center gap-2 rounded px-2 py-1.5 text-left text-sm hover:bg-accent"
            :class="current === drive.path ? 'bg-accent font-medium' : ''"
            :data-testid="`files-drive-${drive.path}`"
            @click="emit('pick', drive.path)"
          >
            <Icon name="lucide:hard-drive" class="size-4 shrink-0" />
            <span class="min-w-0 flex-1 truncate">{{ drive.name }}</span>
            <span
              v-if="drive.free !== null"
              class="shrink-0 text-xs text-muted-foreground"
            >
              {{
                t('files.sidebar.free', { size: formatFileSize(drive.free) })
              }}
            </span>
          </button>
        </li>
      </ul>
    </section>
  </div>
</template>
