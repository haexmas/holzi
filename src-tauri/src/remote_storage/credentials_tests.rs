//! The credentials entry of a connection (data-model.md, T023, T026): created owned by `storage`,
//! read back as the connection's credentials, its state `present`/`missing`/`syncing`, and the
//! warning of the password manager before the user deletes it.

use std::sync::Arc;

use zeroize::Zeroizing;

use super::credentials::{self, StorageUsage, FEATURE};
use super::test_support::{credentials as placeholder, vault};
use super::{store, Addressing, ConnectionRow, CredentialsState, EndpointOrigin, ProviderKind};
use crate::error::HolziError;
use crate::passwords::access::Caller;
use crate::passwords::model::{Target, TargetKind};
use crate::passwords::service::PasswordsService;
use crate::passwords::usage::UsageRegistry;

#[tokio::test]
async fn new_credentials_become_an_owned_entry_that_reads_back() {
    let (_dir, db) = vault();
    let passwords = PasswordsService::new(db.clone());
    let mut typed = placeholder();
    typed.session_token = Some(Zeroizing::new("token-placeholder".to_owned()));
    let id = credentials::create(&passwords, " RustFS ", "http://127.0.0.1:9000", &typed)
        .await
        .expect("create");

    let header = passwords
        .get_item(&Caller::User, id.clone())
        .await
        .expect("the user sees it")
        .header;
    assert_eq!(header.title.as_deref(), Some("S3: RustFS"));
    assert_eq!(header.owner.as_deref(), Some(FEATURE));

    let read = credentials::read(&passwords, &db, &id).await.expect("read");
    assert_eq!(read.access_key_id, typed.access_key_id);
    assert_eq!(
        read.secret_access_key.as_str(),
        typed.secret_access_key.as_str()
    );
    assert_eq!(
        read.session_token.as_deref().map(String::as_str),
        Some("token-placeholder")
    );
    assert!(!format!("{read:?}").contains("placeholder-secret"));
    assert_eq!(
        credentials::state(&db, &id).await.expect("state"),
        CredentialsState::Present
    );
}

#[tokio::test]
async fn an_entry_the_user_deleted_is_missing_and_one_not_yet_arrived_is_syncing() {
    let (_dir, db) = vault();
    let passwords = PasswordsService::new(db.clone());
    let unavailable = |result| match result {
        Err(HolziError::StorageCredentialsUnavailable { state }) => state,
        other => panic!("unexpected {other:?}"),
    };

    assert_eq!(
        unavailable(credentials::read(&passwords, &db, "not-arrived").await),
        CredentialsState::Syncing
    );

    let trashed = credentials::create(&passwords, "A", "", &placeholder())
        .await
        .expect("create");
    passwords
        .trash_targets(
            &Caller::User,
            vec![Target {
                kind: TargetKind::Item,
                id: trashed.clone(),
            }],
        )
        .await
        .expect("the user moves it to the trash");
    assert_eq!(
        unavailable(credentials::read(&passwords, &db, &trashed).await),
        CredentialsState::Missing
    );

    let purged = credentials::create(&passwords, "B", "", &placeholder())
        .await
        .expect("create");
    credentials::delete(&passwords, &purged)
        .await
        .expect("delete");
    assert_eq!(
        unavailable(credentials::read(&passwords, &db, &purged).await),
        CredentialsState::Missing,
        "the delete marker says it was deleted, not that it is on its way"
    );
    credentials::delete(&passwords, &purged)
        .await
        .expect("deleting it again is fine");
}

#[tokio::test]
async fn deleting_the_credentials_of_a_connection_warns_in_the_password_manager() {
    let (_dir, db) = vault();
    let usage = Arc::new(UsageRegistry::new());
    let handle = db.clone();
    usage.register(Arc::new(StorageUsage::new(move || Some(handle.clone()))));
    let passwords = PasswordsService::with_usage(db.clone(), usage);
    let id = credentials::create(&passwords, "RustFS", "", &placeholder())
        .await
        .expect("create");
    assert!(passwords
        .item_usage(&Caller::User, &id)
        .expect("usage")
        .is_empty());

    let row = ConnectionRow {
        id: "c1".to_owned(),
        provider_name: "RustFS".to_owned(),
        provider_kind: ProviderKind::Rustfs,
        endpoint: "http://127.0.0.1:9000".to_owned(),
        endpoint_origin: EndpointOrigin::User,
        region: "us-east-1".to_owned(),
        addressing: Addressing::Path,
        credentials_item_id: id.clone(),
        created_at: "t".to_owned(),
        updated_at: "t".to_owned(),
    };
    db.write(move |tx| Ok(store::put_connection(tx, &row)?))
        .await
        .expect("connection");
    let check = id.clone();
    let features = tokio::task::spawn_blocking(move || {
        passwords.item_usage(&Caller::User, &check).expect("usage")
    })
    .await
    .expect("join");
    assert_eq!(features, [FEATURE]);
}
