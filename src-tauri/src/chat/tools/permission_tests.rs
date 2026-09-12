use super::permission::{decide, Decision, PermissionMode};
use super::RiskClass;

#[test]
fn manual_mode_always_asks() {
    assert_eq!(decide(PermissionMode::Manual, RiskClass::Safe), Decision::Ask);
    assert_eq!(decide(PermissionMode::Manual, RiskClass::Risky), Decision::Ask);
}

#[test]
fn auto_mode_allows_safe_and_asks_risky() {
    assert_eq!(decide(PermissionMode::Auto, RiskClass::Safe), Decision::Allow);
    assert_eq!(decide(PermissionMode::Auto, RiskClass::Risky), Decision::Ask);
}

#[test]
fn plan_mode_allows_safe_and_denies_risky_without_asking() {
    assert_eq!(decide(PermissionMode::Plan, RiskClass::Safe), Decision::Allow);
    assert_eq!(decide(PermissionMode::Plan, RiskClass::Risky), Decision::Deny);
}

#[test]
fn permission_mode_parses_from_preference_string() {
    assert_eq!(PermissionMode::parse("manual"), Some(PermissionMode::Manual));
    assert_eq!(PermissionMode::parse("auto"), Some(PermissionMode::Auto));
    assert_eq!(PermissionMode::parse("plan"), Some(PermissionMode::Plan));
    assert_eq!(PermissionMode::parse("bogus"), None);
}

#[test]
fn default_permission_mode_is_manual() {
    assert_eq!(PermissionMode::default(), PermissionMode::Manual);
}
