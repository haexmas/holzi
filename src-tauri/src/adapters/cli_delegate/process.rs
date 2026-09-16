use tokio::process::{Child, Command};

/// Owns a delegate child and makes the process-group boundary explicit.
/// Normal and cancellation paths call `terminate_and_reap`; `Drop` remains a
/// last-resort guard for task abortion.
pub(super) struct ChildLifecycle {
    child: Child,
}

impl ChildLifecycle {
    pub(super) fn spawn(command: &mut Command) -> std::io::Result<Self> {
        command.kill_on_drop(true);
        Ok(Self {
            child: command.spawn()?,
        })
    }

    pub(super) fn child_mut(&mut self) -> &mut Child {
        &mut self.child
    }

    pub(super) async fn terminate_and_reap(&mut self) {
        kill_process_group(self.child.id());
        let _ = self.child.kill().await;
        let _ = self.child.wait().await;
    }
}

impl Drop for ChildLifecycle {
    fn drop(&mut self) {
        kill_process_group(self.child.id());
    }
}

pub(super) fn configure_process_group(command: &mut Command) {
    #[cfg(unix)]
    command.process_group(0);
}

fn kill_process_group(pid: Option<u32>) {
    #[cfg(unix)]
    if let Some(pid) = pid {
        // SAFETY: the negative pid targets the process group created by
        // `process_group(0)`; no pointers or borrowed memory are involved.
        unsafe {
            libc::kill(-(pid as libc::pid_t), libc::SIGKILL);
        }
    }
    #[cfg(not(unix))]
    let _ = pid;
}
