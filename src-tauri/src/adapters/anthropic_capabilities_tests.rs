//! Tests for the Anthropic capability mapping (spec
//! 012-unified-model-capabilities, contracts/capabilities-json.md fixture
//! cases 1 to 7). The mapping is a pure function over the wire object, so
//! these need no HTTP double.

use serde_json::{json, Value};

use super::{map_capabilities, WireCapabilities};
use crate::adapters::AttachmentKind;
use crate::model_capabilities::{ModelCapabilities, ReasoningControl, ThinkingStyle};

fn map(wire: Value) -> ModelCapabilities {
    let wire: WireCapabilities = serde_json::from_value(wire).expect("wire shape parses");
    map_capabilities(&wire)
}

fn leaf(supported: bool) -> Value {
    json!({ "supported": supported })
}

fn option_ids(caps: &ModelCapabilities) -> Vec<String> {
    match &caps.reasoning {
        Some(ReasoningControl::Presets { options }) => {
            options.iter().map(|o| o.id.clone()).collect()
        }
        other => panic!("expected presets, got {other:?}"),
    }
}

#[test]
fn a_full_adaptive_tree_maps_to_all_five_levels_and_every_attachment_kind() {
    let caps = map(json!({
        "image_input": leaf(true),
        "pdf_input": leaf(true),
        "thinking": {
            "supported": true,
            "types": { "enabled": leaf(false), "adaptive": leaf(true) },
        },
        "effort": {
            "supported": true,
            "low": leaf(true), "medium": leaf(true), "high": leaf(true),
            "xhigh": leaf(true), "max": leaf(true),
        },
    }));

    assert_eq!(option_ids(&caps), ["low", "medium", "high", "xhigh", "max"]);
    assert_eq!(caps.thinking_style, Some(ThinkingStyle::Adaptive));
    assert_eq!(
        caps.accepted_attachment_kinds,
        Some(vec![
            AttachmentKind::Text,
            AttachmentKind::Image,
            AttachmentKind::Document,
        ])
    );
}

#[test]
fn effort_without_xhigh_and_manual_thinking_keep_their_own_shape() {
    let caps = map(json!({
        "image_input": leaf(true),
        "pdf_input": leaf(false),
        "thinking": {
            "supported": true,
            "types": { "enabled": leaf(true), "adaptive": leaf(false) },
        },
        "effort": {
            "supported": true,
            "low": leaf(true), "medium": leaf(true), "high": leaf(true),
            "xhigh": leaf(false), "max": leaf(true),
        },
    }));

    assert_eq!(option_ids(&caps), ["low", "medium", "high", "max"]);
    assert_eq!(caps.thinking_style, Some(ThinkingStyle::Manual));
    assert_eq!(
        caps.accepted_attachment_kinds,
        Some(vec![AttachmentKind::Text, AttachmentKind::Image])
    );
}

#[test]
fn thinking_without_effort_maps_to_model_managed() {
    let caps = map(json!({
        "image_input": leaf(true),
        "pdf_input": leaf(true),
        "thinking": {
            "supported": true,
            "types": { "enabled": leaf(true), "adaptive": leaf(false) },
        },
        "effort": { "supported": false },
    }));

    assert_eq!(caps.reasoning, Some(ReasoningControl::ModelManaged));
    assert_eq!(caps.thinking_style, Some(ThinkingStyle::Manual));
}

#[test]
fn neither_thinking_nor_effort_is_an_authoritative_unavailable() {
    let caps = map(json!({
        "image_input": leaf(true),
        "pdf_input": leaf(true),
        "thinking": { "supported": false, "types": {} },
        "effort": { "supported": false },
    }));

    assert_eq!(caps.reasoning, Some(ReasoningControl::Unavailable));
    assert_eq!(caps.thinking_style, None);
}

#[test]
fn an_absent_capabilities_object_leaves_everything_not_determined() {
    let caps = map(json!({}));

    assert!(caps.is_undetermined());
}

#[test]
fn a_missing_subtree_leaves_reasoning_not_determined_rather_than_unavailable() {
    let caps = map(json!({
        "image_input": leaf(true),
        "pdf_input": leaf(true),
        "effort": { "supported": false },
    }));

    assert_eq!(caps.reasoning, None);
    assert!(caps.accepted_attachment_kinds.is_some());
}

#[test]
fn a_missing_pdf_key_undetermines_attachments_but_not_reasoning() {
    let caps = map(json!({
        "image_input": leaf(true),
        "thinking": { "supported": false },
        "effort": { "supported": false },
    }));

    assert_eq!(caps.accepted_attachment_kinds, None);
    assert_eq!(caps.reasoning, Some(ReasoningControl::Unavailable));
}

#[test]
fn unknown_capability_leaves_are_ignored() {
    let caps = map(json!({
        "image_input": leaf(false),
        "pdf_input": leaf(false),
        "structured_outputs": leaf(true),
        "batch": leaf(true),
        "thinking": { "supported": false, "future_field": { "x": 1 } },
        "effort": { "supported": false, "ultra": leaf(true) },
    }));

    assert_eq!(caps.reasoning, Some(ReasoningControl::Unavailable));
    assert_eq!(
        caps.accepted_attachment_kinds,
        Some(vec![AttachmentKind::Text])
    );
}

#[test]
fn supported_effort_with_no_supported_level_is_not_a_selectable_control() {
    let caps = map(json!({
        "thinking": { "supported": true, "types": { "adaptive": leaf(true) } },
        "effort": { "supported": true, "low": leaf(false) },
    }));

    assert_eq!(caps.reasoning, Some(ReasoningControl::ModelManaged));
}
