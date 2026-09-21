import { describe, it } from 'node:test'
import assert from 'node:assert/strict'
import {
  chmodSync,
  mkdirSync,
  mkdtempSync,
  rmSync,
  symlinkSync,
  writeFileSync,
} from 'node:fs'
import { tmpdir } from 'node:os'
import { join } from 'node:path'
import {
  checkPreflight,
  normalizeVersion,
  resolveTools,
  runCommand,
} from './preflight.ts'
import type { CommandRunner } from './preflight.ts'

function stub(dir: string, name: string, mode = 0o755) {
  const file = join(dir, name)
  writeFileSync(file, '#!/bin/sh\n')
  chmodSync(file, mode)
  return file
}

describe('resolveTools', () => {
  it('returns the path of each tool found on PATH and lists the missing ones', () => {
    const dir = mkdtempSync(join(tmpdir(), 'e2e-tools-'))
    try {
      const driver = stub(dir, 'tauri-driver')
      const webkit = stub(dir, 'WebKitWebDriver')
      const result = resolveTools(dir)
      assert.equal(result.found['tauri-driver'], driver)
      assert.equal(result.found.WebKitWebDriver, webkit)
      assert.deepEqual(result.missing, ['xvfb-run'])
    } finally {
      rmSync(dir, { recursive: true, force: true })
    }
  })

  it('does not count a directory or a file that is not executable', () => {
    const first = mkdtempSync(join(tmpdir(), 'e2e-tools-a-'))
    const second = mkdtempSync(join(tmpdir(), 'e2e-tools-b-'))
    try {
      mkdirSync(join(first, 'xvfb-run'))
      stub(second, 'tauri-driver', 0o644)
      const result = resolveTools([first, second].join(':'))
      assert.deepEqual(result.missing.sort(), [
        'WebKitWebDriver',
        'tauri-driver',
        'xvfb-run',
      ])
    } finally {
      rmSync(first, { recursive: true, force: true })
      rmSync(second, { recursive: true, force: true })
    }
  })

  it('reports every tool missing for an empty or unset PATH', () => {
    assert.equal(resolveTools('').missing.length, 3)
    assert.equal(resolveTools(undefined).missing.length, 3)
  })

  it('prefers the first PATH entry that has the tool', () => {
    const first = mkdtempSync(join(tmpdir(), 'e2e-tools-a-'))
    const second = mkdtempSync(join(tmpdir(), 'e2e-tools-b-'))
    try {
      const winner = stub(first, 'xvfb-run')
      stub(second, 'xvfb-run')
      assert.equal(
        resolveTools([first, second].join(':')).found['xvfb-run'],
        winner,
      )
    } finally {
      rmSync(first, { recursive: true, force: true })
      rmSync(second, { recursive: true, force: true })
    }
  })
})

type Reply = { code: number; stdout: string } | null

function recordingRunner(replies: { pkgConfig: Reply; dpkg?: Reply }) {
  const calls: string[][] = []
  const run: CommandRunner = async (command, args, limitMs) => {
    calls.push([command, ...args])
    assert.equal(limitMs, 5000, 'every command has a 5 second limit')
    if (command === 'pkg-config') return replies.pkgConfig
    if (command === 'dpkg-query') return replies.dpkg ?? null
    throw new Error(`unexpected command ${command}`)
  }
  return { run, calls }
}

/** A PATH with all three tools; the driver's bin directory is a symlink into another package. */
function toolsWithVersionFile(version: string | null) {
  const root = mkdtempSync(join(tmpdir(), 'e2e-preflight-'))
  const bin = join(root, 'bin')
  mkdirSync(bin)
  stub(bin, 'tauri-driver')
  stub(bin, 'xvfb-run')
  const store = join(root, 'webkit-driver-package')
  const realStore = join(root, 'webkitgtk-package')
  mkdirSync(join(store, 'bin'), { recursive: true })
  mkdirSync(join(realStore, 'bin'), { recursive: true })
  stub(join(realStore, 'bin'), 'WebKitWebDriver')
  symlinkSync(
    join(realStore, 'bin', 'WebKitWebDriver'),
    join(store, 'bin', 'WebKitWebDriver'),
  )
  if (version !== null) {
    mkdirSync(join(store, 'share', 'webkit-webdriver'), { recursive: true })
    writeFileSync(
      join(store, 'share', 'webkit-webdriver', 'version'),
      `${version}\n`,
    )
  }
  return { root, pathEnv: [bin, join(store, 'bin')].join(':') }
}

describe('normalizeVersion', () => {
  it('reduces a package version to the upstream major.minor.patch', () => {
    assert.equal(normalizeVersion('2.52.6'), '2.52.6')
    assert.equal(normalizeVersion('2.44.2-0ubuntu0.24.04.1'), '2.44.2')
    assert.equal(normalizeVersion('2:2.44.2-1'), '2.44.2')
    assert.equal(normalizeVersion(' 2.52.6\n'), '2.52.6')
    assert.equal(normalizeVersion('not a version'), null)
  })
})

