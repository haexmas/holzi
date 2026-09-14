use wiremock::matchers::{method, path, path_regex, query_param};
use wiremock::{Mock, MockServer, ResponseTemplate};

use super::*;
use crate::hardware::{Backend, HardwareInfo};

fn generous_hardware() -> HardwareInfo {
    HardwareInfo {
        backend: Backend::Cpu,
        total_ram_bytes: 64 * 1024 * 1024 * 1024,
        available_ram_bytes: 64 * 1024 * 1024 * 1024,
        vram_bytes: None,
    }
}

fn tiny_hardware() -> HardwareInfo {
    HardwareInfo {
        backend: Backend::Cpu,
        total_ram_bytes: 512 * 1024 * 1024,
        available_ram_bytes: 256 * 1024 * 1024,
        vram_bytes: None,
    }
}

// ---------------------------------------------------------------------
// T005: repo id / revision / filename / host validation
// ---------------------------------------------------------------------

#[test]
fn validate_repo_id_accepts_owner_slash_name() {
    assert!(validate_repo_id("bartowski/Llama-3.2-1B-GGUF").is_ok());
}

#[test]
fn validate_repo_id_rejects_missing_or_extra_slashes() {
    assert!(validate_repo_id("no-slash-here").is_err());
    assert!(validate_repo_id("too/many/slashes").is_err());
}

#[test]
fn validate_repo_id_rejects_empty_segment_and_traversal() {
    assert!(validate_repo_id("/name").is_err());
    assert!(validate_repo_id("owner/").is_err());
    assert!(validate_repo_id("owner/..").is_err());
    assert!(validate_repo_id("../owner").is_err());
}

#[test]
fn validate_repo_id_rejects_control_chars_and_whitespace() {
    assert!(validate_repo_id("owner/na\nme").is_err());
    assert!(validate_repo_id("owner/na me").is_err());
    assert!(validate_repo_id("").is_err());
}

#[test]
fn validate_revision_ref_accepts_branch_tag_and_sha() {
    assert!(validate_revision_ref("main").is_ok());
    assert!(validate_revision_ref("v1.2.3").is_ok());
    assert!(validate_revision_ref("a".repeat(40).as_str()).is_ok());
}

#[test]
fn validate_revision_ref_rejects_traversal_and_control_chars() {
    assert!(validate_revision_ref("").is_err());
    assert!(validate_revision_ref("..").is_err());
    assert!(validate_revision_ref("has whitespace").is_err());
    assert!(validate_revision_ref("has\ttab").is_err());
    assert!(validate_revision_ref("has/slash").is_err());
    assert!(validate_revision_ref("has\\backslash").is_err());
}

#[test]
fn is_commit_sha_requires_exactly_40_lowercase_hex_chars() {
    assert!(is_commit_sha(&"a".repeat(40)));
    assert!(is_commit_sha(
        &"0123456789abcdef0123456789abcdef01234567"[..40]
    ));
    assert!(
        !is_commit_sha(&"A".repeat(40)),
        "uppercase must be rejected"
    );
    assert!(!is_commit_sha(&"a".repeat(39)), "too short");
    assert!(!is_commit_sha(&"a".repeat(41)), "too long");
    assert!(!is_commit_sha("main"));
}

#[test]
fn validate_gguf_filename_accepts_gguf_and_rejects_other_formats() {
    assert!(validate_gguf_filename("model.Q4_K_M.gguf").is_ok());
    let err = validate_gguf_filename("model.safetensors").expect_err("non-gguf must be rejected");
    assert!(matches!(err, HolziError::UnsupportedFormat { .. }));
}

#[test]
fn validate_gguf_filename_rejects_path_traversal_and_control_chars() {
    assert!(validate_gguf_filename("../../etc/passwd.gguf").is_err());
    assert!(validate_gguf_filename("../evil.gguf").is_err());
    assert!(validate_gguf_filename("").is_err());
    assert!(validate_gguf_filename(".hidden.gguf").is_err());
}

