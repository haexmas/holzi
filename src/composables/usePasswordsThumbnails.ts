/**
 * Thumbnails of image attachments (spec 036, FR-041, research R14). One cache for the whole window
 * (`lib/passwords/thumbnails.ts`), keyed by the checksum, so the same file in two entries is
 * rendered once. A render fetches the bytes, reads the pixel size from the header (above
 * `MAX_PREVIEW_PIXELS` there is no thumbnail), lets the engine decode at the small size and keeps
 * only the small image as an object URL; the full bytes are dropped at once. The cache is cleared
 * when the last password manager frame closes (`clearPasswordsThumbnails`).
 */
import type { AttachmentView } from '@bindings/AttachmentView'
import { imageMime } from '~/lib/passwords/format'
import { previewableSize } from '~/lib/passwords/imageSize'
import {
  createThumbnailCache,
  ThumbnailNotNow,
  type Thumbnail,
  type ThumbnailCache,
} from '~/lib/passwords/thumbnails'

/** The longest edge of a thumbnail in pixels: twice the 160 CSS pixels of a card, for sharp
 * thumbnails on high-density screens. */
const THUMBNAIL_EDGE = 320

/** What a render needs for a checksum: the latest attachment that has these bytes. */
const sources = new Map<string, { attachmentId: string; mime: string }>()
let cache: ThumbnailCache | null = null

async function scaledUrl(bytes: ArrayBuffer, mime: string): Promise<string> {
  const size = previewableSize(bytes)
  if (!size) throw new Error('no readable size or too many pixels')
  // Only the width is given, so the engine keeps the aspect ratio, also of a turned (EXIF) photo.
  const headerScale = THUMBNAIL_EDGE / Math.max(size.width, size.height)
  const bitmap = await createImageBitmap(
    new Blob([bytes], { type: mime }),
    headerScale < 1
      ? {
          resizeWidth: Math.max(1, Math.round(size.width * headerScale)),
          resizeQuality: 'medium',
        }
      : {},
  )
  try {
    const scale = Math.min(
      1,
      THUMBNAIL_EDGE / Math.max(bitmap.width, bitmap.height),
    )
    const canvas = document.createElement('canvas')
    canvas.width = Math.max(1, Math.round(bitmap.width * scale))
    canvas.height = Math.max(1, Math.round(bitmap.height * scale))
    const context = canvas.getContext('2d')
    if (!context) throw new Error('no 2d context')
    context.drawImage(bitmap, 0, 0, canvas.width, canvas.height)
    // WebP keeps transparency and is much smaller than PNG for photos; an engine without it falls
    // back to PNG.
    const blob = await new Promise<Blob | null>((resolve) =>
      canvas.toBlob(resolve, 'image/webp', 0.8),
    )
    if (!blob) throw new Error('no thumbnail')
    return URL.createObjectURL(blob)
  } finally {
    bitmap.close()
  }
}

function cacheWith(
  preview: (attachmentId: string) => Promise<ArrayBuffer>,
): ThumbnailCache {
  cache ??= createThumbnailCache({
    async render(key) {
      // Without the bytes the next request tries again; only an image that does not decode is
      // remembered as a failure.
      const source = sources.get(key)
      if (!source) throw new ThumbnailNotNow('unknown attachment')
      let bytes: ArrayBuffer
      try {
        bytes = await preview(source.attachmentId)
      } catch (cause) {
        throw new ThumbnailNotNow('reading the attachment failed', { cause })
      }
      return scaledUrl(bytes, source.mime)
    },
    revoke: (url) => URL.revokeObjectURL(url),
  })
  return cache
}

/** Forgets every thumbnail and revokes its URL. */
export function clearPasswordsThumbnails() {
  cache?.clear()
  sources.clear()
}

export function usePasswordsThumbnails() {
  const { attachmentPreviewAsync } = usePasswords()

  /** The stored thumbnail of an attachment, without rendering. */
  function storedThumbnail(attachment: AttachmentView): Thumbnail | undefined {
    return cache?.get(attachment.binaryHash)
  }

  /** The thumbnail of an image attachment; a failure for anything else. */
  function thumbnailAsync(attachment: AttachmentView): Promise<Thumbnail> {
    const mime = imageMime(attachment.fileName)
    if (!mime) return Promise.resolve({ kind: 'failed' })
    sources.set(attachment.binaryHash, {
      attachmentId: attachment.id,
      mime,
    })
    return cacheWith(attachmentPreviewAsync).request(attachment.binaryHash)
  }

  return { storedThumbnail, thumbnailAsync }
}
