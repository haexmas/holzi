import { invoke } from '@tauri-apps/api/core'

import { runPlatformProbe } from '~/lib/platformProbe'

declare global {
  interface Window {
    __HOLZI_PROBE__?: { port: number }
  }
}

/**
 * The CI's platform probe (spec 044, T007): runs only when Rust handed over a port, and reports
 * through IPC, which no network rule can block.
 */
export default defineNuxtPlugin(() => {
  const probe = window.__HOLZI_PROBE__
  if (!probe) return
  void runPlatformProbe(
    probe.port,
    window.fetch.bind(window),
    navigator.userAgent,
  ).then((report) => invoke('platform_probe_report', { report }))
})
