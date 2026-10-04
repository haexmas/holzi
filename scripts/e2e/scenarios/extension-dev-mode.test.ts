import assert from 'node:assert/strict'
import { generateKeyPairSync } from 'node:crypto'
import { mkdtempSync, mkdirSync, readFileSync, writeFileSync } from 'node:fs'
import { createServer, type Server } from 'node:http'
import type { AddressInfo } from 'node:net'
import { tmpdir } from 'node:os'
import { join } from 'node:path'
import { fileURLToPath } from 'node:url'
import { scenario } from '../lib/scenario.ts'
import { unwrap, waitForWorkspace, type FlowInstance } from '../lib/flows.ts'
import type { Page } from '../lib/page.ts'
import { openSettings, setSessionRestore, wmSnapshot } from '../lib/settings.ts'
import {
  inFrame,
  openFromLauncher,
  probeRequest,
  type InstalledExtension,
} from '../lib/extensions.ts'

// Spec 017, US12, T093/T096 (quickstart §7 last bullet): developer mode switched on in the
// settings reloads holzi's window; a project served from 127.0.0.1 loads unsigned, opens with the
// permanent mark on its tab, talks to holzi like an installed extension, keeps its tables on this
// device, shows its console output and gets a new document after a change; switched off, it is gone.

type Instance = Page & FlowInstance

const FIXTURES = fileURLToPath(
  new URL('../../../src-tauri/tests/fixtures/extension_e2e/', import.meta.url),
)

/** A development server for the probe page; `title` is what the page shows next. */
function devServer(): Promise<{
  server: Server
  port: number
  title: { text: string }
}> {
  const title = { text: 'Probe dev' }
  const server = createServer((request, response) => {
    // A sandboxed frame has an opaque origin: module scripts need CORS, as Vite's server sends it.
    response.setHeader('Access-Control-Allow-Origin', '*')
    const path = (request.url ?? '/').split('?')[0]
    if (path === '/' || path === '/index.html') {
      const page = readFileSync(join(FIXTURES, 'probe.html'), 'utf8').replace(
        '<h1 id="title">Probe</h1>',
        `<h1 id="title">${title.text}</h1>`,
      )
      response.writeHead(200, { 'Content-Type': 'text/html' })
      response.end(page)
      return
    }
    if (path === '/probe.js') {
      response.writeHead(200, { 'Content-Type': 'text/javascript' })
      response.end(readFileSync(join(FIXTURES, 'probe.js')))
      return
    }
    response.writeHead(404)
    response.end()
  })
  return new Promise((resolve) => {
    server.listen(0, '127.0.0.1', () => {
      resolve({ server, port: (server.address() as AddressInfo).port, title })
    })
  })
}

/** A project folder for the server on `port`, with a fresh publisher key. */
function project(port: number): string {
  const dir = mkdtempSync(join(tmpdir(), 'holzi-dev-project-'))
  writeFileSync(
    join(dir, 'haextension.config.json'),
    JSON.stringify({ dev: { host: '127.0.0.1', port } }),
  )
  const key = generateKeyPairSync('ed25519')
    .publicKey.export({ format: 'der', type: 'spki' })
    .subarray(-32)
    .toString('hex')
  mkdirSync(join(dir, 'haextension'))
  writeFileSync(
    join(dir, 'haextension', 'manifest.json'),
    JSON.stringify({
      name: 'devprobe',
      version: '0.1.0',
      publicKey: key,
      displayName: 'Dev Probe',
    }),
  )
  return dir
}

/** Waits until `change` replaced holzi's document and the workspace is back. */
async function reloaded(page: Instance, change: () => Promise<unknown>) {
  await page.exec(`window.__beforeDevSwitch = true; return true`)
  await change()
  const end = Date.now() + 15_000
  while (
    await page
      .exec<boolean>(`return window.__beforeDevSwitch === true`)
      .catch(() => true)
  ) {
    if (Date.now() > end) throw new Error('holzi did not reload')
    await new Promise((resolve) => setTimeout(resolve, 100))
  }
  await waitForWorkspace(page)
}

async function frameReady(page: Instance, extension: InstalledExtension) {
  return inFrame<boolean>(
    page,
    extension,
    `return document.documentElement.dataset.probeReady === '1'`,
  ).catch(() => false)
}

