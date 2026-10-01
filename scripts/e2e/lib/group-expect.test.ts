import assert from 'node:assert/strict'
import { describe, it } from 'node:test'
import { expectLastSeen, expectOnline, expectRole } from './group-expect.ts'
import type { DeviceRow, ThisDevice } from './group.ts'
import { pollingWaitFor } from './group.testlib.ts'

const ctx = { waitFor: pollingWaitFor, step: () => {} }
const target = { address: 'anna/phone', pubkey: async () => 'pk-phone' }

function row(overrides: Partial<DeviceRow>): DeviceRow {
  return {
    vaultDeviceUuid: 'u',
    devicePubkey: 'pk-phone',
    alias: null,
    role: 'linked',
    isCurrent: false,
    online: false,
    lastSeen: null,
    problem: null,
    ...overrides,
  }
}

function observer(rows: () => DeviceRow[]) {
  return { address: 'anna/laptop', deviceList: async () => rows() }
}

describe('expectOnline', () => {
  it('returns the row once the target is online', async () => {
    let calls = 0
    const found = await expectOnline(
      ctx,
      observer(() => [row({ online: ++calls > 2 })]),
      target,
      true,
    )
    assert.equal(found.online, true)
  })

  it('waits for not online as well', async () => {
    const found = await expectOnline(
      ctx,
      observer(() => [row({ online: false })]),
      target,
      false,
    )
    assert.equal(found.devicePubkey, 'pk-phone')
  })

  it('names who is waiting for what when the deadline passes', async () => {
    await assert.rejects(
      expectOnline(
        ctx,
        observer(() => []),
        target,
        true,
      ),
      /anna\/laptop to list anna\/phone as online/,
    )
  })
})

describe('expectLastSeen', () => {
  it('accepts a recent last-seen time of a device that is not online', async () => {
    const found = await expectLastSeen(
      ctx,
      observer(() => [row({ lastSeen: Date.now() - 1000 })]),
      target,
      { withinMs: 5000 },
    )
    assert.ok(found.lastSeen !== null)
  })

  it('rejects an old time, a missing time and a device that is still online', async () => {
    for (const rows of [
      [row({ lastSeen: Date.now() - 60_000 })],
      [row({ lastSeen: null })],
      [row({ online: true, lastSeen: Date.now() })],
    ]) {
      await assert.rejects(
        expectLastSeen(
          ctx,
          observer(() => rows),
          target,
          { withinMs: 5000 },
        ),
        /last seen within 5000 ms/,
      )
    }
  })

  it('rejects a time far in the future', async () => {
    await assert.rejects(
      expectLastSeen(
        ctx,
        observer(() => [row({ lastSeen: Date.now() + 120_000 })]),
        target,
        { withinMs: 5000 },
      ),
      /last seen within/,
    )
  })
})

describe('expectRole', () => {
  it('waits until the device reports the role', async () => {
    let calls = 0
    const device = {
      address: 'anna/laptop',
      status: async () => ({
        thisDevice: (++calls > 2 ? 'removed' : 'main') as ThisDevice,
      }),
    }
    await expectRole(ctx, device, 'removed')
    assert.ok(calls > 2)
  })
})
