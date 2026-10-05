// The pixel size of an image attachment, read from its header before anything decodes it (spec 036,
// FR-041, research R14). The 25 MiB limit is on the file size: a small PNG of 20000 × 20000 pixels
// decodes to about 1.6 GB, which brings the webview down. Thumbnails and the lightbox therefore
// check the pixel count first and show "Keine Vorschau" above the limit.

/** The most pixels a thumbnail or the lightbox decodes (about 400 MB as RGBA). */
export const MAX_PREVIEW_PIXELS = 100_000_000

export interface ImageSize {
  width: number
  height: number
}

function ascii(bytes: Uint8Array, at: number, text: string): boolean {
  if (bytes.length < at + text.length) return false
  for (let i = 0; i < text.length; i++) {
    if (bytes[at + i] !== text.charCodeAt(i)) return false
  }
  return true
}

function sized(width: number, height: number): ImageSize | null {
  return width > 0 && height > 0 ? { width, height } : null
}

function png(view: DataView, bytes: Uint8Array): ImageSize | null {
  if (bytes.length < 24 || !ascii(bytes, 12, 'IHDR')) return null
  return sized(view.getUint32(16), view.getUint32(20))
}

function gif(view: DataView, bytes: Uint8Array): ImageSize | null {
  if (bytes.length < 10) return null
  return sized(view.getUint16(6, true), view.getUint16(8, true))
}

function uint24(bytes: Uint8Array, at: number): number {
  return bytes[at]! | (bytes[at + 1]! << 8) | (bytes[at + 2]! << 16)
}

function webp(view: DataView, bytes: Uint8Array): ImageSize | null {
  if (ascii(bytes, 12, 'VP8 ') && bytes.length >= 30) {
    return sized(
      view.getUint16(26, true) & 0x3fff,
      view.getUint16(28, true) & 0x3fff,
    )
  }
  if (ascii(bytes, 12, 'VP8L') && bytes.length >= 25) {
    const [b0, b1, b2, b3] = [bytes[21]!, bytes[22]!, bytes[23]!, bytes[24]!]
    return sized(
      1 + (((b1 & 0x3f) << 8) | b0),
      1 + (((b3 & 0x0f) << 10) | (b2 << 2) | ((b1 & 0xc0) >> 6)),
    )
  }
  if (ascii(bytes, 12, 'VP8X') && bytes.length >= 30) {
    return sized(1 + uint24(bytes, 24), 1 + uint24(bytes, 27))
  }
  return null
}

/** Start-of-frame markers: C0–CF without DHT (C4), JPG (C8) and DAC (CC). */
function isStartOfFrame(marker: number): boolean {
  return (
    marker >= 0xc0 &&
    marker <= 0xcf &&
    marker !== 0xc4 &&
    marker !== 0xc8 &&
    marker !== 0xcc
  )
}

function jpeg(view: DataView, bytes: Uint8Array): ImageSize | null {
  let at = 2
  while (at + 4 <= bytes.length) {
    if (bytes[at] !== 0xff) return null
    const marker = bytes[at + 1]!
    if (marker === 0xff) {
      at += 1
      continue
    }
    // Markers without a length: TEM, RST0–RST7.
    if (marker === 0x01 || (marker >= 0xd0 && marker <= 0xd7)) {
      at += 2
      continue
    }
    if (marker === 0xd9 || marker === 0xda) return null
    const length = view.getUint16(at + 2)
    if (length < 2) return null
    if (isStartOfFrame(marker)) {
      if (at + 9 > bytes.length) return null
      return sized(view.getUint16(at + 7), view.getUint16(at + 5))
    }
    at += 2 + length
  }
  return null
}

/** The width and height of a PNG, JPEG, GIF or WebP from its header; `null` when the header cannot
 * be read. */
export function imageSize(data: ArrayBuffer | Uint8Array): ImageSize | null {
  const bytes = data instanceof Uint8Array ? data : new Uint8Array(data)
  const view = new DataView(bytes.buffer, bytes.byteOffset, bytes.byteLength)
  if (bytes.length >= 8 && bytes[0] === 0x89 && ascii(bytes, 1, 'PNG')) {
    return png(view, bytes)
  }
  if (bytes.length >= 3 && bytes[0] === 0xff && bytes[1] === 0xd8) {
    return jpeg(view, bytes)
  }
  if (ascii(bytes, 0, 'GIF8')) return gif(view, bytes)
  if (ascii(bytes, 0, 'RIFF') && ascii(bytes, 8, 'WEBP')) {
    return webp(view, bytes)
  }
  return null
}

/** The size of an image that may be decoded for a preview; `null` when its header cannot be read or
 * it has more than [`MAX_PREVIEW_PIXELS`]. */
export function previewableSize(
  data: ArrayBuffer | Uint8Array,
): ImageSize | null {
  const size = imageSize(data)
  if (!size || size.width * size.height > MAX_PREVIEW_PIXELS) return null
  return size
}
