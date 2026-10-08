// Part of `pnpm check:files` (spec 044, T030): the window's viewer kinds match
// `src-tauri/src/files/kind_tests.rs` case for case.
import assert from 'node:assert/strict'
import { test } from 'node:test'

import { extensionOf, viewerKind } from '../src/lib/files/viewerKind.ts'

test('the extension is lower case and hidden names have none', () => {
  assert.equal(extensionOf('Brief.PDF'), 'pdf')
  assert.equal(extensionOf('archiv.tar.gz'), 'gz')
  assert.equal(extensionOf('.bashrc'), '')
  assert.equal(extensionOf('Makefile'), '')
})

test('viewer kinds follow the media type, as in Rust', () => {
  for (const [name, kind] of [
    ['notiz.txt', 'text'],
    ['README.md', 'text'],
    ['daten.json', 'text'],
    ['brief.pdf', 'pdf'],
    ['foto.JPG', 'image'],
    ['grafik.svg', 'image'],
    ['film.mp4', 'video'],
    ['film.mkv', 'video'],
    ['lied.mp3', 'audio'],
    ['lied.flac', 'audio'],
    ['handy.heic', 'info'],
    ['bericht.docx', 'info'],
    ['archiv.zip', 'info'],
    ['ohne-endung', 'info'],
  ] as const) {
    assert.equal(viewerKind(name), kind, name)
  }
})