scenario('extension-dev-mode', { timeoutMs: 300_000 }, async (ctx) => {
  const group = await ctx.group({ users: { anna: ['laptop'] } })
  const page = group.device('anna/laptop').page
  // With session restore a tab of the development version would come back if nothing removed it.
  await setSessionRestore(page, true)
  const { server, port, title } = await devServer()
  try {
    const folder = project(port)
    const refused = (await page.invoke('extension_dev_load', {
      projectPath: folder,
    })) as { ok: boolean; error?: { reason?: string } }
    assert.equal(refused.ok, false)
    assert.equal(refused.error?.reason, 'dev_mode_off')

    // Through the switch in the settings, as the developer does it.
    await reloaded(page, async () => {
      await openSettings(page)
      await page.click('settings-category-extensions')
      await page.click('extension-dev-mode')
    })
    ctx.step('developer mode on; holzi reloaded')

    // The folder dialog is native: the scene does what it hands over.
    const preview = unwrap<{ name: string }>(
      'extension_dev_load',
      await page.invoke('extension_dev_load', { projectPath: folder }),
    )
    assert.equal(preview.name, 'devprobe')
    const id = unwrap<string>(
      'extension_dev_confirm',
      await page.invoke('extension_dev_confirm', {
        projectPath: folder,
        accepted: [],
      }),
    )
    const dev: InstalledExtension = { id, title: 'Dev Probe' }
    await ctx.waitFor(
      'the development version in the launcher',
      async () =>
        JSON.stringify(await page.invoke('extension_list')).includes(id),
      { timeoutMs: 10_000 },
    )
    await openFromLauncher(page, dev)
    await ctx.waitFor('the dev page to be ready', () => frameReady(page, dev), {
      timeoutMs: 20_000,
    })
    await page.waitForDisplayed('dev-badge', 5_000)
    ctx.step('loaded from 127.0.0.1, its tab marked')

    const info = await probeRequest(page, dev, 'extension_get_info', {})
    assert.equal((info.result as { name?: string }).name, 'devprobe')
    const key = (info.result as { publicKey: string }).publicKey
    const table = `${key}__devprobe__notes`
    const migrated = await probeRequest(
      page,
      dev,
      'extension_database_register_migrations',
      {
        extensionVersion: '0.1.0',
        migrations: [
          {
            name: '0000_init',
            sql: `CREATE TABLE \`${table}\` (\`id\` text PRIMARY KEY NOT NULL);`,
          },
        ],
      },
    )
    assert.equal(migrated.error, undefined, JSON.stringify(migrated))
    const written = await probeRequest(
      page,
      dev,
      'extension_database_execute',
      { query: `INSERT INTO "${table}" (id) VALUES ('n1')`, params: [] },
    )
    assert.equal(written.error, undefined, JSON.stringify(written))
    ctx.step('its own tables, on this device')

    await inFrame(
      page,
      dev,
      `window.parent.postMessage({ type: 'console.forward', data: { level: 'warn', message: 'hello from dev', timestamp: '12:00' } }, '*'); return true`,
    )
    await page.click('dev-console-toggle')
    await ctx.waitFor(
      'the console line',
      async () =>
        (
          await page.exec<string>(
            `return [...document.querySelectorAll('[data-testid="dev-console-line"]')].map((l) => l.textContent).join('|')`,
          )
        ).includes('hello from dev'),
      { timeoutMs: 5_000 },
    )
    ctx.step('console output shown')

    title.text = 'Probe dev changed'
    await inFrame(page, dev, `location.reload(); return true`).catch(() => {})
    await ctx.waitFor(
      'the changed page after a reload',
      async () =>
        (await inFrame<string>(
          page,
          dev,
          `return document.documentElement.dataset.probeReady === '1' ? document.getElementById('title').textContent : ''`,
        ).catch(() => '')) === 'Probe dev changed',
      { timeoutMs: 20_000 },
    )
    const again = await probeRequest(page, dev, 'extension_database_query', {
      query: `SELECT id FROM "${table}"`,
      params: [],
    })
    assert.deepEqual((again.result as { rows?: unknown[][] }).rows, [['n1']])
    ctx.step('a new document talks to holzi again')

    // The settings came back with the session; switching off as the settings do it.
    await reloaded(page, async () => {
      unwrap(
        'extension_dev_mode_set',
        await page.invoke('extension_dev_mode_set', { enabled: false }),
      )
      await page.exec(`window.location.reload(); return true`).catch(() => {})
    })
    const snapshot = await wmSnapshot(page)
    assert.ok(
      !snapshot.windows.some((w) =>
        w.tabs.some((t) => t.appId === `extension.${id}`),
      ),
      'its tab is gone',
    )
    assert.ok(
      !JSON.stringify(await page.invoke('extension_list')).includes(id),
      'not listed while developer mode is off',
    )
    ctx.step('developer mode off: the development version is gone')
  } finally {
    server.close()
  }
})
