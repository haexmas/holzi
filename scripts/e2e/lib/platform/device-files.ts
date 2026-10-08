// Files the app reads or writes during a scenario, in a folder the app can reach (spec 043, contract
// picked-file.md, test seam): on Linux a temporary folder of this machine, on Android a folder in the
// app's own storage, written and read through `run-as` (debug builds only). Chromedriver cannot work
// the system file picker, so scenarios hand the app such a path where a user would pick a file.
import { mkdtempSync, readFileSync, rmSync, writeFileSync } from 'node:fs'
import { tmpdir } from 'node:os'
import { basename, dirname, join } from 'node:path'
import { randomBytes } from 'node:crypto'
import { createAdb } from './adb.ts'
import type { Adb } from './adb.ts'
import { PACKAGE } from './android.ts'

export interface DeviceFiles {
  folder: string
  path: (name: string) => string
  write: (name: string, content: string | Uint8Array) => void
  read: (name: string) => string
  readBytes: (name: string) => Buffer
  remove: () => void
}

/**
 * The app's data directory on Android, by the path under which the app sees files that `run-as`
 * writes. The app reports its directory as `/data/user/0/<package>` and sees its own files there,
 * but on the API 35 emulator a file `run-as` created was visible to the app only under
 * `/data/data/<package>` (found 2026-10-07: `std::fs::metadata` failed for the one path and worked
 * for the other).
 */
const ANDROID_DATA = `/data/data/${PACKAGE}`

function androidAdb(env: NodeJS.ProcessEnv): Adb | undefined {
  const serial = env.ANDROID_SERIAL
  return env.E2E_PLATFORM === 'android' && serial !== undefined && serial !== ''
    ? createAdb(serial)
    : undefined
}

const quote = (text: string) => `'${text.replaceAll("'", "'\\''")}'`

export function deviceFiles(
  prefix: string,
  env: NodeJS.ProcessEnv = process.env,
): DeviceFiles {
  const adb = androidAdb(env)
  if (adb === undefined) {
    const folder = mkdtempSync(join(tmpdir(), prefix))
    const path = (name: string) => join(folder, name)
    return {
      folder,
      path,
      write: (name, content) => writeFileSync(path(name), content),
      read: (name) => readFileSync(path(name), 'utf8'),
      readBytes: (name) => readFileSync(path(name)),
      remove: () => rmSync(folder, { recursive: true, force: true }),
    }
  }
  const folder = `${ANDROID_DATA}/files/${prefix}${randomBytes(4).toString('hex')}`
  const path = (name: string) => `${folder}/${name}`
  return {
    folder,
    path,
    // Through /data/local/tmp: `adb exec-in` with `run-as` lost the input of larger files (a 2 kB
    // bundle arrived empty), `adb push` plus a copy as the app does not.
    write: (name, content) => {
      const local = join(
        mkdtempSync(join(tmpdir(), 'holzi-e2e-push-')),
        'content',
      )
      const remote = `/data/local/tmp/holzi-e2e-${randomBytes(6).toString('hex')}`
      try {
        writeFileSync(local, content)
        adb.push(local, remote)
        adb.runAs(
          PACKAGE,
          `mkdir -p ${quote(folder)} && cp ${quote(remote)} ${quote(path(name))}`,
        )
      } finally {
        rmSync(dirname(local), { recursive: true, force: true })
        adb.shell('rm', '-f', remote)
      }
    },
    read: (name) => adb.runAs(PACKAGE, `cat ${quote(path(name))}`).toString(),
    readBytes: (name) => adb.runAs(PACKAGE, `cat ${quote(path(name))}`),
    remove: () => void adb.runAs(PACKAGE, `rm -rf ${quote(folder)}`),
  }
}

/**
 * A file of this machine at a path the app can read: the same path on Linux, a copy in the app's
 * storage on Android (it stays until the app's data is cleared with the next scenario).
 */
export function onDevice(
  hostPath: string,
  env: NodeJS.ProcessEnv = process.env,
): string {
  if (androidAdb(env) === undefined) return hostPath
  const files = deviceFiles('e2e-staged-', env)
  files.write(basename(hostPath), readFileSync(hostPath))
  return files.path(basename(hostPath))
}
