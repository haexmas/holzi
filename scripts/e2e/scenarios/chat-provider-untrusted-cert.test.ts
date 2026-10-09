import assert from 'node:assert/strict'
import { randomUUID } from 'node:crypto'
import { scenario } from '../lib/scenario.ts'
import { createAndUnlock, unwrap } from '../lib/flows.ts'

// Spec 043 FR-024 (quickstart §7 step 2), on the desktop and on the phone: a provider whose server
// shows a certificate the device does not trust gets no connection. Its models do not load, the
// answer names the certificate as its own kind of failure, and no request reaches the server: the
// handshake is broken off before anything is sent.
scenario('chat-provider-untrusted-cert', {}, async (ctx) => {
  const instance = await ctx.startInstance()
  await createAndUnlock(instance, { name: 'cert-vault' })
  const provider = await ctx.provider(undefined, { selfSignedTls: true })
  assert.match(provider.baseUrl, /^https:/)

  const added = unwrap<{
    provider: { id: string }
    modelCount: number | null
    refreshError: string | null
  }>(
    'add_provider',
    await instance.invoke('add_provider', {
      args: {
        kind: 'api_key',
        name: 'self-signed',
        adapter: 'anthropic',
        baseUrl: provider.baseUrl,
        apiKey: randomUUID(),
      },
    }),
  )
  assert.equal(added.modelCount, null, 'no models from that server')
  assert.match(added.refreshError ?? '', /certificate is not trusted/)
  ctx.step('adding the provider: its models do not load')

  const refreshed = await instance.invoke('refresh_provider_models', {
    providerId: added.provider.id,
  })
  assert.ok('ok' in refreshed && !refreshed.ok, 'the refresh fails')
  assert.equal(
    (refreshed.error as { kind?: string }).kind,
    'UntrustedCertificate',
  )
  ctx.step('loading the models again names the certificate')

  assert.deepEqual(provider.requests(), [], 'no request reached the server')
  assert.ok(
    provider.refusedHandshakes() >= 2,
    'the device broke off each handshake',
  )
})
