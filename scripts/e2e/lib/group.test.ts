import assert from 'node:assert/strict'
import { describe, it } from 'node:test'
import { createGroup } from './group.ts'
import type { CaptureDevice, Group, GroupDeps } from './group.ts'
import {
  DEFAULT_MAX_DEVICES,
  maxDevicesFrom,
  nextState,
  planGroup,
} from './group-plan.ts'
import type { DeviceOperation, DeviceState } from './group-plan.ts'
import { FakeHost, fakeRelay, pollingWaitFor } from './group.testlib.ts'
import type { FakeData } from './group.testlib.ts'

describe('planning a group', () => {
  it('names every device, its folder and which one makes the vault', () => {
    const plan = planGroup(
      {
        users: {
          anna: ['laptop', 'phone', { name: 'desk', main: true }],
          ben: ['pc'],
        },
      },
      6,
    )
    assert.deepEqual(
      plan.flatMap((user) =>
        user.devices.map((d) => [d.address, d.first, d.main, d.folder]),
      ),
      [
        ['anna/laptop', true, true, '4-anna-6-laptop'],
        ['anna/phone', false, false, '4-anna-5-phone'],
        ['anna/desk', false, true, '4-anna-4-desk'],
        ['ben/pc', true, true, '3-ben-2-pc'],
      ],
    )
    assert.equal(plan[0]?.vaultName, 'e2e-anna')
  })

  it('refuses a device name used twice, also by another user', () => {
    assert.throws(
      () => planGroup({ users: { anna: ['a', 'a'] } }, 6),
      /appears twice for user "anna"/,
    )
    assert.throws(
      () => planGroup({ users: { anna: ['a'], ben: ['a'] } }, 6),
      /used by users "anna" and "ben"/,
    )
  })

  it('refuses unusable names, no user and a user without a device', () => {
    assert.throws(
      () => planGroup({ users: { 'a/b': ['x'] } }, 6),
      /not a usable user name/,
    )
    assert.throws(
      () => planGroup({ users: { anna: ['x/y'] } }, 6),
      /not a usable device name/,
    )
    assert.throws(() => planGroup({ users: {} }, 6), /at least one user/)
    assert.throws(
      () => planGroup({ users: { anna: [] } }, 6),
      /at least one device/,
    )
  })

  it('refuses more devices than the limit and names the variable', () => {
    assert.throws(
      () => planGroup({ users: { anna: ['a', 'b', 'c'] } }, 2),
      /3 devices is more than the limit of 2.*E2E_MAX_DEVICES/,
    )
    assert.doesNotThrow(() => planGroup({ users: { anna: ['a', 'b'] } }, 2))
  })

  it('reads the limit from the environment', () => {
    assert.equal(maxDevicesFrom({}), DEFAULT_MAX_DEVICES)
    assert.equal(maxDevicesFrom({ E2E_MAX_DEVICES: '4' }), 4)
    assert.throws(
      () => maxDevicesFrom({ E2E_MAX_DEVICES: '0' }),
      /E2E_MAX_DEVICES/,
    )
    assert.throws(
      () => maxDevicesFrom({ E2E_MAX_DEVICES: 'many' }),
      /E2E_MAX_DEVICES/,
    )
  })
})

describe('the states of a device', () => {
  const ok: Array<[DeviceState, DeviceOperation, boolean, DeviceState]> = [
    ['stopped', 'start', false, 'running'],
    ['killed', 'start', false, 'running'],
    ['stopped', 'start', true, 'offline'],
    ['running', 'stop', false, 'stopped'],
    ['offline', 'stop', true, 'stopped'],
    ['running', 'kill', false, 'killed'],
    ['offline', 'kill', true, 'killed'],
    ['running', 'goOffline', false, 'offline'],
    ['offline', 'goOnline', true, 'running'],
  ]
  for (const [state, operation, offline, expected] of ok) {
    it(`${state} + ${operation}${offline ? ' (offline mode)' : ''} -> ${expected}`, () => {
      assert.equal(nextState(state, operation, offline), expected)
    })
  }

  it('refuses every other combination', () => {
    const states: DeviceState[] = ['stopped', 'running', 'offline', 'killed']
    const operations: DeviceOperation[] = [
      'start',
      'stop',
      'kill',
      'goOffline',
      'goOnline',
    ]
    const allowed = new Set(ok.map(([s, o]) => `${s}/${o}`))
    for (const state of states) {
      for (const operation of operations) {
        if (allowed.has(`${state}/${operation}`)) continue
        assert.throws(
          () => nextState(state, operation, false),
          new RegExp(`cannot ${operation} a device that is ${state}`),
        )
      }
    }
  })
})

