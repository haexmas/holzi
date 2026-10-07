// Scaling an image down to WebP in the browser engine (spec 036 thumbnails, spec 042 workspace
// background, research R7). The header is read first, so an image above `MAX_PREVIEW_PIXELS` or a
// file that is no PNG, JPEG, GIF or WebP is refused before anything is decoded.
import { previewableSize } from '~/lib/passwords/imageSize'

/** `bytes` as WebP whose longer edge is at most `maxEdge` pixels; smaller images keep their size. */
export async function downscaleToWebp(
  bytes: ArrayBuffer,
  mime: string,
  maxEdge: number,
): Promise<Blob> {
  const size = previewableSize(bytes)
  if (!size) throw new Error('no readable size or too many pixels')
  // Only the width is given, so the engine keeps the aspect ratio, also of a turned (EXIF) photo.
  const headerScale = maxEdge / Math.max(size.width, size.height)
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
    const scale = Math.min(1, maxEdge / Math.max(bitmap.width, bitmap.height))
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
    if (!blob) throw new Error('no scaled image')
    return blob
  } finally {
    bitmap.close()
  }
}
