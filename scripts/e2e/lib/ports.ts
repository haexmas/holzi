import net from 'node:net'

export class PortInUseError extends Error {
  port: number

  constructor(port: number) {
    super(`port ${port} is already in use`)
    this.name = 'PortInUseError'
    this.port = port
  }
}

/**
 * A port on the loopback interface that is free at this moment (FR-009).
 * ponytail: the port is bound and released, so another process can take it before the caller uses it.
 * One retry covers that in practice (`withPortRetry`); the upgrade path is to hand the caller an already
 * bound socket, which needs a driver that accepts an inherited one.
 */
export function freePort(): Promise<number> {
  return new Promise((resolve, reject) => {
    const server = net.createServer()
    server.once('error', reject)
    server.listen(0, '127.0.0.1', () => {
      const address = server.address()
      if (address === null || typeof address === 'string') {
        server.close()
        reject(new Error('the operating system gave no port'))
        return
      }
      server.close(() => resolve(address.port))
    })
  })
}

/** Run an operation that picks its own ports, and run it once more if a port turned out to be taken. */
export async function withPortRetry<T>(
  operation: (attempt: number) => Promise<T>,
): Promise<T> {
  try {
    return await operation(0)
  } catch (error) {
    if (!(error instanceof PortInUseError)) throw error
  }
  return operation(1)
}
