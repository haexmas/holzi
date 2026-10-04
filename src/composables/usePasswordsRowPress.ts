import type { Ref } from 'vue'

/**
 * Clicks and long presses on a row of the password manager's list (spec 036, FR-011, FR-018): a
 * click activates the row; a long press on a touch screen (VueUse `onLongPress`, 500 ms, with a
 * short buzz where the device has one) starts or extends the selection instead, and the click that
 * ends it is no activation. A touch press never opens the row's context menu (reka opens it on its
 * own long press, and Android fires `contextmenu`), so a long press only selects; the menu button
 * of the row offers the menu there.
 */
export function usePasswordsRowPress(
  target: Readonly<Ref<HTMLElement | null>>,
  handlers: {
    activate: (event: MouseEvent) => void
    longPress: () => void
  },
) {
  let pressed = false
  let touch = false

  onLongPress(
    target,
    (event) => {
      if (event.pointerType !== 'touch') return
      pressed = true
      navigator.vibrate?.(15)
      handlers.longPress()
    },
    { delay: 500 },
  )

  function onPointerdown(event: PointerEvent) {
    touch = event.pointerType === 'touch'
    pressed = false
    // Keeps reka from starting the long-press timer of the context menu.
    if (touch) event.preventDefault()
  }

  function onClick(event: MouseEvent) {
    if (pressed) {
      pressed = false
      return
    }
    handlers.activate(event)
  }

  function onContextmenu(event: MouseEvent) {
    if (touch) event.preventDefault()
  }

  return { onPointerdown, onClick, onContextmenu }
}
