import { describe, it } from 'node:test'
import assert from 'node:assert/strict'
import {
  mkdirSync,
  mkdtempSync,
  readFileSync,
  readdirSync,
  rmSync,
  statSync,
  writeFileSync,
} from 'node:fs'
import { tmpdir } from 'node:os'
import { join } from 'node:path'
import {
  buildInstanceEnv,
  driverCommand,
  prepareRoot,
  removeRoot,
} from './instance.ts'

function scratch(): string {
  return mkdtempSync(join(tmpdir(), 'e2e-instance-'))
}

describe('buildInstanceEnv', () => {
  const root = '/scratch/instance-1'
  const parent = {
    PATH: '/usr/bin',
    HOME: '/sentinel/home',
    XDG_DATA_HOME: '/sentinel/data',
    XDG_CONFIG_HOME: '/sentinel/config',
    XDG_CACHE_HOME: '/sentinel/cache',
    XDG_RUNTIME_DIR: '/sentinel/run',
    XDG_STATE_HOME: '/sentinel/state',
    XDG_DATA_DIRS: '/nix/share',
    DBUS_SESSION_BUS_ADDRESS: 'unix:path=/sentinel/bus',
    WAYLAND_DISPLAY: 'wayland-9',
    DISPLAY: ':9',
  }

  it('puts every location the application writes to inside the instance root', () => {
    const env = buildInstanceEnv(root, 'the-marker', parent)
    assert.equal(env.XDG_DATA_HOME, `${root}/data`)
    assert.equal(env.XDG_CONFIG_HOME, `${root}/config`)
    assert.equal(env.XDG_CACHE_HOME, `${root}/cache`)
    assert.equal(env.XDG_RUNTIME_DIR, `${root}/run`)
    assert.equal(env.HOME, `${root}/home`)
  })

  it('cuts the application off from the desktop session and marks the run', () => {
    const env = buildInstanceEnv(root, 'the-marker', parent)
    assert.equal(env.DBUS_SESSION_BUS_ADDRESS, 'disabled:')
    assert.equal(env.HOLZI_E2E_RUN, 'the-marker')
    assert.equal(env.GDK_BACKEND, 'x11')
    assert.equal(env.WAYLAND_DISPLAY, undefined)
    assert.equal(env.DISPLAY, undefined)
    assert.equal(env.XDG_STATE_HOME, undefined)
  })

  it('keeps what the application needs to run and leaks nothing of the parent', () => {
    const env = buildInstanceEnv(root, 'the-marker', parent)
    assert.equal(env.PATH, '/usr/bin')
    assert.equal(env.XDG_DATA_DIRS, '/nix/share')
    for (const [key, value] of Object.entries(env)) {
      assert.ok(
        !String(value).includes('/sentinel'),
        `${key} leaks the parent's value`,
      )
    }
  })
})

describe('prepareRoot and removeRoot', () => {
  it('creates the tree empty, with a private runtime directory, and a light scheme by default', () => {
    const parent = scratch()
    try {
      const root = join(parent, 'root')
      prepareRoot(root)
      for (const dir of ['data', 'cache', 'run', 'home']) {
        assert.deepEqual(
          readdirSync(join(root, dir)),
          [],
          `${dir} starts empty`,
        )
      }
      assert.deepEqual(readdirSync(join(root, 'config')), ['gtk-3.0'])
      assert.equal(statSync(join(root, 'run')).mode & 0o777, 0o700)
      const ini = readFileSync(
        join(root, 'config', 'gtk-3.0', 'settings.ini'),
        'utf8',
      )
      assert.match(ini, /gtk-application-prefer-dark-theme=false/)
    } finally {
      rmSync(parent, { recursive: true, force: true })
    }
  })

  it('writes the dark preference for the dark scheme', () => {
    const parent = scratch()
    try {
      const root = join(parent, 'root')
      prepareRoot(root, { colorScheme: 'dark' })
      const ini = readFileSync(
        join(root, 'config', 'gtk-3.0', 'settings.ini'),
        'utf8',
      )
      assert.match(ini, /gtk-application-prefer-dark-theme=true/)
    } finally {
      rmSync(parent, { recursive: true, force: true })
    }
  })

  it('refuses a root that already holds something, unless it is being reused', () => {
    const parent = scratch()
    try {
      const root = join(parent, 'root')
      mkdirSync(join(root, 'data'), { recursive: true })
      writeFileSync(join(root, 'data', 'vault.db'), 'x')
      assert.throws(() => prepareRoot(root), /not empty/)
      prepareRoot(root, { reuse: true })
      assert.equal(readFileSync(join(root, 'data', 'vault.db'), 'utf8'), 'x')
    } finally {
      rmSync(parent, { recursive: true, force: true })
    }
  })

  it('removes the whole tree', () => {
    const parent = scratch()
    try {
      const root = join(parent, 'root')
      prepareRoot(root)
      removeRoot(root)
      assert.throws(() => statSync(root), /ENOENT/)
    } finally {
      rmSync(parent, { recursive: true, force: true })
    }
  })
})

describe('driverCommand', () => {
  it('starts the driver on its own virtual screen with the given ports and tools', () => {
    const tools = {
      tauriDriver: '/t/tauri-driver',
      webKitWebDriver: '/t/WebKitWebDriver',
      xvfbRun: '/t/xvfb-run',
    }
    assert.deepEqual(driverCommand(tools, { driver: 1111, native: 2222 }), {
      command: '/t/xvfb-run',
      args: [
        '-a',
        '-s',
        '-screen 0 1280x800x24',
        '/t/tauri-driver',
        '--port',
        '1111',
        '--native-port',
        '2222',
        '--native-driver',
        '/t/WebKitWebDriver',
      ],
    })
  })
})
