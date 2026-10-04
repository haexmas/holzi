// Part of `pnpm check:extensions` (spec 017, T042): the relay between an extension frame's port
// and Rust (src/lib/extensions/bridge.ts).
import assert from 'node:assert/strict'
import { test } from 'node:test'

import {
  FrameEventQueue,
  encodeBytes,
  eventMessage,
  readRequest,
  type FrameEvent,
} from '../src/lib/extensions/bridge.ts'

test('requests are read with id, method and params; anything else is not a request', () => {
  assert.deepEqual(
    readRequest({
      id: 'r1',
      method: 'extension_database_query',
      params: { sql: 'x' },
      timestamp: 1,
    }),
    { id: 'r1', method: 'extension_database_query', params: { sql: 'x' } },
  )
  assert.deepEqual(readRequest({ id: 7, method: 'extension_context_get' }), {
    id: 7,
    method: 'extension_context_get',
    params: null,
  })
  for (const data of [
    { type: 'haexspace:port:ready' },
    { id: {}, method: 'x' },
    { id: 'r', method: '' },
    null,
    'x',
  ]) {
    assert.equal(readRequest(data), null)
  }
})

test('bytes become $bytes base64 at any depth, other values stay', () => {
  const bytes = new Uint8Array([0, 1, 2, 250, 255])
  assert.deepEqual(
    encodeBytes({
      sql: 'INSERT',
      params: [1, 'a', bytes, null, true],
      blob: bytes.buffer,
    }),
    {
      sql: 'INSERT',
      params: [1, 'a', { $bytes: 'AAEC+v8=' }, null, true],
      blob: { $bytes: 'AAEC+v8=' },
    },
  )
  const big = new Uint8Array(100_000).fill(65)
  const encoded = encodeBytes(big) as { $bytes: string }
  assert.equal(Buffer.from(encoded.$bytes, 'base64').length, 100_000)
  let deep: unknown = 1
  for (let i = 0; i < 100; i++) deep = [deep]
  assert.doesNotThrow(() => encodeBytes(deep))
})

test('events are held until the port is ready, then delivered in order, only for their frame', () => {
  const delivered: FrameEvent[] = []
  const queue = new FrameEventQueue('f1', (e) => delivered.push(e))
  const event = (frame: string, type: string): FrameEvent => ({
    frame,
    type,
    data: null,
    timestamp: 1,
  })
  queue.push(event('f1', 'a'))
  queue.push(event('f2', 'foreign'))
  queue.push(event('f1', 'b'))
  assert.equal(delivered.length, 0)
  queue.ready()
  assert.deepEqual(
    delivered.map((e) => e.type),
    ['a', 'b'],
  )
  queue.push(event('f1', 'c'))
  queue.reset()
  queue.push(event('f1', 'd'))
  assert.deepEqual(
    delivered.map((e) => e.type),
    ['a', 'b', 'c'],
  )
  queue.ready()
  assert.deepEqual(
    delivered.map((e) => e.type),
    ['a', 'b', 'c', 'd'],
  )
  assert.deepEqual(eventMessage(event('f1', 'x')), {
    type: 'x',
    data: null,
    timestamp: 1,
  })
})

test('a file change reaches the SDK flat, other events keep their data', () => {
  const change = {
    frame: 'f1',
    type: 'filesync:file-changed',
    data: { ruleId: 'r', changeType: 'modified', path: 'a.txt' },
    timestamp: 2,
  }
  assert.deepEqual(eventMessage(change), {
    ruleId: 'r',
    changeType: 'modified',
    path: 'a.txt',
    type: 'filesync:file-changed',
    data: change.data,
    timestamp: 2,
  })
  const tables = {
    frame: 'f1',
    type: 'haextension:sync:tables-updated',
    data: { tables: ['t'] },
    timestamp: 3,
  }
  assert.deepEqual(eventMessage(tables), {
    type: 'haextension:sync:tables-updated',
    data: { tables: ['t'] },
    timestamp: 3,
  })
})
