// The scan behind the seam check (spec 033, SC-005): which lines of a scenario or a scenario-facing
// helper name a platform. Text based, on purpose: it needs no parser and fails loudly on a new import.
import { dirname, join, normalize } from 'node:path'

const FORBIDDEN: Array<{ pattern: RegExp; what: string }> = [
  {
    pattern: /from\s+'node:(?:child_process|os|fs)'/,
    what: 'imports node:child_process, node:os or node:fs',
  },
  { pattern: /process\.kill\b/, what: 'calls process.kill' },
  { pattern: /xvfb/i, what: 'names xvfb' },
  { pattern: /tauri-driver/, what: 'names tauri-driver' },
  { pattern: /\/proc\b/, what: 'names /proc' },
]

/** The code lines of a source file: comments and blank lines left out. */
function codeLines(source: string): Array<{ number: number; text: string }> {
  let inBlock = false
  const lines: Array<{ number: number; text: string }> = []
  source.split('\n').forEach((raw, index) => {
    let text = raw
    if (inBlock) {
      const end = text.indexOf('*/')
      if (end === -1) return
      inBlock = false
      text = text.slice(end + 2)
    }
    text = text.replace(/\/\*.*?\*\//g, '')
    const start = text.indexOf('/*')
    if (start !== -1) {
      inBlock = true
      text = text.slice(0, start)
    }
    text = text.replace(/(^|\s)\/\/.*$/, '$1').trim()
    if (text !== '') lines.push({ number: index + 1, text })
  })
  return lines
}

/** What a source names of a platform, one entry per offending line. */
export function findSeamViolations(source: string, filePath: string): string[] {
  const found: string[] = []
  for (const { number, text } of codeLines(source)) {
    const platformImport = text.match(/from\s+'(\.\.?\/[^']+)'/)
    if (platformImport !== null) {
      const resolved = normalize(join(dirname(filePath), platformImport[1]))
      if (
        [
          'lib/instance.ts',
          'lib/processes.ts',
          'lib/webdriver.ts',
          'lib/build.ts',
        ].includes(resolved)
      ) {
        found.push(
          `line ${number} imports a platform file (instance, processes, webdriver or build): ${text}`,
        )
      }
    }
    for (const { pattern, what } of FORBIDDEN) {
      if (pattern.test(text)) found.push(`line ${number} ${what}: ${text}`)
    }
  }
  return found
}