#[test]
fn resolve_download_url_requires_a_resolved_commit_sha() {
    let sha = "a".repeat(40);
    let ok = resolve_download_url(DEFAULT_BASE_URL, "owner/name", &sha, "model.gguf");
    assert!(ok.is_ok());
    assert_eq!(
        ok.unwrap(),
        format!("https://huggingface.co/owner/name/resolve/{sha}/model.gguf")
    );

    let rejected = resolve_download_url(DEFAULT_BASE_URL, "owner/name", "main", "model.gguf");
    assert!(
        rejected.is_err(),
        "a mutable ref must not reach the download URL"
    );
}

#[test]
fn resolve_download_url_only_accepts_the_real_huggingface_host() {
    let sha = "a".repeat(40);
    assert!(
        resolve_download_url("http://huggingface.co", "owner/name", &sha, "model.gguf").is_err()
    );
    assert!(
        resolve_download_url("https://evil.example.com", "owner/name", &sha, "model.gguf").is_err()
    );
}

#[test]
fn resolve_download_url_host_check_is_not_fooled_by_userinfo() {
    // `https://huggingface.co@evil.example.com/...` parses to host
    // `evil.example.com` — a naive `starts_with("https://huggingface.co")`
    // string check would be fooled by this; the real `Url` parser is not.
    let sha = "a".repeat(40);
    let result = resolve_download_url(
        "https://huggingface.co@evil.example.com",
        "owner/name",
        &sha,
        "model.gguf",
    );
    assert!(result.is_err());
}

// ---------------------------------------------------------------------
// T006: quantization normalization, tokenizer resolution, dedup/sort
// ---------------------------------------------------------------------

#[test]
fn normalize_quantization_prefers_the_longest_boundary_match() {
    assert_eq!(
        normalize_quantization_from_filename("model-Q3_K_M.gguf").as_deref(),
        Some("Q3_K_M")
    );
    assert_eq!(
        normalize_quantization_from_filename("model-Q4_K_S.gguf").as_deref(),
        Some("Q4_K_S")
    );
    assert_eq!(
        normalize_quantization_from_filename("model-Q8_0.gguf").as_deref(),
        Some("Q8_0")
    );
    assert_eq!(
        normalize_quantization_from_filename("model-f16.gguf").as_deref(),
        Some("F16")
    );
}

#[test]
fn normalize_quantization_returns_none_when_no_known_token_present() {
    assert_eq!(normalize_quantization_from_filename("model.gguf"), None);
}

#[test]
fn normalize_quantization_does_not_match_inside_a_larger_word() {
    // "f16" must not spuriously match inside an unrelated token like
    // "af1600" — the boundary check is what prevents that.
    assert_eq!(normalize_quantization_from_filename("af1600.gguf"), None);
}

#[test]
fn resolve_tokenizer_repo_uses_same_repo_only_when_tokenizer_json_present() {
    let with_tokenizer = vec!["model.gguf".to_string(), "tokenizer.json".to_string()];
    assert_eq!(
        resolve_tokenizer_repo("owner/name", &with_tokenizer),
        (Some("owner/name".to_string()), false)
    );

    let without_tokenizer = vec!["model.gguf".to_string()];
    assert_eq!(
        resolve_tokenizer_repo("owner/name", &without_tokenizer),
        (None, true)
    );
}

fn sample_result(repo_id: &str, downloads: Option<u64>) -> HuggingFaceModelResult {
    HuggingFaceModelResult {
        repo_id: repo_id.to_string(),
        display_name: repo_id.to_string(),
        author: None,
        license: None,
        downloads,
        files: vec![],
        source_revision: Some("a".repeat(40)),
        revision_ref: None,
    }
}

