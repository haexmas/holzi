// Spec 038, research R6, T041: the storage dialog over an extension's tab never has a field for
// credentials. An extension can draw a look-alike inside its frame, so holzi asks for credentials
// only in its window over the whole app (`StorageCredentialsModal.vue`). This walks the template of
// `StorageDialog.vue`: radio buttons and checkboxes are fine, every other input is not.
import assert from 'node:assert/strict'
import { readFileSync } from 'node:fs'
import { test } from 'node:test'
import { parse } from 'vue/compiler-sfc'

type Node = {
  type: number
  tag?: string
  props?: { type: number; name: string; value?: { content: string } }[]
  children?: Node[]
}

const ELEMENT = 1
const ATTRIBUTE = 6
const TEXT_FIELDS = new Set([
  'textarea',
  'uiinput',
  'uiinputpassword',
  'uitextarea',
  'shadcninput',
  'shadcntextarea',
  'ui-input',
  'ui-input-password',
])

function fields(node: Node, found: string[] = []): string[] {
  if (node.type === ELEMENT && node.tag) {
    const tag = node.tag.toLowerCase()
    const attribute = (name: string) =>
      node.props?.find((p) => p.type === ATTRIBUTE && p.name === name)?.value
        ?.content
    if (TEXT_FIELDS.has(tag)) found.push(tag)
    if (tag === 'input') {
      const type = attribute('type') ?? 'text'
      if (type !== 'radio' && type !== 'checkbox') found.push(`input[${type}]`)
    }
    if (attribute('contenteditable') !== undefined)
      found.push(`${tag}[contenteditable]`)
  }
  for (const child of node.children ?? []) fields(child, found)
  return found
}

function template(path: string): Node {
  const source = readFileSync(new URL(path, import.meta.url), 'utf8')
  const { descriptor, errors } = parse(source, { filename: path })
  assert.deepEqual(errors, [])
  assert.ok(descriptor.template?.ast, path)
  return descriptor.template.ast as unknown as Node
}

test('the storage dialog over a tab has no field for credentials', () => {
  assert.deepEqual(
    fields(template('../src/components/extensions/StorageDialog.vue')),
    [],
  )
})

test('the credentials are typed in the window over the whole app', () => {
  const found = fields(
    template('../src/components/StorageCredentialsModal.vue'),
  )
  assert.ok(found.includes('uiinputpassword'), JSON.stringify(found))
})
