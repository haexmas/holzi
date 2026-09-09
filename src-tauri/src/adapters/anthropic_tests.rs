//! Tests for the Anthropic adapter using `wiremock` as a local HTTP
//! double. Each test spins its own server on a random port so tests
//! can run in parallel without cross-talk.

use wiremock::matchers::{header, method, path, query_param};
use wiremock::{Mock, MockServer, ResponseTemplate};

use super::anthropic::AnthropicAdapter;
use super::{AdapterError, ProviderAdapter};

const AUTH_HEADER: &str = "x-api-key";
const VERSION_HEADER: &str = "anthropic-version";
const VERSION_VALUE: &str = "2023-06-01";

#[tokio::test]
async fn list_models_returns_single_page_entries_with_context_window() {
    let server = MockServer::start().await;
    Mock::given(method("GET"))
        .and(path("/v1/models"))
        .and(header(AUTH_HEADER, "sk-test-key"))
        .and(header(VERSION_HEADER, VERSION_VALUE))
        .and(query_param("limit", "1000"))
        .respond_with(ResponseTemplate::new(200).set_body_json(serde_json::json!({
            "data": [
                {
                    "id": "claude-opus-5",
                    "display_name": "Claude Opus 5",
                    "type": "model",
                    "created_at": "2026-07-24T00:00:00Z",
                    "max_input_tokens": 200000,
                },
                {
                    "id": "claude-haiku-4-5-20251001",
                    "display_name": "Claude Haiku 4.5",
                    "type": "model",
                    "created_at": "2025-10-01T00:00:00Z",
                    "max_input_tokens": null,
                }
            ],
            "has_more": false,
            "first_id": "claude-opus-5",
            "last_id": "claude-haiku-4-5-20251001"
        })))
        .expect(1)
        .mount(&server)
        .await;

    let adapter = AnthropicAdapter::new(server.uri(), "sk-test-key".to_string()).unwrap();
    let models = adapter.list_models().await.expect("list_models succeeds");

    assert_eq!(models.len(), 2);
    assert_eq!(models[0].remote_id, "claude-opus-5");
    assert_eq!(models[0].display_name, "Claude Opus 5");
    assert_eq!(models[0].context_window, Some(200000));
    assert_eq!(models[1].remote_id, "claude-haiku-4-5-20251001");
    assert!(models[1].context_window.is_none());
}

#[tokio::test]
async fn list_models_paginates_on_has_more() {
    let server = MockServer::start().await;

    // First page: has_more=true, last_id set → adapter must issue a
    // second GET with after_id.
    Mock::given(method("GET"))
        .and(path("/v1/models"))
        .and(query_param("limit", "1000"))
        .and(query_param_missing("after_id"))
        .respond_with(ResponseTemplate::new(200).set_body_json(serde_json::json!({
            "data": [
                {"id": "model-a", "display_name": "A", "type": "model",
                 "created_at": "2026-01-01T00:00:00Z", "max_input_tokens": 100}
            ],
            "has_more": true,
            "first_id": "model-a",
            "last_id": "model-a"
        })))
        .expect(1)
        .mount(&server)
        .await;

    // Second page: uses after_id=model-a, closes with has_more=false.
    Mock::given(method("GET"))
        .and(path("/v1/models"))
        .and(query_param("limit", "1000"))
        .and(query_param("after_id", "model-a"))
        .respond_with(ResponseTemplate::new(200).set_body_json(serde_json::json!({
            "data": [
                {"id": "model-b", "display_name": "B", "type": "model",
                 "created_at": "2026-01-02T00:00:00Z", "max_input_tokens": 200}
            ],
            "has_more": false,
            "first_id": "model-b",
            "last_id": "model-b"
        })))
        .expect(1)
        .mount(&server)
        .await;

    let adapter = AnthropicAdapter::new(server.uri(), "sk-test".to_string()).unwrap();
    let models = adapter.list_models().await.unwrap();

    let ids: Vec<_> = models.iter().map(|m| m.remote_id.as_str()).collect();
    assert_eq!(ids, ["model-a", "model-b"]);
}

#[tokio::test]
async fn list_models_maps_401_to_invalid_credentials() {
    let server = MockServer::start().await;
    Mock::given(method("GET"))
        .and(path("/v1/models"))
        .respond_with(ResponseTemplate::new(401).set_body_json(serde_json::json!({
            "type": "error",
            "error": {"type": "authentication_error", "message": "invalid x-api-key"}
        })))
        .mount(&server)
        .await;

    let adapter = AnthropicAdapter::new(server.uri(), "sk-wrong".to_string()).unwrap();
    let err = adapter.list_models().await.expect_err("401 must error");
    assert!(
        matches!(err, AdapterError::InvalidCredentials),
        "got {err:?}"
    );
}

#[tokio::test]
async fn list_models_maps_403_to_invalid_credentials() {
    let server = MockServer::start().await;
    Mock::given(method("GET"))
        .and(path("/v1/models"))
        .respond_with(ResponseTemplate::new(403))
        .mount(&server)
        .await;

    let adapter = AnthropicAdapter::new(server.uri(), "sk-forbidden".to_string()).unwrap();
    let err = adapter.list_models().await.expect_err("403 must error");
    assert!(
        matches!(err, AdapterError::InvalidCredentials),
        "got {err:?}"
    );
}

#[tokio::test]
async fn list_models_maps_5xx_to_status_error_preserving_body() {
    let server = MockServer::start().await;
    Mock::given(method("GET"))
        .and(path("/v1/models"))
        .respond_with(ResponseTemplate::new(503).set_body_string("upstream unavailable"))
        .mount(&server)
        .await;

    let adapter = AnthropicAdapter::new(server.uri(), "sk-any".to_string()).unwrap();
    let err = adapter.list_models().await.expect_err("5xx must error");
    match err {
        AdapterError::Status { status, body } => {
            assert_eq!(status, 503);
            assert!(body.contains("upstream unavailable"));
        }
        other => panic!("expected Status, got {other:?}"),
    }
}

#[tokio::test]
async fn list_models_treats_zero_max_input_tokens_as_absent() {
    // The docs example ships `max_input_tokens: 0` on newer models —
    // treat that as "not reported" so the picker does not show a
    // useless zero-width context.
    let server = MockServer::start().await;
    Mock::given(method("GET"))
        .and(path("/v1/models"))
        .respond_with(ResponseTemplate::new(200).set_body_json(serde_json::json!({
            "data": [
                {"id": "m", "display_name": "M", "type": "model",
                 "created_at": "2026-01-01T00:00:00Z", "max_input_tokens": 0}
            ],
            "has_more": false,
            "first_id": "m",
            "last_id": "m"
        })))
        .mount(&server)
        .await;

    let adapter = AnthropicAdapter::new(server.uri(), "sk-any".to_string()).unwrap();
    let models = adapter.list_models().await.unwrap();
    assert_eq!(models.len(), 1);
    assert!(models[0].context_window.is_none());
}

fn query_param_missing(name: &'static str) -> impl wiremock::Match {
    // wiremock has no built-in "must not be present" matcher, so build
    // one from the incoming request: the whole query string must lack
    // the exact `name=` prefix. Good enough for the two-page test —
    // real requests only include the params the adapter sets.
    struct Missing(&'static str);
    impl wiremock::Match for Missing {
        fn matches(&self, req: &wiremock::Request) -> bool {
            let query = req.url.query().unwrap_or("");
            let needle = format!("{}=", self.0);
            !query.split('&').any(|part| part.starts_with(&needle))
        }
    }
    Missing(name)
}