#[test]
fn dedupe_and_sort_removes_duplicates_and_orders_deterministically() {
    let mut results = vec![
        sample_result("b/low", Some(1)),
        sample_result("a/high", Some(100)),
        sample_result("a/high", Some(100)), // duplicate repoId, dropped
        sample_result("c/none", None),
        sample_result("b/also_none", None),
    ];
    dedupe_and_sort(&mut results);
    let ids: Vec<&str> = results.iter().map(|r| r.repo_id.as_str()).collect();
    // Descending downloads first, then ascending repoId as tie-breaker
    // (None treated as 0, so "c/none" sorts before "b/also_none").
    assert_eq!(ids, vec!["a/high", "b/low", "b/also_none", "c/none"]);
}

// ---------------------------------------------------------------------
// Deterministic model id
// ---------------------------------------------------------------------

#[test]
fn derive_model_id_is_deterministic_and_prefixed() {
    let a = derive_model_id("owner/name", "model.gguf");
    let b = derive_model_id("owner/name", "model.gguf");
    assert_eq!(a, b);
    assert!(a.starts_with("hf-"));
    assert_eq!(a.len(), 3 + 64);
}

#[test]
fn derive_model_id_length_prefixing_prevents_boundary_collisions() {
    // Naive concatenation would make ("ab", "c") collide with ("a", "bc").
    let first = derive_model_id("ab", "c");
    let second = derive_model_id("a", "bc");
    assert_ne!(first, second);
}

#[test]
fn derive_model_id_differs_for_different_filenames_in_same_repo() {
    let a = derive_model_id("owner/name", "model-q4.gguf");
    let b = derive_model_id("owner/name", "model-q8.gguf");
    assert_ne!(a, b);
}

// ---------------------------------------------------------------------
// GGUF header parsing (research.md Entscheidung 9)
// ---------------------------------------------------------------------

/// Builds a minimal synthetic GGUF v3 header buffer with one metadata KV
/// pair for `llama.context_length` = `value` (uint32), preceded by an
/// unrelated array-typed KV pair so the parser has to correctly skip a
/// nested array before reaching the key under test.
fn synthetic_gguf_header(context_length: u32) -> Vec<u8> {
    let mut buf = Vec::new();
    buf.extend_from_slice(b"GGUF");
    buf.extend_from_slice(&3u32.to_le_bytes()); // version
    buf.extend_from_slice(&0u64.to_le_bytes()); // tensor_count
    buf.extend_from_slice(&2u64.to_le_bytes()); // kv_count

    // KV 1: an array of 3 uint8 values under an unrelated key, to prove
    // array-skipping advances the cursor correctly.
    push_gguf_string(&mut buf, "unrelated.array");
    buf.extend_from_slice(&9u32.to_le_bytes()); // type = ARRAY
    buf.extend_from_slice(&0u32.to_le_bytes()); // element type = UINT8
    buf.extend_from_slice(&3u64.to_le_bytes()); // element count
    buf.extend_from_slice(&[1u8, 2u8, 3u8]);

    // KV 2: the key under test.
    push_gguf_string(&mut buf, "llama.context_length");
    buf.extend_from_slice(&4u32.to_le_bytes()); // type = UINT32
    buf.extend_from_slice(&context_length.to_le_bytes());

    buf
}

fn push_gguf_string(buf: &mut Vec<u8>, s: &str) {
    buf.extend_from_slice(&(s.len() as u64).to_le_bytes());
    buf.extend_from_slice(s.as_bytes());
}

#[test]
fn parse_gguf_context_length_finds_the_key_after_skipping_an_array() {
    let buf = synthetic_gguf_header(8192);
    assert_eq!(parse_gguf_context_length(&buf), Some(8192));
}

#[test]
fn parse_gguf_context_length_rejects_wrong_magic() {
    let mut buf = synthetic_gguf_header(4096);
    buf[0..4].copy_from_slice(b"NOPE");
    assert_eq!(parse_gguf_context_length(&buf), None);
}

