// The media files of the file browser scenarios (spec 044, T040), read from the repository; how they
// were made: src-tauri/tests/fixtures/files/README.md.
import { readFileSync } from 'node:fs'

/** The bytes of `src-tauri/tests/fixtures/files/<name>`. */
export function fileFixture(name: string): Buffer {
  return readFileSync(
    new URL(`../../../src-tauri/tests/fixtures/files/${name}`, import.meta.url),
  )
}
