use super::*;

fn question(ext: Uuid, target: &str) -> Question {
    Question {
        extension_id: ext,
        kind: PermissionKind::Database,
        action: "read".into(),
        target: target.into(),
    }
}

#[test]
fn identical_questions_of_several_frames_are_one_and_vanish_with_their_frames() {
    let state = PermissionState::default();
    let ext = Uuid::new_v4();
    let (id, new) = state.ask(question(ext, "t"), "f1", "t");
    assert!(new);
    let (again, new) = state.ask(question(ext, "t"), "f2", "t");
    assert_eq!((again.as_str(), new), (id.as_str(), false));
    let (other, new) = state.ask(question(ext, "u"), "f1", "u");
    assert!(new && other != id);

    assert!(state.frame_closed("f1").contains(&other));
    assert!(state.frame_closed("f2").contains(&id));
    assert!(state.take(&id).is_none());
}

#[test]
fn a_question_remembers_every_target_its_callers_were_told() {
    let state = PermissionState::default();
    let ext = Uuid::new_v4();
    let (id, _) = state.ask(question(ext, "/usr/bin/dash"), "f1", "sh");
    state.ask(question(ext, "/usr/bin/dash"), "f2", "/bin/sh");
    let asked = state.take(&id).expect("open");
    assert_eq!(asked.question.target, "/usr/bin/dash");
    assert_eq!(
        asked.told.into_iter().collect::<Vec<_>>(),
        ["/bin/sh", "sh"]
    );
}

#[test]
fn held_decisions_replace_each_other_and_can_be_forgotten() {
    let state = PermissionState::default();
    let ext = Uuid::new_v4();
    let target = format!("{}__cal__*", "b".repeat(64));
    let q = question(ext, &target);
    state.hold(&q, PermissionStatus::Denied);
    state.hold(&q, PermissionStatus::Granted);
    let held = state.temporary(ext, PermissionKind::Database);
    assert_eq!(held.len(), 1);
    assert_eq!(held[0].status, PermissionStatus::Granted);
    assert!(state
        .temporary(Uuid::new_v4(), PermissionKind::Database)
        .is_empty());
    state.forget(ext, PermissionKind::Database, "read", &target);
    assert!(state.held(ext).is_empty());
}

#[test]
fn an_unloaded_or_removed_extension_keeps_no_held_decision() {
    let state = PermissionState::default();
    let (ext, other) = (Uuid::new_v4(), Uuid::new_v4());
    state.hold(&question(ext, "t"), PermissionStatus::Granted);
    state.hold(&question(other, "t"), PermissionStatus::Granted);
    state.forget_extension(ext);
    assert!(state.temporary(ext, PermissionKind::Database).is_empty());
    assert_eq!(state.held(other).len(), 1);
}
