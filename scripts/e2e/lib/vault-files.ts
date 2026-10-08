// The vault files of an instance in the app's storage (spec 043, scenario vault-file-import): read
// out to where a person would pick them, and removed as if the vault had gone to another device. On
// Linux the files lie below the instance's data root, on Android in the app's data directory,
// reached through `run-as` (debug builds only).
import { readFileSync, readdirSync, rmSync } from 'node:fs'
import { join } from 'node:path'
import { createAdb } from './platform/adb.ts'
import { PACKAGE } from './platform/android.ts'
import { VAULT_DIR } from './platform/linux.ts'

const ANDROID_PREFIX = 'android:'

export interface VaultFiles {
  /** The bytes of `<vault>.db`; the app must have ended. */
  read: (vault: string) => Buffer
  /** Removes `<vault>.db` and every file beside it that belongs to it. */
  remove: (vault: string) => void
}

export function vaultFiles(root: string): VaultFiles {
  if (root.startsWith(ANDROID_PREFIX)) {
    const adb = createAdb(root.slice(ANDROID_PREFIX.length))
    // Relative to the app's data directory, where `run-as` starts.
    return {
      read: (vault) => adb.runAs(PACKAGE, `cat instances/${vault}.db`),
      remove: (vault) =>
        void adb.runAs(
          PACKAGE,
          `rm -f instances/${vault}.db instances/${vault}.db?*`,
        ),
    }
  }
  const dir = join(root, ...VAULT_DIR)
  return {
    read: (vault) => readFileSync(join(dir, `${vault}.db`)),
    remove: (vault) => {
      for (const file of readdirSync(dir)) {
        if (
          file === `${vault}.db` ||
          file.startsWith(`${vault}.db.`) ||
          file.startsWith(`${vault}.db-`)
        )
          rmSync(join(dir, file), { force: true })
      }
    },
  }
}
