//! Pure scoring of a model's tool calling (spec 032 US5, contracts/eval-format.md). No I/O: the
//! runner hands in what the model did for one sentence, this module says how that counts.

use std::collections::BTreeMap;

use serde::{Deserialize, Serialize};
use serde_json::Value;

use crate::chat::tools::action_tool::AgentActionDef;

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum Lang {
    De,
    En,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum Kind {
    Read,
    Change,
    Smalltalk,
}

/// One tool call a sentence is expected to produce. `args` holds only the fields that matter.
#[derive(Debug, Clone, PartialEq, Deserialize)]
pub struct ExpectedCall {
    pub tool: String,
    #[serde(default = "empty_object")]
    pub args: Value,
}

fn empty_object() -> Value {
    Value::Object(Default::default())
}

/// `"none"` (no tool call) or the complete, non-empty batch of calls, in any order.
#[derive(Debug, Clone, PartialEq)]
pub enum Expect {
    None,
    Calls(Vec<ExpectedCall>),
}

impl<'de> Deserialize<'de> for Expect {
    fn deserialize<D: serde::Deserializer<'de>>(deserializer: D) -> Result<Self, D::Error> {
        #[derive(Deserialize)]
        #[serde(untagged)]
        enum Raw {
            Word(String),
            List(Vec<ExpectedCall>),
        }
        match Raw::deserialize(deserializer)? {
            Raw::Word(word) if word == "none" => Ok(Expect::None),
            Raw::Word(word) => Err(serde::de::Error::custom(format!(
                "expect must be \"none\" or a list, not \"{word}\""
            ))),
            Raw::List(calls) if calls.is_empty() => {
                Err(serde::de::Error::custom("expect must not be an empty list"))
            }
            Raw::List(calls) => Ok(Expect::Calls(calls)),
        }
    }
}

#[derive(Debug, Clone, PartialEq, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Sentence {
    pub id: String,
    pub lang: Lang,
    pub kind: Kind,
    pub text: String,
    pub expect: Expect,
    #[serde(default)]
    pub self_test: bool,
}

#[derive(Debug, Clone, PartialEq, Deserialize)]
pub struct EvalSet {
    pub version: u32,
    pub sentences: Vec<Sentence>,
}

/// A tool call as the model made it.
#[derive(Debug, Clone, PartialEq, Serialize)]
pub struct ObservedCall {
    pub tool: String,
    pub args: Value,
}

/// What the runner saw for one sentence: the calls of the scored step (empty for a text answer)
/// and what the search did, if the sentence needed one.
#[derive(Debug, Clone, PartialEq, Default)]
pub struct Observed {
    pub calls: Vec<ObservedCall>,
    /// The sentence took an extra step to search for its tool.
    pub searched: bool,
    /// The expected tools were offered by the second step at the latest (core offer or search).
    pub reached: bool,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum SentenceResult {
    Pass,
    WrongTool,
    BadArgs,
    Missed,
    Spurious,
    NotFound,
}

/// Whether `value` fits a schema of the action subset (`type`, `properties`, `required`, `items`,
/// `enum`). An object with declared `properties` is strict, like the runner in the frontend.
pub fn valid_against(schema: &Value, value: &Value) -> bool {
    let type_ok = match schema.get("type").and_then(Value::as_str) {
        Some("object") => value.is_object(),
        Some("array") => value.is_array(),
        Some("string") => value.is_string(),
        Some("boolean") => value.is_boolean(),
        Some("integer") => value.is_i64() || value.is_u64(),
        Some("number") => value.is_number(),
        _ => false,
    };
    if !type_ok {
        return false;
    }
    if let Some(allowed) = schema.get("enum").and_then(Value::as_array) {
        if !allowed.contains(value) {
            return false;
        }
    }
    if let (Some(items), Some(values)) = (schema.get("items"), value.as_array()) {
        if !values.iter().all(|item| valid_against(items, item)) {
            return false;
        }
    }
    if let (Some(object), Some(properties)) = (
        value.as_object(),
        schema.get("properties").and_then(Value::as_object),
    ) {
        if object.keys().any(|key| !properties.contains_key(key)) {
            return false;
        }
        let required = schema
            .get("required")
            .and_then(Value::as_array)
            .into_iter()
            .flatten()
            .filter_map(Value::as_str);
        for key in required {
            if !object.contains_key(key) {
                return false;
            }
        }
        for (key, child) in properties {
            if let Some(field) = object.get(key) {
                if !valid_against(child, field) {
                    return false;
                }
            }
        }
    }
    true
}

/// Every field of `expected` is present in `actual` with the same value; objects match as a
/// subset, everything else exactly.
fn contains_subset(actual: &Value, expected: &Value) -> bool {
    match (actual, expected) {
        (Value::Object(actual), Value::Object(expected)) => expected.iter().all(|(key, value)| {
            actual
                .get(key)
                .is_some_and(|found| contains_subset(found, value))
        }),
        _ => actual == expected,
    }
}

fn names_of<'a>(names: impl Iterator<Item = &'a str>) -> Vec<&'a str> {
    let mut names: Vec<&str> = names.collect();
    names.sort_unstable();
    names
}

