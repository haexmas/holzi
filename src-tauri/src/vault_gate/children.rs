//! The child processes started for the vault (spec 013 FR-003, data-model.md `ChildRegistry`).
//!
//! Ending the process on close skips every `Drop`-based kill, so a shell the agent started would
//! outlive the vault it belonged to. The registry knows each such child, each one the leader of
//! its own process group, and ends them all when the close runs out of patience: at the abort rung
//! of the drain ladder and again right before a forced end.

use std::collections::HashSet;
use std::sync::{Arc, Mutex, MutexGuard};

// ponytail: process groups on Unix and `taskkill /T` on Windows. Ceiling: a descendant that left
// its group (`setsid`, a double fork) survives. Upgrade path: a Windows Job Object with
// kill-on-close, and `PR_SET_PDEATHSIG` on Linux.

/// The registered children of one app process. Cheap to clone; every clone is the same registry.
#[derive(Clone, Default)]
pub struct ChildRegistry {
    inner: Arc<Mutex<Inner>>,
}

#[derive(Default)]
struct Inner {
    live: HashSet<u32>,
    /// Set by the first `kill_all`; from then on a registration kills its child at once.
    ended: bool,
}

/// Keeps one child registered. Dropping it removes the entry, so register it for exactly as long
/// as the child can run: a process id the system has handed on must never stay listed.
pub struct ChildGuard {
    registry: ChildRegistry,
    pid: u32,
}

impl ChildRegistry {
    fn lock(&self) -> MutexGuard<'_, Inner> {
        // The state is a set and a flag, so a poisoned lock holds nothing half-written.
        self.inner.lock().unwrap_or_else(|e| e.into_inner())
    }

    /// Registers `pid`, the leader of its own process group. After [`ChildRegistry::kill_all`]
    /// the child is killed at once instead.
    pub fn register(&self, pid: u32) -> ChildGuard {
        let mut inner = self.lock();
        if inner.ended {
            drop(inner);
            kill_process_tree(pid);
        } else {
            inner.live.insert(pid);
        }
        ChildGuard {
            registry: self.clone(),
            pid,
        }
    }

    /// Kills every registered group and makes later registrations kill at once. Harmless to
    /// repeat. Never waits: it runs from the drain ladder and from a plain thread at the forced
    /// end.
    pub fn kill_all(&self) {
        let pids = {
            let mut inner = self.lock();
            inner.ended = true;
            std::mem::take(&mut inner.live)
        };
        for pid in pids {
            kill_process_tree(pid);
        }
    }

    /// How many children are registered right now.
    pub fn live(&self) -> usize {
        self.lock().live.len()
    }
}

impl Drop for ChildGuard {
    fn drop(&mut self) {
        self.registry.lock().live.remove(&self.pid);
    }
}

/// The value to signal to end the whole group led by `pid`, or `None` for a pid that must never
/// be signalled. A group signal targets `-pid`: pid 1 would give `-1`, which reaches every
/// process the user owns, pid 0 gives the caller's own group, and a value above `i32::MAX` wraps
/// to a negative pid.
fn safe_group_pid(pid: u32) -> Option<i32> {
    if pid <= 1 {
        return None;
    }
    i32::try_from(pid).ok()
}

/// Ends `pid` and everything below it. Returns without waiting for the processes to go.
fn kill_process_tree(pid: u32) {
    let Some(group) = safe_group_pid(pid) else {
        log::error!("refusing to signal process group {pid}");
        return;
    };
    #[cfg(unix)]
    {
        // SAFETY: FFI call with a plain integer group id and a signal constant, no pointers
        // involved; `safe_group_pid` excluded every id that would name more than one group.
        unsafe {
            libc::kill(-group, libc::SIGKILL);
        }
    }
    // Windows has no group signal; `taskkill /T` walks the OS's own parent-child tree instead.
    #[cfg(windows)]
    {
        let _ = std::process::Command::new("taskkill")
            .args(["/T", "/F", "/PID", &group.to_string()])
            .output();
    }
    #[cfg(not(any(unix, windows)))]
    let _ = group;
}

#[cfg(test)]
#[path = "children_tests.rs"]
mod children_tests;
