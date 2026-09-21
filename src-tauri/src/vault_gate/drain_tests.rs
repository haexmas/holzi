//! Tests for the drain ladder and the forced end (research R5, data-model.md `DrainOutcome`).
//!
//! The ladder runs on paused time so its deadlines cost no wall-clock time. Tokio's clock does not
//! auto-advance while a `spawn_blocking` task is alive, so un-abortable work is stood in for by a
//! plain thread that holds a tracker token; the tracked blocking closure itself is covered by one
//! test with short real deadlines.

use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{mpsc, Arc};
use std::time::Duration;

use tokio::time::Instant;

use super::{hard_end_after, on_plain_thread, DrainOutcome, COOPERATIVE_WINDOW, TOTAL_LIMIT};
use crate::vault_gate::VaultGate;

fn gate_on_this_runtime() -> VaultGate {
    VaultGate::with_runtime(tokio::runtime::Handle::current())
}

#[tokio::test(start_paused = true)]
async fn a_task_that_stops_on_the_token_yields_drained() {
    let gate = gate_on_this_runtime();
    let token = gate.token();
    let cleaned_up = Arc::new(AtomicBool::new(false));
    let flag = cleaned_up.clone();
    gate.spawn(async move {
        token.cancelled().await;
        flag.store(true, Ordering::SeqCst);
    })
    .expect("open gate");

    let started = Instant::now();
    let outcome = gate.drain().await;

    assert_eq!(outcome, DrainOutcome::Drained);
    assert!(cleaned_up.load(Ordering::SeqCst), "cooperative cleanup ran");
    assert!(started.elapsed() < COOPERATIVE_WINDOW);
}

#[tokio::test(start_paused = true)]
async fn a_task_that_ignores_the_token_yields_drained_after_abort() {
    let gate = gate_on_this_runtime();
    gate.spawn(async {
        loop {
            tokio::time::sleep(Duration::from_millis(10)).await;
        }
    })
    .expect("open gate");

    let started = Instant::now();
    let outcome = gate.drain().await;

    assert_eq!(outcome, DrainOutcome::DrainedAfterAbort);
    assert!(
        started.elapsed() >= COOPERATIVE_WINDOW,
        "the abort waits for the cooperative window"
    );
    assert!(started.elapsed() <= TOTAL_LIMIT);
}

