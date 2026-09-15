//! The turn/step loop: runs one `send_message` turn to completion,
//! including tool-permission gating, automatic retry of transient
//! failures, and persisting every row along the way.
//!
//! Split out of `chat/commands.rs` (2026-09-15 review) — fourth of five
//! steps. See `chat/commands.rs`'s own history for the rest of the split
//! plan.

use std::sync::Arc;

use serde_json::Value;
use tokio_util::sync::CancellationToken;
use uuid::Uuid;

use crate::adapters::types::{
    ChatMessage as LlmMessage, ChatRequest, ChatRole, StreamChunk, StreamError,
    ToolCall as LlmToolCall,
};
use crate::chat::tools::permission::{self, PermissionMode};
use crate::chat::tools::{ApprovalDecision, Tool, ToolResult as ToolExecResult};
use crate::storage::chat_messages::{self as msg_store, ChatMessage, FinishReason, MessageRole};
use crate::storage::chat_threads as thread_store;
use crate::storage::preferences::{self, PrefScope};

use super::events::{
    risk_class_str, strip_leaked_tool_call_markup, MessageCompleteEvent, MessageErrorEvent,
    RetryEvent, TokenEvent, ToolCallEvent, ToolPermissionRequestEvent, ToolResultEvent,
    TurnCompleteEvent, EVENT_CHAT_MESSAGE_COMPLETE, EVENT_CHAT_MESSAGE_ERROR, EVENT_CHAT_RETRY,
    EVENT_CHAT_TOKEN, EVENT_CHAT_TOOL_CALL, EVENT_CHAT_TOOL_RESULT, EVENT_CHAT_TURN_COMPLETE,
    EVENT_TOOL_PERMISSION_REQUEST,
};
use super::session::{ActiveSession, ChatState};

/// Device preference read fresh before every tool call (T024B); parsed via
/// `PermissionMode::parse`, defaulting to `Manual` when unset or invalid
/// (spec.md Assumptions). No dedicated get/set command — read/written
/// through the existing generic `get_pref`/`set_pref` (data-model.md).
const PREF_PERMISSION_MODE: &str = "chat.permission_mode";

/// Fixed cap on the number of tool-calling rounds within one turn
/// (spec.md FR-016). A "round" is one step whose response contained at
/// least one tool call. Reaching the cap ends the turn with
/// `FinishReason::ToolLimitReached` instead of issuing a further step.
pub const MAX_TOOL_ROUNDS: usize = 8;

/// Bounded automatic retry for a transient LLM-request failure (spec.md
/// FR-012), either from `stream_chat` itself or mid-stream. Worst case
/// adds 500ms + 1s + 2s = 3.5s of backoff across the 3 retries (4 attempts
/// total) before falling back to a terminal error.
pub const MAX_RETRY_ATTEMPTS: usize = 3;

/// Exponential backoff for retry attempt `attempt` (0-based: the first
/// retry is `attempt == 0`).
fn retry_backoff(attempt: usize) -> std::time::Duration {
    std::time::Duration::from_millis(500u64 << attempt.min(4))
}

/// Persists one row. Used for every row except the turn's terminal
/// assistant row, which also needs `chat_threads` updated atomically
/// (see [`persist_final_message`]).
async fn persist_message(
    db: &haex_crdt::Database,
    msg: ChatMessage,
) -> std::result::Result<(), String> {
    let db = db.clone();
    tauri::async_runtime::spawn_blocking(move || {
        db.with_connection(|conn| {
            msg_store::insert_message(conn, &msg)
                .map(|_| ())
                .map_err(haex_crdt::Error::from)
        })
    })
    .await
    .map_err(|e| format!("persist join: {e}"))?
    .map_err(|e| format!("persist failed: {e}"))
}

/// Persists the turn's terminal assistant row and updates `chat_threads`
/// in the same connection call — mirrors the pre-tool-loop behavior where
/// both happened atomically together.
async fn persist_final_message(
    db: &haex_crdt::Database,
    msg: ChatMessage,
    provider_id: Option<Uuid>,
    model_id: String,
) -> std::result::Result<(), String> {
    let db = db.clone();
    let thread_id = msg.thread_id;
    let now = msg.created_at;
    tauri::async_runtime::spawn_blocking(move || {
        db.with_connection(|conn| {
            let tx = conn.unchecked_transaction()?;
            let thread = thread_store::get_thread(&tx, thread_id)?
                .ok_or(haex_crdt::rusqlite::Error::QueryReturnedNoRows)?;
            msg_store::insert_message(&tx, &msg)?;
            thread_store::update_thread(
                &tx,
                thread_id,
                &thread.title,
                provider_id,
                Some(&model_id),
                now,
            )?;
            tx.commit().map_err(haex_crdt::Error::from)
        })
    })
    .await
    .map_err(|e| format!("persist join: {e}"))?
    .map_err(|e| format!("persist failed: {e}"))?;
    Ok(())
}

