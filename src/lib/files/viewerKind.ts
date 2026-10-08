// How the viewer shows a file by its name (spec 044 FR-009, FR-014). Mirrors
// `src-tauri/src/files/kind.rs`; `check-files-viewer.ts` runs the same cases as its Rust tests.
import type { ViewerKind } from '@bindings/ViewerKind'

const TEXT = new Set([
  'txt',
  'text',
  'log',
  'md',
  'markdown',
  'ini',
  'conf',
  'cfg',
  'toml',
  'yaml',
  'yml',
  'rs',
  'ts',
  'js',
  'mjs',
  'vue',
  'py',
  'sh',
  'c',
  'h',
  'cpp',
  'java',
  'kt',
  'go',
  'rb',
  'php',
  'sql',
  'env',
  'csv',
  'json',
  'xml',
  'html',
  'htm',
  'css',
])
const IMAGE = new Set([
  'png',
  'jpg',
  'jpeg',
  'gif',
  'webp',
  'svg',
  'bmp',
  'avif',
])
const VIDEO = new Set(['mp4', 'm4v', 'webm', 'mkv', 'mov', 'ogv'])
const AUDIO = new Set([
  'mp3',
  'm4a',
  'aac',
  'ogg',
  'oga',
  'opus',
  'wav',
  'flac',
])

/** The lower-case extension of `name`; empty for none or a hidden name like `.bashrc`. */
export function extensionOf(name: string): string {
  const dot = name.lastIndexOf('.')
  return dot > 0 ? name.slice(dot + 1).toLowerCase() : ''
}

export function viewerKind(name: string): ViewerKind {
  const ext = extensionOf(name)
  if (ext === 'pdf') return 'pdf'
  if (IMAGE.has(ext)) return 'image'
  if (VIDEO.has(ext)) return 'video'
  if (AUDIO.has(ext)) return 'audio'
  if (TEXT.has(ext)) return 'text'
  return 'info'
}
