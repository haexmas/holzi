use serde_json::{json, Value};

use super::scoring::{
    report, score, valid_against, EvalSet, Expect, Observed, ObservedCall, Scored, Sentence,
    SentenceResult::{self, *},
};
use super::{embedded_set, embedded_tools};
use crate::chat::tools::action_tool::AgentActionDef;

fn defs() -> Vec<AgentActionDef> {
    serde_json::from_value(json!([
        {
            "toolName": "settings_appearance_setColorScheme", "actionId": "settings.appearance.setColorScheme",
            "description": "d", "effect": "write", "core": true,
            "inputSchema": { "type": "object", "properties": {
                "scheme": { "type": "string", "enum": ["light", "dark", "system"] },
                "note": { "type": "string" } }, "required": ["scheme"] },
        },
        {
            "toolName": "wm_state_get", "actionId": "wm.state.get", "description": "d",
            "effect": "read", "core": true, "inputSchema": { "type": "object", "properties": {} },
        },
        {
            "toolName": "wm_tab_close", "actionId": "wm.tab.close", "description": "d",
            "effect": "destructive", "core": true,
            "inputSchema": { "type": "object", "properties": { "tabId": { "type": "string" } },
                "required": ["tabId"] },
        },
    ]))
    .unwrap()
}

fn sentence(expect: Value) -> Sentence {
    serde_json::from_value(json!({
        "id": "s", "lang": "en", "kind": "change", "text": "t", "expect": expect,
    }))
    .unwrap()
}

fn call(tool: &str, args: Value) -> ObservedCall {
    ObservedCall {
        tool: tool.to_owned(),
        args,
    }
}

fn observed(calls: Vec<ObservedCall>) -> Observed {
    Observed {
        calls,
        searched: false,
        reached: true,
    }
}

fn scheme(value: &str) -> Value {
    json!({ "scheme": value })
}

fn dark() -> Sentence {
    sentence(
        json!([{ "tool": "settings_appearance_setColorScheme", "args": { "scheme": "dark" } }]),
    )
}

fn result_of(sentence: &Sentence, observed: &Observed) -> SentenceResult {
    score(sentence, observed, &defs())
}

#[test]
fn the_right_call_with_the_right_args_passes() {
    let seen = observed(vec![call(
        "settings_appearance_setColorScheme",
        scheme("dark"),
    )]);
    assert_eq!(result_of(&dark(), &seen), Pass);
}

#[test]
fn extra_schema_valid_fields_do_no_harm() {
    let seen = observed(vec![call(
        "settings_appearance_setColorScheme",
        json!({ "scheme": "dark", "note": "because" }),
    )]);
    assert_eq!(result_of(&dark(), &seen), Pass);
}

#[test]
fn a_wrong_or_extra_or_doubled_or_missing_call_is_wrong_tool() {
    let right = call("settings_appearance_setColorScheme", scheme("dark"));
    let other = call("wm_state_get", json!({}));
    assert_eq!(
        result_of(&dark(), &observed(vec![other.clone()])),
        WrongTool
    );
    assert_eq!(
        result_of(&dark(), &observed(vec![right.clone(), other.clone()])),
        WrongTool,
        "an unexpected call next to the right one"
    );
    assert_eq!(
        result_of(&dark(), &observed(vec![right.clone(), right.clone()])),
        WrongTool,
        "a doubled call"
    );
    let both = sentence(json!([
        { "tool": "settings_appearance_setColorScheme", "args": { "scheme": "dark" } },
        { "tool": "wm_state_get" },
    ]));
    assert_eq!(result_of(&both, &observed(vec![right.clone()])), WrongTool);
    // The order of a complete batch does not matter.
    assert_eq!(result_of(&both, &observed(vec![other, right])), Pass);
}

#[test]
fn an_invalid_or_wrong_input_is_bad_args() {
    let name = "settings_appearance_setColorScheme";
    for args in [
        json!({}),
        json!({ "scheme": "purple" }),
        json!({ "scheme": 3 }),
        json!({ "scheme": "dark", "unknown": 1 }),
        scheme("light"),
    ] {
        assert_eq!(
            result_of(&dark(), &observed(vec![call(name, args.clone())])),
            BadArgs,
            "{args}"
        );
    }
}

#[test]
fn text_only_for_an_expected_call_is_missed() {
    assert_eq!(result_of(&dark(), &observed(Vec::new())), Missed);
}

#[test]
fn a_call_for_smalltalk_is_spurious_and_text_passes() {
    let talk = sentence(json!("none"));
    assert_eq!(result_of(&talk, &observed(Vec::new())), Pass);
    let seen = observed(vec![call("wm_state_get", json!({}))]);
    assert_eq!(result_of(&talk, &seen), Spurious);
}

#[test]
fn a_search_that_did_not_offer_the_tool_is_not_found_not_a_wrong_call() {
    let seen = Observed {
        calls: Vec::new(),
        searched: true,
        reached: false,
    };
    assert_eq!(result_of(&dark(), &seen), NotFound);
    let found = Observed {
        calls: vec![call("settings_appearance_setColorScheme", scheme("dark"))],
        searched: true,
        reached: true,
    };
    assert_eq!(result_of(&dark(), &found), Pass);
}