interface Made {
  group: Group
  host: FakeHost
  teardowns: Array<() => Promise<void> | void>
  steps: Array<{ name: string; detail?: string; device?: string }>
  captured: Array<() => CaptureDevice[]>
}

async function make(
  users: Parameters<typeof createGroup>[1]['users'],
  options: { keep?: boolean; maxDevices?: number } = {},
): Promise<Made> {
  const host = new FakeHost()
  const teardowns: Made['teardowns'] = []
  const steps: Made['steps'] = []
  const captured: Made['captured'] = []
  let counter = 0
  const deps: GroupDeps = {
    host,
    relay: fakeRelay(),
    credentials: () => ({ passphrase: `pass-${++counter}` }),
    onTeardown: (action) => void teardowns.push(action),
    keep: options.keep ?? false,
    maxDevices: options.maxDevices ?? 6,
    waitFor: pollingWaitFor,
    step: (name, detail, device) => void steps.push({ name, detail, device }),
    captureWith: (devices) => void captured.push(devices),
  }
  const group = await createGroup(deps, { users })
  return { group, host, teardowns, steps, captured }
}

describe('creating a group', () => {
  it('creates each vault on its first device, on the relay only, and reopens it', async () => {
    const { group, host, steps } = await make({ anna: ['laptop'], ben: ['pc'] })
    assert.deepEqual(group.devices.size, 2)
    assert.deepEqual(
      host.servers.map((entry) => entry.folder),
      ['4-anna-6-laptop', '3-ben-2-pc'],
    )
    const args = host.servers[0]?.args as {
      nostrRelays: string[]
      disabled: string[]
    }
    assert.deepEqual(args.nostrRelays, ['ws://127.0.0.1:1'])
    assert.ok(args.disabled.includes('wss://default-nostr'))
    // start (create) -> stop -> start (open), for each user
    const lines = host.calls.filter((c) => /^(start|stop) 4-anna/.test(c))
    assert.deepEqual(lines, [
      'start 4-anna-6-laptop',
      'stop 4-anna-6-laptop',
      'start 4-anna-6-laptop',
    ])
    assert.equal(group.device('anna/laptop').state, 'running')
    assert.ok(
      steps.some(
        (step) =>
          step.name === 'device-started' && step.device === 'anna/laptop',
      ),
    )
  })

  it('links the other devices of a user with a code, as main or linked', async () => {
    const { group, host } = await make({
      anna: ['laptop', 'phone', { name: 'desk', main: true }],
    })
    assert.deepEqual(
      host.joins.map((join) => (join as { deviceName: string }).deviceName),
      ['phone', 'desk'],
    )
    assert.equal(group.device('anna/phone').role, 'linked')
    assert.equal(group.device('anna/desk').role, 'main')
    assert.ok(
      host.calls.includes('4-anna-4-desk: link_code_create') === false,
      'only the first device creates codes',
    )
    assert.ok(host.calls.includes('4-anna-6-laptop: link_code_create'))
    for (const device of group.devices.values()) {
      assert.equal(device.state, 'running')
    }
  })

  it('gives every user and linked device its own passphrase', async () => {
    const { group } = await make({ anna: ['a', 'b'], ben: ['c'] })
    const passphrases = [...group.devices.values()].map((d) => d.passphrase)
    assert.equal(new Set(passphrases).size, passphrases.length)
  })

  it('refuses a group over the limit before starting anything', async () => {
    const host = new FakeHost()
    await assert.rejects(
      createGroup(
        {
          host,
          relay: fakeRelay(),
          credentials: () => ({ passphrase: 'p' }),
          onTeardown: () => {},
          keep: false,
          maxDevices: 2,
          waitFor: pollingWaitFor,
          step: () => {},
        },
        { users: { anna: ['a', 'b', 'c'] } },
      ),
      /E2E_MAX_DEVICES/,
    )
    assert.deepEqual(host.calls, [])
  })

  it('stops and removes everything at the end, in reverse order', async () => {
    const { host, teardowns } = await make({ anna: ['laptop', 'phone'] })
    host.calls.length = 0
    for (const action of teardowns.reverse()) await action()
    assert.deepEqual(host.calls, [
      'stop 4-anna-5-phone',
      'dispose 4-anna-5-phone',
      'stop 4-anna-6-laptop',
      'dispose 4-anna-6-laptop',
    ])
  })

  it('keeps the data when asked', async () => {
    const { host, teardowns } = await make({ anna: ['laptop'] }, { keep: true })
    for (const action of teardowns.reverse()) await action()
    assert.equal(
      [...host.data.values()].some((d: FakeData) => d.disposed),
      false,
    )
  })

  it('still removes the data of a device that was killed', async () => {
    const { group, host, teardowns } = await make({ anna: ['laptop'] })
    await group.device('anna/laptop').kill()
    for (const action of teardowns.reverse()) await action()
    assert.ok(host.calls.includes('dispose 4-anna-6-laptop'))
  })

  it('lists its devices for the failure material', async () => {
    const { group, captured } = await make({ anna: ['laptop'] })
    const devices = captured[0]?.() ?? []
    assert.equal(devices.length, 1)
    assert.equal(devices[0]?.folder, '4-anna-6-laptop')
    assert.equal(devices[0]?.alive(), true)
    await group.device('anna/laptop').stop()
    assert.equal(captured[0]?.()[0]?.alive(), false)
  })
})

