//! Tests for the extras of an import (spec 037, R4): a tag keeps its own colour, a passkey of no
//! entry arrives once, presets are not doubled and never take the default from the user.

#![allow(clippy::disallowed_methods)]

use std::sync::Arc;

use zeroize::Zeroizing;

use super::*;
use crate::passwords::test_support::open_test_vault;
use crate::vault_gate::{VaultDb, VaultGate};

fn vault() -> (tempfile::TempDir, haex_crdt::Database, VaultDb) {
    let (dir, db) = open_test_vault();
    let vault_db = VaultGate::new()
        .vault_db(Arc::new(db.clone()))
        .expect("open the gate");
    (dir, db, vault_db)
}

fn passkey(credential_id: &str) -> PasskeyInput {
    PasskeyInput {
        item_id: Some("ignored".into()),
        credential_id: credential_id.into(),
        relying_party_id: "example.invalid".into(),
        relying_party_name: None,
        user_name: None,
        user_display_name: None,
        user_handle: "dXNlcg==".into(),
        private_key: Zeroizing::new("cHJpdmF0ZQ==".into()),
        public_key: "cHVibGlj".into(),
        algorithm: -7,
        sign_count: 3,
        is_discoverable: false,
        icon: None,
        color: None,
        nickname: Some("Key".into()),
        created_at: None,
        last_used_at: None,
    }
}

fn preset(name: &str, is_default: bool) -> PresetInput {
    PresetInput {
        name: name.into(),
        length: 20,
        is_default,
        ..PresetInput::default()
    }
}

async fn write_extras(db: &VaultDb, extras: Extras) -> ExtrasOutcome {
    db.write(move |tx| write(tx, &extras).map_err(Into::into))
        .await
        .expect("write")
}

fn count(db: &haex_crdt::Database, sql: &str) -> i64 {
    db.with_connection(|c| Ok(c.query_row(sql, [], |r| r.get(0))?))
        .expect("count")
}

#[tokio::test]
async fn a_tag_takes_the_colour_of_the_source_only_without_one_of_its_own() {
    let (_dir, db, vault_db) = vault();
    vault_db
        .write(|tx| {
            tags::get_or_create(tx, "Work")?;
            let own = tags::get_or_create(tx, "Home")?;
            tags::set_color(tx, &own, Some("#111111"))?;
            Ok(())
        })
        .await
        .expect("tags");
    write_extras(
        &vault_db,
        Extras {
            tag_colors: vec![
                ("work".into(), "#222222".into()),
                ("Home".into(), "#333333".into()),
                ("Unused".into(), "#444444".into()),
            ],
            passkeys: Vec::new(),
            presets: Vec::new(),
        },
    )
    .await;
    assert_eq!(
        count(
            &db,
            "SELECT COUNT(*) FROM haex_passwords_tags WHERE name = 'Work' AND color = '#222222'"
        ),
        1
    );
    assert_eq!(
        count(
            &db,
            "SELECT COUNT(*) FROM haex_passwords_tags WHERE name = 'Home' AND color = '#111111'"
        ),
        1,
        "the user's colour stays"
    );
    assert_eq!(
        count(&db, "SELECT COUNT(*) FROM haex_passwords_tags"),
        2,
        "no tag is created for a colour"
    );
}

#[tokio::test]
async fn a_passkey_of_no_entry_arrives_once_and_a_second_is_reported() {
    let (_dir, db, vault_db) = vault();
    let extras = || Extras {
        tag_colors: Vec::new(),
        passkeys: vec![passkey("Y3JlZA==")],
        presets: Vec::new(),
    };
    let first = write_extras(&vault_db, extras()).await;
    assert_eq!(first.passkeys.len(), 1);
    assert!(first.problems.is_empty());
    assert_eq!(
        count(
            &db,
            "SELECT COUNT(*) FROM haex_passwords_passkeys \
             WHERE item_id IS NULL AND public_key = 'cHVibGlj' AND sign_count = 3"
        ),
        1
    );
    let second = write_extras(&vault_db, extras()).await;
    assert!(second.passkeys.is_empty());
    assert_eq!(second.problems[0].kind, AttentionKind::PasskeyDuplicate);
}

#[tokio::test]
async fn presets_are_not_doubled_and_do_not_take_the_users_default() {
    let (_dir, db, vault_db) = vault();
    vault_db
        .write(|tx| {
            presets::save(tx, &preset("Mine", true))
                .map(|_| ())
                .map_err(Into::into)
        })
        .await
        .expect("own preset");
    let outcome = write_extras(
        &vault_db,
        Extras {
            tag_colors: Vec::new(),
            passkeys: Vec::new(),
            presets: vec![
                preset("mine ", false),
                preset("Strong", true),
                preset(" ", false),
            ],
        },
    )
    .await;
    assert_eq!(outcome.presets.len(), 1, "only Strong is new");
    assert_eq!(outcome.problems[0].kind, AttentionKind::ValueNotStorable);
    assert_eq!(
        count(
            &db,
            "SELECT COUNT(*) FROM haex_passwords_generator_presets WHERE is_default = 1 AND name = 'Mine'"
        ),
        1
    );
    assert_eq!(
        count(
            &db,
            "SELECT COUNT(*) FROM haex_passwords_generator_presets WHERE name = 'Strong' AND is_default = 0"
        ),
        1
    );
}

#[tokio::test]
async fn an_imported_default_becomes_the_default_when_the_vault_has_none() {
    let (_dir, db, vault_db) = vault();
    write_extras(
        &vault_db,
        Extras {
            tag_colors: Vec::new(),
            passkeys: Vec::new(),
            presets: vec![preset("Strong", true)],
        },
    )
    .await;
    assert_eq!(
        count(
            &db,
            "SELECT COUNT(*) FROM haex_passwords_generator_presets WHERE name = 'Strong' AND is_default = 1"
        ),
        1
    );
}

#[tokio::test]
async fn undo_removes_what_the_run_created() {
    let (_dir, db, vault_db) = vault();
    let outcome = write_extras(
        &vault_db,
        Extras {
            tag_colors: Vec::new(),
            passkeys: vec![passkey("dW5kbw==")],
            presets: vec![preset("Strong", false)],
        },
    )
    .await;
    vault_db
        .write(move |tx| {
            undo(tx, &outcome.tag_colors, &outcome.passkeys, &outcome.presets).map_err(Into::into)
        })
        .await
        .expect("undo");
    assert_eq!(
        count(&db, "SELECT COUNT(*) FROM haex_passwords_passkeys"),
        0
    );
    assert_eq!(
        count(&db, "SELECT COUNT(*) FROM haex_passwords_generator_presets"),
        0
    );
}

#[tokio::test]
async fn undo_restores_imported_tag_colours() {
    let (_dir, db, vault_db) = vault();
    let tag_id = vault_db
        .write(|tx| tags::get_or_create(tx, "Work").map_err(Into::into))
        .await
        .expect("tag");
    let outcome = write_extras(
        &vault_db,
        Extras {
            tag_colors: vec![("Work".into(), "#222222".into())],
            passkeys: Vec::new(),
            presets: Vec::new(),
        },
    )
    .await;
    assert_eq!(outcome.tag_colors, vec![(tag_id.clone(), None)]);
    vault_db
        .write(move |tx| {
            undo(tx, &outcome.tag_colors, &outcome.passkeys, &outcome.presets).map_err(Into::into)
        })
        .await
        .expect("undo");
    let sql =
        format!("SELECT COUNT(*) FROM haex_passwords_tags WHERE id = '{tag_id}' AND color IS NULL");
    assert_eq!(count(&db, &sql), 1);
}
