// The device side of the files scene (spec 017, US9): a folder of the device the probe reads,
// writes and watches, outside holzi's own places.
import { mkdtempSync, readFileSync, rmSync, writeFileSync } from 'node:fs'
import { tmpdir } from 'node:os'
import { join } from 'node:path'

/** A fresh folder of the device with helpers to name, write and read files in it. */
export function deviceFiles(prefix = 'holzi-ext-files-'): {
  folder: string
  path: (name: string) => string
  write: (name: string, text: string) => void
  read: (name: string) => string
  remove: () => void
} {
  const folder = mkdtempSync(join(tmpdir(), prefix))
  const path = (name: string) => join(folder, name)
  return {
    folder,
    path,
    write: (name, text) => writeFileSync(path(name), text),
    read: (name) => readFileSync(path(name), 'utf8'),
    remove: () => rmSync(folder, { recursive: true, force: true }),
  }
}
