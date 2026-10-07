import { describe, it } from 'node:test'
import assert from 'node:assert/strict'
import { checkAndroidPreflight, majorOf } from './android-preflight.ts'
import type { AndroidPreflightDeps } from './android-preflight.ts'

/** A device that answers like an emulator with the debug app on it; `overrides` change one answer. */
function deps(
  overrides: Record<string, string | Error> = {},
  driverVersion = 'ChromeDriver 124.0.6367.207 (a9001a6)',
) {
  const calls: string[] = []
  const answers: Record<string, string | Error> = {
    devices: 'List of devices attached\nemulator-5554\tdevice\n',
    '-s emulator-5554 shell pm path com.haex.holzi':
      'package:/data/app/base.apk',
    '-s emulator-5554 shell dumpsys package com.haex.holzi':
      'flags=[ DEBUGGABLE HAS_CODE ALLOW_CLEAR_USER_DATA ]',
    '-s emulator-5554 shell dumpsys package com.google.android.webview':
      'versionName=124.0.6367.219',
    ...overrides,
  }
  const result: AndroidPreflightDeps = {
    run(args) {
      const line = args.join(' ')
      calls.push(line)
      const answer = answers[line]
      if (answer instanceof Error) throw answer
      return Buffer.from(answer ?? '')
    },
    version: () => driverVersion,
    exists: () => true,
  }
  return { calls, deps: result }
}

describe('majorOf', () => {
  it('reads the major version from a version text', () => {
    assert.equal(majorOf('ChromeDriver 124.0.6367.207 (abc)'), 124)
    assert.equal(majorOf('124.0.6367.219'), 124)
    assert.equal(majorOf('no version'), undefined)
    assert.equal(majorOf(undefined), undefined)
  })
})

describe('checkAndroidPreflight', () => {
  it('passes with one device, the debug app and a matching chromedriver', () => {
    const { deps: d } = deps()
    const result = checkAndroidPreflight(
      { env: { E2E_CHROMEDRIVER: '/cd' } },
      d,
    )
    assert.deepEqual(result.messages, [])
    assert.equal(result.ok, true)
    assert.deepEqual(result.env, {
      ANDROID_SERIAL: 'emulator-5554',
      E2E_CHROMEDRIVER: '/cd',
    })
    assert.equal(result.versions.webview, '124.0.6367.219')
  })

  it('names a chromedriver of another major version and how to fix it', () => {
    const { deps: d } = deps({}, 'ChromeDriver 130.0.1.2')
    const result = checkAndroidPreflight(
      { env: { E2E_CHROMEDRIVER: '/cd' } },
      d,
    )
    assert.equal(result.ok, false)
    assert.match(
      result.messages.join(),
      /chromedriver 130 does not match the device web view 124/,
    )
  })

  it('stops when the app is missing or not a debug build', () => {
    const missing = checkAndroidPreflight(
      { env: { E2E_CHROMEDRIVER: '/cd' } },
      deps({ '-s emulator-5554 shell pm path com.haex.holzi': '' }).deps,
    )
    assert.match(missing.messages.join(), /not installed/)
    const release = checkAndroidPreflight(
      { env: { E2E_CHROMEDRIVER: '/cd' } },
      deps({
        '-s emulator-5554 shell dumpsys package com.haex.holzi':
          'flags=[ HAS_CODE ]',
      }).deps,
    )
    assert.match(release.messages.join(), /not a debug build/)
  })

  it('replaces an app signed with another key when it installs --apk', () => {
    const { calls, deps: d } = deps({
      '-s emulator-5554 install -r app.apk': new Error(
        'Failure [INSTALL_FAILED_UPDATE_INCOMPATIBLE: signatures do not match]',
      ),
    })
    const result = checkAndroidPreflight(
      { env: { E2E_CHROMEDRIVER: '/cd' }, apk: 'app.apk' },
      d,
    )
    assert.equal(result.ok, true)
    assert.ok(calls.includes('-s emulator-5554 uninstall com.haex.holzi'))
    assert.ok(calls.includes('-s emulator-5554 install app.apk'))
  })

  it('stops without a device', () => {
    const result = checkAndroidPreflight(
      { env: {} },
      deps({ devices: 'List of devices attached\n' }).deps,
    )
    assert.equal(result.ok, false)
    assert.match(result.messages.join(), /no device/)
  })
})