describe('device operations', () => {
  it('goOffline sets no Nostr relay, restarts and stays offline over a restart', async () => {
    const { group, host } = await make({ anna: ['laptop'] })
    const laptop = group.device('anna/laptop')
    host.calls.length = 0
    host.servers.length = 0
    await laptop.goOffline()
    assert.equal(laptop.state, 'offline')
    assert.deepEqual(host.servers[0]?.args, {
      nostrRelays: [],
      irohRelays: [],
      disabled: ['wss://default-nostr', 'https://default-iroh'],
    })
    assert.deepEqual(
      host.calls.filter((c) => /^(start|stop) /.test(c)),
      ['stop 4-anna-6-laptop', 'start 4-anna-6-laptop'],
    )
    await laptop.restart()
    assert.equal(laptop.state, 'offline')
  })

  it('goOnline sets the relay back and restarts', async () => {
    const { group, host } = await make({ anna: ['laptop'] })
    const laptop = group.device('anna/laptop')
    await laptop.goOffline()
    host.servers.length = 0
    await laptop.goOnline()
    assert.equal(laptop.state, 'running')
    assert.deepEqual(
      (host.servers[0]?.args as { nostrRelays: string[] }).nostrRelays,
      ['ws://127.0.0.1:1'],
    )
    await laptop.restart()
    assert.equal(laptop.state, 'running')
  })

  it('refuses goOffline on a stopped device and goOnline on a running one', async () => {
    const { group } = await make({ anna: ['laptop'] })
    const laptop = group.device('anna/laptop')
    await assert.rejects(
      laptop.goOnline(),
      /cannot goOnline a device that is running/,
    )
    await laptop.stop()
    await assert.rejects(
      laptop.goOffline(),
      /cannot goOffline a device that is stopped/,
    )
    assert.throws(() => laptop.page, /it has no page/)
  })

  it('starts a killed device over the same data', async () => {
    const { group } = await make({ anna: ['laptop'] })
    const laptop = group.device('anna/laptop')
    await laptop.kill()
    assert.equal(laptop.state, 'killed')
    await laptop.start()
    assert.equal(laptop.state, 'running')
  })

  it('reads the device list, the status and the identity through the interface', async () => {
    const { group } = await make({ anna: ['laptop'] })
    const laptop = group.device('anna/laptop')
    assert.equal((await laptop.deviceList())[0]?.isCurrent, true)
    assert.equal((await laptop.status()).thisDevice, 'main')
    assert.equal((await laptop.identity()).npub, 'npub1fake')
    assert.equal(await laptop.pubkey(), 'pk-4-anna-6-laptop')
  })

  it('says which devices exist when an address is unknown', async () => {
    const { group } = await make({ anna: ['laptop'] })
    assert.throws(() => group.device('anna/phone'), /anna\/laptop/)
  })
})

