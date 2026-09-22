// Convenience built on the interaction surface (contracts/helpers.md "Convenience built on the above"):
// what almost every scenario needs, so a scenario itself stays short. Field names mirror the frontend's
// own command wrappers (src/composables/useInstance.ts, useProviders.ts, useChat.ts), duck-typed here
// rather than imported, since scripts/e2e runs outside the frontend's own build.
import { randomBytes, randomUUID } from 'node:crypto'
import type { InvokeOptions, InvokeResult } from './webdriver.ts'
import type { StepRecorder } from './page.ts'
import type { Provider } from './provider.ts'

/** The slice of an instance these flows need — any real `Instance` (scripts/e2e/lib/instance.ts) satisfies it. */
export interface FlowInstance {
  invoke(
    command: string,
    args?: unknown,
    options?: InvokeOptions,
  ): Promise<InvokeResult>
  click(hook: string, deadlineMs?: number): Promise<void>
  type(hook: string, text: string, deadlineMs?: number): Promise<void>
  exec<T = unknown>(script: string, args?: unknown[]): Promise<T>
  step: StepRecorder
}

function generateSecret(): string {
  return randomBytes(18).toString('base64url')
}

function describe(value: unknown): string {
  try {
    return JSON.stringify(value) ?? String(value)
  } catch {
    return String(value)
  }
}

/** The data of a successful `invoke`, or a clear error naming the command that failed or never answered. */
function unwrap<T>(action: string, result: InvokeResult): T {
  if ('ended' in result) {
    throw new Error(`${action}: the application ended before it answered`)
  }
  if (!result.ok) {
    throw new Error(`${action} failed: ${describe(result.error)}`)
  }
  return result.data as T
}

async function waitForPath(
  instance: FlowInstance,
  prefix: string,
  deadlineMs = 5000,
): Promise<void> {
  const end = Date.now() + deadlineMs
  for (;;) {
    const last = await instance.exec<string>('return location.pathname')
    if (last.startsWith(prefix)) return
    if (Date.now() >= end) {
      throw new Error(
        `timed out after ${deadlineMs} ms waiting for the address to start with "${prefix}" (last observed: "${last}")`,
      )
    }
    await new Promise((resolve) => setTimeout(resolve, 50))
  }
}

interface CreateInstanceResult {
  info: { name: string }
}

/**
 * Creates a vault and unlocks it: `create_instance` by backend call, click the instance entry by hook,
 * type the generated passphrase, click the unlock button, wait for the workspace address. Records
 * `unlocked`.
 */
export async function createAndUnlock(
  instance: FlowInstance,
  options: { name: string },
): Promise<void> {
  const passphrase = generateSecret()
  unwrap<CreateInstanceResult>(
    'create_instance',
    await instance.invoke('create_instance', {
      args: { name: options.name, passphrase },
    }),
  )
  const entryHook = `[data-testid="instance-entry"][data-instance-name="${options.name.replace(/"/g, '\\"')}"]`
  await instance.click(entryHook)
  await instance.type('#unlock-passphrase', passphrase)
  await instance.click('[form="unlock-form"]')
  await waitForPath(instance, '/workspace/')
  instance.step('unlocked')
}

/** Clicks the open-chat hook and waits for the chat address. */
export async function openChat(instance: FlowInstance): Promise<void> {
  await instance.click('open-chat')
  await waitForPath(instance, '/chat/')
}

interface AddProviderResult {
  provider: { id: string }
}

/**
 * `add_provider` with the stand-in's base address and a generated key, then `load_model` for the
 * stand-in's model. Returns the model id (`<provider id>:stand-in-model`).
 */
export async function connectProvider(
  instance: FlowInstance,
  provider: Provider,
): Promise<string> {
  const added = unwrap<AddProviderResult>(
    'add_provider',
    await instance.invoke('add_provider', {
      args: {
        kind: 'api_key',
        name: 'stand-in',
        adapter: 'anthropic',
        baseUrl: provider.baseUrl,
        apiKey: generateSecret(),
      },
    }),
  )
  const modelId = `${added.provider.id}:${provider.modelId}`
  unwrap('load_model', await instance.invoke('load_model', { modelId }))
  return modelId
}

/**
 * `send_message`; records `reply-streaming` once the stand-in provider reports an open connection, so a
 * scenario can rely on the reply being genuinely in flight before it acts (e.g. pressing lock).
 */
export async function startReply(
  instance: FlowInstance,
  provider: Provider,
  text: string,
): Promise<void> {
  unwrap(
    'send_message',
    await instance.invoke('send_message', {
      args: { content: text, idempotencyKey: randomUUID() },
    }),
  )
  await provider.waitForOpen()
  instance.step('reply-streaming')
}
