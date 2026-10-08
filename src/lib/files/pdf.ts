/**
 * Loading and drawing PDFs with pdf.js (spec 044 FR-010, T038), shared by the PDF viewer and the
 * platform probe (T041), so the probe checks what the viewer runs. pdf.js reads the file from the
 * media server in ranges of 1 MiB and fetches only what a page needs. No wasm (the CSP has no
 * `wasm-unsafe-eval`; pdf.js tests for eval itself); CMaps and standard fonts come from `/pdfjs/`
 * (copied there by `nuxt.config.ts`) and are fetched by the page, not the worker. Imported only
 * lazily, so pdf.js stays out of the main bundle. The legacy build: Android System WebView 124
 * (the CI emulator, and phones that update their web view late) has no `URL.parse`, which pdf.js 6
 * calls; the legacy build brings it along (platform probe, PR #341).
 */
import {
  getDocument,
  GlobalWorkerOptions,
  type PDFDocumentLoadingTask,
  type PDFDocumentProxy,
  type RenderTask,
} from 'pdfjs-dist/legacy/build/pdf.mjs'
import workerUrl from 'pdfjs-dist/legacy/build/pdf.worker.min.mjs?url'

GlobalWorkerOptions.workerSrc = workerUrl

const asset = (path: string) => new URL(path, window.location.href).href

/** Starts loading the PDF at `url`; `destroy()` on the task ends it. */
export function loadPdf(url: string): PDFDocumentLoadingTask {
  return getDocument({
    url,
    rangeChunkSize: 1024 * 1024,
    disableAutoFetch: true,
    useWasm: false,
    useWorkerFetch: false,
    cMapUrl: asset('/pdfjs/cmaps/'),
    standardFontDataUrl: asset('/pdfjs/standard_fonts/'),
  })
}

/** Draws page `number` of `doc` into `canvas` at `scale` (sharp on high-density screens). */
export async function renderPdfPage(
  doc: PDFDocumentProxy,
  number: number,
  canvas: HTMLCanvasElement,
  scale: number,
): Promise<RenderTask> {
  const page = await doc.getPage(number)
  const ratio = window.devicePixelRatio || 1
  const viewport = page.getViewport({ scale: scale * ratio })
  canvas.width = viewport.width
  canvas.height = viewport.height
  canvas.style.width = `${viewport.width / ratio}px`
  canvas.style.height = `${viewport.height / ratio}px`
  return page.render({ canvas, viewport })
}
