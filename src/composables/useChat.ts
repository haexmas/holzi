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
  async function listThreadsAsync(): Promise<Thread[]> {
    return await invoke<Thread[]>('list_threads')
  }

  async function listMessagesAsync(threadId: string): Promise<Message[]> {
    return await invoke<Message[]>('list_messages', { threadId })
  }

  async function createThreadAsync(title?: string): Promise<Thread> {
    return await invoke<Thread>('create_thread', { args: { title: title ?? null } })
  }

  async function sendMessageAsync(args: SendMessageArgs): Promise<SendMessageResult> {
    return await invoke<SendMessageResult>('send_message', { args })
  }

  async function abortAsync(): Promise<void> {
    return await invoke<void>('abort_current_generation')
  }

  async function loadModelAsync(modelId: string): Promise<LoadedModelInfo> {
    return await invoke<LoadedModelInfo>('load_local_model', { modelId })
  }

  async function unloadModelAsync(): Promise<void> {
    return await invoke<void>('unload_local_model')
  }

  async function activeModelInfoAsync(): Promise<LoadedModelInfo | null> {
    return await invoke<LoadedModelInfo | null>('active_model_info')
  }

  async function onToken(
    handler: (e: TokenEvent) => void,
  ): Promise<UnlistenFn> {
    return await listen<TokenEvent>('chat-token', (ev) => handler(ev.payload))
  }

  async function onMessageComplete(
    handler: (e: MessageCompleteEvent) => void,
  ): Promise<UnlistenFn> {
    return await listen<MessageCompleteEvent>('chat-message-complete', (ev) =>
      handler(ev.payload),
    )
  }

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
