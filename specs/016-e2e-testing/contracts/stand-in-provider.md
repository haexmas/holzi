# Contract: the stand-in provider

**Feature**: [spec.md](../spec.md) | **Plan**: [plan.md](../plan.md)

A local server that plays an external model provider of the Anthropic kind, so a reply can be running,
slow or failing without the internet (FR-007, FR-015).

## Facing the application

The application is pointed at it through its existing provider settings, with no application change:
a backend call `add_provider` with `kind: 'api_key'`, `adapter: 'anthropic'`, `baseUrl` set to the
server's address and any generated key, then `load_model` with the model id `<provider id>:stand-in-model`
and `send_message`. The application talks to it as it would to the real service.

| Request             | Answer                                                                                                                   |
| ------------------- | ------------------------------------------------------------------------------------------------------------------------ |
| `GET /v1/models`    | JSON list with one model, `stand-in-model`, in the shape the adapter parses (`data`, `has_more`, `first_id`, `last_id`). |
| `POST /v1/messages` | Depends on the behavior below. The request body is recorded.                                                             |
| anything else       | 404.                                                                                                                     |

Event shapes follow what the adapter's own tests feed it (`message_start`, `content_block_start`,
`content_block_delta` with `text_delta`, `content_block_stop`, `message_delta`, `message_stop`, sent as
server-sent events with `event:` and `data:` lines).

## Behaviors

| Behavior             | Parameters (defaults)                    | Effect on `POST /v1/messages`                                                                     |
| -------------------- | ---------------------------------------- | ------------------------------------------------------------------------------------------------- |
| `stream-forever`     | `intervalMs` (100), `text` (`tick `)     | Sends the start events, then a text delta every interval, until the client closes the connection. |
| `stream-then-finish` | `chunks` (5), `intervalMs` (200), `text` | Sends that many deltas, then the stop events, and ends the response.                              |
| `error`              | `status` (500), `body`                   | Answers at once with that status and a JSON error body of the provider's error shape.             |

The behavior applies to requests that arrive after it is set, so a scenario can change it between turns.

## Facing the scenario

```ts
provider.baseUrl: string
provider.modelId: string                          // 'stand-in-model'
provider.behave(behavior: Behavior): void
provider.connections(): Connection[]              // { id, openedAt, closedAt?, method, path }
provider.requests(): Request[]                    // { id, at, method, path, body? }
provider.waitForOpen(opts?): Promise<Connection>  // resolves when a POST /v1/messages connection is open
provider.close(): Promise<void>
```

- Times come from the runner's one clock, so "closed within 1 second of the press" compares two
  timestamps taken by the same clock (`Date.now()` of the runner process).
- `closedAt` is set from the socket's close event, not from the end of the response, so an aborted
  stream is recorded when the application drops the connection.
- Credentials (`x-api-key`, `authorization`) are never recorded.

## Boundaries

- Listens on `127.0.0.1` only, on a port the operating system chooses.
- Serves nothing but the routes above and holds no state beyond the records.
- Started and closed by the scenario context; it never outlives its scenario.
