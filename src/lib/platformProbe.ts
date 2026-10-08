/**
 * The window's half of the CI's platform probe (spec 044, T007, research R4). Rust sets
 * `window.__HOLZI_PROBE__` only in builds with the feature `platform-probe` and only when the run
 * asked for it, so in every other window this does nothing.
 */

export interface ProbeStep {
  name: string
  ok: boolean
  detail?: string
}

export interface ProbeReport {
  steps: ProbeStep[]
  userAgent: string
}

/** The answer of holzi's health route (`platform_probe::HEALTH_BODY`). */
export const HEALTH_BODY = 'ok'

/**
 * Checks that the web view can fetch from the loopback server. A fetch the web view blocks (policy
 * or Local Network Access) rejects; its message becomes the step's detail.
 */
export async function runPlatformProbe(
  port: number,
  fetchFn: typeof fetch,
  userAgent: string,
): Promise<ProbeReport> {
  const steps: ProbeStep[] = [{ name: 'webview', ok: true }]
  try {
    const response = await fetchFn(`http://127.0.0.1:${port}/__probe`, {
      cache: 'no-store',
    })
    const body = await response.text()
    const ok = response.ok && body === HEALTH_BODY
    steps.push({
      name: 'loopback-fetch',
      ok,
      detail: ok
        ? undefined
        : `status ${response.status}, body ${JSON.stringify(body)}`,
    })
  } catch (error) {
    steps.push({ name: 'loopback-fetch', ok: false, detail: String(error) })
  }
  return { steps, userAgent }
}
