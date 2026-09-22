// Reads the virtual screen's XWD image (research R11, T068): whether the relaunched application has
// painted a window since the last check. `Xvfb`, started with `-fbdir <dir>`, keeps one such file per
// screen (`<dir>/Xvfb_screen0`) current as it renders - no extra tool needed to read it.
//
// The XWD file header's 25 fields are fixed-size (4 bytes each, `CARD32`) and always big-endian - a
// historical X11 convention independent of the host's own byte order and of the `byte_order` field
// (which only describes the *pixel* data that follows). Confirmed against a real capture (Xvfb 21.1.24,
// a 1280x800x24 screen): `header_size` 160, `colormap_entries` 256, and the file's total size matched
// `header_size + colormap_entries * 12 + bytes_per_line * height` exactly (T068's validation record has
// the raw bytes). Field offsets below are from that same struct (`XWDFileHeader` in `X11/XWDFile.h`).

const FIXED_HEADER_SIZE = 100 // 25 CARD32 fields
const COLOR_ENTRY_SIZE = 12 // XWDColor: pixel (4) + red/green/blue (2 each) + flags + pad (1 each)
// T068 measured 295 non-zero bytes while the screen was cleared and about 1.90 MB after the
// replacement window painted. A 1% ratio leaves a wide margin between those observed states.
// ponytail: This is calibrated to the current 1280x800 capture; replace it with a
// region-aware metric if the UI later paints less than 1% of the screen.
const MIN_PAINTED_BYTE_RATIO = 0.01

const OFFSET = {
  headerSize: 0,
  width: 16,
  height: 20,
  bitsPerPixel: 44,
  bytesPerLine: 48,
  colormapEntries: 72,
} as const

export interface FramebufferHeader {
  /** Total header length, including the window name that follows the fixed 100-byte struct. */
  headerSize: number
  width: number
  height: number
  bitsPerPixel: number
  bytesPerLine: number
  colormapEntries: number
}

export interface Framebuffer {
  header: FramebufferHeader
  /** Just the pixel data: everything past the header and the colormap. */
  pixels: Buffer
}

function truncated(detail: string): never {
  throw new Error(`framebuffer file is truncated: ${detail}`)
}

/** Parses one XWD capture. Throws if the file is shorter than its own header says it must be. */
export function readFramebuffer(buffer: Buffer): Framebuffer {
  if (buffer.length < FIXED_HEADER_SIZE) {
    truncated(
      `${buffer.length} bytes, less than the ${FIXED_HEADER_SIZE}-byte fixed header`,
    )
  }
  const header: FramebufferHeader = {
    headerSize: buffer.readUInt32BE(OFFSET.headerSize),
    width: buffer.readUInt32BE(OFFSET.width),
    height: buffer.readUInt32BE(OFFSET.height),
    bitsPerPixel: buffer.readUInt32BE(OFFSET.bitsPerPixel),
    bytesPerLine: buffer.readUInt32BE(OFFSET.bytesPerLine),
    colormapEntries: buffer.readUInt32BE(OFFSET.colormapEntries),
  }
  if (header.headerSize < FIXED_HEADER_SIZE) {
    truncated(
      `header_size ${header.headerSize} is smaller than the ${FIXED_HEADER_SIZE}-byte fixed header itself`,
    )
  }
  const pixelsStart =
    header.headerSize + header.colormapEntries * COLOR_ENTRY_SIZE
  const pixelsEnd = pixelsStart + header.bytesPerLine * header.height
  if (buffer.length < pixelsEnd) {
    truncated(
      `${buffer.length} bytes, expected at least ${pixelsEnd} (header ${header.headerSize} + colormap ${
        header.colormapEntries * COLOR_ENTRY_SIZE
      } + pixels ${header.bytesPerLine * header.height})`,
    )
  }
  return { header, pixels: buffer.subarray(pixelsStart, pixelsEnd) }
}

/** Whether enough pixel bytes are non-zero to represent a painted window, not a transient clear. */
export function isPainted(fb: Framebuffer): boolean {
  const minimumNonZeroBytes = Math.max(
    1,
    Math.ceil(fb.pixels.length * MIN_PAINTED_BYTE_RATIO),
  )
  let nonZeroBytes = 0
  for (const byte of fb.pixels) {
    if (byte !== 0 && ++nonZeroBytes >= minimumNonZeroBytes) {
      return true
    }
  }
  return false
}
