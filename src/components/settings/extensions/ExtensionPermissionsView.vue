<script setup lang="ts">
/**
 * The permissions of one extension (spec 017, US3, T070): every remembered permission with kind,
 * action, target, state and where it holds ("alle Geräte" or the device's name), declared or not;
 * the state and, for device-scoped kinds, the scope change on selection; "Widerrufen" removes it.
 * Decisions held until holzi is closed are listed with "Entfernen".
 */
import { invoke } from '@tauri-apps/api/core'
import type { PermissionView } from '@bindings/PermissionView'
import type { SettingsSelectOption } from '~/components/settings/Select.vue'

const props = defineProps<{ extensionId: string }>()
const { t } = useI18n()
const { errString } = useErrorString()

const permissions = ref<PermissionView[]>([])
const failure = ref<string | null>(null)

const statusOptions = computed<SettingsSelectOption[]>(() =>
  ['granted', 'ask', 'denied'].map((value) => ({
    value,
    label: t(`settings.extensions.status.${value}`),
  })),
)

const DEVICE_SCOPED = new Set(['filesystem', 'shell'])

async function loadAsync() {
  try {
    permissions.value = await invoke<PermissionView[]>(
      'extension_permissions_list',
      { extensionId: props.extensionId },
    )
    failure.value = null
  } catch (error) {
    failure.value = errString(error)
  }
}

async function runAsync(action: () => Promise<unknown>) {
  try {
    await action()
  } catch (error) {
    failure.value = errString(error)
  }
  await loadAsync()
}

function setAsync(
  permission: PermissionView,
  status: string,
  allDevices: boolean,
) {
  return runAsync(() =>
    invoke('extension_permission_set', {
      args: {
        extensionId: props.extensionId,
        kind: permission.kind,
        action: permission.action,
        target: permission.target,
        status,
        allDevices,
        replaces: permission.id,
      },
    }),
  )
}

function removeAsync(permission: PermissionView) {
  return runAsync(() =>
    invoke('extension_permission_remove', {
      args: permission.temporary
        ? {
            extensionId: props.extensionId,
            temporaryKey: `${permission.kind}|${permission.action}|${permission.target}`,
          }
        : { extensionId: props.extensionId, permissionId: permission.id },
    }),
  )
}

function scopeText(permission: PermissionView): string {
  if (permission.temporary) return t('settings.extensions.temporary')
  return permission.allDevices
    ? t('settings.extensions.allDevices')
    : (permission.deviceName ?? t('settings.extensions.thisDevice'))
}

onMounted(loadAsync)
</script>

<template>
  <SettingsGroup :label="t('settings.extensions.permissions')">
    <li
      v-if="permissions.length === 0"
      class="px-4 py-3 text-sm text-muted-foreground"
    >
      {{ t('settings.extensions.noPermissions') }}
    </li>
    <li
      v-for="permission in permissions"
      :key="
        permission.id ??
        `${permission.kind}|${permission.action}|${permission.target}`
      "
      class="flex flex-wrap items-center gap-x-4 gap-y-2 px-4 py-3"
      :data-permission-target="permission.target"
    >
      <div class="flex min-w-48 flex-1 flex-col">
        <span class="text-sm">
          {{
            t(
              `extensions.permissions.kinds.${permission.kind}`,
              permission.kind,
            )
          }}
          · {{ permission.action }}
        </span>
        <span class="font-mono text-xs break-all">{{ permission.target }}</span>
        <span class="text-xs text-muted-foreground">
          {{ scopeText(permission) }}
          <template v-if="!permission.declared">
            · {{ t('settings.extensions.notDeclared') }}
          </template>
          <template v-if="permission.unreadable">
            · {{ t('settings.extensions.unreadable') }}
          </template>
        </span>
      </div>
      <SettingsSelect
        v-if="!permission.temporary && !permission.unreadable"
        :model-value="permission.status"
        :options="statusOptions"
        class="w-40"
        @update:model-value="
          setAsync(permission, $event, permission.allDevices)
        "
      />
      <span v-else-if="permission.temporary" class="text-sm">{{
        t(`settings.extensions.status.${permission.status}`)
      }}</span>
      <label
        v-if="DEVICE_SCOPED.has(permission.kind) && !permission.temporary"
        class="flex items-center gap-2 text-xs"
      >
        <ShadcnCheckbox
          :model-value="permission.allDevices"
          @update:model-value="
            setAsync(permission, permission.status, $event === true)
          "
        />
        {{ t('settings.extensions.allDevices') }}
      </label>
      <UiButton size="sm" variant="outline" @click="removeAsync(permission)">
        {{
          permission.temporary
            ? t('settings.extensions.remove')
            : t('settings.extensions.revoke')
        }}
      </UiButton>
    </li>
    <li v-if="failure" class="px-4 py-3 text-sm text-destructive" role="alert">
      {{ failure }}
    </li>
  </SettingsGroup>
</template>
