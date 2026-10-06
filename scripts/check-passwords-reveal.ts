// Part of `pnpm check:passwords`: the reveal control keeps a mouse hold alive when the value
// changes the surrounding responsive layout.
import assert from 'node:assert/strict'
import { test } from 'node:test'

import { loadScriptSetup } from './lib/script-setup-sandbox.ts'

type PointerTarget = {
  captured: boolean
  setPointerCapture: (pointerId: number) => void
  hasPointerCapture: (pointerId: number) => boolean
  releasePointerCapture: (pointerId: number) => void
}

type PointerEventLike = {
  pointerId: number
  pointerType: 'mouse'
  currentTarget: PointerTarget
  type: 'pointerdown' | 'pointerleave' | 'pointerup'
}

function pointerEvent(
  target: PointerTarget,
  type: 'pointerdown' | 'pointerleave' | 'pointerup',
): PointerEventLike {
  return {
    pointerId: 1,
    pointerType: 'mouse',
    currentTarget: target,
    type,
  }
}

test('a responsive reflow does not hide a mouse-held revealed value', async () => {
  const revealed = 'a-password-long-enough-to-change-the-row-layout'
  const component = loadScriptSetup<{
    value: { value: string | null }
    onPointerDown: (event: PointerEventLike) => void
    onPointerEnd: (event: PointerEventLike) => void
  }>(
    'src/components/passwords/MaskedValue.vue',
    ['value', 'onPointerDown', 'onPointerEnd'],
    {
      props: {
        fetch: async () => revealed,
        identity: 'entry:password',
        present: true,
        label: 'Password',
        kind: 'password',
      },
      useErrorString: () => ({ errString: () => 'error' }),
    },
  )
  const target: PointerTarget = {
    captured: false,
    setPointerCapture() {
      this.captured = true
    },
    hasPointerCapture() {
      return this.captured
    },
    releasePointerCapture() {
      this.captured = false
    },
  }

  component.onPointerDown(pointerEvent(target, 'pointerdown'))
  await Promise.resolve()
  await Promise.resolve()
  assert.equal(component.value.value, revealed)

  component.onPointerEnd(pointerEvent(target, 'pointerleave'))
  assert.equal(component.value.value, revealed)

  component.onPointerEnd(pointerEvent(target, 'pointerup'))
  assert.equal(component.value.value, null)
})
