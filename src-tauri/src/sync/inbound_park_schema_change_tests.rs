//! A column that a later migration drops or renames (spec 017, FR-037, research R10): a device
//! still on the older version writes it before it updates. The group waits until that device
//! updated and comes again then, shaped by its own migration.

use haex_crdt::rusqlite::params;

use super::super::*;
use super::{count, ddl, parked, threads, write, write_thread, PAGES};
use crate::extensions::registry::status::DeviceStatus;
use crate::extensions::sql::test_support::own;
use crate::sync::test_support::Device;

const EXT: &str = "5d0c9e8e-3a51-4c43-9a8f-1f0f6f0a2b77";

/// Registers the own extension on `device` and marks it `status` there, as starting it does.
fn mark(device: &Device, status: DeviceStatus) {
    let prefix = own();
    let row = crate::extensions::ids::device_status_id(
        uuid::Uuid::parse_str(EXT).unwrap(),
        device.db().device_id(),
    )
    .to_string();
    device
        .db()
        .write(|tx| {
            tx.execute(
                "INSERT INTO extensions (id, public_key, name, installed_at, updated_at) \
                 VALUES (?1, ?2, ?3, 1, 1)",
                params![EXT, prefix.public_key.as_str(), prefix.name.as_str()],
            )?;
            tx.execute(
                "INSERT INTO extension_device_status \
                 (id, extension_id, vault_device_uuid, status, updated_at) \
                 VALUES (?1, ?2, ?3, ?4, 1)",
                params![
                    row,
                    EXT,
                    device.db().device_id().to_string(),
                    status.as_str()
                ],
            )
            .map(drop)
        })
        .expect("status");
}

/// A and B on the version with `tag`; B updates with `migration` and pulls a write of `tag` from
/// A, which still has the older version, and some core data A wrote after it. Returns both.
fn old_write_pulled(migration: &str) -> (Device, Device) {
    let (a, b) = (Device::new(), Device::new());
    for device in [&a, &b] {
        ddl(device, PAGES);
        ddl(device, "ALTER TABLE t:pages ADD COLUMN tag TEXT");
    }
    mark(&b, DeviceStatus::Ready);
    ddl(&b, migration);
    write(
        &a,
        "INSERT INTO t:pages (id, body, tag) VALUES ('p1', 'one', 'red')",
    );
    write_thread(&a, "core");

    b.pull_from(&a);
    assert_eq!(
        parked(&b),
        vec![],
        "nothing parked for a column that is gone"
    );
    assert_eq!(count(&b, "SELECT COUNT(*) FROM t:pages"), 0);
    assert_eq!(threads(&b), 1, "other data of that device still arrives");

    // Until A updated, it comes again and waits again.
    b.pull_from(&a);
    assert_eq!(parked(&b), vec![]);
    assert_eq!(count(&b, "SELECT COUNT(*) FROM t:pages"), 0);

    ddl(&a, migration);
    write(&a, "INSERT INTO t:pages (id, body) VALUES ('p2', 'two')");
    b.pull_from(&a);
    assert_eq!(parked(&b), vec![]);
    assert_eq!(count(&b, "SELECT COUNT(*) FROM t:pages"), 2);
    (a, b)
}

#[test]
fn a_write_to_a_dropped_column_waits_until_its_device_updated() {
    old_write_pulled("ALTER TABLE t:pages DROP COLUMN tag");
}

#[test]
fn a_write_to_a_renamed_column_arrives_renamed_once_its_device_updated() {
    let (_, b) = old_write_pulled("ALTER TABLE t:pages RENAME COLUMN tag TO label");
    assert_eq!(
        count(&b, "SELECT COUNT(*) FROM t:pages WHERE label = 'red'"),
        1
    );
}

#[test]
fn a_dropped_table_waits_on_a_device_where_the_extension_is_disabled() {
    let (a, b) = (Device::new(), Device::new());
    const NOTES: &str = "CREATE TABLE t:notes (id TEXT PRIMARY KEY, body TEXT)";
    for device in [&a, &b] {
        ddl(device, PAGES);
        ddl(device, NOTES);
    }
    mark(&b, DeviceStatus::Disabled);
    ddl(&b, "DROP TABLE t:notes");
    write(&a, "INSERT INTO t:notes (id, body) VALUES ('n1', 'one')");

    b.pull_from(&a);
    assert_eq!(parked(&b), vec![]);

    ddl(&a, "DROP TABLE t:notes");
    write(&a, "INSERT INTO t:pages (id, body) VALUES ('p1', 'one')");
    b.pull_from(&a);
    assert_eq!(parked(&b), vec![]);
    assert_eq!(count(&b, "SELECT COUNT(*) FROM t:pages"), 1);
}

#[test]
fn a_column_of_a_newer_version_waits_until_this_device_has_it() {
    let (a, b) = (Device::new(), Device::new());
    ddl(&a, PAGES);
    ddl(&b, PAGES);
    mark(&b, DeviceStatus::Ready);
    // A updated; B still starts its older version, the newer bundle has not reached it.
    ddl(&a, "ALTER TABLE t:pages ADD COLUMN tag TEXT");
    write(
        &a,
        "INSERT INTO t:pages (id, body, tag) VALUES ('p1', 'one', 'red')",
    );

    b.pull_from(&a);
    assert_eq!(parked(&b), vec![]);
    assert_eq!(count(&b, "SELECT COUNT(*) FROM t:pages"), 0);

    ddl(&b, "ALTER TABLE t:pages ADD COLUMN tag TEXT");
    b.pull_from(&a);
    assert_eq!(
        count(&b, "SELECT COUNT(*) FROM t:pages WHERE tag = 'red'"),
        1
    );
}

#[test]
fn a_device_not_ready_for_the_extension_still_parks() {
    let (a, b) = (Device::new(), Device::new());
    ddl(&a, PAGES);
    ddl(&b, PAGES);
    mark(&b, DeviceStatus::Transferring);
    ddl(&a, "ALTER TABLE t:pages ADD COLUMN tag TEXT");
    write(
        &a,
        "INSERT INTO t:pages (id, body, tag) VALUES ('p1', 'one', 'red')",
    );

    b.pull_from(&a);
    assert_eq!(
        parked(&b),
        vec![(own().to_string(), MISSING_COLUMN.to_owned())]
    );
}

#[test]
fn a_column_whose_migration_arrives_in_the_same_pull_is_parked() {
    let (a, b) = (Device::new(), Device::new());
    ddl(&a, PAGES);
    ddl(&b, PAGES);
    mark(&b, DeviceStatus::Ready);
    // A installs a newer version with a migration that adds the column, then writes it.
    mark(&a, DeviceStatus::Ready);
    write(
        &a,
        &format!(
            "INSERT INTO extension_migrations (id, extension_id, name, position, sql, sql_sha256) \
             VALUES ('m1', '{EXT}', '0001_tag', 1, 'ALTER TABLE x ADD COLUMN tag TEXT', 'x')"
        ),
    );
    ddl(&a, "ALTER TABLE t:pages ADD COLUMN tag TEXT");
    write(
        &a,
        "INSERT INTO t:pages (id, body, tag) VALUES ('p1', 'one', 'red')",
    );

    b.pull_from(&a);
    assert_eq!(
        parked(&b),
        vec![(own().to_string(), MISSING_COLUMN.to_owned())],
        "this device is behind: the group is replayed after its migration"
    );
}
