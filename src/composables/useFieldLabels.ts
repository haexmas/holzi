import { computed } from 'vue'

/**
 * Translated texts of the helper buttons of haex-ui's fields (spec 035-appearance-and-fields,
 * FR-009). haex-ui ships German defaults only, so every field passes these: `input` goes to
 * `UiInput` (`clearable`, `copyable`), `password` to `UiInputPassword` (show, hide, copy).
 */
export function useFieldLabels() {
  const { t } = useI18n()
  const input = computed(() => ({
    copy: t('fields.copy'),
    copied: t('fields.copied'),
    clear: t('fields.clear'),
  }))
  const password = computed(() => ({
    show: t('fields.show'),
    hide: t('fields.hide'),
    copy: t('fields.copy'),
    copied: t('fields.copied'),
  }))
  return { input, password }
}
