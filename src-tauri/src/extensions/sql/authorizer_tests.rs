use uuid::Uuid;

use super::*;
use crate::extensions::ids::{ExtensionName, PublicKey, TablePrefix};

fn own_table(table: &str) -> String {
    format!("{}__notes__{table}", "a".repeat(64))
}

fn authorizer(writable: &[&str]) -> SqlAuthorizer {
    authorizer_with(writable, &[])
}

fn authorizer_with(writable: &[&str], ctes: &[&str]) -> SqlAuthorizer {
    let own = TablePrefix {
        public_key: PublicKey::parse(&"a".repeat(64)).unwrap(),
        name: ExtensionName::parse("notes").unwrap(),
    };
    runtime(
        Arc::new(SqlPolicy::own_only(own, Uuid::new_v4())),
        writable.iter().map(|t| t.to_string()).collect(),
        ctes.iter().map(|t| t.to_string()).collect(),
    )
}

fn decide(
    auth: &SqlAuthorizer,
    action: AuthAction<'_>,
    db: Option<&str>,
    accessor: Option<&str>,
) -> bool {
    auth(&AuthContext {
        action,
        database_name: db,
        accessor,
    }) == Authorization::Allow
}

#[test]
fn reads_and_writes_follow_the_policy_in_main_only() {
    let pages = own_table("pages");
    let auth = authorizer(&[&pages]);
    let read = |table: &str, db| {
        decide(
            &auth,
            AuthAction::Read {
                table_name: table,
                column_name: "id",
            },
            db,
            None,
        )
    };
    assert!(read(&pages, Some("main")));
    assert!(!read(&pages, Some("temp")));
    assert!(!read("chat_threads", Some("main")));
    assert!(!read("sqlite_master", Some("main")));
    assert!(decide(
        &auth,
        AuthAction::Insert { table_name: &pages },
        Some("main"),
        None
    ));
    let drafts = own_table("drafts");
    assert!(
        !decide(
            &auth,
            AuthAction::Insert {
                table_name: &drafts
            },
            Some("main"),
            None
        ),
        "an own table this statement does not write"
    );
    assert!(!decide(
        &auth,
        AuthAction::Delete {
            table_name: "chat_threads"
        },
        Some("main"),
        None
    ));
}

#[test]
fn functions_follow_the_allowlist_and_everything_else_is_denied() {
    let auth = authorizer(&[]);
    assert!(decide(
        &auth,
        AuthAction::Function {
            function_name: "lower"
        },
        None,
        None
    ));
    assert!(!decide(
        &auth,
        AuthAction::Function {
            function_name: "load_extension"
        },
        None,
        None
    ));
    assert!(decide(&auth, AuthAction::Recursive, None, None));
    assert!(!decide(
        &auth,
        AuthAction::Pragma {
            pragma_name: "writable_schema",
            pragma_value: Some("1")
        },
        None,
        None
    ));
    assert!(!decide(
        &auth,
        AuthAction::Attach { filename: "x.db" },
        None,
        None
    ));
}

#[test]
fn a_change_trigger_may_act_only_for_a_table_the_statement_writes() {
    let pages = own_table("pages");
    let auth = authorizer(&[&pages]);
    let in_trigger = |trigger: &str| {
        decide(
            &auth,
            AuthAction::Insert {
                table_name: "haex_crdt_dirty_tables_no_sync",
            },
            Some("main"),
            Some(trigger),
        )
    };
    assert!(in_trigger(&format!("z_dirty_{pages}_insert")));
    assert!(
        in_trigger("z_dirty_haex_deleted_rows_insert"),
        "haex-crdt's nested trigger"
    );
    assert!(!in_trigger(&format!(
        "z_dirty_{}_insert",
        own_table("drafts")
    )));
    assert!(!in_trigger("some_other_trigger"));
    // Inside a real trigger nothing but its table and haex-crdt's tables.
    assert!(!decide(
        &auth,
        AuthAction::Read {
            table_name: "chat_threads",
            column_name: "id"
        },
        Some("main"),
        Some(&format!("z_dirty_{pages}_insert"))
    ));
}

#[test]
fn a_cte_is_read_without_database_and_its_body_is_judged_like_top_level_sql() {
    let pages = own_table("pages");
    let fake = format!("z_dirty_{pages}_insert");
    let auth = authorizer_with(&[&pages], &["n", &fake]);
    assert!(decide(
        &auth,
        AuthAction::Read {
            table_name: "n",
            column_name: ""
        },
        None,
        None
    ));
    assert!(decide(&auth, AuthAction::Recursive, None, Some("n")));
    assert!(!decide(
        &auth,
        AuthAction::Read {
            table_name: "chat_threads",
            column_name: "id"
        },
        Some("main"),
        Some("n")
    ));
    // A CTE named like a change trigger is still a CTE.
    assert!(!decide(
        &auth,
        AuthAction::Read {
            table_name: "chat_threads",
            column_name: "id"
        },
        Some("main"),
        Some(&fake)
    ));
    assert!(!decide(
        &auth,
        AuthAction::Read {
            table_name: "other",
            column_name: ""
        },
        None,
        None
    ));
}
