import { describe, it } from 'node:test'
import assert from 'node:assert/strict'
import {
  chmodSync,
  mkdirSync,
  mkdtempSync,
  rmSync,
  writeFileSync,
} from 'node:fs'
import { tmpdir } from 'node:os'
import { join } from 'node:path'
import { resolveTools } from './preflight.ts'

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
