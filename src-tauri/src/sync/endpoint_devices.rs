//! What a connection tells about the device on its other end (spec 024, FR-029,
//! FR-030, FR-033): a halted device and its reason, the device coming online
//! and going offline, and telling the interface about both. Part of
//! [`super`], so it works on the node's private state.

use std::sync::Arc;

use super::{Inner, Problem, RejectCode};
use crate::sync::handshake;
use crate::sync::problems;
use crate::sync::wire::ErrorCode;

/// Records `device` as seen right now. Best-effort.
pub(super) async fn mark_seen(inner: &Arc<Inner>, device: [u8; 32]) {
    let replica = Arc::clone(&inner.replica);
    let now = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map(|d| d.as_millis() as u64)
        .unwrap_or(0);
    match tokio::task::spawn_blocking(move || crate::sync::seen::touch(&replica, &device, now))
        .await
    {
        Ok(Ok(_)) => {}
        Ok(Err(error)) => log::debug!("sync: recording a device's time failed: {error}"),
        Err(error) => log::debug!("sync: recording a device's time did not run: {error}"),
    }
}

/// Turns a handshake failure into a halted device where it names one: the
/// device that proved its key, or, when the peer refused this device for
/// its version or as a duplicate, the device the list gives that endpoint.
pub(super) async fn note_failure(
    inner: &Arc<Inner>,
    error: &handshake::HandshakeError,
    remote: [u8; 32],
) {
    use handshake::HandshakeError::{Halted, RefusedByPeer};
    let (device, problem) = match error {
        Halted { problem, device } => (Some(*device), *problem),
        RefusedByPeer(RejectCode::Incompatible) => {
            (device_at(inner, remote).await, Problem::IncompatibleVersion)
        }
        RefusedByPeer(RejectCode::Duplicate) => {
            (device_at(inner, remote).await, Problem::Duplicate)
        }
        _ => return,
    };
    if let Some(device) = device {
        flag(inner, device, problem).await;
    }
}

async fn device_at(inner: &Arc<Inner>, endpoint: [u8; 32]) -> Option<[u8; 32]> {
    let replica = Arc::clone(&inner.replica);
    let vault = inner.vault;
    tokio::task::spawn_blocking(move || problems::device_at_endpoint(&replica, vault, &endpoint))
        .await
        .ok()?
        .ok()?
}

pub(super) async fn flag(inner: &Arc<Inner>, device: [u8; 32], problem: Problem) {
    if problem == Problem::Duplicate {
        inner.duplicates.note(device);
        let live = inner
            .peers
            .lock()
            .unwrap_or_else(|e| e.into_inner())
            .remove(&device);
        if let Some(live) = live {
            live.close(ErrorCode::Rejected.as_u32().into(), b"duplicate");
        }
    }
    let replica = Arc::clone(&inner.replica);
    let changed = tokio::task::spawn_blocking(move || problems::set(&replica, &device, problem))
        .await
        .map_err(|e| e.to_string())
        .and_then(|r| r.map_err(|e| e.to_string()));
    match changed {
        Ok(true) => notify_devices_changed(inner),
        Ok(false) => {}
        Err(error) => log::warn!("sync: recording a device problem failed: {error}"),
    }
}

pub(super) async fn clear_problem(inner: &Arc<Inner>, device: [u8; 32]) {
    let replica = Arc::clone(&inner.replica);
    let cleared = tokio::task::spawn_blocking(move || problems::clear(&replica, &device))
        .await
        .map_err(|e| e.to_string())
        .and_then(|r| r.map_err(|e| e.to_string()));
    match cleared {
        Ok(true) => notify_devices_changed(inner),
        Ok(false) => {}
        Err(error) => log::warn!("sync: clearing a device problem failed: {error}"),
    }
}

pub(super) fn notify_devices_changed(inner: &Inner) {
    let hook = inner
        .devices_changed
        .lock()
        .unwrap_or_else(|e| e.into_inner())
        .clone();
    if let Some(hook) = hook {
        hook();
    }
}
