use super::interrupt::{match_interrupt, InterruptCommand};

#[test]
fn matches_exact_words() {
    assert_eq!(match_interrupt("stop"), Some(InterruptCommand::Stop));
    assert_eq!(match_interrupt("halt"), Some(InterruptCommand::Halt));
    assert_eq!(
        match_interrupt("abbrechen"),
        Some(InterruptCommand::Abbrechen)
    );
}

#[test]
fn matches_case_variants() {
    assert_eq!(match_interrupt("STOP"), Some(InterruptCommand::Stop));
    assert_eq!(match_interrupt("Halt"), Some(InterruptCommand::Halt));
    assert_eq!(
        match_interrupt("AbBrEcHeN"),
        Some(InterruptCommand::Abbrechen)
    );
}

#[test]
fn matches_with_surrounding_whitespace() {
    assert_eq!(match_interrupt("  stop  "), Some(InterruptCommand::Stop));
    assert_eq!(match_interrupt("\nhalt\t"), Some(InterruptCommand::Halt));
}

#[test]
fn does_not_match_word_inside_a_longer_sentence() {
    assert_eq!(
        match_interrupt("bitte nicht mehr stoppen mitten im Satz"),
        None
    );
    assert_eq!(match_interrupt("please stop now"), None);
    assert_eq!(match_interrupt("halt on a second"), None);
    assert_eq!(match_interrupt("abbrechen bitte"), None);
}

#[test]
fn does_not_match_empty_or_unrelated_text() {
    assert_eq!(match_interrupt(""), None);
    assert_eq!(match_interrupt("   "), None);
    assert_eq!(match_interrupt("hello world"), None);
}
