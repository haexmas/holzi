import { describe, it } from 'node:test'
import assert from 'node:assert/strict'
import { isPainted, readFramebuffer } from './framebuffer.ts'

const FIXED_HEADER_SIZE = 100
const COLOR_ENTRY_SIZE = 12

/** Builds one synthetic XWD buffer: the 25-field big-endian header, a window name, a colormap of
 * `colormapEntries` zeroed entries, then `height` rows of `bytesPerLine` bytes, filled by `fillPixels`. */
function buildXwd(options: {
  width: number
  height: number
  bitsPerPixel: number
  bytesPerLine: number
  colormapEntries: number
  name?: string
  fillPixels?: (pixels: Buffer) => void
}): Buffer {
  const name = options.name ?? 'spike'
  const headerSize = FIXED_HEADER_SIZE + name.length + 1
  const header = Buffer.alloc(headerSize)
  header.writeUInt32BE(headerSize, 0)
  header.writeUInt32BE(7, 4) // file_version
  header.writeUInt32BE(2, 8) // pixmap_format (ZPixmap)
  header.writeUInt32BE(24, 12) // pixmap_depth
  header.writeUInt32BE(options.width, 16)
  header.writeUInt32BE(options.height, 20)
  header.writeUInt32BE(0, 24) // xoffset
  header.writeUInt32BE(0, 28) // byte_order
  header.writeUInt32BE(32, 32) // bitmap_unit
  header.writeUInt32BE(0, 36) // bitmap_bit_order
  header.writeUInt32BE(32, 40) // bitmap_pad
  header.writeUInt32BE(options.bitsPerPixel, 44)
  header.writeUInt32BE(options.bytesPerLine, 48)
  header.writeUInt32BE(4, 52) // visual_class (TrueColor)
  header.writeUInt32BE(0x00ff0000, 56) // red_mask
  header.writeUInt32BE(0x0000ff00, 60) // green_mask
  header.writeUInt32BE(0x000000ff, 64) // blue_mask
  header.writeUInt32BE(8, 68) // bits_per_rgb
  header.writeUInt32BE(options.colormapEntries, 72)
  header.writeUInt32BE(options.colormapEntries, 76) // ncolors
  header.writeUInt32BE(options.width, 80) // window_width
  header.writeUInt32BE(options.height, 84) // window_height
  header.writeUInt32BE(0, 88) // window_x
  header.writeUInt32BE(0, 92) // window_y
  header.writeUInt32BE(0, 96) // window_bdrwidth
  header.write(name, FIXED_HEADER_SIZE, 'ascii')

  const colormap = Buffer.alloc(options.colormapEntries * COLOR_ENTRY_SIZE)
  const pixels = Buffer.alloc(options.bytesPerLine * options.height)
  options.fillPixels?.(pixels)

  return Buffer.concat([header, colormap, pixels])
}

describe('readFramebuffer', () => {
  it('parses the header fields the suite needs', () => {
    const buffer = buildXwd({
      width: 4,
      height: 2,
      bitsPerPixel: 32,
      bytesPerLine: 16,
      colormapEntries: 3,
    })
    const fb = readFramebuffer(buffer)
    assert.equal(fb.header.width, 4)
    assert.equal(fb.header.height, 2)
    assert.equal(fb.header.bitsPerPixel, 32)
    assert.equal(fb.header.bytesPerLine, 16)
    assert.equal(fb.header.colormapEntries, 3)
    assert.equal(fb.header.headerSize, FIXED_HEADER_SIZE + 'spike'.length + 1)
  })

  it('slices out exactly the pixel data, past the header and the colormap', () => {
    const buffer = buildXwd({
      width: 2,
      height: 2,
      bitsPerPixel: 32,
      bytesPerLine: 8,
      colormapEntries: 5,
      fillPixels: (pixels) => pixels.fill(0xab),
    })
    const fb = readFramebuffer(buffer)
    assert.equal(fb.pixels.length, 8 * 2)
    assert.ok(fb.pixels.every((byte) => byte === 0xab))
  })

  it('rejects a file shorter than the fixed 100-byte header', () => {
    assert.throws(
      () => readFramebuffer(Buffer.alloc(40)),
      /truncated.*40 bytes.*100-byte/,
    )
  })

  it('rejects a file whose header_size claims less than the fixed header itself', () => {
    const buffer = Buffer.alloc(100)
    buffer.writeUInt32BE(50, 0) // header_size, corrupt
    assert.throws(() => readFramebuffer(buffer), /truncated.*header_size 50/)
  })

  it('rejects a file cut off before its own pixel data ends', () => {
    const whole = buildXwd({
      width: 4,
      height: 4,
      bitsPerPixel: 32,
      bytesPerLine: 16,
      colormapEntries: 2,
      fillPixels: (pixels) => pixels.fill(1),
    })
    const cut = whole.subarray(0, whole.length - 10)
    assert.throws(() => readFramebuffer(cut), /truncated/)
  })
})

describe('isPainted', () => {
  it('reports a blank image (all-zero pixel data) as not painted', () => {
    const buffer = buildXwd({
      width: 1280,
      height: 800,
      bitsPerPixel: 32,
      bytesPerLine: 5120,
      colormapEntries: 256,
    })
    assert.equal(isPainted(readFramebuffer(buffer)), false)
  })

  it('keeps a cleared screen with a small non-zero region as not painted', () => {
    const buffer = buildXwd({
      width: 1280,
      height: 800,
      bitsPerPixel: 32,
      bytesPerLine: 5120,
      colormapEntries: 256,
      fillPixels: (pixels) => pixels.fill(0x01, 0, 295),
    })
    assert.equal(isPainted(readFramebuffer(buffer)), false)
  })

  it('reports an image with a window-sized non-blank region as painted', () => {
    const buffer = buildXwd({
      width: 1280,
      height: 800,
      bitsPerPixel: 32,
      bytesPerLine: 5120,
      colormapEntries: 256,
      fillPixels: (pixels) => pixels.fill(0x33),
    })
    assert.equal(isPainted(readFramebuffer(buffer)), true)
  })

  it('ignores non-zero bytes in the header or the colormap, only the pixel data counts', () => {
    const buffer = buildXwd({
      width: 4,
      height: 2,
      bitsPerPixel: 32,
      bytesPerLine: 16,
      colormapEntries: 4,
      name: 'not-blank-name',
    })
    // The colormap area defaults to zero from buildXwd, but the window name inside the header is
    // non-zero ASCII bytes - isPainted must not be fooled by it.
    assert.equal(isPainted(readFramebuffer(buffer)), false)
  })
})