#[test]
fn two_calls_of_one_tool_match_in_any_order() {
    let close = |id: &str| call("wm_tab_close", json!({ "tabId": id }));
    let two = sentence(json!([
        { "tool": "wm_tab_close", "args": { "tabId": "a" } },
        { "tool": "wm_tab_close", "args": { "tabId": "b" } },
    ]));
    assert_eq!(
        result_of(&two, &observed(vec![close("b"), close("a")])),
        Pass
    );
    assert_eq!(
        result_of(&two, &observed(vec![close("a"), close("a")])),
        BadArgs
    );
}

#[test]
fn the_schema_check_is_strict_like_the_frontend_runner() {
    let schema = json!({ "type": "object", "properties": { "n": { "type": "integer" },
        "xs": { "type": "array", "items": { "type": "string" } } }, "required": ["n"] });
    assert!(valid_against(&schema, &json!({ "n": 1, "xs": ["a"] })));
    assert!(!valid_against(&schema, &json!({ "n": 1.5 })));
    assert!(!valid_against(&schema, &json!({ "xs": [] })));
    assert!(!valid_against(&schema, &json!({ "n": 1, "xs": [2] })));
    assert!(!valid_against(&schema, &json!({ "n": 1, "other": true })));
    // An object without declared properties accepts any object.
    assert!(valid_against(
        &json!({ "type": "object" }),
        &json!({ "any": 1 })
    ));
}

fn set_of(sentences: Value) -> EvalSet {
    serde_json::from_value(json!({ "version": 2, "sentences": sentences })).unwrap()
}

#[test]
fn rates_come_overall_per_language_and_per_kind_with_the_reach_of_the_search() {
    let set = set_of(json!([
        { "id": "a", "lang": "de", "kind": "read", "text": "t",
          "expect": [{ "tool": "wm_state_get" }] },
        { "id": "b", "lang": "en", "kind": "change", "text": "t",
          "expect": [{ "tool": "wm_tab_close", "args": { "tabId": "x" } }] },
        { "id": "c", "lang": "en", "kind": "smalltalk", "text": "t", "expect": "none" },
        { "id": "d", "lang": "de", "kind": "smalltalk", "text": "t", "expect": "none" },
    ]));
    let defs = defs();
    let runs = [
        // a passes directly.
        observed(vec![call("wm_state_get", json!({}))]),
        // b needed a search and it did not deliver.
        Observed {
            calls: Vec::new(),
            searched: true,
            reached: false,
        },
        // c passes, d is spurious.
        observed(Vec::new()),
        observed(vec![call("wm_state_get", json!({}))]),
    ];
    let scored: Vec<Scored<'_>> = set
        .sentences
        .iter()
        .zip(runs)
        .map(|(sentence, observed)| Scored {
            result: score(sentence, &observed, &defs),
            sentence,
            observed,
        })
        .collect();
    let report = report(2, "m", true, &scored);

    assert_eq!((report.total.pass, report.total.of), (2, 4));
    assert!((report.total.rate - 0.5).abs() < 1e-9);
    let json = serde_json::to_value(&report).unwrap();
    assert_eq!(json["perLang"]["de"]["of"], 2);
    assert_eq!(json["perLang"]["en"]["pass"], 1);
    assert_eq!(json["perKind"]["smalltalk"]["pass"], 1);
    assert_eq!(json["perKind"]["read"]["pass"], 1);
    assert!(
        (report.reach_rate - 0.5).abs() < 1e-9,
        "one of two acting sentences reached"
    );
    assert_eq!(report.extra_steps, 1);
    assert_eq!(report.spurious_calls, 1);
    let ids: Vec<(&str, SentenceResult)> = report
        .failures
        .iter()
        .map(|f| (f.id.as_str(), f.result))
        .collect();
    assert_eq!(ids, [("b", NotFound), ("d", Spurious)]);
    assert_eq!(
        report.failures[1].got.as_ref().unwrap().tool,
        "wm_state_get"
    );
    assert_eq!(json["setVersion"], 2);
    assert_eq!(json["deterministic"], true);
}

#[test]
fn an_expectation_must_be_none_or_a_non_empty_list() {
    for bad in [json!("some"), json!([]), json!(3)] {
        let parsed = serde_json::from_value::<Sentence>(json!({
            "id": "s", "lang": "en", "kind": "read", "text": "t", "expect": bad,
        }));
        assert!(parsed.is_err(), "{bad}");
    }
    assert!(matches!(sentence(json!("none")).expect, Expect::None));
}

#[test]
fn the_embedded_set_and_snapshot_are_well_formed() {
    let set = embedded_set();
    let tools = embedded_tools();
    assert_eq!(set.version, 2);
    assert!(!tools.is_empty());
    let mut ids = std::collections::HashSet::new();
    for sentence in &set.sentences {
        assert!(ids.insert(&sentence.id), "duplicate id {}", sentence.id);
        if let Expect::Calls(calls) = &sentence.expect {
            for call in calls {
                assert!(
                    tools.iter().any(|t| t.tool_name == call.tool),
                    "{}: unknown tool {}",
                    sentence.id,
                    call.tool
                );
            }
        }
    }
}
