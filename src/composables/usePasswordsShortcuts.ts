import type { InjectionKey, Ref } from 'vue'
import { detectPlatform } from '~/lib/wm/keybindings'
import {
  resolveListShortcut,
  type ListCommand,
} from '~/lib/passwords/shortcuts'

/** Runs a list command; `true` when it did something (the key is then consumed). */
export type ListCommandHandler = (command: ListCommand) => boolean

const LIST_COMMANDS: InjectionKey<Ref<ListCommandHandler | null>> = Symbol(
  'passwordsListCommands',
)

/**
 * The keyboard shortcuts of the password manager (spec 036, FR-016, research R10): the frame
 * (`PasswordsApp.vue`) listens on its root, so they only act with the focus in its window, and hands
 * the command to the list while one shows; the editor, the entry and the other places register
 * nothing, so there the keys do what they always do. The window manager's chords are taken in the
 * capture phase before this (`useWmKeyboard`), and the resolver never claims them.
 */
export function providePasswordsShortcuts() {
  const handler = ref<ListCommandHandler | null>(null)
  provide(LIST_COMMANDS, handler)
  const platform = detectPlatform(
    navigator as Navigator & { userAgentData?: { platform?: string } },
  )

  function onKeydown(event: KeyboardEvent) {
    if (event.defaultPrevented || !handler.value) return
    const target = event.target instanceof Element ? event.target : null
    const command = resolveListShortcut({
      key: event.key,
      ctrlKey: event.ctrlKey,
      metaKey: event.metaKey,
      shiftKey: event.shiftKey,
      altKey: event.altKey,
      target,
      platform,
      inList: Boolean(target?.closest('[data-passwords-list]')),
      inDialog: Boolean(
        target?.closest('[role="dialog"], [role="alertdialog"], [role="menu"]'),
      ),
      textSelected: (window.getSelection()?.toString() ?? '') !== '',
    })
    if (command && handler.value(command)) event.preventDefault()
  }

  return { onKeydown }
}

/** Lets the list take the commands while it is mounted. */
export function useListCommands(handler: ListCommandHandler) {
  const slot = inject(LIST_COMMANDS, null)
  onMounted(() => {
    if (slot) slot.value = handler
  })
  onBeforeUnmount(() => {
    if (slot?.value === handler) slot.value = null
  })
}