/// Ends a turn as `Cancelled` from inside a tool round, persisting a
/// terminal assistant row with no content (spec.md FR-009–FR-011). Mirrors
/// the per-step event pair the plain-generation cancellation path already
/// emits (`chat-message-complete` then `chat-turn-complete`, not
/// `chat-message-error` — cancelling is not itself an error).
async fn persist_cancelled_turn(
    db: &haex_crdt::Database,
    session: &ActiveSession,
    thread_id: Uuid,
    assistant_message_id: Uuid,
    parent_id: Uuid,
    created_at: i64,
    emit: &mut (dyn FnMut(&'static str, Value) + Send),
) {
    let final_msg = ChatMessage {
        role: MessageRole::Assistant,
        finish_reason: Some(FinishReason::Cancelled),
        created_at,
        ..empty_tool_message(assistant_message_id, thread_id, Some(parent_id))
    };
    let persisted =
        persist_final_message(db, final_msg, session.provider_id, session.model_id.clone()).await;
    match persisted {
        Ok(()) => {
            emit(
                EVENT_CHAT_MESSAGE_COMPLETE,
                serde_json::to_value(MessageCompleteEvent {
                    message_id: assistant_message_id,
                    thread_id,
                    prompt_tokens: None,
                    completion_tokens: None,
                    ttft_ms: None,
                })
                .expect("MessageCompleteEvent always serializes"),
            );
            emit(
                EVENT_CHAT_TURN_COMPLETE,
                serde_json::to_value(TurnCompleteEvent {
                    thread_id,
                    assistant_message_id: Some(assistant_message_id),
                    finish_reason: FinishReason::Cancelled,
                })
                .expect("TurnCompleteEvent always serializes"),
            );
        }
        Err(reason) => {
            emit(
                EVENT_CHAT_MESSAGE_ERROR,
                serde_json::to_value(MessageErrorEvent {
                    message_id: assistant_message_id,
                    thread_id,
                    reason,
                })
                .expect("MessageErrorEvent always serializes"),
            );
            emit(
                EVENT_CHAT_TURN_COMPLETE,
                serde_json::to_value(TurnCompleteEvent {
                    thread_id,
                    assistant_message_id: None,
                    finish_reason: FinishReason::Error,
                })
                .expect("TurnCompleteEvent always serializes"),
            );
        }
    }
}

/// Reads `chat.permission_mode` for this device, defaulting to `Manual`
/// when unset or unparseable (spec.md Assumptions).
async fn read_permission_mode(db: &haex_crdt::Database) -> PermissionMode {
    let db = db.clone();
    let this_device = db.device_id();
    let raw = tauri::async_runtime::spawn_blocking(move || {
        db.with_connection(|conn| {
            preferences::get(conn, PrefScope::Device(this_device), PREF_PERMISSION_MODE)
                .map_err(haex_crdt::Error::from)
        })
    })
    .await
    .ok()
    .and_then(|r| r.ok())
    .flatten();
    raw.as_deref()
        .and_then(PermissionMode::parse)
        .unwrap_or_default()
}

/// What to do with one tool call, decided before any concurrent
/// waiting/execution begins (see the comment at its only call site).
enum ToolPlan {
    Allow(Arc<dyn Tool>),
    Ask {
        tool: Arc<dyn Tool>,
        rx: tokio::sync::oneshot::Receiver<ApprovalDecision>,
        request_id: Uuid,
    },
    Deny(Arc<dyn Tool>),
    Unknown,
}

fn empty_tool_message(id: Uuid, thread_id: Uuid, parent_id: Option<Uuid>) -> ChatMessage {
    ChatMessage {
        id,
        thread_id,
        parent_id,
        role: MessageRole::User, // overwritten by every caller
        content: String::new(),
        provider_id: None,
        model_id: None,
        prompt_tokens: None,
        completion_tokens: None,
        finish_reason: None,
        created_at: now_ms(),
        idempotency_key: None,
        tool_name: None,
        tool_call_id: None,
        tool_input: None,
        tool_is_error: None,
        tool_source: None,
    }
}

/// Result of driving one step (a single LLM request/response, including
/// any retries) to completion.
enum StepOutcome {
    /// Reached `Done`. Carries the successful attempt's content.
    Success {
        assembled: String,
        tool_calls: Vec<LlmToolCall>,
        prompt_tokens: Option<usize>,
        completion_tokens: Option<usize>,
        ttft_ms: Option<u64>,
    },
    /// `abort_current_generation` fired — either while consuming a
    /// stream, or during a retry's backoff wait. Carries whatever text
    /// the current (now-abandoned) attempt had already produced.
    Cancelled(String),
    /// Not retryable, or the retry budget was spent. Carries whatever
    /// text the final attempt had already produced before it failed.
    Error { reason: String, partial: String },
}

enum RetryDecision {
    Retry,
    Cancelled,
    Bail(String),
}

/// Decides what to do about one failure while running a step (spec.md
/// FR-012): bail immediately if it is not transient or `MAX_RETRY_ATTEMPTS`
/// is already spent; otherwise emit `chat-retry` and wait out the backoff
/// (cancellable) before telling the caller to try again. Shared by both
/// `stream_chat` itself failing and a mid-stream `StreamError`, so one
/// budget covers either — a failure restarting the stream counts the same
/// as one that happened mid-stream.
#[allow(clippy::too_many_arguments)]
async fn retry_or_bail(
    is_transient: bool,
    error_msg: String,
    attempt: &mut usize,
    cancel: &CancellationToken,
    thread_id: Uuid,
    assistant_message_id: Uuid,
    emit: &mut (dyn FnMut(&'static str, Value) + Send),
) -> RetryDecision {
    if !is_transient || *attempt >= MAX_RETRY_ATTEMPTS {
        return RetryDecision::Bail(error_msg);
    }
    *attempt += 1;
    emit(
        EVENT_CHAT_RETRY,
        serde_json::to_value(RetryEvent {
            thread_id,
            assistant_message_id,
            attempt: *attempt,
        })
        .expect("RetryEvent always serializes"),
    );
    let sleep = tokio::time::sleep(retry_backoff(*attempt - 1));
    tokio::select! {
        biased;
        _ = cancel.cancelled() => RetryDecision::Cancelled,
        _ = sleep => RetryDecision::Retry,
    }
}

pub(crate) enum StreamStartError {
    Cancelled,
    Failed(String),
}

/// Shared by the initial send and subsequent steps; retries share the
/// caller's budget with mid-stream failures and remain cancellable.
#[allow(clippy::too_many_arguments)]
pub(crate) async fn start_step_stream(
    session: &ActiveSession,
    chat_state: &ChatState,
    request: &ChatRequest,
    cancel: &CancellationToken,
    attempt: &mut usize,
    thread_id: Uuid,
    assistant_message_id: Uuid,
    emit: &mut (dyn FnMut(&'static str, Value) + Send),
) -> std::result::Result<crate::adapters::types::AdapterStream, StreamStartError> {
    loop {
        let started = tokio::select! {
            biased;
            _ = cancel.cancelled() => return Err(StreamStartError::Cancelled),
            result = session.adapter.stream_chat(request.clone()) => result,
        };
        match started {
            Ok(stream) => {
                *chat_state
                    .current_generation
                    .lock()
                    .unwrap_or_else(|e| e.into_inner()) = Some(stream.abort_handle());
                return Ok(stream);
            }
            Err(error) => match retry_or_bail(
                error.is_transient(),
                format!("adapter start: {error}"),
                attempt,
                cancel,
                thread_id,
                assistant_message_id,
                emit,
            )
            .await
            {
                RetryDecision::Retry => continue,
                RetryDecision::Cancelled => return Err(StreamStartError::Cancelled),
                RetryDecision::Bail(reason) => return Err(StreamStartError::Failed(reason)),
            },
        }
    }
}

/// Drives one step to completion, automatically retrying a transient
/// failure — from either the initial `stream_chat` call or mid-stream —
/// with backoff, up to `MAX_RETRY_ATTEMPTS` (spec.md FR-012). `stream`
/// is `Some` only for the turn's already-in-flight first step; every
/// other call (a new round after tool use, or a retry attempt) passes
/// `None` and this function calls `stream_chat` itself. Tokens stream
/// live via `emit` exactly as a non-retried step would — on a transient
/// failure `chat-retry` fires immediately so the frontend can clear that
/// attempt's now-discarded partial text before the next attempt's tokens
/// arrive (T039), rather than buffering server-side.
#[allow(clippy::too_many_arguments)]
async fn run_step(
    session: &ActiveSession,
    chat_state: &ChatState,
    request: &ChatRequest,
    mut stream: Option<crate::adapters::types::AdapterStream>,
    attempt: &mut usize,
    cancel: &CancellationToken,
    thread_id: Uuid,
    assistant_message_id: Uuid,
    emit: &mut (dyn FnMut(&'static str, Value) + Send),
) -> StepOutcome {
    loop {
        let mut live_stream = match stream.take() {
            Some(s) => s,
            None => match start_step_stream(
                session,
                chat_state,
                request,
                cancel,
                attempt,
                thread_id,
                assistant_message_id,
                emit,
            )
            .await
            {
                Ok(stream) => stream,
                Err(StreamStartError::Cancelled) => return StepOutcome::Cancelled(String::new()),
                Err(StreamStartError::Failed(reason)) => {
                    return StepOutcome::Error {
                        reason,
                        partial: String::new(),
                    }
                }
            },
        };

        let mut assembled = String::new();
        let mut tool_calls: Vec<LlmToolCall> = Vec::new();
        let mut prompt_tokens: Option<usize> = None;
        let mut completion_tokens: Option<usize> = None;
        let mut ttft_ms: Option<u64> = None;
        let mut saw_done = false;
        let mut stream_error: Option<StreamError> = None;

        loop {
            let item = tokio::select! {
                biased;
                _ = cancel.cancelled() => return StepOutcome::Cancelled(assembled),
                item = live_stream.next() => item,
            };
            match item {
                None => break,
                Some(Ok(StreamChunk::Delta { content, reasoning })) => {
                    if !content.is_empty() || reasoning.is_some() {
                        assembled.push_str(&content);
                        emit(
                            EVENT_CHAT_TOKEN,
                            serde_json::to_value(TokenEvent {
                                message_id: assistant_message_id,
                                delta: content,
                                reasoning,
                            })
                            .expect("TokenEvent always serializes"),
                        );
                    }
                }
                Some(Ok(StreamChunk::ToolCalls(calls))) => tool_calls = calls,
                Some(Ok(StreamChunk::Done {
                    prompt_tokens: pt,
                    completion_tokens: ct,
                    ttft_ms: t,
                    ..
                })) => {
                    saw_done = true;
                    prompt_tokens = pt;
                    completion_tokens = ct;
                    ttft_ms = t;
                    break;
                }
                Some(Err(e)) => {
                    stream_error = Some(e);
                    break;
                }
            }
        }

        if let Some(e) = stream_error {
            match retry_or_bail(
                e.is_transient(),
                e.to_string(),
                attempt,
                cancel,
                thread_id,
                assistant_message_id,
                emit,
            )
            .await
            {
                RetryDecision::Retry => continue,
                RetryDecision::Cancelled => return StepOutcome::Cancelled(assembled),
                RetryDecision::Bail(reason) => {
                    return StepOutcome::Error {
                        reason,
                        partial: assembled,
                    }
                }
            }
        }

        if !saw_done && tool_calls.is_empty() {
            // `abort_current_generation` aborted the stream producer —
            // the channel closed with neither a `Done` frame nor an
            // error. A closed channel right after `ToolCalls` (no `Done`
            // in between) is not itself a cancellation signal — every
            // adapter emits `Done` in the same terminal frame as
            // `ToolCalls`, so the caller decides what to do next purely
            // from `tool_calls` being non-empty, same as before this
            // function existed.
            return StepOutcome::Cancelled(assembled);
        }

        return StepOutcome::Success {
            assembled,
            tool_calls,
            prompt_tokens,
            completion_tokens,
            ttft_ms,
        };
    }
}

/// Drives one `send_message` turn to completion: consumes the
/// already-started first step's stream, executes any tool calls the model
/// requests, issues further steps as needed (bounded by
/// `MAX_TOOL_ROUNDS`), and persists every row along the way. Takes a plain
/// `emit` callback rather than an `AppHandle` so it runs without a live
/// Tauri app — `tests/chat_tool_loop.rs` passes a closure that records
/// events instead of dispatching them; the production caller above wraps
/// `app.emit`. `request` already carries the model/system-prompt/tools
/// used for `stream`'s already-in-flight first step; its `messages` grow
/// as tool rounds are appended for subsequent steps.
#[allow(clippy::too_many_arguments)]
pub async fn run_turn(
    db: &haex_crdt::Database,
    chat_state: &ChatState,
    session: &ActiveSession,
    thread_id: Uuid,
    user_message_id: Uuid,
    assistant_message_id: Uuid,
    mut request: ChatRequest,
    stream: crate::adapters::types::AdapterStream,
    initial_attempt: usize,
    cancel: CancellationToken,
    emit: &mut (dyn FnMut(&'static str, Value) + Send),
) {
    let mut parent_id = user_message_id;
    let mut rounds_used = 0usize;
    // SQLite orders rows by `created_at, id`; UUIDv4 is random, so rows
    // created in one millisecond must receive distinct logical timestamps
    // to keep each call immediately before its result. Carried across tool
    // rounds (not reset per round) so two rounds landing in the same
    // millisecond still sort in round order.
    let mut next_tool_created_at = now_ms();
    // `Some` only for the first iteration (the already-in-flight stream
    // `send_message` started); every later iteration leaves this `None` so
    // `run_step` starts a fresh `stream_chat` call itself — the same
    // uniform path a retry attempt also takes (T037).
    let mut next_stream = Some(stream);
    let mut next_attempt = initial_attempt;

    loop {
        let outcome = run_step(
            session,
            chat_state,
            &request,
            next_stream.take(),
            &mut next_attempt,
            &cancel,
            thread_id,
            assistant_message_id,
            emit,
        )
        .await;
        let (
            assembled,
            tool_calls,
            prompt_tokens,
            completion_tokens,
            ttft_ms,
            error_reason,
            saw_done,
        ) = match outcome {
            StepOutcome::Success {
                assembled,
                tool_calls,
                prompt_tokens,
                completion_tokens,
                ttft_ms,
            } => (
                assembled,
                tool_calls,
                prompt_tokens,
                completion_tokens,
                ttft_ms,
                None,
                true,
            ),
            StepOutcome::Cancelled(partial) => (partial, Vec::new(), None, None, None, None, false),
            StepOutcome::Error { reason, partial } => {
                (partial, Vec::new(), None, None, None, Some(reason), false)
            }
        };

        if error_reason.is_none() && !tool_calls.is_empty() {
            // A step that ends by calling tools. Any text the model
            // emitted first becomes its own interim assistant row
            // (data-model.md's `assistant(*)` — optional, only present
            // when the step actually produced text before its tool use).
            //
            // Strip first: a reasoning-capable local model can leak its raw
            // `<tool_call>` tag into this text (see
            // `strip_leaked_tool_call_markup`) — left in, that markup would
            // otherwise be persisted and shown as if it were the model's
            // own reply.
            let assembled = strip_leaked_tool_call_markup(&assembled);
            if !assembled.trim().is_empty() {
                let interim_id = Uuid::new_v4();
                let msg = ChatMessage {
                    role: MessageRole::Assistant,
                    content: assembled.clone(),
                    provider_id: session.provider_id,
                    model_id: Some(session.model_id.clone()),
                    ..empty_tool_message(interim_id, thread_id, Some(parent_id))
                };
                if let Err(reason) = persist_message(db, msg).await {
                    emit(
                        EVENT_CHAT_MESSAGE_ERROR,
                        serde_json::to_value(MessageErrorEvent {
                            message_id: assistant_message_id,
                            thread_id,
                            reason,
                        })
                        .expect("MessageErrorEvent always serializes"),
                    );
                    emit(
                        EVENT_CHAT_TURN_COMPLETE,
                        serde_json::to_value(TurnCompleteEvent {
                            thread_id,
                            assistant_message_id: None,
                            finish_reason: FinishReason::Error,
                        })
                        .expect("TurnCompleteEvent always serializes"),
                    );
                    return;
                }
                parent_id = interim_id;
                request.messages.push(LlmMessage {
                    role: ChatRole::Assistant,
                    content: assembled,
                });
            }

            // Cancellation may already have fired between this step's
            // `Done`/`ToolCalls` frame and here (e.g. a stray abort that
            // raced the previous step's own completion) — checked before
            // minting any new approval wait so a freshly-inserted sender
            // never sits in `pending_tool_approvals` forever, unreachable
            // by the one-time drain in `abort_turn` (T032/FR-011).
            if cancel.is_cancelled() {
                next_tool_created_at = next_tool_created_at.max(now_ms()).saturating_add(1);
                persist_cancelled_turn(
                    db,
                    session,
                    thread_id,
                    assistant_message_id,
                    parent_id,
                    next_tool_created_at,
                    emit,
                )
                .await;
                return;
            }

            // Decide + (for `Ask`) mint the approval wait sequentially —
            // `emit` is a single `&mut` closure, not shareable across
            // concurrent futures, so every `tool-permission-request` fires
            // here, before any concurrent waiting/execution begins below.
            // This is also what lets two independent Risky calls each get
            // their own simultaneously-pending approval (T024A): each gets
            // its own oneshot the moment its `Ask` is decided, well before
            // either one's wait resolves.
            let mut plans: Vec<(LlmToolCall, ToolPlan)> = Vec::with_capacity(tool_calls.len());
            for call in tool_calls {
                let tool = {
                    let registry = chat_state
                        .tool_registry
                        .lock()
                        .unwrap_or_else(|e| e.into_inner());
                    registry.get(&call.name)
                };
                let Some(tool) = tool else {
                    plans.push((call, ToolPlan::Unknown));
                    continue;
                };
                // Read fresh per call, not cached for the round: a mode
                // change must not retroactively affect a decision already
                // made for an earlier call, but the very next tool use
                // must observe it (T024B).
                let mode = read_permission_mode(db).await;
                match permission::decide(mode, tool.risk_class()) {
                    permission::Decision::Allow => plans.push((call, ToolPlan::Allow(tool))),
                    permission::Decision::Deny => plans.push((call, ToolPlan::Deny(tool))),
                    permission::Decision::Ask => {
                        let request_id = Uuid::new_v4();
                        let (tx, rx) = tokio::sync::oneshot::channel();
                        chat_state
                            .pending_tool_approvals
                            .lock()
                            .unwrap_or_else(|e| e.into_inner())
                            .insert(request_id, tx);
                        emit(
                            EVENT_TOOL_PERMISSION_REQUEST,
                            serde_json::to_value(ToolPermissionRequestEvent {
                                request_id,
                                thread_id,
                                tool_name: call.name.clone(),
                                tool_input: call.input.clone(),
                                risk_class: risk_class_str(tool.risk_class()),
                            })
                            .expect("ToolPermissionRequestEvent always serializes"),
                        );
                        plans.push((
                            call,
                            ToolPlan::Ask {
                                tool,
                                rx,
                                request_id,
                            },
                        ));
                    }
                }
            }

            let executed: Vec<(LlmToolCall, ToolExecResult, &'static str)> =
                futures::future::join_all(plans.into_iter().map(|(call, plan)| {
                    let cancel = cancel.clone();
                    async move {
                        match plan {
                            ToolPlan::Allow(tool) => {
                                let source = tool.source();
                                let result = tool.execute(call.input.clone(), cancel).await;
                                (call, result, source)
                            }
                            ToolPlan::Ask {
                                tool,
                                rx,
                                request_id,
                            } => {
                                // `abort_turn` drops every pending sender
                                // (T032), so a dropped-without-answer `rx`
                                // below always means cancellation, never a
                                // silent auto-decision (FR-005).
                                let tool_cancel = cancel.clone();
                                let decision = tokio::select! {
                                    biased;
                                    _ = cancel.cancelled() => Err(()),
                                    decision = rx => decision.map_err(|_| ()),
                                };
                                match decision {
                                    Ok(ApprovalDecision::Allow) => {
                                        let source = tool.source();
                                        let result =
                                            tool.execute(call.input.clone(), tool_cancel).await;
                                        (call, result, source)
                                    }
                                    Ok(ApprovalDecision::Deny) => (
                                        call,
                                        ToolExecResult::error("denied_by_user"),
                                        tool.source(),
                                    ),
                                    Err(_) => {
                                        let mut pending = chat_state
                                            .pending_tool_approvals
                                            .lock()
                                            .unwrap_or_else(|e| e.into_inner());
                                        chat_state
                                            .cancelled_tool_approvals
                                            .lock()
                                            .unwrap_or_else(|e| e.into_inner())
                                            .insert(request_id);
                                        pending.remove(&request_id);
                                        (
                                            call,
                                            ToolExecResult::error("tool_call_cancelled"),
                                            tool.source(),
                                        )
                                    }
                                }
                            }
                            ToolPlan::Deny(tool) => (
                                call,
                                // Fixed, non-localized marker — the frontend
                                // translates it (CONTEXT.md i18n boundary),
                                // same convention as `LoadPhase` above.
                                ToolExecResult::error("blocked_by_plan_mode"),
                                tool.source(),
                            ),
                            ToolPlan::Unknown => {
                                // The model named a tool no longer in the
                                // registry (e.g. its MCP server disconnected
                                // mid-conversation, spec.md Edge Cases) or one
                                // that never existed. `cli` never disappears
                                // (registered unconditionally, T019), so `mcp`
                                // is the more plausible source to record here.
                                let name = call.name.clone();
                                (
                                    call,
                                    ToolExecResult::error(format!("unknown tool: {name}")),
                                    "mcp",
                                )
                            }
                        }
                    }
                }))
                .await;

            // Aborted mid-round (either an in-flight `execute()` was cut
            // short, or a pending approval's sender was dropped): none of
            // this round's rows are persisted, matching the invariant that
            // an interrupted round leaves no trace (data-model.md). The
            // turn ends here — no further step is issued (FR-011/T033).
            if cancel.is_cancelled() {
                next_tool_created_at = next_tool_created_at.max(now_ms()).saturating_add(1);
                persist_cancelled_turn(
                    db,
                    session,
                    thread_id,
                    assistant_message_id,
                    parent_id,
                    next_tool_created_at,
                    emit,
                )
                .await;
                return;
            }

            next_tool_created_at = next_tool_created_at.max(now_ms()).saturating_add(1);
            for (call, result, source) in &executed {
                let tool_call_row_id = Uuid::new_v4();
                let input_json =
                    serde_json::to_string(&call.input).unwrap_or_else(|_| "{}".to_string());
                let call_msg = ChatMessage {
                    role: MessageRole::ToolCall,
                    provider_id: session.provider_id,
                    model_id: Some(session.model_id.clone()),
                    tool_name: Some(call.name.clone()),
                    tool_call_id: Some(call.id.clone()),
                    tool_input: Some(input_json),
                    tool_source: Some(source.to_string()),
                    created_at: next_tool_created_at,
                    ..empty_tool_message(tool_call_row_id, thread_id, Some(parent_id))
                };
                next_tool_created_at = next_tool_created_at.saturating_add(1);
                if let Err(reason) = persist_message(db, call_msg).await {
                    emit(
                        EVENT_CHAT_MESSAGE_ERROR,
                        serde_json::to_value(MessageErrorEvent {
                            message_id: assistant_message_id,
                            thread_id,
                            reason,
                        })
                        .expect("MessageErrorEvent always serializes"),
                    );
                    emit(
                        EVENT_CHAT_TURN_COMPLETE,
                        serde_json::to_value(TurnCompleteEvent {
                            thread_id,
                            assistant_message_id: None,
                            finish_reason: FinishReason::Error,
                        })
                        .expect("TurnCompleteEvent always serializes"),
                    );
                    return;
                }
                parent_id = tool_call_row_id;
                emit(
                    EVENT_CHAT_TOOL_CALL,
                    serde_json::to_value(ToolCallEvent {
                        message_id: tool_call_row_id,
                        thread_id,
                        tool_name: call.name.clone(),
                        tool_input: call.input.clone(),
                        tool_source: source.to_string(),
                    })
                    .expect("ToolCallEvent always serializes"),
                );

                let tool_result_row_id = Uuid::new_v4();
                let result_msg = ChatMessage {
                    role: MessageRole::ToolResult,
                    content: result.content.clone(),
                    provider_id: session.provider_id,
                    model_id: Some(session.model_id.clone()),
                    tool_call_id: Some(call.id.clone()),
                    tool_is_error: Some(result.is_error),
                    created_at: next_tool_created_at,
                    ..empty_tool_message(tool_result_row_id, thread_id, Some(parent_id))
                };
                next_tool_created_at = next_tool_created_at.saturating_add(1);
                if let Err(reason) = persist_message(db, result_msg).await {
                    emit(
                        EVENT_CHAT_MESSAGE_ERROR,
                        serde_json::to_value(MessageErrorEvent {
                            message_id: assistant_message_id,
                            thread_id,
                            reason,
                        })
                        .expect("MessageErrorEvent always serializes"),
                    );
                    emit(
                        EVENT_CHAT_TURN_COMPLETE,
                        serde_json::to_value(TurnCompleteEvent {
                            thread_id,
                            assistant_message_id: None,
                            finish_reason: FinishReason::Error,
                        })
                        .expect("TurnCompleteEvent always serializes"),
                    );
                    return;
                }
                parent_id = tool_result_row_id;
                emit(
                    EVENT_CHAT_TOOL_RESULT,
                    serde_json::to_value(ToolResultEvent {
                        message_id: tool_result_row_id,
                        thread_id,
                        tool_call_id: call.id.clone(),
                        content: result.content.clone(),
                        is_error: result.is_error,
                    })
                    .expect("ToolResultEvent always serializes"),
                );
            }

            // Grouped by role in a separate pass (not interleaved above):
            // both adapters' wire-format builders only merge strictly
            // consecutive same-role rows into one message, so a round with
            // several tool calls must land as one assistant tool-use
            // message followed by one tool-result message, not call/result
            // pairs per call.
            for (call, _, _) in &executed {
                request.messages.push(LlmMessage {
                    role: ChatRole::ToolCall {
                        id: call.id.clone(),
                        name: call.name.clone(),
                        input: call.input.clone(),
                    },
                    content: String::new(),
                });
            }
            for (call, result, _) in &executed {
                request.messages.push(LlmMessage {
                    role: ChatRole::ToolResult {
                        call_id: call.id.clone(),
                        content: result.content.clone(),
                        is_error: result.is_error,
                    },
                    content: String::new(),
                });
            }

            rounds_used += 1;
            if rounds_used >= MAX_TOOL_ROUNDS {
                // Must not reuse a bare `now_ms()` here: a fast round can
                // finish within the same millisecond as its own tool_result
                // row above, and SQLite's `(created_at, id)` ordering would
                // then fall back to comparing random UUIDs, which can sort
                // this terminal row before the round it concludes.
                next_tool_created_at = next_tool_created_at.max(now_ms()).saturating_add(1);
                let final_msg = ChatMessage {
                    role: MessageRole::Assistant,
                    finish_reason: Some(FinishReason::ToolLimitReached),
                    created_at: next_tool_created_at,
                    ..empty_tool_message(assistant_message_id, thread_id, Some(parent_id))
                };
                if let Err(reason) = persist_final_message(
                    db,
                    final_msg,
                    session.provider_id,
                    session.model_id.clone(),
                )
                .await
                {
                    emit(
                        EVENT_CHAT_MESSAGE_ERROR,
                        serde_json::to_value(MessageErrorEvent {
                            message_id: assistant_message_id,
                            thread_id,
                            reason,
                        })
                        .expect("MessageErrorEvent always serializes"),
                    );
                    emit(
                        EVENT_CHAT_TURN_COMPLETE,
                        serde_json::to_value(TurnCompleteEvent {
                            thread_id,
                            assistant_message_id: None,
                            finish_reason: FinishReason::Error,
                        })
                        .expect("TurnCompleteEvent always serializes"),
                    );
                    return;
                }
                emit(
                    EVENT_CHAT_MESSAGE_COMPLETE,
                    serde_json::to_value(MessageCompleteEvent {
                        message_id: assistant_message_id,
                        thread_id,
                        prompt_tokens: None,
                        completion_tokens: None,
                        ttft_ms: None,
                    })
                    .expect("MessageCompleteEvent always serializes"),
                );
                emit(
                    EVENT_CHAT_TURN_COMPLETE,
                    serde_json::to_value(TurnCompleteEvent {
                        thread_id,
                        assistant_message_id: Some(assistant_message_id),
                        finish_reason: FinishReason::ToolLimitReached,
                    })
                    .expect("TurnCompleteEvent always serializes"),
                );
                return;
            }

            // Next round's stream is started by `run_step` itself at the
            // top of the loop (`next_stream` is `None` here) — including
            // its own retry-on-transient-failure handling (T037).
            continue;
        }

        // Final step: a plain answer (no tool calls), a step-level
        // adapter error, or cancellation (the stream closed without
        // `Done` and without an error — `abort_current_generation`).
        let finish_reason = if error_reason.is_some() {
            FinishReason::Error
        } else if saw_done {
            FinishReason::Complete
        } else {
            FinishReason::Cancelled
        };
        // Same tie-breaking reasoning as the `ToolLimitReached` branch
        // above: a fast final step can land in the same millisecond as a
        // preceding round's own rows. Harmless when no round preceded (the
        // `.max(now_ms())` just picks the current time, same as before).
        next_tool_created_at = next_tool_created_at.max(now_ms()).saturating_add(1);
        let final_msg = ChatMessage {
            role: MessageRole::Assistant,
            content: assembled,
            provider_id: session.provider_id,
            model_id: Some(session.model_id.clone()),
            prompt_tokens: prompt_tokens.map(|n| n as i64),
            completion_tokens: completion_tokens.map(|n| n as i64),
            finish_reason: Some(finish_reason),
            created_at: next_tool_created_at,
            ..empty_tool_message(assistant_message_id, thread_id, Some(parent_id))
        };
        if let Err(reason) =
            persist_final_message(db, final_msg, session.provider_id, session.model_id.clone())
                .await
        {
            emit(
                EVENT_CHAT_MESSAGE_ERROR,
                serde_json::to_value(MessageErrorEvent {
                    message_id: assistant_message_id,
                    thread_id,
                    reason,
                })
                .expect("MessageErrorEvent always serializes"),
            );
            emit(
                EVENT_CHAT_TURN_COMPLETE,
                serde_json::to_value(TurnCompleteEvent {
                    thread_id,
                    assistant_message_id: None,
                    finish_reason: FinishReason::Error,
                })
                .expect("TurnCompleteEvent always serializes"),
            );
            return;
        }

        if let Some(reason) = error_reason {
            emit(
                EVENT_CHAT_MESSAGE_ERROR,
                serde_json::to_value(MessageErrorEvent {
                    message_id: assistant_message_id,
                    thread_id,
                    reason,
                })
                .expect("MessageErrorEvent always serializes"),
            );
        } else {
            emit(
                EVENT_CHAT_MESSAGE_COMPLETE,
                serde_json::to_value(MessageCompleteEvent {
                    message_id: assistant_message_id,
                    thread_id,
                    prompt_tokens,
                    completion_tokens,
                    ttft_ms,
                })
                .expect("MessageCompleteEvent always serializes"),
            );
        }
        emit(
            EVENT_CHAT_TURN_COMPLETE,
            serde_json::to_value(TurnCompleteEvent {
                thread_id,
                assistant_message_id: Some(assistant_message_id),
                finish_reason,
            })
            .expect("TurnCompleteEvent always serializes"),
        );
        return;
    }
}

pub(crate) fn now_ms() -> i64 {
    std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map(|d| d.as_millis() as i64)
        .unwrap_or(0)
}

#[cfg(test)]
#[path = "turn_tests.rs"]
mod tests;
