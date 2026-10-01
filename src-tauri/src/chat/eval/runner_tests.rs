use std::sync::{Arc, Mutex};

use async_trait::async_trait;
use serde_json::json;
use tokio_util::sync::CancellationToken;

use super::runner::{run_eval, EvalError, Which};
use super::scoring::{Expect, Sentence};
use super::{embedded_set, embedded_tools};
use crate::adapters::types::{
    AdapterStream, ChatRequest, ChatRole, Sampling, StreamChunk, ToolCall,
};
use crate::adapters::{AdapterError, ProviderAdapter, ProviderModel};

type Script = Box<dyn Fn(&ChatRequest) -> Vec<StreamChunk> + Send + Sync>;

/// A model that answers by a script and remembers what it was asked.
struct Scripted {
    script: Script,
    seen: Arc<Mutex<Vec<ChatRequest>>>,
}

impl Scripted {
    fn new(script: impl Fn(&ChatRequest) -> Vec<StreamChunk> + Send + Sync + 'static) -> Self {
        Self {
            script: Box::new(script),
            seen: Arc::default(),
        }
    }
}

#[async_trait]
impl ProviderAdapter for Scripted {
    async fn list_models(&self) -> Result<Vec<ProviderModel>, AdapterError> {
        Ok(Vec::new())
    }

    async fn stream_chat(&self, req: ChatRequest) -> Result<AdapterStream, AdapterError> {
        let chunks = (self.script)(&req);
        self.seen.lock().unwrap().push(req);
        let (tx, rx) = tokio::sync::mpsc::unbounded_channel();
        let task = tokio::spawn(async move {
            for chunk in chunks {
                let _ = tx.send(Ok(chunk));
            }
            let _ = tx.send(Ok(StreamChunk::Done {
                finish_reason: Some("end_turn".to_owned()),
                prompt_tokens: None,
                completion_tokens: None,
                ttft_ms: None,
                total_ms: 0,
            }));
        });
        Ok(AdapterStream::new(rx, task.abort_handle()))
    }
}

fn calls(batch: Vec<(&str, serde_json::Value)>) -> StreamChunk {
    StreamChunk::ToolCalls(
        batch
            .into_iter()
            .enumerate()
            .map(|(i, (name, input))| ToolCall {
                id: format!("c{i}"),
                name: name.to_owned(),
                input,
            })
            .collect(),
    )
}

fn user_text(req: &ChatRequest) -> &str {
    &req.messages[0].content
}

fn sentence<'a>(text: &str, set: &'a super::scoring::EvalSet) -> &'a Sentence {
    set.sentences.iter().find(|s| s.text == text).unwrap()
}

/// The model the set was written for: calls what a sentence expects, searches first when the tool
/// is not offered yet, and only talks for smalltalk.
fn oracle() -> Scripted {
    let set = embedded_set();
    Scripted::new(move |req| {
        let sentence = sentence(user_text(req), &set);
        let Expect::Calls(expected) = &sentence.expect else {
            return vec![StreamChunk::Delta {
                content: "Gern.".to_owned(),
                reasoning: None,
            }];
        };
        let offered = |name: &str| req.tools.iter().any(|t| t.name == name);
        if expected.iter().all(|c| offered(&c.tool)) {
            return vec![calls(
                expected
                    .iter()
                    .map(|c| (c.tool.as_str(), c.args.clone()))
                    .collect(),
            )];
        }
        // Not offered: search by the words of the tool's name, the way a model would.
        let query = expected[0].tool.replace('_', " ");
        vec![calls(vec![("find_actions", json!({ "query": query }))])]
    })
}

async fn run(adapter: &Scripted, which: Which, deterministic: bool) -> super::scoring::EvalReport {
    run_eval(
        adapter,
        "test-model",
        deterministic,
        &embedded_set(),
        &embedded_tools(),
        which,
        &CancellationToken::new(),
    )
    .await
    .unwrap()
}

#[tokio::test]
async fn a_model_that_does_what_the_set_expects_passes_every_sentence() {
    let report = run(&oracle(), Which::All, true).await;
    let ids: Vec<&str> = report.failures.iter().map(|f| f.id.as_str()).collect();
    assert!(ids.is_empty(), "{ids:?}");
    assert_eq!(report.total.pass, report.total.of);
    assert_eq!(report.total.of, embedded_set().sentences.len());
    assert!((report.reach_rate - 1.0).abs() < 1e-9);
    assert!(
        report.extra_steps >= 8,
        "the set has at least eight sentences outside the core offer"
    );
    assert_eq!(report.spurious_calls, 0);
    assert!(report.deterministic);
    assert_eq!(report.model, "test-model");
}