#[tokio::test(start_paused = true)]
async fn work_that_outlives_the_limit_yields_stuck_within_the_limit() {
    let gate = gate_on_this_runtime();
    // Stands for un-abortable native work: a plain thread that holds a tracker token.
    let token = gate.tracker_token().expect("open gate");
    let (release, released) = mpsc::channel::<()>();
    let thread = std::thread::spawn(move || {
        let _held = token;
        let _ = released.recv();
    });

    let started = Instant::now();
    let outcome = gate.drain().await;

    assert_eq!(outcome, DrainOutcome::Stuck);
    assert!(
        started.elapsed() <= TOTAL_LIMIT,
        "the call returns within the total limit"
    );
    release.send(()).expect("release the thread");
    thread.join().expect("thread ends");
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn a_tracked_blocking_closure_is_reported_stuck_and_waited_for_once_released() {
    let gate = VaultGate::with_runtime(tokio::runtime::Handle::current());
    let (release, released) = mpsc::channel::<()>();
    gate.spawn_blocking(move || {
        let _ = released.recv();
    })
    .expect("open gate");

    let outcome = gate
        .drain_with(Duration::from_millis(50), Duration::from_millis(150))
        .await;
    assert_eq!(outcome, DrainOutcome::Stuck);
    assert!(!gate.is_idle());

    release.send(()).expect("release the closure");
    let outcome = gate
        .drain_with(Duration::from_secs(5), Duration::from_secs(10))
        .await;
    assert_eq!(outcome, DrainOutcome::Drained);
    assert!(gate.is_idle());
}

#[test]
fn the_forced_end_runs_once_after_the_grace_period_and_never_before() {
    let grace = Duration::from_millis(80);
    let started = std::time::Instant::now();
    let (ran, ran_at) = mpsc::channel();
    hard_end_after(grace, move || {
        ran.send(started.elapsed()).expect("report the run");
    });

    assert!(
        ran_at.try_recv().is_err(),
        "the action must not run before the grace period"
    );
    let elapsed = ran_at
        .recv_timeout(Duration::from_secs(5))
        .expect("the action runs");
    assert!(elapsed >= grace, "ran after {elapsed:?}, before {grace:?}");
    assert!(
        ran_at.recv_timeout(Duration::from_millis(100)).is_err(),
        "the action runs once"
    );
}

#[test]
fn an_action_on_a_plain_thread_runs_once_on_a_thread_of_that_name() {
    let (ran, ran_on) = mpsc::channel();
    on_plain_thread("vault-test", Duration::ZERO, move || {
        ran.send(std::thread::current().name().map(str::to_owned))
            .expect("report the run");
    });

    let thread = ran_on
        .recv_timeout(Duration::from_secs(5))
        .expect("the action runs");
    assert_eq!(thread.as_deref(), Some("vault-test"));
    assert!(ran_on.recv_timeout(Duration::from_millis(100)).is_err());
}

#[cfg(unix)]
mod children {
    //! The ladder ends every registered child (spec 013 FR-003, T037).

    use std::os::unix::process::{CommandExt, ExitStatusExt};
    use std::process::{Child, Command};

    use super::*;

    fn long_lived_child() -> Child {
        Command::new("sleep")
            .arg("60")
            .process_group(0)
            .spawn()
            .expect("spawn sleep")
    }

    /// Blocks until the child is gone; the ladder already sent the kill, so this is quick.
    fn killed_by_the_ladder(mut child: Child) -> bool {
        child.wait().expect("wait").signal() == Some(libc::SIGKILL)
    }

    #[tokio::test(start_paused = true)]
    async fn a_drained_close_still_ends_a_registered_child() {
        let gate = gate_on_this_runtime();
        let child = long_lived_child();
        let _guard = gate.children().register(child.id());

        let outcome = gate.drain().await;

        assert_eq!(outcome, DrainOutcome::Drained);
        assert!(killed_by_the_ladder(child));
    }

    #[tokio::test(start_paused = true)]
    async fn the_ladder_ends_a_registered_child_before_it_returns_drained_after_abort() {
        let gate = gate_on_this_runtime();
        gate.spawn(async {
            loop {
                tokio::time::sleep(Duration::from_millis(10)).await;
            }
        })
        .expect("open gate");
        let child = long_lived_child();
        let _guard = gate.children().register(child.id());

        let outcome = gate.drain().await;

        assert_eq!(outcome, DrainOutcome::DrainedAfterAbort);
        assert!(killed_by_the_ladder(child));
    }

    #[tokio::test(start_paused = true)]
    async fn the_ladder_ends_a_registered_child_before_it_returns_stuck() {
        let gate = gate_on_this_runtime();
        let token = gate.tracker_token().expect("open gate");
        let (release, released) = mpsc::channel::<()>();
        let thread = std::thread::spawn(move || {
            let _held = token;
            let _ = released.recv();
        });
        let child = long_lived_child();
        let _guard = gate.children().register(child.id());

        let outcome = gate.drain().await;

        assert_eq!(outcome, DrainOutcome::Stuck);
        assert!(killed_by_the_ladder(child));
        release.send(()).expect("release the thread");
        thread.join().expect("thread ends");
    }

    #[tokio::test(flavor = "multi_thread", worker_threads = 2)]
    async fn ending_the_child_at_the_abort_rung_frees_work_that_was_waiting_for_it() {
        let gate = VaultGate::with_runtime(tokio::runtime::Handle::current());
        let mut child = long_lived_child();
        let guard = gate.children().register(child.id());
        // A blocking thread that only ends once its child is gone, and cannot be aborted.
        gate.spawn_blocking(move || {
            let _ = child.wait();
            drop(guard);
        })
        .expect("open gate");

        let outcome = gate
            .drain_with(Duration::from_millis(50), Duration::from_secs(5))
            .await;

        assert_eq!(
            outcome,
            DrainOutcome::DrainedAfterAbort,
            "killing the child at the abort rung lets the waiting thread finish"
        );
    }
}
