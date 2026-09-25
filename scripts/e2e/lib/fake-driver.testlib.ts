// Test-only fake of the WebDriver server that tauri-driver exposes. It speaks the W3C calls the suite's
// client uses, records every request, and lets a test decide what a script call answers, including
// dropping the connection the way a closing application does.
import http from 'node:http'

export const ELEMENT_KEY = 'element-6066-11e4-a52e-4f735466cecf'

export interface Recorded {
  method: string
  path: string
  body: unknown
}

export type ExecuteReply =
  { value: unknown } | { status: number; body: unknown } | 'drop'
export type ExecuteHandler = (
  kind: 'sync' | 'async',
  script: string,
  args: unknown[],
) => ExecuteReply

export interface FakeDriver {
  url: string
  sessionId: string
  requests: Recorded[]
  /** What the next script calls answer. The default answers `null`, and `{ ok: true, data: null }` for async. */
  onExecute(handler: ExecuteHandler): void
  /** Which element ids a lookup by `using` and `value` returns. The default is one element, `el-1`. */
  onFind(finder: (using: string, value: string) => string[]): void
  /** Whether a given element id is displayed. The default says every element is. */
  onDisplayed(displayed: (id: string) => boolean): void
  /**
   * Whether a click on the given element succeeds, drops the connection (gone-session style), or
   * answers with an element reference error after the DOM moved on since it was found.
   */
  onClick(
    click: (id: string) => 'ok' | 'drop' | 'stale' | 'not-interactable',
  ): void
  /** Whether a `POST /session` (new session) succeeds, drops the connection, or is rejected. */
  onNewSession(newSession: () => 'ok' | 'drop' | 'not-created'): void
  close(): Promise<void>
}

export async function startFakeDriver(): Promise<FakeDriver> {
  const sessionId = 'fake-session'
  const requests: Recorded[] = []
  let handler: ExecuteHandler = (kind) =>
    kind === 'async' ? { value: { ok: true, data: null } } : { value: null }
  let finder: (using: string, value: string) => string[] = () => ['el-1']
  let displayed: (id: string) => boolean = () => true
  let click: (
    id: string,
  ) => 'ok' | 'drop' | 'stale' | 'not-interactable' = () => 'ok'
  let newSession: () => 'ok' | 'drop' | 'not-created' = () => 'ok'

  const server = http.createServer((req, res) => {
    const chunks: Buffer[] = []
    req.on('data', (c: Buffer) => chunks.push(c))
    req.on('end', () => {
      const raw = Buffer.concat(chunks).toString('utf8')
      const body: unknown = raw ? JSON.parse(raw) : undefined
      const path = req.url ?? ''
      const method = req.method ?? ''
      requests.push({ method, path, body })
      const reply = (value: unknown, status = 200) => {
        res.writeHead(status, { 'content-type': 'application/json' })
        res.end(JSON.stringify({ value }))
      }
      const base = `/session/${sessionId}`
      if (method === 'POST' && path === '/session') {
        const outcome = newSession()
        if (outcome === 'drop') {
          req.socket.destroy()
          return
        }
        if (outcome === 'not-created')
          return reply(
            {
              error: 'session not created',
              message: 'native driver unavailable',
            },
            500,
          )
        return reply({ sessionId, capabilities: {} })
      }
      if (method === 'DELETE' && path === base) return reply(null)
      if (method === 'POST' && path === `${base}/timeouts`) return reply(null)
      const exec = path.match(new RegExp(`^${base}/execute/(sync|async)$`))
      if (method === 'POST' && exec) {
        const { script, args } = body as { script: string; args: unknown[] }
        const answer = handler(exec[1] as 'sync' | 'async', script, args ?? [])
        if (answer === 'drop') {
          req.socket.destroy()
          return
        }
        if ('status' in answer) {
          res.writeHead(answer.status, { 'content-type': 'application/json' })
          res.end(JSON.stringify(answer.body))
          return
        }
        return reply(answer.value)
      }
      if (method === 'POST' && path === `${base}/element`) {
        const { using, value } = body as { using: string; value: string }
        const [id] = finder(using, value)
        return id
          ? reply({ [ELEMENT_KEY]: id })
          : reply({ error: 'no such element' }, 404)
      }
      if (method === 'POST' && path === `${base}/elements`) {
        const { using, value } = body as { using: string; value: string }
        return reply(finder(using, value).map((id) => ({ [ELEMENT_KEY]: id })))
      }
      const clickMatch = path.match(/\/element\/([^/]+)\/click$/)
      if (method === 'POST' && clickMatch) {
        const outcome = click(clickMatch[1])
        if (outcome === 'drop') {
          req.socket.destroy()
          return
        }
        if (outcome === 'stale') {
          return reply({ error: 'stale element reference', message: '' }, 404)
        }
        if (outcome === 'not-interactable') {
          return reply({ error: 'element not interactable', message: '' }, 400)
        }
        return reply(null)
      }
      if (method === 'POST' && /\/element\/[^/]+\/value$/.test(path))
        return reply(null)
      const displayedMatch = path.match(/\/element\/([^/]+)\/displayed$/)
      if (method === 'GET' && displayedMatch)
        return reply(displayed(displayedMatch[1]))
      if (method === 'GET' && path === `${base}/screenshot`)
        return reply(Buffer.from('png').toString('base64'))
      if (method === 'DELETE' && path === `${base}/window`) return reply([])
      if (method === 'POST' && path === `${base}/url`) return reply(null)
      reply({ error: 'unknown command', message: `${method} ${path}` }, 404)
    })
  })

  await new Promise<void>((resolve) => server.listen(0, '127.0.0.1', resolve))
  const address = server.address()
  if (address === null || typeof address === 'string')
    throw new Error('fake driver has no port')
  return {
    url: `http://127.0.0.1:${address.port}`,
    sessionId,
    requests,
    onExecute(next) {
      handler = next
    },
    onFind(next) {
      finder = next
    },
    onDisplayed(next) {
      displayed = next
    },
    onClick(next) {
      click = next
    },
    onNewSession(next) {
      newSession = next
    },
    close: () =>
      new Promise<void>((resolve) => {
        server.closeAllConnections()
        server.close(() => resolve())
      }),
  }
}
