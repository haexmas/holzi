import type { FlowInstance } from './flows.ts'

// Helpers for the appearance scenarios (spec 035-appearance-and-fields). The theme variables are set
// as an inline style on `<html>` (`useAppearance`), so they are read from there.

/** The value of a theme variable as the appearance wrote it, e.g. `oklch(0.62 0.112 180)`. */
export async function theme(
  instance: FlowInstance,
  name: string,
): Promise<string> {
  return instance.exec<string>(
    `return document.documentElement.style.getPropertyValue(arguments[0])`,
    [name],
  )
}

/** The hue of a theme variable, or null when it is not set. */
export async function themeHue(
  instance: FlowInstance,
  name: string,
): Promise<number | null> {
  const match = /oklch\([\d.]+ [\d.]+ ([\d.]+)\)/.exec(
    await theme(instance, name),
  )
  return match ? Number(match[1]) : null
}

/** The stored appearance of the vault (the JSON the preference holds), or null. */
export async function storedAppearance(
  instance: FlowInstance,
): Promise<unknown> {
  const answer = (await instance.invoke('get_pref', {
    args: { scope: { kind: 'vault' }, key: 'appearance.theme' },
  })) as { ok: boolean; data?: string | null }
  return answer.ok && answer.data ? JSON.parse(answer.data) : null
}

/** Scrolls the control with this hook into the middle of its container (the window is small, so rows
 * below the fold cannot be clicked until they are on screen). */
export async function reveal(
  instance: FlowInstance,
  hook: string,
): Promise<void> {
  await instance.exec(
    `document.querySelector('[data-testid="' + arguments[0] + '"]')?.scrollIntoView({ block: 'center' })`,
    [hook],
  )
}

/** Whether no dialog is on screen any more: while its closing animation runs, the overlay still takes
 * the clicks that follow. */
export async function dialogClosed(instance: FlowInstance): Promise<boolean> {
  return instance.exec<boolean>(
    `return !document.querySelector('[role="alertdialog"], [role="dialog"]')`,
  )
}

/** The lightness (L) of a theme variable, or null when it is not set. */
export async function themeLightness(
  instance: FlowInstance,
  name: string,
): Promise<number | null> {
  const match = /oklch\(([\d.]+) /.exec(await theme(instance, name))
  return match ? Number(match[1]) : null
}

/** Whether the border of the window with this id has the colour of `--primary`: both are measured as
 * the engine computes them, through a probe with the same border, so the notation does not matter. */
export async function windowBorderIsAccent(
  instance: FlowInstance,
  windowId: string,
): Promise<boolean> {
  return instance.exec<boolean>(
    `const frame = document.querySelector('[data-wm-window-id="' + arguments[0] + '"]')
     const probe = document.createElement('div')
     probe.style.cssText = 'border: 1px solid var(--primary); position: absolute; visibility: hidden'
     document.body.append(probe)
     const accent = getComputedStyle(probe).borderTopColor
     probe.remove()
     return getComputedStyle(frame).borderTopColor === accent`,
    [windowId],
  )
}
