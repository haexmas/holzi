use super::*;
use crate::extensions::ids::{ExtensionName, PublicKey};

fn prefix(key: char, name: &str) -> TablePrefix {
    TablePrefix {
        public_key: PublicKey::parse(&key.to_string().repeat(64)).unwrap(),
        name: ExtensionName::parse(name).unwrap(),
    }
}

#[test]
fn tables_are_own_foreign_or_core_by_their_exact_prefix() {
    let own = prefix('a', "notes");
    let a = "a".repeat(64);
    let b = "b".repeat(64);
    assert!(matches!(
        classify(&format!("{a}__notes__pages"), &own),
        TableClass::Own(_)
    ));
    assert!(matches!(
        classify(&format!("{}__NOTES__Pages", a.to_uppercase()), &own),
        TableClass::Own(_)
    ));
    assert!(matches!(
        classify(&format!("{b}__notes__pages"), &own),
        TableClass::Foreign(_)
    ));
    assert!(matches!(
        classify(&format!("{a}__notes2__pages"), &own),
        TableClass::Foreign(_)
    ));
    // `a` versus `a__b`: four parts are no extension table.
    assert_eq!(
        classify(&format!("{a}__notes__x__pages"), &own),
        TableClass::Core
    );
    for core in [
        "chat_threads",
        "sqlite_master",
        "haex_crdt_configs_no_sync",
        "",
    ] {
        assert_eq!(classify(core, &own), TableClass::Core, "{core}");
    }
}

#[test]
fn only_listed_functions_are_allowed() {
    for ok in ["lower", "COUNT", "json_each", "->>", "strftime"] {
        assert!(function_allowed(ok), "{ok}");
    }
    for no in [
        "load_extension",
        "fts3_tokenizer",
        "sqlite_version",
        "sqlcipher_export",
        "pragma_table_info",
        "readfile",
        "haex_hlc",
    ] {
        assert!(!function_allowed(no), "{no}");
    }
}
