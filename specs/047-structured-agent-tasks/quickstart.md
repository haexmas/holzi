# Quickstart: Structured Agent Tasks

This guide validates the host contract without requiring a real model or a
network connection.

## Prerequisites

- Rust toolchain and repository dependencies installed.
- An active Holzi test vault or the existing Rust integration-test fixture.
- A fixture provider/adapter that returns a deterministic JSON payload.

## Automated checks

From the repository root:

```bash
cargo test --manifest-path src-tauri/Cargo.toml agent_tasks
cargo test --manifest-path src-tauri/Cargo.toml --test extension_agent_tasks
cargo fmt --manifest-path src-tauri/Cargo.toml --check
```

Expected results:

- Invalid task names, schema versions, image sizes and capabilities are rejected.
- A valid fixture task emits exactly one completion event to the originating extension.
- A different extension cannot receive the task result; other frames of the
  originating extension receive only the extension-scoped completion event.
- Explicit cancellation produces a terminal `cancelled` result. Closing the
  originating frame cancels the run and prevents any late completion event.
- Local mode never invokes a remote fixture or reports a remote destination.
- Invalid fixture JSON produces `invalid_result` and no successful result.

## Manual bridge scenario

1. Open an extension frame with the fixture task adapter enabled.
2. Call `extension_ai_task_start` with a small image payload and task
   `paper-note-recognition`.
3. Confirm that the immediate response contains a `taskId` and `running`.
4. Wait for `haextension:ai-task:completed` and validate its result against
   [contracts/task.md](contracts/task.md).
5. Repeat with a text-only model profile; confirm `unsupported_capability`
   arrives before any inference request.
6. Repeat with a remote profile; confirm consent is required and the fixture
   receives no image before consent.
7. Cancel a running task and close the frame; confirm no late completion is
   delivered to another extension.

## Follow-up model validation

Once a local Vision/OCR backend is available, repeat the same scenarios with a
de-identified handwritten page. Compare the structured regions and provenance;
do not make the automated test depend on a particular model's exact wording.
