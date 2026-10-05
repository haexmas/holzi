// Part of `pnpm check:passwords` (spec 036, FR-041, research R14): the pixel size of an image read
// from its header (src/lib/passwords/imageSize.ts), so an image with too many pixels is never
// decoded for a thumbnail or the lightbox.
import assert from 'node:assert/strict'
import { test } from 'node:test'

import {
  imageSize,
  MAX_PREVIEW_PIXELS,
  previewableSize,
} from '../src/lib/passwords/imageSize.ts'

function bytes(...parts: (number[] | string)[]): Uint8Array {
  return Uint8Array.from(
    parts.flatMap((part) =>
      typeof part === 'string'
        ? Array.from(part, (char) => char.charCodeAt(0))
        : part,
    ),
  )
}

const be32 = (n: number) => [
  n >>> 24,
  (n >>> 16) & 255,
  (n >>> 8) & 255,
  n & 255,
]
const be16 = (n: number) => [n >>> 8, n & 255]
const le16 = (n: number) => [n & 255, n >>> 8]
const le24 = (n: number) => [n & 255, (n >>> 8) & 255, n >>> 16]

function png(width: number, height: number): Uint8Array {
  return bytes(
    [0x89],
    'PNG\r\n\x1a\n',
    be32(13),
    'IHDR',
    be32(width),
    be32(height),
    [8, 6, 0, 0, 0],
  )
}

test('a PNG header names width and height', () => {
  assert.deepEqual(imageSize(png(640, 480)), { width: 640, height: 480 })
})

test('a JPEG is read from its start-of-frame after other segments', () => {
  const jpeg = bytes(
    [0xff, 0xd8],
    [0xff, 0xe0],
    be16(16),
    'JFIF\0',
    [1, 1, 0, 0, 1, 0, 1, 0, 0],
    [0xff, 0xff, 0xc2],
    be16(17),
    [8],
    be16(300),
    be16(400),
    [3, 1, 0x22, 0, 2, 0x11, 1, 3, 0x11, 1],
  )
  assert.deepEqual(imageSize(jpeg), { width: 400, height: 300 })
})

test('a JPEG without a frame before the scan has no size', () => {
  assert.equal(imageSize(bytes([0xff, 0xd8, 0xff, 0xda], be16(2))), null)
})

test('a GIF header names width and height', () => {
  assert.deepEqual(imageSize(bytes('GIF89a', le16(32), le16(16), [0, 0, 0])), {
    width: 32,
    height: 16,
  })
})

test('the three WebP kinds are read', () => {
  const lossy = bytes(
    'RIFF',
    [0, 0, 0, 0],
    'WEBPVP8 ',
    [0, 0, 0, 0],
    [0, 0, 0],
    [0x9d, 0x01, 0x2a],
    le16(100),
    le16(50),
  )
  assert.deepEqual(imageSize(lossy), { width: 100, height: 50 })
  // VP8L packs width - 1 and height - 1 into 14 bits each.
  const w = 99
  const h = 49
  const lossless = bytes(
    'RIFF',
    [0, 0, 0, 0],
    'WEBPVP8L',
    [0, 0, 0, 0],
    [0x2f],
    [
      w & 0xff,
      ((w >> 8) & 0x3f) | ((h & 0x3) << 6),
      (h >> 2) & 0xff,
      (h >> 10) & 0xf,
    ],
  )
  assert.deepEqual(imageSize(lossless), { width: 100, height: 50 })
  const extended = bytes(
    'RIFF',
    [0, 0, 0, 0],
    'WEBPVP8X',
    [10, 0, 0, 0],
    [0, 0, 0, 0],
    le24(99),
    le24(49),
  )
  assert.deepEqual(imageSize(extended), { width: 100, height: 50 })
})

test('text, a cut header and a zero size have no size', () => {
  assert.equal(imageSize(bytes('this is no image')), null)
  assert.equal(imageSize(png(10, 10).slice(0, 20)), null)
  assert.equal(imageSize(png(0, 10)), null)
})

test('an image above the pixel limit is not previewable', () => {
  assert.ok(previewableSize(png(10_000, 10_000)))
  assert.equal(MAX_PREVIEW_PIXELS, 100_000_000)
  assert.equal(previewableSize(png(20_000, 20_000)), null)
  assert.equal(previewableSize(bytes('no image')), null)
})
