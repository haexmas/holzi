// A Nostr relay for scenarios with more than one application process (spec 024, T078): the
// `e2e_nostr_relay` binary, the same in-process relay the integration tests use, built with
// `--features e2e` and run as a process the instances share, so devices find each other without a
// network. The relay never outlives its scenario.
import { existsSync, readFileSync } from 'node:fs'
import { dirname, join, resolve } from 'node:path'
import { fileURLToPath } from 'node:url'
import { targetDirectory } from './build.ts'
import { spawnMarked, stopGroup } from './processes.ts'

const HERE = dirname(fileURLToPath(import.meta.url))
const REPO_ROOT = resolve(HERE, '..', '..', '..')
const URL_LINE = /stdout (ws:\/\/\S+)/

export interface NostrRelay {
  /** `ws://127.0.0.1:<port>`, what the instances are pointed at. */
  url: string
  stop(): Promise<void>
}

let built: Promise<string> | undefined

/** Builds the relay once per process and returns its path. */
export function buildNostrRelay(
  logFile: string,
  env: NodeJS.ProcessEnv = process.env,
): Promise<string> {
  built ??= (async () => {
    const binary = join(
      targetDirectory(REPO_ROOT, env),
      'debug',
      'e2e_nostr_relay',
    )
    const { child, closed } = spawnMarked(
      'cargo',
      [
        'build',
        '--manifest-path',
        join(REPO_ROOT, 'src-tauri', 'Cargo.toml'),
        '--features',
        'e2e',
        '--bin',
        'e2e_nostr_relay',
      ],
      { env: { ...env }, logFile, cwd: REPO_ROOT },
    )
    const code = await new Promise<number | null>((done) =>
      child.once('exit', (exitCode) => done(exitCode)),
    )
    await closed
    if (code !== 0 || !existsSync(binary)) {
      built = undefined
      throw new Error(
        `building the Nostr test relay failed with exit status ${code}; see ${logFile}`,
      )
    }
    return binary
  })()
  return built
}

/** Starts the relay and waits for the address it prints. */
export async function startNostrRelay(options: {
  logFile: string
  env?: NodeJS.ProcessEnv
  limitMs?: number
}): Promise<NostrRelay> {
  const env = options.env ?? process.env
  const binary = await buildNostrRelay(options.logFile, env)
  const { child } = spawnMarked(binary, [], {
    env: { ...env },
    logFile: options.logFile,
  })
  const end = Date.now() + (options.limitMs ?? 15_000)
  for (;;) {
    if (child.exitCode !== null) {
      throw new Error(`the Nostr test relay ended; see ${options.logFile}`)
    }
    const log = existsSync(options.logFile)
      ? readFileSync(options.logFile, 'utf8')
      : ''
    const match = [...log.matchAll(new RegExp(URL_LINE, 'g'))].at(-1)
    if (match?.[1] !== undefined) {
      return {
        url: match[1],
        stop: async () => {
          if (child.pid !== undefined) await stopGroup(child.pid)
        },
      }
    }
    if (Date.now() >= end) {
      if (child.pid !== undefined) await stopGroup(child.pid)
      throw new Error(
        `the Nostr test relay printed no address; see ${options.logFile}`,
      )
    }
    await new Promise((resolveWait) => setTimeout(resolveWait, 100))
  }
}
