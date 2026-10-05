//! Async work from a bridge method: the methods run on a blocking thread of the app's runtime
//! (`commands::frames`), and some of what they call is async (HTTP, the password service).

use std::future::Future;

/// Runs `future` to its end on the app's runtime, or, in tests without one, on a runtime of its own.
pub fn block_on<F: Future>(future: F) -> F::Output {
    match tokio::runtime::Handle::try_current() {
        Ok(handle) => handle.block_on(future),
        Err(_) => tokio::runtime::Builder::new_current_thread()
            .enable_all()
            .build()
            .expect("a runtime for one bridge call")
            .block_on(future),
    }
}
