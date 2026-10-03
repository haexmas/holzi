<script setup lang="ts">
/**
 * "Importieren" and "Exportieren" of the appearance (spec 035-appearance-and-fields, FR-021,
 * contracts/appearance-file.md): the file dialog and the file access happen here, the checking and
 * the writing in the actions `settings.appearance.export` and `.import`. A file is read only after
 * the user has chosen it, and never larger than 16 KiB.
 */
import { open, save } from '@tauri-apps/plugin-dialog'
import { readTextFile, stat, writeTextFile } from '@tauri-apps/plugin-fs'
import { FILE_MAX_BYTES } from '~/lib/appearance/schema'

const emit = defineEmits<{
  done: [message: string]
  failed: [message: string]
}>()

const { t } = useI18n()
const { errString } = useErrorString()
const exportAction = useActionOrThrow('settings.appearance.export')
const importAction = useActionOrThrow('settings.appearance.import')
const busy = ref(false)

const FILTERS = [{ name: 'holzi appearance', extensions: ['json'] }]

async function exportAsync() {
  busy.value = true
  try {
    const path = await save({
      defaultPath: 'holzi.holzi-appearance.json',
      filters: FILTERS,
    })
    if (!path) return
    const { file } = (await exportAction()) as { file: string }
    await writeTextFile(path, file)
    emit('done', t('settings.appearance.exported'))
  } catch (error: unknown) {
    emit('failed', errString(error))
  } finally {
    busy.value = false
  }
}

async function importAsync() {
  busy.value = true
  try {
    const path = await open({ multiple: false, filters: FILTERS })
    if (typeof path !== 'string') return
    const info = await stat(path)
    if (info.size > FILE_MAX_BYTES) {
      throw {
        kind: 'AppearanceError',
        reason: 'settings.appearance.import.notJson',
      }
    }
    await importAction({ file: await readTextFile(path) })
    emit('done', t('settings.appearance.imported'))
  } catch (error: unknown) {
    emit('failed', errString(error))
  } finally {
    busy.value = false
  }
}
</script>

<template>
  <div class="flex flex-wrap items-center gap-2">
    <UiButton
      variant="outline"
      size="sm"
      :disabled="busy"
      data-testid="appearance-import"
      @click="importAsync"
    >
      {{ t('settings.appearance.importFile') }}
    </UiButton>
    <UiButton
      variant="outline"
      size="sm"
      :disabled="busy"
      data-testid="appearance-export"
      @click="exportAsync"
    >
      {{ t('settings.appearance.exportFile') }}
    </UiButton>
  </div>
</template>
