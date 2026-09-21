//! Tests for the gate state machine (data-model.md: `VaultGate` and `VaultPhase`).

use std::sync::atomic::{AtomicUsize, Ordering};
use std::sync::{Arc, Barrier};

use super::{VaultGate, VaultPhase};
use crate::error::HolziError;

#[test]
fn a_new_gate_is_idle_and_open_for_a_session() {
    let gate = VaultGate::new();
    assert_eq!(gate.phase(), VaultPhase::Idle);
    assert!(!gate.is_closing());
    assert!(gate.ensure_can_open().is_ok());
}

/// "`phase` only moves forward (`Idle` → `Active` → `Closing`), and `Idle` → `Closing` is allowed"
#[test]
fn phase_moves_forward_from_idle_through_active_to_closing() {
    let gate = VaultGate::new();
    gate.begin_session().expect("Idle to Active");
    assert_eq!(gate.phase(), VaultPhase::Active);
    assert!(gate.request_close());
    assert_eq!(gate.phase(), VaultPhase::Closing);
    assert!(gate.is_closing());
}

#[test]
fn closing_before_any_vault_is_open_is_allowed() {
    let gate = VaultGate::new();
    assert!(gate.request_close());
    assert_eq!(gate.phase(), VaultPhase::Closing);
}

#[test]
fn the_phase_never_moves_backward() {
    let gate = VaultGate::new();
    gate.request_close();
    assert!(matches!(gate.begin_session(), Err(HolziError::VaultClosed)));
    assert_eq!(gate.phase(), VaultPhase::Closing);
}

/// "`cancel` fires at most once, on the first transition into `Closing`"
#[test]
fn the_token_fires_on_the_first_close_and_not_before() {
    let gate = VaultGate::new();
    let token = gate.token();
    assert!(!token.is_cancelled());
    gate.begin_session().expect("Idle to Active");
    assert!(!token.is_cancelled(), "opening a session does not cancel");
    gate.request_close();
    assert!(token.is_cancelled());
    gate.request_close();
    assert!(gate.token().is_cancelled(), "still the one fired token");
}

/// "A second close request changes nothing and reports that a close is already running (FR-002)"
#[test]
fn a_second_close_request_changes_nothing_and_reports_it() {
    let gate = VaultGate::new();
    gate.begin_session().expect("Idle to Active");
    assert!(gate.request_close(), "the first call starts the close");
    assert!(
        !gate.request_close(),
        "the second call only reports that a close is running"
    );
    assert_eq!(gate.phase(), VaultPhase::Closing);
}

#[test]
fn begin_session_while_active_is_vault_already_active() {
    let gate = VaultGate::new();
    gate.begin_session().expect("Idle to Active");
    assert!(matches!(
        gate.begin_session(),
        Err(HolziError::VaultAlreadyActive)
    ));
    assert_eq!(gate.phase(), VaultPhase::Active);
}

#[test]
fn begin_session_while_closing_is_vault_closed() {
    let gate = VaultGate::new();
    gate.begin_session().expect("Idle to Active");
    gate.request_close();
    assert!(matches!(gate.begin_session(), Err(HolziError::VaultClosed)));
}

#[test]
fn ensure_can_open_reports_the_same_errors_without_changing_the_phase() {
    let gate = VaultGate::new();
    assert!(gate.ensure_can_open().is_ok());
    assert_eq!(gate.phase(), VaultPhase::Idle);

    gate.begin_session().expect("Idle to Active");
    assert!(matches!(
        gate.ensure_can_open(),
        Err(HolziError::VaultAlreadyActive)
    ));
    assert_eq!(gate.phase(), VaultPhase::Active);

    gate.request_close();
    assert!(matches!(
        gate.ensure_can_open(),
        Err(HolziError::VaultClosed)
    ));
}

#[test]
fn begin_session_from_idle_succeeds_exactly_once_when_two_threads_race() {
    for _ in 0..200 {
        let gate = VaultGate::new();
        let barrier = Arc::new(Barrier::new(2));
        let wins = Arc::new(AtomicUsize::new(0));
        let threads: Vec<_> = (0..2)
            .map(|_| {
                let (gate, barrier, wins) = (gate.clone(), barrier.clone(), wins.clone());
                std::thread::spawn(move || {
                    barrier.wait();
                    if gate.begin_session().is_ok() {
                        wins.fetch_add(1, Ordering::SeqCst);
                    }
                })
            })
            .collect();
        for thread in threads {
            thread.join().expect("racing thread");
        }
        assert_eq!(wins.load(Ordering::SeqCst), 1);
        assert_eq!(gate.phase(), VaultPhase::Active);
    }
}

#[tokio::test]
async fn run_returns_the_output_while_open_and_vault_closed_once_closing() {
    let gate = VaultGate::new();
    assert_eq!(gate.run(async { 7 }).await.expect("open gate"), 7);

    gate.request_close();
    let pending = gate.run(std::future::pending::<()>()).await;
    assert!(matches!(pending, Err(HolziError::VaultClosed)));
    // A result that is ready at the same moment still loses to the close (FR-001).
    assert!(matches!(
        gate.run(async { 1 }).await,
        Err(HolziError::VaultClosed)
    ));
}

#[tokio::test]
async fn admission_rejects_new_tasks_after_close() {
    let gate = VaultGate::new();
    gate.request_close();

    assert!(matches!(gate.tracker_token(), Err(HolziError::VaultClosed)));
    assert!(matches!(gate.spawn(async {}), Err(HolziError::VaultClosed)));
    assert!(matches!(
        gate.spawn_blocking(|| {}),
        Err(HolziError::VaultClosed)
    ));
}
