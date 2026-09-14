use super::session::{ChatState, ModelLoadStatus};

#[test]
fn load_ids_are_monotonic_within_a_vault_generation() {
    let chat = ChatState::new();
    let generation = chat.vault_generation();
    let (first_generation, first_load) = chat.begin_model_load();
    let (second_generation, second_load) = chat.begin_model_load();

    assert_eq!(first_generation, generation);
    assert_eq!(second_generation, generation);
    assert!(second_load > first_load);
}

#[test]
fn vault_generation_invalidates_old_status_updates() {
    let chat = ChatState::new();
    let (old_generation, old_load) = chat.begin_model_load();
    assert!(chat.set_model_loading(
        old_generation,
        old_load,
        "old-model".into(),
        "Old model".into(),
        "loading".into(),
        None,
    ));

    let new_generation = chat.bump_vault_generation();
    let (current_generation, current_load) = chat.begin_model_load();

    assert!(new_generation > old_generation);
    assert_eq!(current_generation, new_generation);
    assert!(!chat.set_model_ready(
        old_generation,
        old_load,
        "old-model".into(),
        "Old model".into(),
    ));
    assert!(chat.set_model_loading(
        current_generation,
        current_load,
        "new-model".into(),
        "New model".into(),
        "loading".into(),
        None,
    ));
    assert!(matches!(
        chat.model_load_status(),
        ModelLoadStatus::Loading {
            vault_generation,
            load_id,
            ..
        } if vault_generation == new_generation && load_id == current_load
    ));
}

#[test]
fn stale_load_id_cannot_replace_newer_load_in_same_vault() {
    let chat = ChatState::new();
    let (generation, old_load) = chat.begin_model_load();
    let (_, new_load) = chat.begin_model_load();

    assert!(!chat.set_model_ready(generation, old_load, "old-model".into(), "Old model".into(),));
    assert!(chat.set_model_ready(generation, new_load, "new-model".into(), "New model".into(),));
}
