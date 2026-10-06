import assert from 'node:assert/strict'
import { scenario } from '../lib/scenario.ts'
import { unwrap } from '../lib/flows.ts'
import { KEY, openSettings, waitForLocation } from '../lib/settings.ts'
import { listKeys, startRustfs } from '../lib/rustfs.ts'

const BUCKET = 'holzi-e2e'

type Overview = {
  connections: Array<{ id: string; credentials: string; insecure: boolean }>
  storages: Array<{ id: string; name: string; bucket: string }>
}

type Header = { id: string; title: string | null; owner?: string | null }

/** Replaces the text of a field with `text`. */
const replaceText = (text: string) =>
  `${KEY.control}a${KEY.release}${KEY.backspace}${text}`

// Spec 038, US1 and US4 (quickstart §4 and §5 step 5, SC-001, SC-004, SC-005): in Einstellungen →
// Speicher the user connects a local RustFS. Wrong credentials are named and save nothing; the
// right ones are tested before saving, land in the password manager as an entry of `storage`, and
// leave no test object in the bucket. After the sync the storage and its credentials are on the
// second device, which can test it on its own. Removing the connection takes its storage and its
// credentials with it on both devices. Needs Docker for RustFS; skipped without it.
scenario(
  'storage-two-devices',
  { timeoutMs: 360_000, needs: { container: true } },
  async (ctx) => {
    const rustfs = await startRustfs([BUCKET])
    ctx.onTeardown(() => rustfs.stop())
    ctx.step('RustFS started')

    const group = await ctx.group({ users: { anna: ['laptop', 'phone'] } })
    const laptop = group.device('anna/laptop').page
    const phone = group.device('anna/phone').page
    const overview = async (device: typeof laptop) =>
      unwrap<Overview>('storage_list', await device.invoke('storage_list'))

    await openSettings(laptop)
    await laptop.click('settings-category-storage')
    await waitForLocation(laptop, 'storage')
    const opened = Date.now()
    await laptop.click('storage-add-connection')
    await waitForLocation(laptop, 'storage.connection')
    await laptop.type('storage-endpoint', replaceText(rustfs.endpoint))
    await laptop.waitForDisplayed('storage-endpoint-insecure')
    await laptop.type('storage-access-key', rustfs.accessKeyId)
    await laptop.type('storage-secret', 'not-the-secret')
    await laptop.type('storage-bucket', BUCKET)
    await laptop.type('storage-name', 'Fotos')
    await laptop.click('storage-save')
    await laptop.waitForDisplayed('storage-failure', 20_000)
    const refused = await laptop.exec<string>(
      `return document.querySelector('[data-testid="storage-failure"]').textContent.trim()`,
    )
    assert.match(refused, /Zugangsdaten falsch|Credentials wrong/)
    assert.deepEqual((await overview(laptop)).connections, [], 'nothing saved')
    ctx.step('wrong credentials are named and save nothing')

    await laptop.type('storage-secret', replaceText(rustfs.secretAccessKey))
    const saving = Date.now()
    await laptop.click('storage-save')
    await waitForLocation(laptop, 'storage', 20_000)
    await laptop.waitForDisplayed('storage-row')
    await laptop.waitForDisplayed('storage-insecure')
    ctx.step(
      'connected after a passed test',
      `form ${Math.round((Date.now() - opened) / 1000)} s, test and save ${Date.now() - saving} ms`,
    )

    const saved = await overview(laptop)
    assert.equal(saved.connections.length, 1)
    assert.deepEqual(
      saved.storages.map((s) => [s.name, s.bucket]),
      [['Fotos', BUCKET]],
    )
    const headers = unwrap<{ headers: Header[] }>(
      'passwords_load_overview',
      await laptop.invoke('passwords_load_overview'),
    ).headers
    assert.deepEqual(
      headers.map((h) => [h.title, h.owner]),
      [['S3: RustFS', 'storage']],
      'the credentials are an entry of the storage function',
    )
    assert.deepEqual(
      (await listKeys(rustfs, BUCKET)).filter((k) =>
        k.startsWith('holzi-test/'),
      ),
      [],
      'no test object stays in the bucket',
    )
    ctx.step('credentials in the password manager, no test object left')

    const storageId = saved.storages[0]!.id
    await ctx.waitFor(
      'the storage and its credentials to reach the phone',
      async () => {
        const there = await overview(phone)
        return (
          there.storages.some((s) => s.id === storageId) &&
          there.connections[0]?.credentials === 'present'
        )
      },
      { timeoutMs: 60_000, fixed: true },
    )
    const tested = unwrap<{ outcome: string; leftoverKey: string | null }>(
      'storage_test',
      await phone.invoke('storage_test', { id: storageId }),
    )
    assert.deepEqual(tested, { outcome: 'passed', leftoverKey: null })
    ctx.step(
      'the phone has the storage and tests it with the synced credentials',
    )

    await laptop.click('storage-connection-row')
    await waitForLocation(laptop, 'storage.connection')
    await laptop.click('storage-remove-connection')
    await waitForLocation(laptop, 'storage.remove')
    await laptop.waitForDisplayed('storage-removal-storages')
    await laptop.click('storage-removal-confirm')
    await waitForLocation(laptop, 'storage')
    assert.deepEqual(await overview(laptop), { connections: [], storages: [] })
    assert.deepEqual(
      unwrap<{ headers: Header[] }>(
        'passwords_load_overview',
        await laptop.invoke('passwords_load_overview'),
      ).headers,
      [],
      'the credentials are deleted for good',
    )
    await ctx.waitFor(
      'the removal to reach the phone',
      async () => (await overview(phone)).connections.length === 0,
      { timeoutMs: 60_000, fixed: true },
    )
    ctx.step('removed with its storage and credentials on both devices')
  },
)
