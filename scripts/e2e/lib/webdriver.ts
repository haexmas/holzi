// A minimal W3C WebDriver client over fetch, for exactly the calls the suite needs. Not a general
// client: it talks to the driver that starts the real application (tauri-driver).
const ELEMENT_KEY = 'element-6066-11e4-a52e-4f735466cecf'

export type InvokeResult =
  { ok: true; data: unknown } | { ok: false; error: unknown } | { ended: true }

export interface InvokeOptions {
  /** The call closes the application, so no answer is expected: report `{ ended: true }` instead of failing. */
  expectEnd?: boolean
}

/** The driver answered with an error of its own, such as a script error. */
export class WebDriverError extends Error {
  code: string

  constructor(code: string, message: string) {
    super(`${code}: ${message}`)
    this.name = 'WebDriverError'
    this.code = code
  }
}

/** The session or the application behind it is gone, or the driver cannot be reached. */
export class SessionGoneError extends Error {
  constructor(message: string) {
    super(message)
    this.name = 'SessionGoneError'
  }
}

const GONE_CODES = new Set([
  'invalid session id',
  'no such window',
  'session not created',
])

export class WebDriverClient {
  private base: string
  private id: string | null = null
  private limitMs: number

  constructor(baseUrl: string, options: { callLimitMs?: number } = {}) {
    this.base = baseUrl
    // Longer than the 60 s script timeout the session sets, so the driver's own timeout answers first.
    this.limitMs = options.callLimitMs ?? 90_000
  }

  get sessionId(): string | null {
    return this.id
  }

  private async call(
    method: string,
    path: string,
    body?: unknown,
    limitMs = this.limitMs,
  ): Promise<unknown> {
    let response: Response
    try {
      response = await fetch(`${this.base}${path}`, {
        method,
        headers: { 'content-type': 'application/json' },
        body: body === undefined ? undefined : JSON.stringify(body),
        signal: AbortSignal.timeout(limitMs),
      })
    } catch (error) {
      throw new SessionGoneError(
        `${method} ${path} got no answer: ${(error as Error).message}`,
      )
    }
    const text = await response.text()
    let parsed: { value?: unknown } | undefined
    try {
      parsed = text ? (JSON.parse(text) as { value?: unknown }) : undefined
    } catch {
      parsed = undefined
    }
    if (!response.ok) {
      const value = parsed?.value as
        { error?: string; message?: string } | undefined
      const code = value?.error ?? `http ${response.status}`
      const message = value?.message ?? text.slice(0, 200)
      if (
        response.status >= 500 &&
        (value?.error === undefined || value.error === 'unknown error')
      ) {
        throw new SessionGoneError(
          `${method} ${path} failed with ${response.status}: ${message}`,
        )
      }
      if (GONE_CODES.has(code))
        throw new SessionGoneError(`${method} ${path}: ${code}: ${message}`)
      throw new WebDriverError(code, message)
    }
    return parsed?.value
  }

  private session(path: string): string {
    if (this.id === null) throw new Error('no WebDriver session is open')
    return `/session/${this.id}${path}`
  }

  async newSession(
    application: string,
    options: { scriptTimeoutMs?: number } = {},
  ): Promise<string> {
    const created = (await this.call('POST', '/session', {
      capabilities: { alwaysMatch: { 'tauri:options': { application } } },
    })) as { sessionId: string }
    this.id = created.sessionId
    await this.call('POST', this.session('/timeouts'), {
      script: options.scriptTimeoutMs ?? 60_000,
    })
    return this.id
  }

  async deleteSession(): Promise<void> {
    if (this.id === null) return
    const path = this.session('')
    this.id = null
    // A hung application must not hang the teardown.
    await this.call('DELETE', path, undefined, 5000)
  }

  async execute<T = unknown>(script: string, args: unknown[] = []): Promise<T> {
    return (await this.call('POST', this.session('/execute/sync'), {
      script,
      args,
    })) as T
  }

  /**
   * Call a backend command from the page. Resolves to an object instead of throwing, so an error of the
   * command is told apart from an application that ended before it could answer.
   */
  async invoke(
    command: string,
    args: unknown = {},
    options: InvokeOptions = {},
  ): Promise<InvokeResult> {
    const script = `
      const done = arguments[arguments.length - 1]
      window.__TAURI_INTERNALS__.invoke(${JSON.stringify(command)}, ${JSON.stringify(args)})
        .then((data) => done({ ok: true, data }))
        .catch((error) => done({ ok: false, error }))
    `
    try {
      return (await this.call('POST', this.session('/execute/async'), {
        script,
        args: [],
      })) as InvokeResult
    } catch (error) {
      if (options.expectEnd === true && error instanceof SessionGoneError)
        return { ended: true }
      throw new Error(
        `backend call ${command} failed: ${(error as Error).message}`,
        { cause: error },
      )
    }
  }

  async findElements(using: string, value: string): Promise<string[]> {
    const found = (await this.call('POST', this.session('/elements'), {
      using,
      value,
    })) as Record<string, string>[]
    return found.map((element) => element[ELEMENT_KEY])
  }

  async click(element: string): Promise<void> {
    await this.call('POST', this.session(`/element/${element}/click`), {})
  }

  async sendKeys(element: string, text: string): Promise<void> {
    await this.call('POST', this.session(`/element/${element}/value`), { text })
  }

  async screenshot(): Promise<Buffer> {
    return Buffer.from(
      (await this.call('GET', this.session('/screenshot'))) as string,
      'base64',
    )
  }

  async closeWindow(): Promise<void> {
    await this.call('DELETE', this.session('/window'))
  }

  async navigate(url: string): Promise<void> {
    await this.call('POST', this.session('/url'), { url })
  }
}