#[test]
fn parse_gguf_context_length_handles_truncated_buffer_safely() {
    let buf = synthetic_gguf_header(4096);
    // Cut the buffer off mid-header — must return None, never panic.
    for cut in [4usize, 8, 16, 24, 30, buf.len() - 1] {
        assert_eq!(parse_gguf_context_length(&buf[..cut]), None);
    }
}

#[test]
fn parse_gguf_context_length_returns_none_when_key_absent() {
    let mut buf = Vec::new();
    buf.extend_from_slice(b"GGUF");
    buf.extend_from_slice(&3u32.to_le_bytes());
    buf.extend_from_slice(&0u64.to_le_bytes());
    buf.extend_from_slice(&1u64.to_le_bytes());
    push_gguf_string(&mut buf, "some.other_key");
    buf.extend_from_slice(&4u32.to_le_bytes());
    buf.extend_from_slice(&42u32.to_le_bytes());
    assert_eq!(parse_gguf_context_length(&buf), None);
}

// ---------------------------------------------------------------------
// T013/T014: search_huggingface_models / get_huggingface_model_details
// contract tests against a wiremock Hub double.
// ---------------------------------------------------------------------

fn model_json(id: &str, sha: &str, files: &[&str]) -> serde_json::Value {
    serde_json::json!({
        "id": id,
        "author": "someone",
        "downloads": 42,
        "gated": false,
        "private": false,
        "sha": sha,
        "cardData": { "license": "apache-2.0" },
        "siblings": files.iter().map(|f| serde_json::json!({"rfilename": f})).collect::<Vec<_>>(),
    })
}

#[tokio::test]
async fn search_rejects_a_too_short_query_without_any_http_call() {
    let server = MockServer::start().await;
    // No mock registered — a network call here would fail wiremock's
    // "no matching mock" and surface as a different error kind, proving
    // the short-circuit happened before any request was sent.
    let hf = HfClient::new(server.uri()).expect("client");
    let err = search_models(&hf, Some("x"), None)
        .await
        .expect_err("too short");
    assert!(matches!(err, HolziError::InvalidInput { .. }));
}

#[tokio::test]
async fn search_filters_out_repos_without_gguf_and_normalizes_hits() {
    let server = MockServer::start().await;
    let sha_a = "a".repeat(40);
    let sha_b = "b".repeat(40);
    Mock::given(method("GET"))
        .and(path("/api/models"))
        .and(query_param("search", "llama"))
        .respond_with(ResponseTemplate::new(200).set_body_json(serde_json::json!([
            model_json(
                "owner/has-gguf",
                &sha_a,
                &["model.Q4_K_M.gguf", "README.md"]
            ),
            model_json("owner/no-gguf", &sha_b, &["model.safetensors"]),
        ])))
        .mount(&server)
        .await;

    let hf = HfClient::new(server.uri()).expect("client");
    let results = search_models(&hf, Some("llama"), None)
        .await
        .expect("search ok");
    assert_eq!(results.len(), 1);
    assert_eq!(results[0].repo_id, "owner/has-gguf");
    assert_eq!(results[0].files.len(), 1);
    assert_eq!(results[0].files[0].filename, "model.Q4_K_M.gguf");
    assert_eq!(results[0].license.as_deref(), Some("apache-2.0"));
}

#[tokio::test]
async fn search_returns_an_empty_list_for_no_hits() {
    let server = MockServer::start().await;
    Mock::given(method("GET"))
        .and(path("/api/models"))
        .respond_with(ResponseTemplate::new(200).set_body_json(serde_json::json!([])))
        .mount(&server)
        .await;

    let hf = HfClient::new(server.uri()).expect("client");
    let results = search_models(&hf, Some("nothing matches"), None)
        .await
        .expect("search ok");
    assert!(results.is_empty());
}

