use super::*;
use crate::extensions::ids::{ExtensionName, PublicKey};
use crate::extensions::permissions::{GrantScope, PermissionStatus, Target};

fn prefix(key: char, name: &str) -> TablePrefix {
    TablePrefix {
        public_key: PublicKey::parse(&key.to_string().repeat(64)).unwrap(),
        name: ExtensionName::parse(name).unwrap(),
    }
}

fn table(key: char, name: &str, table: &str) -> String {
    format!("{}__{name}__{table}", key.to_string().repeat(64))
}

#[test]
fn own_tables_always_core_never_and_others_by_their_permission() {
    let device = Uuid::new_v4();
    let calendar = prefix('b', "cal");
    let mut policy = SqlPolicy::own_only(prefix('a', "notes"), device);
    assert!(policy.allows(&table('a', "notes", "pages"), true));
    assert!(!policy.allows("chat_threads", false));
    assert!(!policy.allows(&table('b', "cal", "events"), false));

    policy.grants.push(Permission {
        kind: PermissionKind::Database,
        action: Action::Read,
        target: Target::ExtensionTables(calendar.clone()),
        status: PermissionStatus::Granted,
        scope: GrantScope::Vault,
    });
    assert!(
        !policy.allows(&table('b', "cal", "events"), false),
        "not installed: no tables, whatever is granted"
    );
    policy.installed.insert(calendar);
    assert!(policy.allows(&table('b', "cal", "events"), false));
    assert!(
        !policy.allows(&table('b', "cal", "events"), true),
        "read is not readWrite"
    );
    assert!(
        !policy.allows(&table('c', "cal", "events"), false),
        "another publisher"
    );
}
