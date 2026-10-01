// `pnpm check:chat-state` (spec 032): tool calls and results are shown indented, and anything
// that is not JSON is shown as it is.
import assert from 'node:assert/strict'
import { test } from 'node:test'

import { prettyToolText } from '../src/lib/chat/toolText.ts'

test('a JSON object or array is indented', () => {
  assert.equal(
    prettyToolText('{"a":1,"b":[2]}'),
    '{\n  "a": 1,\n  "b": [\n    2\n  ]\n}',
  )
  assert.equal(prettyToolText(' [1] '), '[\n  1\n]')
  assert.equal(prettyToolText('{}'), '{}')
})

test('markers, plain text and broken JSON stay as they are', () => {
  for (const text of ['denied_by_user', 'Tabs: 3', '{"a":', '', '"text"', '42'])
    assert.equal(prettyToolText(text), text)
})
