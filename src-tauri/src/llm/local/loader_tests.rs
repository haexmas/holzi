//! Unit tests that do not require a real GGUF on disk. Tests that
//! actually load and generate live in `src-tauri/tests/local_inference.rs`
//! and are gated on the `HOLZI_TEST_GGUF` env var.

use super::{LocalModel, LocalModelError};
use std::path::PathBuf;

#[tokio::test]
async fn load_returns_not_found_for_missing_path() {
    // `LocalModel` wraps a `mistralrs::Model` which does not implement
    // `Debug`, so we cannot use `unwrap_err`. Match the two arms directly.
    let missing = PathBuf::from("/tmp/holzi-nonexistent-model.gguf");
    match LocalModel::load(&missing, None).await {
        Err(LocalModelError::NotFound(p)) => assert_eq!(p, missing),
        Err(other) => panic!("expected NotFound, got: {other:?}"),
        Ok(_) => panic!("expected error, got Ok"),
    }
}
