use std::io;

use haex_crdt::db::error::DatabaseError;
use haex_crdt::Error as CrdtError;

use super::HolziError;

#[test]
fn held_vault_lock_maps_to_vault_already_open_elsewhere() {
    let err = CrdtError::Database(DatabaseError::VaultAlreadyOpenElsewhere {
        path: "/vault.db".to_string(),
        source: io::Error::new(io::ErrorKind::WouldBlock, "locked"),
    });

    assert!(matches!(
        HolziError::from(err),
        HolziError::VaultAlreadyOpenElsewhere
    ));
}

#[test]
fn oversized_transaction_keeps_its_sizes() {
    let err = CrdtError::Database(DatabaseError::TransactionTooLarge {
        bytes: 11,
        limit: 10,
    });

    assert!(matches!(
        HolziError::from(err),
        HolziError::TransactionTooLarge {
            bytes: 11,
            limit: 10
        }
    ));
}

#[test]
fn holzi_error_survives_the_consumer_round_trip() {
    let err = CrdtError::from(HolziError::NotFound {
        name: "work".to_string(),
    });

    assert!(matches!(
        HolziError::from(err),
        HolziError::NotFound { name } if name == "work"
    ));
}

#[test]
fn foreign_consumer_error_falls_back_to_crdt_init_with_its_text() {
    let err = CrdtError::consumer(io::Error::other("hook failed"));

    assert!(matches!(
        HolziError::from(err),
        HolziError::CrdtInit { reason } if reason == "hook failed"
    ));
}
