<script setup lang="ts">
/**
 * The lightbox over the images of an entry (spec 036, US6, FR-038, FR-041, research R14):
 * PhotoSwipe, loaded only when it opens, with the images in card order. A slide fetches the full
 * bytes only when it is shown (and the next and previous one in advance); its size is read from the
 * decoded image. Counter "2 von 5", the name, Speichern unter, zoom by tap, pinch and wheel, arrows,
 * arrow keys and swipe, Escape and the close button come from PhotoSwipe; the focus returns to the
 * card that opened it. An image that cannot be shown leaves a message on its slide and the lightbox
 * open. Every object URL is revoked when it closes. Only images enter; a PDF never does.
 */
import type { AttachmentView } from '@bindings/AttachmentView'
import type PhotoSwipe from 'photoswipe'
import type { SlideData } from 'photoswipe'
import { imageMime } from '~/lib/passwords/format'

const props = defineProps<{
  /** The images of the entry in card order. */
  images: AttachmentView[]
}>()

const emit = defineEmits<{
  save: [attachment: AttachmentView]
  closed: []
}>()

const { t } = useI18n()
const { attachmentPreviewAsync } = usePasswords()
const reducedMotion = usePreferredReducedMotion()

let pswp: PhotoSwipe | null = null
let urls: string[] = []

function escapeHtml(text: string): string {
  return text.replace(
    /[&<>"']/g,
    (char) =>
      ({ '&': '&amp;', '<': '&lt;', '>': '&gt;', '"': '&quot;', "'": '&#39;' })[
        char
      ] ?? char,
  )
}

function messageSlide(text: string, testid: string): SlideData {
  return {
    html: `<div class="passwords-lightbox-message" role="alert" data-testid="${testid}">${escapeHtml(text)}</div>`,
  }
}

/** The full image of one slide as an object URL with its natural size. */
async function loadSlide(attachment: AttachmentView): Promise<SlideData> {
  const mime = imageMime(attachment.fileName)
  if (!mime) throw new Error('not an image')
  const bytes = await attachmentPreviewAsync(attachment.id)
  const url = URL.createObjectURL(new Blob([bytes], { type: mime }))
  urls.push(url)
  const image = new Image()
  image.src = url
  await image.decode()
  return {
    src: url,
    width: image.naturalWidth,
    height: image.naturalHeight,
    alt: attachment.fileName,
  }
}

const loaded = new Set<number>()

async function ensureSlide(instance: PhotoSwipe, index: number) {
  const count = props.images.length
  if (index < 0 || index >= count || loaded.has(index)) return
  loaded.add(index)
  const attachment = props.images[index]
  if (!attachment) return
  let data: SlideData
  try {
    data = await loadSlide(attachment)
  } catch {
    data = messageSlide(
      t('passwords.lightbox.unreadable', { name: attachment.fileName }),
      `passwords-lightbox-error-${attachment.id}`,
    )
  }
  if (pswp !== instance) return
  ;(instance.options.dataSource as SlideData[])[index] = data
  instance.refreshSlideContent(index)
}

function ensureAround(instance: PhotoSwipe) {
  const index = instance.currIndex
  void ensureSlide(instance, index)
  void ensureSlide(instance, index + 1)
  void ensureSlide(instance, index - 1)
}

/** Opens the lightbox on the image at `index`. */
async function open(index: number) {
  if (pswp || !props.images.length) return
  const [{ default: PhotoSwipeClass }] = await Promise.all([
    import('photoswipe'),
    import('photoswipe/style.css'),
  ])
  loaded.clear()
  urls = []
  const dataSource: SlideData[] = props.images.map(() => ({
    html: '<div class="passwords-lightbox-message" aria-busy="true"></div>',
  }))
  const still = reducedMotion.value === 'reduce'
  const instance = new PhotoSwipeClass({
    dataSource,
    index,
    returnFocus: true,
    bgOpacity: 0.92,
    showHideAnimationType: still ? 'none' : 'fade',
    zoomAnimationDuration: still ? false : 250,
    indexIndicatorSep: t('passwords.lightbox.of'),
    closeTitle: t('passwords.lightbox.close'),
    zoomTitle: t('passwords.lightbox.zoom'),
    arrowPrevTitle: t('passwords.lightbox.previous'),
    arrowNextTitle: t('passwords.lightbox.next'),
    errorMsg: t('passwords.lightbox.unreadableShort'),
    mainClass: 'passwords-lightbox',
  })
  pswp = instance
  instance.on('uiRegister', () => {
    instance.ui?.registerElement({
      name: 'save',
      order: 8,
      isButton: true,
      title: t('passwords.attachments.download'),
      html: '<svg aria-hidden="true" class="pswp__icn" style="fill: none; color: var(--pswp-icon-color)" viewBox="-4 -4 32 32" width="32" height="32" fill="none" stroke="currentColor" stroke-width="2" stroke-linecap="round" stroke-linejoin="round"><path d="M12 15V3"/><path d="M21 15v4a2 2 0 0 1-2 2H5a2 2 0 0 1-2-2v-4"/><path d="m7 10 5 5 5-5"/></svg>',
      onInit: (element) => {
        element.setAttribute('data-testid', 'passwords-lightbox-save')
      },
      onClick: () => {
        const attachment = props.images[instance.currIndex]
        if (attachment) emit('save', attachment)
      },
    })
    instance.ui?.registerElement({
      name: 'name',
      order: 9,
      isButton: false,
      appendTo: 'root',
      onInit: (element) => {
        element.classList.add('passwords-lightbox-name')
        element.setAttribute('data-testid', 'passwords-lightbox-name')
        const show = () => {
          element.textContent = props.images[instance.currIndex]?.fileName ?? ''
        }
        instance.on('change', show)
        show()
      },
    })
  })
  instance.on('afterInit', () => ensureAround(instance))
  instance.on('change', () => ensureAround(instance))
  instance.on('destroy', () => {
    for (const url of urls) URL.revokeObjectURL(url)
    urls = []
    loaded.clear()
    if (pswp === instance) pswp = null
    emit('closed')
  })
  instance.init()
}

onBeforeUnmount(() => pswp?.destroy())

defineExpose({ open })
</script>

<template>
  <span hidden />
</template>

<style>
.passwords-lightbox .passwords-lightbox-message {
  display: flex;
  height: 100%;
  align-items: center;
  justify-content: center;
  padding: 1rem;
  color: #fff;
  text-align: center;
}
.passwords-lightbox .passwords-lightbox-name {
  position: absolute;
  bottom: 0;
  left: 0;
  right: 0;
  padding: 0.75rem 1rem;
  color: #fff;
  text-align: center;
  font-size: 0.875rem;
  overflow: hidden;
  text-overflow: ellipsis;
  white-space: nowrap;
  background: linear-gradient(transparent, rgb(0 0 0 / 0.6));
}
</style>
