import { invoke } from '@tauri-apps/api/core'
import { listen, type UnlistenFn } from '@tauri-apps/api/event'

export interface Thread {
  id: string
  title: string
  lastModelId: string | null
  createdAt: number
  updatedAt: number
}

export interface Message {
  id: string
  threadId: string
  parentId: string | null
  role: 'user' | 'assistant' | 'system'
  content: string
  modelId: string | null
  promptTokens: number | null
  completionTokens: number | null
  finishReason: 'complete' | 'cancelled' | 'error' | null
  createdAt: number
}

export interface LoadedModelInfo {
  modelId: string
  name: string
  tokenizerRepo: string
  contextWindow: number | null
}

export interface SendMessageArgs {
  threadId?: string | null
  content: string
  systemPrompt?: string
  maxNewTokens?: number
}

export interface SendMessageResult {
  threadId: string
  userMessageId: string
  assistantMessageId: string
}

export interface TokenEvent {
  messageId: string
  delta: string
  /**
   * Reasoning-content delta (chain-of-thought for Harmony-format local
   * models or Anthropic `thinking_delta` events). `null` for chunks
   * that carry only regular content.
   */
  reasoning: string | null
}

export interface MessageCompleteEvent {
  messageId: string
  threadId: string
  promptTokens: number | null
  completionTokens: number | null
  ttftMs: number | null
}

export interface MessageErrorEvent {
  messageId: string
  threadId: string
  reason: string
}

/**
 * Chat runtime: thread + message CRUD, model load/unload, streaming
 * send with three companion Tauri events (`chat-token`,
 * `chat-message-complete`, `chat-message-error`).
 */
export function useChat() {
  /** Lists chat threads for the active instance. */
  async function listThreadsAsync(): Promise<Thread[]> {
    return await invoke<Thread[]>('list_threads')
  }

  /** Lists persisted messages in chronological order for one thread. */
  async function listMessagesAsync(threadId: string): Promise<Message[]> {
    return await invoke<Message[]>('list_messages', { threadId })
  }

  /** Creates and returns an empty chat thread. */
  async function createThreadAsync(title?: string): Promise<Thread> {
    return await invoke<Thread>('create_thread', { args: { title: title ?? null } })
  }

  /** Persists a user turn and starts streaming the assistant response. */
  async function sendMessageAsync(args: SendMessageArgs): Promise<SendMessageResult> {
    return await invoke<SendMessageResult>('send_message', { args })
  }

  /** Aborts the active generation, if one is running. */
  async function abortAsync(): Promise<void> {
    return await invoke<void>('abort_current_generation')
  }

  /**
   * Loads a model into the chat session. Accepts either a local
   * catalog id (`"llama-3.1-8b"`) or a composite api_key id
   * (`"<provider_uuid>:claude-opus-5"`); the backend routes on the
   * colon.
   */
  async function loadModelAsync(modelId: string): Promise<LoadedModelInfo> {
    return await invoke<LoadedModelInfo>('load_model', { modelId })
  }

  /** Unloads the model currently held by the chat session. */
  async function unloadModelAsync(): Promise<void> {
    return await invoke<void>('unload_local_model')
  }

  /** Returns metadata for the loaded model, or `null` when none is loaded. */
  async function activeModelInfoAsync(): Promise<LoadedModelInfo | null> {
    return await invoke<LoadedModelInfo | null>('active_model_info')
  }

  /** Subscribes to streamed token deltas and returns the unlisten function. */
  async function onToken(
    handler: (e: TokenEvent) => void,
  ): Promise<UnlistenFn> {
    return await listen<TokenEvent>('chat-token', (ev) => handler(ev.payload))
  }

  /** Subscribes to successful generation completions and returns the unlisten function. */
  async function onMessageComplete(
    handler: (e: MessageCompleteEvent) => void,
  ): Promise<UnlistenFn> {
    return await listen<MessageCompleteEvent>('chat-message-complete', (ev) =>
      handler(ev.payload),
    )
  }

  /** Subscribes to failed generations and returns the unlisten function. */
  async function onMessageError(
    handler: (e: MessageErrorEvent) => void,
  ): Promise<UnlistenFn> {
    return await listen<MessageErrorEvent>('chat-message-error', (ev) =>
      handler(ev.payload),
    )
  }

  return {
    listThreadsAsync,
    listMessagesAsync,
    createThreadAsync,
    sendMessageAsync,
    abortAsync,
    loadModelAsync,
    unloadModelAsync,
    activeModelInfoAsync,
    onToken,
    onMessageComplete,
    onMessageError,
  }
}