describe('a device added later', () => {
  it('belongs to an existing user, has its own passphrase and no process yet', async () => {
    const { group } = await make({ anna: ['laptop'] })
    const phone = group.addDevice('anna', 'phone')
    assert.equal(phone.state, 'stopped')
    assert.equal(phone.vaultName, 'e2e-anna')
    assert.equal(phone.role, 'linked')
    assert.notEqual(phone.passphrase, group.device('anna/laptop').passphrase)
    assert.equal(group.device('anna/phone'), phone)
  })

  it('starts without a vault and then counts as running', async () => {
    const { group, host } = await make({ anna: ['laptop'] })
    const phone = group.addDevice('anna', 'phone')
    host.calls.length = 0
    await phone.startUnopened()
    assert.equal(phone.state, 'running')
    assert.deepEqual(host.calls, ['start 4-anna-5-phone'])
  })

  it('refuses an unknown user and a device over the limit', async () => {
    const { group } = await make({ anna: ['laptop'] }, { maxDevices: 1 })
    assert.throws(() => group.addDevice('ben', 'pc'), /no user "ben"/)
    assert.throws(
      () => group.addDevice('anna', 'phone'),
      /more than the limit of 1/,
    )
  })
})

describe('copying a vault file', () => {
  it('needs a stopped source and makes a stopped device of the same user', async () => {
    const { group, host } = await make({ anna: ['laptop'] })
    const laptop = group.device('anna/laptop')
    await assert.rejects(laptop.copyVaultTo('copy'), /stop device anna\/laptop/)
    await laptop.stop()
    const copy = await laptop.copyVaultTo('copy')
    assert.equal(copy.state, 'stopped')
    assert.equal(copy.address, 'anna/copy')
    assert.equal(copy.passphrase, laptop.passphrase)
    assert.equal(copy.role, laptop.role)
    assert.ok(
      host.calls.includes('copy 4-anna-6-laptop -> 4-anna-4-copy (e2e-anna)'),
    )
    assert.equal(group.device('anna/copy'), copy)
  })

  it('counts the copy against the device limit', async () => {
    const { group } = await make({ anna: ['laptop'] }, { maxDevices: 1 })
    const laptop = group.device('anna/laptop')
    await laptop.stop()
    await assert.rejects(laptop.copyVaultTo('copy'), /more than the limit of 1/)
  })

  it('removes and disposes a copy when copying fails', async () => {
    const { group, host } = await make({ anna: ['laptop'] })
    const laptop = group.device('anna/laptop')
    await laptop.stop()
    const source = host.data.get('4-anna-6-laptop')
    assert.ok(source)
    source.copyVaultFile = async () => {
      throw new Error('copy failed')
    }

    await assert.rejects(laptop.copyVaultTo('copy'), /copy failed/)
    assert.equal(group.devices.has('anna/copy'), false)
    assert.equal(host.data.get('4-anna-4-copy')?.disposed, true)
  })

  it('stops a process when opening its vault fails', async () => {
    const { group, host } = await make({ anna: ['laptop'] })
    const laptop = group.device('anna/laptop')
    await laptop.stop()
    host.failOpenVault = true

    await assert.rejects(laptop.start(), /opening failed/)
    assert.equal(laptop.state, 'stopped')
    assert.equal(host.data.get('4-anna-6-laptop')?.running, false)
    assert.throws(() => laptop.page, /it has no page/)
  })
})