#[tokio::test]
async fn a_found_tool_is_offered_in_the_second_step_with_the_search_result() {
    let adapter = oracle();
    run(&adapter, Which::All, false).await;
    let seen = adapter.seen.lock().unwrap();
    let second = seen
        .iter()
        .find(|req| req.messages.len() == 3)
        .expect("a second step exists");
    assert!(matches!(second.messages[1].role, ChatRole::ToolCall { .. }));
    assert!(matches!(
        second.messages[2].role,
        ChatRole::ToolResult { .. }
    ));
    assert!(second.tools.iter().any(|t| t.name == "find_actions"));
    assert!(
        second.tools.len() <= 15,
        "core and search plus at most five hits"
    );
    // Every first step offers the same core tools and the search, whatever the sentence.
    let first_offers: std::collections::HashSet<Vec<String>> = seen
        .iter()
        .filter(|req| req.messages.len() == 1)
        .map(|req| req.tools.iter().map(|t| t.name.clone()).collect())
        .collect();
    assert_eq!(first_offers.len(), 1);
    assert!(first_offers.iter().next().unwrap().len() <= 10);
}

#[tokio::test]
async fn the_sampling_follows_the_flag() {
    for (deterministic, expected) in [(true, Sampling::Deterministic), (false, Sampling::Default)] {
        let adapter = oracle();
        run(&adapter, Which::SelfTest, deterministic).await;
        assert!(adapter
            .seen
            .lock()
            .unwrap()
            .iter()
            .all(|r| r.sampling == expected));
    }
}

#[tokio::test]
async fn the_self_test_asks_only_the_marked_sentences() {
    let adapter = oracle();
    let report = run(&adapter, Which::SelfTest, true).await;
    let marked = embedded_set()
        .sentences
        .iter()
        .filter(|s| s.self_test)
        .count();
    assert_eq!(report.total.of, marked);
    assert!((5..=6).contains(&marked));
    assert_eq!(
        adapter.seen.lock().unwrap().len(),
        marked,
        "all five are core or smalltalk: one step each"
    );
}

#[tokio::test]
async fn a_search_that_finds_nothing_is_not_found_and_costs_no_second_step() {
    let adapter = Scripted::new(|req| {
        if req.messages.len() == 1 {
            vec![calls(vec![(
                "find_actions",
                json!({ "query": "zzzz nothing" }),
            )])]
        } else {
            panic!("no second step expected when the tool was not found");
        }
    });
    let set = embedded_set();
    let report = run_eval(
        &adapter,
        "m",
        true,
        &set,
        &embedded_tools(),
        Which::All,
        &CancellationToken::new(),
    )
    .await
    .unwrap();
    let not_found = report
        .failures
        .iter()
        .filter(|f| serde_json::to_value(f.result).unwrap() == "not_found")
        .count();
    assert!(not_found >= 8);
    assert!(
        report.reach_rate < 1.0,
        "the sentences outside the core offer were not reached"
    );
}

#[tokio::test]
async fn a_model_that_only_talks_misses_and_one_that_always_calls_is_spurious() {
    let talker = Scripted::new(|_| {
        vec![StreamChunk::Delta {
            content: "I cannot do that.".to_owned(),
            reasoning: None,
        }]
    });
    let report = run(&talker, Which::All, true).await;
    assert_eq!(report.per_kind[&super::scoring::Kind::Smalltalk].pass, 6);
    assert_eq!(report.spurious_calls, 0);

    let eager = Scripted::new(|_| vec![calls(vec![("wm_state_get", json!({}))])]);
    let report = run(&eager, Which::All, true).await;
    assert_eq!(report.spurious_calls, 6);
}

#[tokio::test]
async fn cancelling_stops_the_run() {
    let cancel = CancellationToken::new();
    cancel.cancel();
    let result = run_eval(
        &oracle(),
        "m",
        true,
        &embedded_set(),
        &embedded_tools(),
        Which::All,
        &cancel,
    )
    .await;
    assert!(matches!(result, Err(EvalError::Cancelled)));
}
