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

/** What the page knows about itself; it explains a blocked fetch (policy or network access). */
export interface ProbeContext {
  origin: string
  secure: boolean
  /** Content-Security-Policy violations seen so far, as `directive blocked-uri`. */
  violations: () => string[]
}

/**
 * Checks that the web view can fetch from the loopback server. A fetch the web view blocks (policy
 * or Local/Private Network Access) rejects; its message and any policy violations become the
 * step's detail.
 */
export async function runPlatformProbe(
  port: number,
  fetchFn: typeof fetch,
  userAgent: string,
  context?: ProbeContext,
): Promise<ProbeReport> {
  const steps: ProbeStep[] = [{ name: 'webview', ok: true }]
  if (context) {
    steps.push({
      name: 'context',
      ok: true,
      detail: `origin ${context.origin}, secure context ${context.secure}`,
    })
  }
  const blocked = () => {
    const seen = context?.violations() ?? []
    return seen.length ? `; policy violations: ${seen.join(', ')}` : ''
  }
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
        : `status ${response.status}, body ${JSON.stringify(body)}${blocked()}`,
    })
  } catch (error) {
    steps.push({
      name: 'loopback-fetch',
      ok: false,
      detail: `${String(error)}${blocked()}`,
    })
  }
  return { steps, userAgent }
}
