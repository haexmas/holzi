use std::collections::HashMap;
use std::sync::{Arc, Mutex};

use serde_json::Value;
use tokio::sync::{oneshot, Mutex as AsyncMutex};
use uuid::Uuid;

use super::approval_bridge::request_approval;
use super::{EventEmitter, PendingToolApprovals};
use crate::chat::tools::ApprovalDecision;

#[tokio::test]
async fn manual_approval_uses_the_existing_pending_request_flow() {
    let pending: PendingToolApprovals = Arc::new(Mutex::new(HashMap::new()));
    let (event_sender, event_receiver) = oneshot::channel();
    let event_sender = Arc::new(AsyncMutex::new(Some(event_sender)));
    let emit: EventEmitter = {
        let event_sender = Arc::clone(&event_sender);
        Arc::new(move |_event: &str, payload: Value| {
            let event_sender = Arc::clone(&event_sender);
            tokio::spawn(async move {
                if let Some(sender) = event_sender.lock().await.take() {
                    let _ = sender.send(payload);
                }
            });
        })
    };

    let pending_for_request = Arc::clone(&pending);
    let emit_for_request = Arc::clone(&emit);
    let task = tokio::spawn(async move {
        request_approval(
            &pending_for_request,
            &emit_for_request,
            None,
            Some(Uuid::nil()),
            "Bash".to_string(),
            serde_json::json!({"command": "echo test"}),
        )
        .await
    });

    let payload = event_receiver
        .await
        .expect("approval event should be emitted");
    let request_id = payload["requestId"]
        .as_str()
        .and_then(|value| Uuid::parse_str(value).ok())
        .expect("approval event should contain a UUID");
    let sender = pending
        .lock()
        .expect("pending map lock")
        .remove(&request_id)
        .expect("approval sender should be registered");
    sender
        .send(ApprovalDecision::Allow)
        .expect("receiver is alive");
    assert_eq!(
        task.await.expect("approval task should finish"),
        ApprovalDecision::Allow
    );
}
