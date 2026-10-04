// The console output a development version forwards (spec 017, US12, FR-045): the SDK's
// `console.forward` message (`polyfills/consoleForwarding.ts`) from the frame's own window. Pure:
// `scripts/check-extensions-apps.ts` tests it.

/** Lines kept per tab; older ones go. */
export const DEV_CONSOLE_LINES = 500

export type DevConsoleLevel = 'log' | 'info' | 'warn' | 'error' | 'debug'

export interface DevConsoleLine {
  level: DevConsoleLevel
  message: string
  /** As the SDK formats it, the extension's local time. */
  time: string
}

const LEVELS = new Set<string>(['log', 'info', 'warn', 'error', 'debug'])

/** The line a `console.forward` message carries, or `null` for anything else. */
export function readConsoleForward(data: unknown): DevConsoleLine | null {
  if (typeof data !== 'object' || data === null) return null
  const message = data as { type?: unknown; data?: unknown }
  if (message.type !== 'console.forward') return null
  if (typeof message.data !== 'object' || message.data === null) return null
  const line = message.data as {
    level?: unknown
    message?: unknown
    timestamp?: unknown
  }
  if (typeof line.level !== 'string' || !LEVELS.has(line.level)) return null
  if (typeof line.message !== 'string') return null
  return {
    level: line.level as DevConsoleLevel,
    message: line.message.slice(0, 16 * 1024),
    time: typeof line.timestamp === 'string' ? line.timestamp : '',
  }
}