describe('checkPreflight', () => {
  it('reads the driver version from the file next to the driver as found, not next to its link target', async () => {
    const t = toolsWithVersionFile('2.52.6')
    try {
      const { run, calls } = recordingRunner({
        pkgConfig: { code: 0, stdout: '2.52.6\n' },
      })
      const check = await checkPreflight({ pathEnv: t.pathEnv, run })
      assert.equal(check.result, 'ok', check.messages.join('; '))
      assert.equal(check.driverVersion, '2.52.6')
      assert.equal(check.webviewVersion, '2.52.6')
      assert.deepEqual(calls, [
        ['pkg-config', '--modversion', 'webkit2gtk-4.1'],
      ])
      assert.ok(check.tools.every((tool) => tool.ok && tool.path !== undefined))
    } finally {
      rmSync(t.root, { recursive: true, force: true })
    }
  })

  it('falls back to the installed package version of the driver when there is no version file', async () => {
    const t = toolsWithVersionFile(null)
    try {
      const { run, calls } = recordingRunner({
        pkgConfig: { code: 0, stdout: '2.44.2\n' },
        dpkg: { code: 0, stdout: '2.44.2-0ubuntu0.24.04.1' },
      })
      const check = await checkPreflight({ pathEnv: t.pathEnv, run })
      assert.equal(check.result, 'ok', check.messages.join('; '))
      assert.equal(check.driverVersion, '2.44.2')
      assert.deepEqual(calls, [
        ['dpkg-query', '-W', '-f=${Version}', 'webkit2gtk-driver'],
        ['pkg-config', '--modversion', 'webkit2gtk-4.1'],
      ])
    } finally {
      rmSync(t.root, { recursive: true, force: true })
    }
  })

  it('fails on a mismatch and shows both versions and the remedy', async () => {
    const t = toolsWithVersionFile('2.52.6')
    try {
      const { run } = recordingRunner({
        pkgConfig: { code: 0, stdout: '2.44.2\n' },
      })
      const check = await checkPreflight({ pathEnv: t.pathEnv, run })
      assert.equal(check.result, 'failed')
      const text = check.messages.join('\n')
      assert.match(text, /2\.52\.6/)
      assert.match(text, /2\.44\.2/)
      assert.match(text, /must be the same version/)
      assert.match(text, /nix develop/)
    } finally {
      rmSync(t.root, { recursive: true, force: true })
    }
  })

  it('fails and names the missing source when a version cannot be determined', async () => {
    const t = toolsWithVersionFile(null)
    try {
      const noDriver = await checkPreflight({
        pathEnv: t.pathEnv,
        run: recordingRunner({
          pkgConfig: { code: 0, stdout: '2.52.6\n' },
          dpkg: null,
        }).run,
      })
      assert.equal(noDriver.result, 'failed')
      assert.equal(noDriver.driverVersion, undefined)
      assert.match(noDriver.messages.join('\n'), /version file/)
      assert.match(noDriver.messages.join('\n'), /dpkg-query/)

      const noWebview = await checkPreflight({
        pathEnv: toolsWithVersionFile('2.52.6').pathEnv,
        run: recordingRunner({ pkgConfig: { code: 1, stdout: '' } }).run,
      })
      assert.equal(noWebview.result, 'failed')
      assert.equal(noWebview.webviewVersion, undefined)
      assert.match(
        noWebview.messages.join('\n'),
        /pkg-config --modversion webkit2gtk-4\.1/,
      )
    } finally {
      rmSync(t.root, { recursive: true, force: true })
    }
  })

  it('fails for a missing tool and names the tool, the dev shell and the distribution package', async () => {
    const empty = mkdtempSync(join(tmpdir(), 'e2e-preflight-'))
    try {
      const check = await checkPreflight({
        pathEnv: empty,
        run: recordingRunner({ pkgConfig: { code: 0, stdout: '2.52.6\n' } })
          .run,
      })
      assert.equal(check.result, 'failed')
      const xvfb = check.tools.find((tool) => tool.name === 'xvfb-run')
      assert.equal(xvfb?.ok, false)
      assert.match(xvfb?.remedy ?? '', /nix develop/)
      assert.match(xvfb?.remedy ?? '', /xvfb/)
      const driver = check.tools.find((tool) => tool.name === 'WebKitWebDriver')
      assert.match(driver?.remedy ?? '', /webkit2gtk-driver/)
      assert.match(check.messages.join('\n'), /tauri-driver/)
    } finally {
      rmSync(empty, { recursive: true, force: true })
    }
  })
})

describe('runCommand', () => {
  it('gives up on a command that outlives its limit and answers null for one that cannot start', async () => {
    const started = Date.now()
    assert.equal(await runCommand('sleep', ['30'], 50), null)
    assert.ok(Date.now() - started < 3000)
    assert.equal(await runCommand('/no/such/command', [], 1000), null)
    const echoed = await runCommand('echo', ['hello'], 2000)
    assert.deepEqual(echoed, { code: 0, stdout: 'hello\n' })
  })
})
