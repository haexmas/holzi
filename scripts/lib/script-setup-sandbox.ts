// A small sandbox for pages whose `<script setup>` block can be replayed on its own, for the
// frontend checks of spec 013 (`node scripts/check-vault-lifecycle.ts`). The chat page has its own,
// much larger harness (`chat-state-harness.ts`); this one is for short pages such as the federation
// page, where booting every composable would only hide what the case is about.
//
// The block is transpiled with the `typescript` package already used by the other harness and run
// in its own `new Function` scope. Nuxt's auto-imports and compiler macros have no module behind
// them here, so they are injected as bare names. The three that reach outside the page
// (`useInstance`, `useInstancesStore`, `navigateTo`) throw unless the case provides them, so a page
// that starts using one of them without the case knowing fails loudly instead of doing nothing.
import { readFileSync } from 'node:fs'
import { createRequire } from 'node:module'
import { dirname, resolve as resolvePath } from 'node:path'
import { fileURLToPath } from 'node:url'
import ts from 'typescript'

const nodeRequire = createRequire(import.meta.url)
const repoRoot = resolvePath(dirname(fileURLToPath(import.meta.url)), '../..')

/** What a case can provide to the page. Anything left out is a loud failure or a plain double. */
export interface ScriptSetupGlobals {
  useInstance?: () => unknown
  useInstancesStore?: () => unknown
  navigateTo?: (to: string) => unknown
  /** The route params the page reads, `{ instance: 'vault' }` by default. */
  params?: Record<string, string>
}

function notProvided(name: string) {
  return () => {
    throw new Error(
      `script-setup sandbox: the page used '${name}', which the case did not provide`,
    )
  }
}

/**
 * Replays the `<script setup lang="ts">` block of `vueFile` (a path relative to the repository
 * root) and returns the top-level bindings named in `bindings`.
 */
export function loadScriptSetup<T>(
  vueFile: string,
  bindings: string[],
  globals: ScriptSetupGlobals = {},
): T {
  const source = readFileSync(resolvePath(repoRoot, vueFile), 'utf8')
  const block = source.match(/<script setup lang="ts">([\s\S]*?)<\/script>/)
  if (!block) {
    throw new Error(
      `No <script setup lang="ts"> block found in ${vueFile}. The cases replay that block ` +
        'verbatim, so a renamed tag silently removes all coverage: fix the pattern here.',
    )
  }
  const code = ts.transpileModule(block[1], {
    compilerOptions: {
      target: ts.ScriptTarget.ES2022,
      module: ts.ModuleKind.CommonJS,
      esModuleInterop: true,
    },
  }).outputText

  const vue = nodeRequire('vue') as {
    ref: unknown
    computed: unknown
    watch: unknown
  }
  const scope: Record<string, unknown> = {
    ref: vue.ref,
    computed: vue.computed,
    watch: vue.watch,
    onBeforeUnmount: () => {},
    defineProps: () => ({}),
    defineEmits: () => () => {},
    useI18n: () => ({ t: (key: string) => key }),
    useRoute: () => ({ params: globals.params ?? { instance: 'vault' } }),
    useInstance: globals.useInstance ?? notProvided('useInstance'),
    useInstancesStore:
      globals.useInstancesStore ?? notProvided('useInstancesStore'),
    navigateTo: globals.navigateTo ?? notProvided('navigateTo'),
  }
  const names = Object.keys(scope)
  return new Function(
    'exports',
    'require',
    ...names,
    `${code}\nreturn { ${bindings.join(', ')} };`,
  )(
    {},
    (specifier: string) => {
      throw new Error(
        `script-setup sandbox: unexpected import '${specifier}' in ${vueFile}`,
      )
    },
    ...names.map((name) => scope[name]),
  ) as T
}