/// How one sentence counts (contracts/eval-format.md, "Bewertung je Satz").
pub fn score(sentence: &Sentence, observed: &Observed, defs: &[AgentActionDef]) -> SentenceResult {
    let expected = match &sentence.expect {
        Expect::None => {
            return if observed.calls.is_empty() {
                SentenceResult::Pass
            } else {
                SentenceResult::Spurious
            }
        }
        Expect::Calls(expected) => expected,
    };
    if observed.searched && !observed.reached {
        return SentenceResult::NotFound;
    }
    if observed.calls.is_empty() {
        return SentenceResult::Missed;
    }
    // The whole batch counts: a missing, a doubled and an extra call all make it wrong.
    let want = names_of(expected.iter().map(|call| call.tool.as_str()));
    let got = names_of(observed.calls.iter().map(|call| call.tool.as_str()));
    if want != got {
        return SentenceResult::WrongTool;
    }
    let mut unmatched: Vec<&ExpectedCall> = expected.iter().collect();
    for call in &observed.calls {
        let schema = defs
            .iter()
            .find(|def| def.tool_name == call.tool)
            .map(|def| &def.input_schema);
        if !schema.is_some_and(|schema| valid_against(schema, &call.args)) {
            return SentenceResult::BadArgs;
        }
        // Calls of one tool are matched to expectations of that tool in any order.
        let position = unmatched
            .iter()
            .position(|e| e.tool == call.tool && contains_subset(&call.args, &e.args));
        match position {
            Some(position) => {
                unmatched.remove(position);
            }
            None => return SentenceResult::BadArgs,
        }
    }
    SentenceResult::Pass
}

/// `pass` of `of`, with the rate.
#[derive(Debug, Clone, Copy, PartialEq, Serialize)]
pub struct Rate {
    pub pass: usize,
    pub of: usize,
    pub rate: f64,
}

impl Rate {
    fn new(pass: usize, of: usize) -> Self {
        let rate = if of == 0 {
            0.0
        } else {
            pass as f64 / of as f64
        };
        Self { pass, of, rate }
    }
}

#[derive(Debug, Clone, PartialEq, Serialize)]
pub struct Failure {
    pub id: String,
    pub result: SentenceResult,
    /// The first call the model made, if it made any.
    pub got: Option<ObservedCall>,
}

#[derive(Debug, Clone, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct EvalReport {
    pub set_version: u32,
    pub model: String,
    /// The sampler was deterministic. Cloud models cannot be, so their runs differ more.
    pub deterministic: bool,
    pub total: Rate,
    pub per_lang: BTreeMap<Lang, Rate>,
    pub per_kind: BTreeMap<Kind, Rate>,
    /// Of the sentences that expect an action, the share whose tool was offered by the second
    /// step at the latest (spec SC-005).
    pub reach_rate: f64,
    /// Sentences that needed one extra search step.
    pub extra_steps: usize,
    /// Sentences that expected no call and got one.
    pub spurious_calls: usize,
    pub failures: Vec<Failure>,
}

/// One scored sentence, as the runner collects them.
pub struct Scored<'a> {
    pub sentence: &'a Sentence,
    pub observed: Observed,
    pub result: SentenceResult,
}

/// Builds the report: rates overall, per language and per kind, the reach of the search and the
/// failures.
pub fn report(
    set_version: u32,
    model: &str,
    deterministic: bool,
    scored: &[Scored<'_>],
) -> EvalReport {
    let passed = |s: &Scored<'_>| s.result == SentenceResult::Pass;
    let mut per_lang: BTreeMap<Lang, (usize, usize)> = BTreeMap::new();
    let mut per_kind: BTreeMap<Kind, (usize, usize)> = BTreeMap::new();
    for item in scored {
        let lang = per_lang.entry(item.sentence.lang).or_default();
        lang.1 += 1;
        let kind = per_kind.entry(item.sentence.kind).or_default();
        kind.1 += 1;
        if passed(item) {
            lang.0 += 1;
            kind.0 += 1;
        }
    }
    let acting: Vec<&Scored<'_>> = scored
        .iter()
        .filter(|s| matches!(s.sentence.expect, Expect::Calls(_)))
        .collect();
    let reached = acting.iter().filter(|s| s.observed.reached).count();
    EvalReport {
        set_version,
        model: model.to_owned(),
        deterministic,
        total: Rate::new(scored.iter().filter(|s| passed(s)).count(), scored.len()),
        per_lang: per_lang
            .into_iter()
            .map(|(k, (p, o))| (k, Rate::new(p, o)))
            .collect(),
        per_kind: per_kind
            .into_iter()
            .map(|(k, (p, o))| (k, Rate::new(p, o)))
            .collect(),
        reach_rate: if acting.is_empty() {
            1.0
        } else {
            reached as f64 / acting.len() as f64
        },
        extra_steps: scored.iter().filter(|s| s.observed.searched).count(),
        spurious_calls: scored
            .iter()
            .filter(|s| s.result == SentenceResult::Spurious)
            .count(),
        failures: scored
            .iter()
            .filter(|s| !passed(s))
            .map(|s| Failure {
                id: s.sentence.id.clone(),
                result: s.result,
                got: s.observed.calls.first().cloned(),
            })
            .collect(),
    }
}
