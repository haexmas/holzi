use super::effort::{anthropic_supported_levels, clamp, claude_delegate_levels, EffortLevel};

#[test]
fn every_level_round_trips_through_as_str_and_parse() {
    for level in [
        EffortLevel::Low,
        EffortLevel::Medium,
        EffortLevel::High,
        EffortLevel::XHigh,
        EffortLevel::Max,
    ] {
        assert_eq!(EffortLevel::parse(level.as_str()), Some(level));
    }
}

#[test]
fn parse_rejects_unknown_strings() {
    assert_eq!(EffortLevel::parse(""), None);
    assert_eq!(EffortLevel::parse("HIGH"), None);
    assert_eq!(EffortLevel::parse("extra-high"), None);
}

#[test]
fn sonnet_5_includes_xhigh() {
    let levels = anthropic_supported_levels("claude-sonnet-5");
    assert!(levels.contains(&EffortLevel::XHigh));
    assert!(levels.contains(&EffortLevel::Max));
}

#[test]
fn opus_4_6_supports_max_but_not_xhigh() {
    let levels = anthropic_supported_levels("claude-opus-4-6");
    assert!(levels.contains(&EffortLevel::Max));
    assert!(!levels.contains(&EffortLevel::XHigh));
}

#[test]
fn unrecognized_model_supports_no_effort_levels() {
    assert_eq!(
        anthropic_supported_levels("claude-3-5-sonnet-20241022"),
        &[]
    );
    assert_eq!(anthropic_supported_levels("gpt-4o"), &[]);
}

#[test]
fn claude_delegate_always_offers_the_full_set() {
    let levels = claude_delegate_levels();
    for level in [
        EffortLevel::Low,
        EffortLevel::Medium,
        EffortLevel::High,
        EffortLevel::XHigh,
        EffortLevel::Max,
    ] {
        assert!(levels.contains(&level));
    }
}

#[test]
fn clamp_keeps_a_directly_supported_level() {
    let supported = [EffortLevel::Low, EffortLevel::Medium, EffortLevel::High];
    assert_eq!(
        clamp(EffortLevel::Medium, &supported),
        Some(EffortLevel::Medium)
    );
}

#[test]
fn clamp_falls_back_to_the_nearest_lower_supported_level() {
    let supported = [EffortLevel::Low, EffortLevel::Medium, EffortLevel::High];
    assert_eq!(
        clamp(EffortLevel::XHigh, &supported),
        Some(EffortLevel::High)
    );
}

#[test]
fn clamp_skips_over_a_gap_to_the_nearest_lower_supported_level() {
    // opus-4-6-shaped table: High and Max supported, XHigh is not.
    let supported = [
        EffortLevel::Low,
        EffortLevel::Medium,
        EffortLevel::High,
        EffortLevel::Max,
    ];
    assert_eq!(
        clamp(EffortLevel::XHigh, &supported),
        Some(EffortLevel::High)
    );
}

#[test]
fn clamp_returns_none_when_nothing_is_supported() {
    assert_eq!(clamp(EffortLevel::Low, &[]), None);
}
