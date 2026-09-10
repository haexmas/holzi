//! [`LocalModel`] wrapped as a [`ProviderAdapter`] so the chat
//! dispatch does not special-case in-process vs. `api_key` models. The
//! `list_models` call is a no-op — local models are inserted into the
//! `models` table by the download and import commands, not by a
//! provider-side listing endpoint.

use async_trait::async_trait;

use super::types::{AdapterStream, ChatRequest};
use super::{AdapterError, ProviderAdapter, ProviderModel};
use crate::llm::local::LocalModel;

pub struct LocalAdapter {
    model: LocalModel,
}

impl LocalAdapter {
    pub fn new(model: LocalModel) -> Self {
        Self { model }
    }
}

#[async_trait]
impl ProviderAdapter for LocalAdapter {
    async fn list_models(&self) -> Result<Vec<ProviderModel>, AdapterError> {
        Ok(Vec::new())
    }

    async fn stream_chat(&self, req: ChatRequest) -> Result<AdapterStream, AdapterError> {
        // `LocalModel::stream_chat` spawns its own background task and
        // returns an `AdapterStream` directly — the shared types live
        // in `adapters::types` so no adapter has to translate between
        // stream shapes.
        Ok(self.model.stream_chat(req))
    }
}