#[tokio::test]
async fn search_caps_results_at_the_default_limit_of_20() {
    let server = MockServer::start().await;
    let hits: Vec<serde_json::Value> = (0..25)
        .map(|i| {
            model_json(
                &format!("owner/repo-{i:02}"),
                &"a".repeat(40),
                &["model.gguf"],
            )
        })
        .collect();
    Mock::given(method("GET"))
        .and(path("/api/models"))
        .respond_with(ResponseTemplate::new(200).set_body_json(serde_json::json!(hits)))
        .mount(&server)
        .await;

    let hf = HfClient::new(server.uri()).expect("client");
    let results = search_models(&hf, Some("many"), None)
        .await
        .expect("search ok");
    assert_eq!(results.len(), DEFAULT_LIMIT);
}

#[tokio::test]
async fn search_without_query_returns_the_top_ten_downloaded_gguf_repositories() {
    let server = MockServer::start().await;
    let hits: Vec<serde_json::Value> = (0..12)
        .map(|i| {
            let mut hit = model_json(
                &format!("owner/top-{i:02}"),
                &"a".repeat(40),
                &["model.gguf"],
            );
            hit["downloads"] = serde_json::json!(i);
            hit
        })
        .collect();
    Mock::given(method("GET"))
        .and(path("/api/models"))
        .respond_with(ResponseTemplate::new(200).set_body_json(serde_json::json!(hits)))
        .mount(&server)
        .await;

    let hf = HfClient::new(server.uri()).expect("client");
    let results = search_models(&hf, None, None)
        .await
        .expect("top-model search ok");
    assert_eq!(results.len(), DEFAULT_TOP_LIMIT);
    assert_eq!(results[0].repo_id, "owner/top-11");

    let results = search_models(&hf, None, Some(DEFAULT_LIMIT))
        .await
        .expect("top-model search remains capped");
    assert_eq!(results.len(), DEFAULT_TOP_LIMIT);
}

#[tokio::test]
async fn search_maps_http_error_status_and_rate_limit() {
    let server = MockServer::start().await;
    Mock::given(method("GET"))
        .and(path("/api/models"))
        .respond_with(ResponseTemplate::new(503))
        .mount(&server)
        .await;
    let hf = HfClient::new(server.uri()).expect("client");
    let err = search_models(&hf, Some("llama"), None)
        .await
        .expect_err("503");
    assert!(matches!(err, HolziError::HttpStatus { status: 503, .. }));
}

#[tokio::test]
async fn search_maps_429_to_rate_limited() {
    let server = MockServer::start().await;
    Mock::given(method("GET"))
        .and(path("/api/models"))
        .respond_with(ResponseTemplate::new(429).insert_header("retry-after", "7"))
        .mount(&server)
        .await;
    let hf = HfClient::new(server.uri()).expect("client");
    let err = search_models(&hf, Some("llama"), None)
        .await
        .expect_err("429");
    match err {
        HolziError::RateLimited {
            retry_after_seconds,
        } => {
            assert_eq!(retry_after_seconds, Some(7));
        }
        other => panic!("expected RateLimited, got {other:?}"),
    }
}

#[tokio::test]
async fn details_rejects_a_repository_without_any_gguf_file() {
    let server = MockServer::start().await;
    let sha = "a".repeat(40);
    Mock::given(method("GET"))
        .and(path("/api/models/owner/no-gguf"))
        .respond_with(ResponseTemplate::new(200).set_body_json(model_json(
            "owner/no-gguf",
            &sha,
            &["model.safetensors"],
        )))
        .mount(&server)
        .await;
    let hf = HfClient::new(server.uri()).expect("client");
    let err = get_model_details(&hf, &generous_hardware(), "owner/no-gguf", None)
        .await
        .expect_err("no gguf");
    assert!(matches!(err, HolziError::UnsupportedFormat { .. }));
}

