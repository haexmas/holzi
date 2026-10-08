/**
 * Thumbnails of image attachments (spec 036, FR-041, research R14). One cache for the whole window
 * (`lib/images/thumbnailCache.ts`), keyed by the checksum, so the same file in two entries is
 * rendered once. A render fetches the bytes, reads the pixel size from the header (above
 * `MAX_PREVIEW_PIXELS` there is no thumbnail), lets the engine decode at the small size and keeps
 * only the small image as an object URL; the full bytes are dropped at once. The cache is cleared
 * when the last password manager frame closes (`clearPasswordsThumbnails`).
 */
import type { AttachmentView } from '@bindings/AttachmentView'
import { imageMime } from '~/lib/passwords/format'
import { downscaleToWebp } from '~/lib/images/downscale'
import {
  createThumbnailCache,
  ThumbnailNotNow,
  type Thumbnail,
  type ThumbnailCache,
} from '~/lib/images/thumbnailCache'

/** The longest edge of a thumbnail in pixels: twice the 160 CSS pixels of a card, for sharp
 * thumbnails on high-density screens. */
const THUMBNAIL_EDGE = 320

/** What a render needs for a checksum: the latest attachment that has these bytes. */
const sources = new Map<string, { attachmentId: string; mime: string }>()
let cache: ThumbnailCache | null = null

async function scaledUrl(bytes: ArrayBuffer, mime: string): Promise<string> {
  return URL.createObjectURL(await downscaleToWebp(bytes, mime, THUMBNAIL_EDGE))
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
