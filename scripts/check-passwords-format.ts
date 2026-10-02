// Part of `pnpm check:passwords` (spec 034-password-manager, FR-001, FR-008, FR-021): the small
// helpers of the window (src/lib/passwords/format.ts).
import assert from 'node:assert/strict'
import { test } from 'node:test'

import {
  displayTitle,
  formatFileSize,
  isExpired,
  localDay,
  safeFileName,
} from '../src/lib/passwords/format.ts'

test('an entry without a title has no display title, a spaced one is trimmed', () => {
  assert.equal(displayTitle(null), null)
  assert.equal(displayTitle(undefined), null)
  assert.equal(displayTitle(''), null)
  assert.equal(displayTitle('   '), null)
  assert.equal(displayTitle('  Mail '), 'Mail')
})

test('an entry is expired the day after its expiry day', () => {
  assert.equal(isExpired('2026-10-01', '2026-10-02'), true)
  assert.equal(isExpired('2026-10-02', '2026-10-02'), false)
  assert.equal(isExpired('2026-10-03', '2026-10-02'), false)
  assert.equal(isExpired(null, '2026-10-02'), false)
  assert.equal(isExpired('soon', '2026-10-02'), false)
})

test('localDay formats the local calendar day', () => {
  assert.equal(localDay(new Date(2026, 9, 2, 23, 59)), '2026-10-02')
  assert.equal(localDay(new Date(2026, 0, 5)), '2026-01-05')
})

test('file sizes read in bytes, KiB, MiB and GiB with two decimals', () => {
  assert.equal(formatFileSize(0), '0 Bytes')
  assert.equal(formatFileSize(1023), '1023 Bytes')
  assert.equal(formatFileSize(1024), '1.00 KiB')
  assert.equal(formatFileSize(1536), '1.50 KiB')
  assert.equal(formatFileSize(25 * 1024 * 1024), '25.00 MiB')
  assert.equal(formatFileSize(3 * 1024 ** 3), '3.00 GiB')
  assert.equal(formatFileSize(-1), '0 Bytes')
  assert.equal(formatFileSize(Number.NaN), '0 Bytes')
})

test('a file name becomes text: separators and control characters are replaced', () => {
  assert.equal(safeFileName('report.pdf'), 'report.pdf')
  assert.equal(safeFileName('../../etc/passwd'), '.._.._etc_passwd')
  assert.equal(safeFileName('a\\b/c'), 'a_b_c')
  assert.equal(safeFileName('line\nbreak\t\u0000'), 'line_break__')
  assert.equal(safeFileName('Übergrößen 🔑.txt'), 'Übergrößen 🔑.txt')
  assert.equal(safeFileName('x'.repeat(300)).length, 255)
})