#[tokio::test]
async fn details_returns_multiple_files_with_size_and_fit_from_the_tree() {
    let server = MockServer::start().await;
    let sha = "a".repeat(40);
    Mock::given(method("GET"))
        .and(path("/api/models/owner/multi"))
        .respond_with(ResponseTemplate::new(200).set_body_json(model_json(
            "owner/multi",
            &sha,
            &["small.Q4_K_M.gguf", "huge.Q8_0.gguf", "tokenizer.json"],
        )))
        .mount(&server)
        .await;
    Mock::given(method("GET"))
        .and(path(format!("/api/models/owner/multi/tree/{sha}")))
        .respond_with(ResponseTemplate::new(200).set_body_json(serde_json::json!([
            {"path": "small.Q4_K_M.gguf", "size": 1_000_000},
            {"path": "huge.Q8_0.gguf", "lfs": {"size": 999_999_999_999u64}},
        ])))
        .mount(&server)
        .await;

    let hf = HfClient::new(server.uri()).expect("client");
    let details = get_model_details(&hf, &tiny_hardware(), "owner/multi", None)
        .await
        .expect("details ok");
    assert_eq!(details.files.len(), 2);
    let small = details
        .files
        .iter()
        .find(|f| f.filename == "small.Q4_K_M.gguf")
        .unwrap();
    let huge = details
        .files
        .iter()
        .find(|f| f.filename == "huge.Q8_0.gguf")
        .unwrap();
    assert_eq!(small.size_bytes, Some(1_000_000));
    assert_eq!(huge.size_bytes, Some(999_999_999_999));
    assert_eq!(huge.fit, crate::hardware::Fit::TooBig);
    // Both files see the same repo-level tokenizer.json.
    assert_eq!(small.tokenizer_repo.as_deref(), Some("owner/multi"));
    assert!(!small.tokenizer_required);
}

#[tokio::test]
async fn details_missing_license_size_and_tokenizer_are_honestly_absent() {
    let server = MockServer::start().await;
    let sha = "a".repeat(40);
    let mut body = model_json("owner/sparse", &sha, &["model.gguf"]);
    body.as_object_mut().unwrap().remove("cardData");
    Mock::given(method("GET"))
        .and(path("/api/models/owner/sparse"))
        .respond_with(ResponseTemplate::new(200).set_body_json(body))
        .mount(&server)
        .await;
    Mock::given(method("GET"))
        .and(path_regex(r"^/api/models/owner/sparse/tree/.*$"))
        .respond_with(ResponseTemplate::new(200).set_body_json(serde_json::json!([])))
        .mount(&server)
        .await;

    let hf = HfClient::new(server.uri()).expect("client");
    let details = get_model_details(&hf, &generous_hardware(), "owner/sparse", None)
        .await
        .expect("details ok");
    assert_eq!(details.license, None);
    let file = &details.files[0];
    assert_eq!(file.size_bytes, None);
    assert_eq!(file.fit, crate::hardware::Fit::Unknown);
    assert!(file.tokenizer_required);
    assert_eq!(file.tokenizer_repo, None);
}

#[tokio::test]
async fn details_returns_exact_catalog_match_annotation() {
    let entry = crate::catalog::entries()
        .first()
        .expect("built-in catalog is never empty");
    let (repo_id, filename) = (entry.hf_repo.clone(), entry.hf_filename.clone());
    let server = MockServer::start().await;
    let sha = "a".repeat(40);
    Mock::given(method("GET"))
        .and(path(format!("/api/models/{repo_id}")))
        .respond_with(ResponseTemplate::new(200).set_body_json(model_json(
            &repo_id,
            &sha,
            &[filename.as_str()],
        )))
        .mount(&server)
        .await;
    Mock::given(method("GET"))
        .and(path_regex(format!(r"^/api/models/{repo_id}/tree/.*$")))
        .respond_with(ResponseTemplate::new(200).set_body_json(serde_json::json!([])))
        .mount(&server)
        .await;

    let hf = HfClient::new(server.uri()).expect("client");
    let details = get_model_details(&hf, &generous_hardware(), &repo_id, None)
        .await
        .expect("details ok");
    assert!(details.files[0].catalog_match);
    assert_eq!(
        details.files[0].catalog_entry_id.as_deref(),
        Some(entry.id.as_str())
    );
}

