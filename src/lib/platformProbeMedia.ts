/**
 * The media half of the platform probe (spec 044, T041, SC-008): plays, seeks and draws the files
 * the probe serves through the media server, the way the viewer does (`components/files/
 * MediaViewer.vue`, `PdfViewer.vue` through `lib/files/pdf.ts`). Needs the web view's DOM, so it is
 * not part of the Node checks of `platformProbe.ts`.
 */
import type { ProbeStep } from '~/lib/platformProbe'

/** The URLs of the probe's files on the media server (`platform_probe::serve_fixtures`). */
export interface ProbeMedia {
  video: string
  audio: string
  pdf: string
}

/** The five steps together stay well inside the probe's own limit (`REPORT_TIMEOUT`, 90 s). */
const STEP_TIMEOUT_MS = 12_000

/** Runs `check` as one step; a rejection or a timeout fails it with the reason. */
async function step(
  name: string,
  check: () => Promise<string | undefined>,
): Promise<ProbeStep> {
  let timer: ReturnType<typeof setTimeout> | undefined
  const timeout = new Promise<never>((_, reject) => {
    timer = setTimeout(
      () => reject(new Error(`timed out after ${STEP_TIMEOUT_MS} ms`)),
      STEP_TIMEOUT_MS,
    )
  })
  try {
    const detail = await Promise.race([check(), timeout])
    return { name, ok: true, detail }
  } catch (error) {
    return { name, ok: false, detail: String(error) }
  } finally {
    clearTimeout(timer)
  }
}

/** Resolves once `test` holds for `element`, rejects on the element's `error`. */
function until(element: HTMLMediaElement, test: () => boolean): Promise<void> {
  return new Promise((resolve, reject) => {
    const check = () => {
      if (element.error) {
        reject(
          new Error(
            `media error ${element.error.code}: ${element.error.message}`,
          ),
        )
      } else if (test()) {
        resolve()
      } else {
        setTimeout(check, 50)
      }
    }
    check()
  })
}

/** A muted player for `url` in the page (no user gesture here); playing past 0.3 s. */
async function play(
  tag: 'video' | 'audio',
  url: string,
): Promise<HTMLMediaElement> {
  const element = document.createElement(tag)
  element.muted = true
  element.preload = 'metadata'
  element.style.cssText = 'position:fixed;width:1px;height:1px;opacity:0'
  element.src = url
  document.body.append(element)
  await element.play()
  await until(element, () => element.currentTime > 0.3)
  return element
}

export async function runMediaProbe(media: ProbeMedia): Promise<ProbeStep[]> {
  const steps: ProbeStep[] = []
  let video: HTMLMediaElement | undefined
  steps.push(
    await step('video-play', async () => {
      video = await play('video', media.video)
      return `currentTime ${video.currentTime.toFixed(2)}`
    }),
  )
  steps.push(
    await step('video-seek', async () => {
      if (!video) throw new Error('the video did not play')
      const player = video
      player.currentTime = 6
      await until(
        player,
        () =>
          !player.seeking && player.currentTime >= 6 && player.readyState >= 2,
      )
      return `currentTime ${player.currentTime.toFixed(2)}`
    }),
  )
  video?.remove()
  steps.push(
    await step('audio-play', async () => {
      const audio = await play('audio', media.audio)
      audio.remove()
      return undefined
    }),
  )
  steps.push(
    await step('media-range', async () => {
      const response = await fetch(media.video, {
        headers: { Range: 'bytes=1000-1999' },
      })
      const length = (await response.arrayBuffer()).byteLength
      const range = response.headers.get('Content-Range')
      if (response.status !== 206 || length !== 1000 || !range)
        throw new Error(`status ${response.status}, ${length} bytes, ${range}`)
      return range
    }),
  )
  steps.push(
    await step('pdf-render', async () => {
      // Lazily, as the viewer does: pdf.js stays out of the main bundle.
      const { loadPdf, renderPdfPage } = await import('~/lib/files/pdf')
      const task = loadPdf(media.pdf)
      try {
        const doc = await task.promise
        const canvas = document.createElement('canvas')
        await (
          await renderPdfPage(doc, doc.numPages, canvas, 1)
        ).promise
        return `${doc.numPages} pages, last one ${canvas.width}×${canvas.height}`
      } finally {
        void task.destroy().catch(() => {})
      }
    }),
  )
  return steps
}
