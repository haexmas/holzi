//! Tests for the child registry (spec 013 T037, FR-003).
//!
//! The process tests are Unix only: each child is an `sh` that starts a background `sleep`, so a
//! kill that reached only the shell would leave the `sleep` running and fail the test.

use super::{safe_group_pid, ChildRegistry};

#[test]
fn only_a_pid_that_names_exactly_one_group_is_ever_signalled() {
    // `kill(-1, ..)` reaches every process the user owns and `kill(0, ..)` the caller's own group,
    // so the pids that would produce those targets must never get through.
    assert_eq!(safe_group_pid(0), None);
    assert_eq!(safe_group_pid(1), None);
    assert_eq!(safe_group_pid(u32::MAX), None);
    assert_eq!(safe_group_pid(i32::MAX as u32 + 1), None);
    assert_eq!(safe_group_pid(2), Some(2));
    assert_eq!(safe_group_pid(4242), Some(4242));
    assert_eq!(safe_group_pid(i32::MAX as u32), Some(i32::MAX));
}

#[test]
fn dropping_the_guard_removes_the_entry() {
    let registry = ChildRegistry::default();

    let first = registry.register(4_000_001);
    let second = registry.register(4_000_002);
    assert_eq!(registry.live(), 2);

    drop(first);
    assert_eq!(registry.live(), 1);
    drop(second);
    assert_eq!(registry.live(), 0);
}

#[test]
fn a_clone_of_the_registry_is_the_same_registry() {
    let registry = ChildRegistry::default();
    let clone = registry.clone();

    let _guard = clone.register(4_000_003);

    assert_eq!(registry.live(), 1);
}

#[cfg(unix)]
mod processes {
    use std::os::unix::process::ExitStatusExt;
    use std::process::Stdio;
    use std::time::Duration;

    use tokio::io::{AsyncBufReadExt, BufReader};
    use tokio::process::{Child, Command};

    use super::ChildRegistry;

    const PATIENCE: Duration = Duration::from_secs(5);

    /// A shell in its own process group that starts a background `sleep` and prints its pid.
    async fn shell_with_background_sleep() -> (Child, u32, u32) {
        let mut command = Command::new("sh");
        command
            .arg("-c")
            .arg("sleep 60 & echo $!; wait")
            .stdout(Stdio::piped())
            .process_group(0)
            .kill_on_drop(true);
        let mut shell = command.spawn().expect("spawn sh");
        let mut lines = BufReader::new(shell.stdout.take().expect("piped stdout")).lines();
        let line = tokio::time::timeout(PATIENCE, lines.next_line())
            .await
            .expect("the shell printed the pid of its sleep in time")
            .expect("read the pid line")
            .expect("a pid line");
        let sleep_pid = line.trim().parse().expect("a numeric pid");
        let shell_pid = shell.id().expect("the shell is running");
        (shell, shell_pid, sleep_pid)
    }

    /// Whether `pid` no longer runs. A zombie counts as gone: it is dead and only waits for
    /// whatever adopted it to collect it.
    async fn is_gone(pid: u32) -> bool {
        let deadline = tokio::time::Instant::now() + PATIENCE;
        loop {
            let listed = std::process::Command::new("ps")
                .args(["-o", "stat=", "-p", &pid.to_string()])
                .output()
                .expect("run ps");
            let state = String::from_utf8_lossy(&listed.stdout).trim().to_string();
            if state.is_empty() || state.starts_with('Z') {
                return true;
            }
            if tokio::time::Instant::now() >= deadline {
                return false;
            }
            tokio::time::sleep(Duration::from_millis(20)).await;
        }
    }

    async fn killed_by_sigkill(shell: &mut Child) -> bool {
        let status = tokio::time::timeout(PATIENCE, shell.wait())
            .await
            .expect("the shell ended in time")
            .expect("wait for the shell");
        status.signal() == Some(libc::SIGKILL)
    }

    #[tokio::test]
    async fn kill_all_ends_a_child_and_the_grandchild_its_shell_started() {
        let registry = ChildRegistry::default();
        let (mut shell, shell_pid, sleep_pid) = shell_with_background_sleep().await;
        let _guard = registry.register(shell_pid);

        registry.kill_all();

        assert!(killed_by_sigkill(&mut shell).await, "the shell was killed");
        assert!(
            is_gone(sleep_pid).await,
            "the sleep it started was killed too"
        );
    }

    #[tokio::test]
    async fn a_second_kill_all_is_harmless() {
        let registry = ChildRegistry::default();
        let (mut shell, shell_pid, _sleep_pid) = shell_with_background_sleep().await;
        let guard = registry.register(shell_pid);
        registry.kill_all();
        assert!(killed_by_sigkill(&mut shell).await);
        drop(guard);

        registry.kill_all();
        registry.kill_all();

        assert_eq!(registry.live(), 0);
    }

    #[tokio::test]
    async fn a_child_registered_after_kill_all_is_killed_at_once() {
        let registry = ChildRegistry::default();
        registry.kill_all();
        let (mut shell, shell_pid, sleep_pid) = shell_with_background_sleep().await;

        let _guard = registry.register(shell_pid);

        assert!(
            killed_by_sigkill(&mut shell).await,
            "the late shell was killed"
        );
        assert!(is_gone(sleep_pid).await, "and so was its sleep");
    }

    #[tokio::test]
    async fn a_child_that_was_unregistered_is_left_alone() {
        let registry = ChildRegistry::default();
        let (mut shell, shell_pid, sleep_pid) = shell_with_background_sleep().await;
        drop(registry.register(shell_pid));

        registry.kill_all();

        // The entry was removed when the child was reaped, so its pid is no longer ours to signal:
        // the operating system may have handed it to something else by then.
        assert!(
            shell.try_wait().expect("poll the shell").is_none(),
            "the unregistered shell still runs"
        );
        shell.kill().await.expect("clean up the shell");
        let _ = std::process::Command::new("kill")
            .arg(sleep_pid.to_string())
            .status();
    }
}