// ---------------------------------------------------------------------
// resolve_revision / preview_install
// ---------------------------------------------------------------------

#[tokio::test]
async fn resolve_revision_resolves_a_branch_name_to_a_commit_sha_and_tracks_the_ref() {
    let server = MockServer::start().await;
    let sha = "c".repeat(40);
    Mock::given(method("GET"))
        .and(path("/api/models/owner/name/revision/main"))
        .respond_with(ResponseTemplate::new(200).set_body_json(model_json(
            "owner/name",
            &sha,
            &["model.gguf"],
        )))
        .mount(&server)
        .await;
    let hf = HfClient::new(server.uri()).expect("client");
    let resolved = resolve_revision(&hf, "owner/name", Some("main"))
        .await
        .expect("resolve ok");
    assert_eq!(resolved.sha, sha);
    assert_eq!(resolved.revision_ref.as_deref(), Some("main"));
}

#[tokio::test]
async fn resolve_revision_with_no_input_defaults_to_main_and_tracks_it() {
    let server = MockServer::start().await;
    let sha = "d".repeat(40);
    Mock::given(method("GET"))
        .and(path("/api/models/owner/name"))
        .respond_with(ResponseTemplate::new(200).set_body_json(model_json(
            "owner/name",
            &sha,
            &["model.gguf"],
        )))
        .mount(&server)
        .await;
    let hf = HfClient::new(server.uri()).expect("client");
    let resolved = resolve_revision(&hf, "owner/name", None)
        .await
        .expect("resolve ok");
    assert_eq!(resolved.sha, sha);
    assert_eq!(resolved.revision_ref.as_deref(), Some("main"));
}

#[tokio::test]
async fn resolve_revision_direct_sha_pin_does_not_track_a_ref() {
    let server = MockServer::start().await;
    let sha = "e".repeat(40);
    Mock::given(method("GET"))
        .and(path(format!("/api/models/owner/name/revision/{sha}")))
        .respond_with(ResponseTemplate::new(200).set_body_json(model_json(
            "owner/name",
            &sha,
            &["model.gguf"],
        )))
        .mount(&server)
        .await;
    let hf = HfClient::new(server.uri()).expect("client");
    let resolved = resolve_revision(&hf, "owner/name", Some(&sha))
        .await
        .expect("resolve ok");
    assert_eq!(resolved.sha, sha);
    assert_eq!(resolved.revision_ref, None);
}

#[tokio::test]
async fn resolve_revision_offline_error_surfaces_as_network_error() {
    // No server started at all — connecting must fail as `Network`, not
    // panic or hang.
    let hf = HfClient::new("http://127.0.0.1:1").expect("client");
    let err = resolve_revision(&hf, "owner/name", Some("main"))
        .await
        .expect_err("offline");
    assert!(matches!(err, HolziError::Network { .. }));
}

#[tokio::test]
async fn preview_install_flags_too_big_and_missing_tokenizer_requires_confirmation() {
    let server = MockServer::start().await;
    let sha = "f".repeat(40);
    Mock::given(method("GET"))
        .and(path("/api/models/owner/name"))
        .respond_with(ResponseTemplate::new(200).set_body_json(model_json(
            "owner/name",
            &sha,
            &["model.Q8_0.gguf"],
        )))
        .mount(&server)
        .await;
    Mock::given(method("GET"))
        .and(path(format!("/api/models/owner/name/revision/{sha}")))
        .respond_with(ResponseTemplate::new(200).set_body_json(model_json(
            "owner/name",
            &sha,
            &["model.Q8_0.gguf"],
        )))
        .mount(&server)
        .await;
    Mock::given(method("GET"))
        .and(path(format!("/api/models/owner/name/tree/{sha}")))
        .respond_with(ResponseTemplate::new(200).set_body_json(serde_json::json!([
            {"path": "model.Q8_0.gguf", "size": 900_000_000_000u64},
        ])))
        .mount(&server)
        .await;

    let hf = HfClient::new(server.uri()).expect("client");
    let preview = preview_install(
        &hf,
        &tiny_hardware(),
        "owner/name",
        "model.Q8_0.gguf",
        None,
        None,
        None,
    )
    .await
    .expect("preview ok");
    assert_eq!(preview.fit, crate::hardware::Fit::TooBig);
    assert!(preview.requires_explicit_too_big_confirmation);
    assert!(preview.tokenizer_required);
    assert_eq!(preview.revision, sha);
    assert_eq!(preview.revision_ref.as_deref(), Some("main"));
    assert_eq!(
        preview.model_id,
        derive_model_id("owner/name", "model.Q8_0.gguf")
    );
}

