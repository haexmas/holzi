// Run with `node scripts/check-vue-templates.ts`. Compiles every tracked
// `.vue` file's template with the same compiler the build uses, without
// running a build.
//
// Why this exists: `pnpm typecheck` does not parse template expressions and
// `pnpm lint` does not either, so a template that the SFC compiler rejects
// passes both. One did — `HuggingFaceModelManagement.vue`'s tab handler broke
// `pnpm generate` on `main` and nobody noticed, because no CI job builds.
//
// The specific trap is a multi-statement inline handler. Vue collapses a
// template attribute's newlines before parsing it, so `@click="a = 1\nb = 2"`
// needs `;` separators — and Prettier's `semi: false` strips exactly those,
// leaving markup that will not compile. Put such handlers in a named function
// in `<script setup>`; that is the only form Prettier and Vue agree on.
//
// Plain `node scripts/<name>.ts`: Node strips the types itself from 22.19
// which is this repository's declared `engines` floor and the version CI
// pins, so no bundler, runner or extra dependency is involved.
import { execFileSync } from 'node:child_process'
import { readFileSync } from 'node:fs'
import { compileTemplate, parse } from 'vue/compiler-sfc'

const files = execFileSync('git', ['ls-files', '-z', '--', '*.vue'], {
  encoding: 'utf8',
})
  .split('\0')
  .filter(Boolean)

if (files.length === 0) {
  console.error('No tracked .vue files found — is this a Git checkout?')
  process.exit(1)
}

const failures: { filename: string; message: string }[] = []

for (const filename of files) {
  const source = readFileSync(filename, 'utf8')
  const { descriptor, errors: parseErrors } = parse(source, { filename })

  for (const error of parseErrors) {
    failures.push({ filename, message: describe(error) })
  }

  // A component may legitimately be render-function-only or template-less.
  if (!descriptor.template) continue

  const { errors } = compileTemplate({
    id: filename,
    filename,
    source: descriptor.template.content,
    scoped: descriptor.styles.some((style) => style.scoped),
  })
  for (const error of errors) {
    failures.push({ filename, message: describe(error) })
  }
}

function describe(error: unknown) {
  const detail = error as {
    message?: string
    loc?: { start?: { line?: number } }
  }
  const message = String(detail?.message ?? error)
  const line = detail?.loc?.start?.line
  return line ? `${message} (template line ${line})` : message
}

if (failures.length > 0) {
  console.error(
    `${failures.length} template error(s) in ${new Set(failures.map((f) => f.filename)).size} file(s):\n`,
  )
  for (const { filename, message } of failures) {
    console.error(`  ${filename}\n    ${message}`)
  }
  console.error(
    '\nA multi-statement inline handler is the usual cause. Move it into a' +
      '\nnamed function in `<script setup>` — see AliasSetting.vue.',
  )
  process.exit(1)
}

console.log(`${files.length} Vue templates compile.`)
