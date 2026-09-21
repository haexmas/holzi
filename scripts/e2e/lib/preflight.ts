import { accessSync, constants, statSync } from 'node:fs'
import { delimiter, join } from 'node:path'

/** The tools the suite needs from the machine, by the name they are found under. */
export const REQUIRED_TOOLS = [
  'tauri-driver',
  'WebKitWebDriver',
  'xvfb-run',
] as const
export type ToolName = (typeof REQUIRED_TOOLS)[number]

export interface Tools {
  tauriDriver: string
  webKitWebDriver: string
  xvfbRun: string
}

export interface ResolvedTools {
  found: Partial<Record<ToolName, string>>
  missing: ToolName[]
}

function isExecutableFile(path: string): boolean {
  try {
    if (!statSync(path).isFile()) return false
    accessSync(path, constants.X_OK)
    return true
  } catch {
    return false
  }
}

/** Find each required tool on `PATH` without a shell. The path is the one found, not a resolved link. */
export function resolveTools(pathEnv: string | undefined): ResolvedTools {
  const directories = (pathEnv ?? '')
    .split(delimiter)
    .filter((entry) => entry !== '')
  const found: Partial<Record<ToolName, string>> = {}
  const missing: ToolName[] = []
  for (const name of REQUIRED_TOOLS) {
    const hit = directories.map((dir) => join(dir, name)).find(isExecutableFile)
    if (hit === undefined) missing.push(name)
    else found[name] = hit
  }
  return { found, missing }
}

/** The tools as the instance code names them. Only valid when none is missing. */
export function toTools(found: Partial<Record<ToolName, string>>): Tools {
  const need = (name: ToolName): string => {
    const path = found[name]
    if (path === undefined) throw new Error(`${name} is not available`)
    return path
  }
  return {
    tauriDriver: need('tauri-driver'),
    webKitWebDriver: need('WebKitWebDriver'),
    xvfbRun: need('xvfb-run'),
  }
}
