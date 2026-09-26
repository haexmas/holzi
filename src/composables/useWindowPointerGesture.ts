import { clampDragPosition, clampResizeSize } from '~/lib/wm/geometry'
import type { Size } from '~/lib/wm/types'

export type ResizeDirection = 'n' | 's' | 'e' | 'w' | 'ne' | 'nw' | 'se' | 'sw'

type Geometry = { x: number; y: number; width: number; height: number }

/**
 * Pointer-driven move and eight-way resize for a window manager window (spec
 * 015-workspace-shell, T028, plan research R7). DOM/event wiring only —
 * the actual geometry math (clamping, minimum size) is `geometry.ts`'s
 * pure functions. Multiple pointermove events per frame collapse into one
 * `onUpdate` call via `requestAnimationFrame`.
 *
 * Callers attach `startMove`/`startResize` to `pointerdown` on the title
 * bar (drag) and each resize handle (`touch-action: none` on both is the
 * caller's responsibility, so the browser never starts a scroll gesture
 * mid-drag). Text selection is off for the whole page while a gesture
 * runs, so dragging across window content never selects it.
 */
export function useWindowPointerGesture(
  getGeometry: () => Geometry,
  getMinSize: () => Size,
  getArea: () => Size,
  onUpdate: (geometry: Geometry) => void,
) {
  function runGesture(
    startEvent: PointerEvent,
    compute: (deltaX: number, deltaY: number, start: Geometry) => Geometry,
  ) {
    const target = startEvent.currentTarget as HTMLElement
    const pointerId = startEvent.pointerId
    const start = getGeometry()
    const startX = startEvent.clientX
    const startY = startEvent.clientY
    let pending: Geometry | null = null
    let frameScheduled = false
    const root = document.documentElement.style
    const previousUserSelect = root.userSelect
    const previousWebkitUserSelect = root.webkitUserSelect

    function flush() {
      frameScheduled = false
      if (pending) onUpdate(pending)
    }

    function onMove(event: PointerEvent) {
      pending = compute(event.clientX - startX, event.clientY - startY, start)
      if (!frameScheduled) {
        frameScheduled = true
        requestAnimationFrame(flush)
      }
    }

    function stop() {
      target.removeEventListener('pointermove', onMove)
      target.removeEventListener('pointerup', stop)
      target.removeEventListener('pointercancel', stop)
      if (target.hasPointerCapture(pointerId)) {
        target.releasePointerCapture(pointerId)
      }
      root.userSelect = previousUserSelect
      root.webkitUserSelect = previousWebkitUserSelect
    }

    // WebKitGTK still honors only the prefixed property.
    root.userSelect = 'none'
    root.webkitUserSelect = 'none'
    window.getSelection()?.removeAllRanges()
    target.setPointerCapture(pointerId)
    target.addEventListener('pointermove', onMove)
    target.addEventListener('pointerup', stop)
    target.addEventListener('pointercancel', stop)
  }

  /** Moves the window by the pointer's delta, clamped so it stays reachable (FR-009). */
  function startMove(event: PointerEvent) {
    runGesture(event, (deltaX, deltaY, start) => {
      const area = getArea()
      const size = { width: start.width, height: start.height }
      const { x, y } = clampDragPosition(
        { x: start.x + deltaX, y: start.y + deltaY },
        size,
        area,
      )
      return { x, y, width: start.width, height: start.height }
    })
  }

  /** Resizes from one of eight directions, clamped to the app's minimum size (FR-009). North/west
   * edges move `x`/`y` as they shrink so the opposite edge stays anchored. */
  function startResize(event: PointerEvent, direction: ResizeDirection) {
    runGesture(event, (deltaX, deltaY, start) => {
      const minSize = getMinSize()
      const grows = { x: 0, y: 0, width: 0, height: 0 }
      if (direction.includes('e')) grows.width = deltaX
      if (direction.includes('s')) grows.height = deltaY
      if (direction.includes('w')) grows.width = -deltaX
      if (direction.includes('n')) grows.height = -deltaY

      const { width, height } = clampResizeSize(
        {
          width: start.width + grows.width,
          height: start.height + grows.height,
        },
        minSize,
      )
      const x = direction.includes('w')
        ? start.x + (start.width - width)
        : start.x
      const y = direction.includes('n')
        ? start.y + (start.height - height)
        : start.y
      return { x, y, width, height }
    })
  }

  return { startMove, startResize }
}
