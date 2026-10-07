import { describe, it } from 'node:test'
import assert from 'node:assert/strict'
import { createAdb, deviceSerial } from './adb.ts'
import type { AdbRunner } from './adb.ts'
import {
  AndroidData,
  PACKAGE,
  createAndroidHost,
  mapAndroidUrl,
  prepareDevice,
} from './android.ts'

/** An adb that records every call and answers from `answers` (by the joined argument list). */
function fakeAdb(answers: Record<string, string | Error> = {}) {
  const calls: string[] = []
  const run: AdbRunner = (args) => {
    const line = args.join(' ')
    calls.push(line)
    for (const [prefix, answer] of Object.entries(answers)) {
      if (line.startsWith(prefix)) {
        if (answer instanceof Error) throw answer
        return Buffer.from(answer)
      }
    }
    return Buffer.from('')
  }
  return { calls, run }
}

describe('createAdb', () => {
  it('addresses the one device by serial and reads the pid of the app', () => {
    const fake = fakeAdb({ '-s emu shell pidof': '4321\n' })
    const adb = createAdb('emu', fake.run)
    assert.equal(adb.pidof(PACKAGE), 4321)
    assert.deepEqual(fake.calls, [`-s emu shell pidof ${PACKAGE}`])
  })

  it('reads no pid when pidof finds nothing', () => {
    const fake = fakeAdb({ '-s emu shell pidof': new Error('exit 1') })
    assert.equal(createAdb('emu', fake.run).pidof(PACKAGE), undefined)
  })

  it('sees the web view open for debugging by its DevTools socket', () => {
    const fake = fakeAdb({
      '-s emu shell cat /proc/net/unix':
        '00: 0001 @webview_devtools_remote_77\n00: 0001 @other\n',
    })
    const adb = createAdb('emu', fake.run)
    assert.equal(adb.devtoolsOpen(77), true)
    assert.equal(adb.devtoolsOpen(78), false)
  })

  it('makes a port of this machine reachable from the device under the same number', () => {
    const fake = fakeAdb()
    createAdb('emu', fake.run).reverse(4711)
    assert.deepEqual(fake.calls, ['-s emu reverse tcp:4711 tcp:4711'])
  })
})

describe('deviceSerial', () => {
  it('takes ANDROID_SERIAL when set', () => {
    assert.equal(
      deviceSerial({ ANDROID_SERIAL: 'x' }, () => Buffer.from('')),
      'x',
    )
  })

  it('takes the one device adb sees and refuses none or several', () => {
    const list = (body: string) => () =>
      Buffer.from(`List of devices attached\n${body}\n`)
    assert.equal(
      deviceSerial({}, list('emulator-5554\tdevice')),
      'emulator-5554',
    )
    assert.throws(() => deviceSerial({}, list('')), /no device/)
    assert.throws(
      () => deviceSerial({}, list('a\tdevice\nb\tdevice')),
      /2 devices/,
    )
    assert.throws(() => deviceSerial({}, list('a\toffline')), /no device/)
  })
})

describe('prepareDevice', () => {
  it('clears the data, sizes the display like the Linux screen and sets the colour scheme', () => {
    const fake = fakeAdb()
    prepareDevice(createAdb('emu', fake.run), { colorScheme: 'dark' })
    assert.deepEqual(fake.calls, [
      `-s emu shell pm clear ${PACKAGE}`,
      '-s emu shell wm size 2560x1600',
      '-s emu shell wm density 320',
      '-s emu shell cmd uimode night yes',
    ])
  })

  it('keeps the data when the scenario reuses it', () => {
    const fake = fakeAdb()
    prepareDevice(createAdb('emu', fake.run), { reuse: true })
    assert.ok(!fake.calls.some((call) => call.includes('pm clear')))
    assert.ok(fake.calls.includes('-s emu shell cmd uimode night no'))
  })
})

describe('mapAndroidUrl', () => {
  it('maps the desktop app and extension origins to the ones Tauri serves on Android', () => {
    assert.equal(
      mapAndroidUrl('tauri://localhost/closing.html'),
      'http://tauri.localhost/closing.html',
    )
    assert.equal(
      mapAndroidUrl('holzi-ext://localhost/a/b'),
      'http://holzi-ext.localhost/a/b',
    )
    assert.equal(mapAndroidUrl('https://example.org/'), 'https://example.org/')
  })
})

describe('createAndroidHost', () => {
  it('gives one device per scenario and clears its data on dispose', () => {
    const fake = fakeAdb()
    const host = createAndroidHost({
      adb: createAdb('emu', fake.run),
      chromedriver: '/nonexistent/chromedriver',
      marker: 'm',
      logDir: '/tmp',
    })
    const data = host.newData('anna-phone')
    assert.ok(data instanceof AndroidData)
    assert.throws(() => host.newData('anna-laptop'), /one Android device/)
    data.dispose()
    assert.deepEqual(fake.calls, [`-s emu shell pm clear ${PACKAGE}`])
  })
})

describe('reachFromDevice', () => {
  it('reverses the port on an Android run and does nothing otherwise', async () => {
    const { reachFromDevice } = await import('./reach.ts')
    const fake = fakeAdb()
    reachFromDevice(
      5000,
      { E2E_PLATFORM: 'android', ANDROID_SERIAL: 'emu' },
      fake.run,
    )
    reachFromDevice(5001, {}, fake.run)
    assert.deepEqual(fake.calls, ['-s emu reverse tcp:5000 tcp:5000'])
  })
})
