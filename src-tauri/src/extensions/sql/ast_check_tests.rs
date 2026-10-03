use std::collections::HashSet;

use super::*;
use crate::extensions::ids::{ExtensionName, PublicKey};
use crate::extensions::sql::parse::parse_one;

fn own() -> TablePrefix {
    TablePrefix {
        public_key: PublicKey::parse(&"a".repeat(64)).unwrap(),
        name: ExtensionName::parse("notes").unwrap(),
    }
}

/// `t:` stands for the own prefix, `f:` for another extension's.
fn sql(template: &str) -> String {
    template
        .replace("t:", &format!("{}__notes__", "a".repeat(64)))
        .replace("f:", &format!("{}__cal__", "b".repeat(64)))
}

fn existing() -> HashSet<String> {
    ["chat_threads", "sqlite_master"]
        .into_iter()
        .map(str::to_owned)
        .chain([sql("t:pages")])
        .collect()
}

fn run(template: &str) -> Result<RequiredAccess, BridgeError> {
    let text = sql(template);
    let statement = parse_one(&text)?;
    check(&statement, &text, &own(), &existing())
}

fn names(tables: &[ExtensionTable]) -> Vec<String> {
    tables.iter().map(|t| t.table.clone()).collect()
}

#[test]
fn own_reads_and_writes_are_listed_by_what_the_statement_does() {
    let access =
        run("INSERT INTO t:pages (id, body) SELECT id, body FROM t:drafts RETURNING id").unwrap();
    assert_eq!(names(&access.writes), ["pages"]);
    assert_eq!(names(&access.reads), ["drafts"]);

    let access =
        run("UPDATE main.t:pages SET body = (SELECT x FROM f:events) WHERE id = ?").unwrap();
    assert_eq!(names(&access.writes), ["pages"]);
    assert_eq!(names(&access.reads), ["events"]);

    let access = run("DELETE FROM t:pages WHERE id IN (SELECT id FROM t:trash)").unwrap();
    assert_eq!(names(&access.writes), ["pages"]);
    assert_eq!(names(&access.reads), ["trash"]);
}

#[test]
fn allowed_forms_pass() {
    for ok in [
        "SELECT * FROM t:pages",
        "SELECT lower(title), count(*) FROM t:pages GROUP BY 1 ORDER BY 2 DESC LIMIT 5",
        "INSERT INTO t:pages (id) VALUES (?) ON CONFLICT (id) DO UPDATE SET id = excluded.id",
        "WITH RECURSIVE n(x) AS (SELECT 1 UNION ALL SELECT x + 1 FROM n WHERE x < 5) SELECT x FROM n",
        "WITH recent AS (SELECT * FROM t:pages) SELECT * FROM recent",
        "SELECT value FROM json_each('[1,2,3]')",
        "SELECT body ->> '$.title' FROM t:pages",
        "SELECT 'chat_threads', 'haex_hlc_no_sync' FROM t:pages",
    ] {
        assert!(run(ok).is_ok(), "{ok}: {:?}", run(ok));
    }
}

#[test]
fn core_tables_are_refused_wherever_they_hide() {
    for bad in [
        "SELECT * FROM chat_threads",
        "WITH x AS (SELECT * FROM chat_threads) SELECT * FROM x",
        "WITH t:pages AS (SELECT * FROM chat_threads) SELECT * FROM t:pages",
        "WITH chat_threads AS (SELECT 1) SELECT * FROM chat_threads",
        "SELECT * FROM t:pages WHERE EXISTS (SELECT 1 FROM chat_threads)",
        "SELECT (SELECT count(*) FROM chat_threads) FROM t:pages",
        "SELECT * FROM t:pages GROUP BY id HAVING count(*) > (SELECT 1 FROM chat_threads)",
        "SELECT * FROM t:pages ORDER BY (SELECT id FROM chat_threads)",
        "SELECT * FROM t:pages LIMIT (SELECT count(*) FROM chat_threads)",
        "INSERT INTO t:pages (id) VALUES ((SELECT id FROM chat_threads))",
        "INSERT INTO t:pages (id) VALUES (?) RETURNING (SELECT id FROM chat_threads)",
        "INSERT INTO t:pages (id) VALUES (?) ON CONFLICT (id) DO UPDATE SET id = (SELECT id FROM chat_threads)",
        "SELECT * FROM t:pages p JOIN t:drafts d ON (SELECT 1 FROM chat_threads)",
        "SELECT CASE WHEN 1 THEN (SELECT id FROM chat_threads) END FROM t:pages",
        "SELECT lower((SELECT id FROM chat_threads)) FROM t:pages",
        "SELECT * FROM main.chat_threads",
        "SELECT * FROM temp.t:pages",
        "SELECT * FROM \"Chat_Threads\"",
        "SELECT * FROM [chat_threads]",
        "SELECT * FROM `chat_threads`",
        "SELECT * FROM sqlite_master",
        "SELECT * FROM sqlite_schema",
        "SELECT * FROM pragma_table_info('chat_threads')",
        "SELECT * FROM json_each((SELECT id FROM chat_threads))",
        "UPDATE chat_threads SET title = 'x'",
        "DELETE FROM chat_threads",
    ] {
        assert!(run(bad).is_err(), "{bad} was accepted");
    }
}

#[test]
fn forbidden_kinds_functions_and_names_are_refused() {
    for bad in [
        "SELECT load_extension('x')",
        "SELECT sqlcipher_export('x')",
        "SELECT fts3_tokenizer('x')",
        "SELECT sqlite_version()",
        "SELECT main.lower('x')",
        "BEGIN",
        "COMMIT",
        "SAVEPOINT a",
        "ATTACH 'x.db' AS x",
        "PRAGMA writable_schema = 1",
        "CREATE TABLE t:x (id TEXT)",
        "DROP TABLE t:pages",
        "WITH x AS (SELECT 1) INSERT INTO t:pages (id) SELECT * FROM x",
        "SELECT haex_hlc_no_sync FROM t:pages",
        "UPDATE t:pages SET haex_hlc_no_sync = '0'",
        "INSERT INTO t:pages (id, \"HAEX_HLC_NO_SYNC\") VALUES (1, 2)",
        "SELECT * FROM t:pages; SELECT * FROM chat_threads",
        "SELECT * FROM t:pages /* ; */ ; DELETE FROM t:pages",
        "SELECT * FROM \"t:pagesſ\"",
        // `a` against `a__b`: four parts are no extension table.
        "SELECT * FROM t:x__pages",
    ] {
        assert!(run(bad).is_err(), "{bad} was accepted");
    }
}

#[test]
fn a_well_formed_prefix_of_another_extension_is_foreign_not_refused() {
    let access = run("SELECT * FROM f:events").unwrap();
    assert_eq!(names(&access.reads), ["events"]);
    assert_ne!(access.reads[0].prefix, own());
}