#[tokio::test]
async fn preview_install_rejects_a_filename_not_present_in_the_repository() {
    let server = MockServer::start().await;
    let sha = "1".repeat(40);
    Mock::given(method("GET"))
        .and(path("/api/models/owner/name"))
        .respond_with(ResponseTemplate::new(200).set_body_json(model_json(
            "owner/name",
            &sha,
            &["other.gguf"],
        )))
        .mount(&server)
        .await;
    Mock::given(method("GET"))
        .and(path(format!("/api/models/owner/name/revision/{sha}")))
        .respond_with(ResponseTemplate::new(200).set_body_json(model_json(
            "owner/name",
            &sha,
            &["other.gguf"],
        )))
        .mount(&server)
        .await;
    let hf = HfClient::new(server.uri()).expect("client");
    let err = preview_install(
        &hf,
        &generous_hardware(),
        "owner/name",
        "model.gguf",
        None,
        None,
        None,
    )
    .await
    .expect_err("missing file");
    assert!(matches!(err, HolziError::InvalidInput { .. }));
}

// ---------------------------------------------------------------------
// check_update
// ---------------------------------------------------------------------

#[tokio::test]
async fn check_update_returns_the_current_sha_for_the_tracked_ref() {
    let server = MockServer::start().await;
    let sha = "2".repeat(40);
    Mock::given(method("GET"))
        .and(path("/api/models/owner/name/revision/main"))
        .respond_with(ResponseTemplate::new(200).set_body_json(model_json(
            "owner/name",
            &sha,
            &["model.gguf"],
        )))
        .mount(&server)
        .await;
    let hf = HfClient::new(server.uri()).expect("client");
    let latest = check_update(&hf, "owner/name", "main")
        .await
        .expect("check ok");
    assert_eq!(latest, sha);
}

#[tokio::test]
async fn check_update_http_error_does_not_panic_and_is_retryable() {
    let server = MockServer::start().await;
    Mock::given(method("GET"))
        .and(path("/api/models/owner/name/revision/main"))
        .respond_with(ResponseTemplate::new(404))
        .mount(&server)
        .await;
    let hf = HfClient::new(server.uri()).expect("client");
    let err = check_update(&hf, "owner/name", "main")
        .await
        .expect_err("404");
    assert!(matches!(err, HolziError::HttpStatus { status: 404, .. }));
}

// ---------------------------------------------------------------------
// Range-request header fetch
// ---------------------------------------------------------------------

#[tokio::test]
async fn fetch_header_bytes_sends_a_range_request_and_returns_the_body() {
    let server = MockServer::start().await;
    let sha = "3".repeat(40);
    Mock::given(method("GET"))
        .and(path(format!("/owner/name/resolve/{sha}/model.gguf")))
        .respond_with(ResponseTemplate::new(206).set_body_bytes(b"GGUF-HEADER-BYTES".to_vec()))
        .mount(&server)
        .await;
    let hf = HfClient::new(server.uri()).expect("client");
    let bytes = hf
        .fetch_header_bytes("owner/name", &sha, "model.gguf", 4096)
        .await
        .expect("range fetch ok");
    assert_eq!(bytes, b"GGUF-HEADER-BYTES".to_vec());
}
