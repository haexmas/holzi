//! A KeePass database and passkey material built at run time (spec 034, T079). Nothing here is
//! stored in the repository: the key pairs are made by the test (constitution I), the database is
//! written with the `keepass` crate (a dev-dependency) with a password **and** a key file.
//! Included per binary via `#[path = "common/kdbx_fixture.rs"] mod kdbx_fixture;`.
#![allow(dead_code)]

use base64::engine::general_purpose::{STANDARD, URL_SAFE_NO_PAD};
use base64::Engine;
use keepass::db::{
    fields, AutoType, AutoTypeAssociation, Color, CustomDataItem, CustomDataValue,
    DataTransferObfuscation, Times, Value,
};
use keepass::{Database, DatabaseKey};
use p256::pkcs8::{EncodePrivateKey as _, EncodePublicKey as _};

pub const PASSWORD: &str = "kdbx-fixture-password";
pub const KEY_FILE: &[u8] = b"key file contents of the fixture";
/// Planted in the secrets of the fixture; no report, error or text may carry it.
pub const MARKER: &str = "SECRET-MARKER-IMPORT";

/// A passkey made for the test: the private key as the sources deliver it and the public key the
/// importer has to derive.
pub struct TestPasskey {
    pub algorithm: i64,
    /// Standard Base64 of the PKCS8 DER.
    pub private_b64: String,
    pub pem: String,
    /// Standard Base64 of the SPKI DER.
    pub public_b64: String,
}

fn pem_of(der: &[u8]) -> String {
    let body = STANDARD.encode(der);
    let lines: Vec<&str> = body
        .as_bytes()
        .chunks(64)
        .map(|c| std::str::from_utf8(c).expect("ascii"))
        .collect();
    format!(
        "-----BEGIN PRIVATE KEY-----\n{}\n-----END PRIVATE KEY-----\n",
        lines.join("\n")
    )
}

fn seed() -> [u8; 32] {
    let mut seed = [0u8; 32];
    getrandom::fill(&mut seed).expect("random");
    seed
}

/// A key pair of the algorithm (-7 ES256, -8 EdDSA, -257 RS256) generated now.
pub fn passkey(algorithm: i64) -> TestPasskey {
    let (private, public): (Vec<u8>, Vec<u8>) = match algorithm {
        -7 => {
            let secret = p256::SecretKey::from_slice(&seed()).expect("key");
            (
                secret.to_pkcs8_der().expect("pkcs8").as_bytes().to_vec(),
                secret
                    .public_key()
                    .to_public_key_der()
                    .expect("spki")
                    .as_bytes()
                    .to_vec(),
            )
        }
        -8 => {
            let signing = ed25519_dalek::SigningKey::from_bytes(&seed());
            (
                signing.to_pkcs8_der().expect("pkcs8").as_bytes().to_vec(),
                signing
                    .verifying_key()
                    .to_public_key_der()
                    .expect("spki")
                    .as_bytes()
                    .to_vec(),
            )
        }
        -257 => {
            use rsa::pkcs8::{EncodePrivateKey as _, EncodePublicKey as _};
            let mut rng = getrandom::rand_core::UnwrapErr(getrandom::SysRng);
            let key = rsa::RsaPrivateKey::new(&mut rng, 2048).expect("rsa key");
            (
                key.to_pkcs8_der().expect("pkcs8").as_bytes().to_vec(),
                rsa::RsaPublicKey::from(&key)
                    .to_public_key_der()
                    .expect("spki")
                    .as_bytes()
                    .to_vec(),
            )
        }
        other => panic!("no test key for {other}"),
    };
    TestPasskey {
        algorithm,
        private_b64: STANDARD.encode(&private),
        pem: pem_of(&private),
        public_b64: STANDARD.encode(public),
    }
}

/// What `build` wrote, so a test can compare.
pub struct Fixture {
    pub bytes: Vec<u8>,
    pub es256: TestPasskey,
    pub eddsa: TestPasskey,
    pub rs256: TestPasskey,
}

pub fn key() -> DatabaseKey {
    DatabaseKey::new()
        .with_password(PASSWORD)
        .with_keyfile(&mut std::io::Cursor::new(KEY_FILE))
        .expect("key file")
}

/// Sets the KeePassXC passkey attributes of an entry.
fn set_passkey(
    entry: &mut keepass::db::EntryMut<'_>,
    key: &TestPasskey,
    rp: &str,
    credential: &[u8],
) {
    entry.set_unprotected(
        "KPEX_PASSKEY_CREDENTIAL_ID",
        URL_SAFE_NO_PAD.encode(credential),
    );
    entry.set_protected("KPEX_PASSKEY_PRIVATE_KEY_PEM", key.pem.clone());
    entry.set_unprotected("KPEX_PASSKEY_RELYING_PARTY", rp);
    entry.set_unprotected("KPEX_PASSKEY_USERNAME", "alice");
    entry.set_unprotected(
        "KPEX_PASSKEY_USER_HANDLE",
        URL_SAFE_NO_PAD.encode(b"user-handle"),
    );
}

/// The fixture database: nested groups with notes and icons, the recycle bin with an entry and a
/// sub-group, tags, expiry, protected and unprotected custom fields, colours, override URL,
/// Auto-Type, custom data, an attachment, a 26 MiB attachment, a custom icon, an `otp` field, an
/// entry with `TOTP Seed` and `TOTP Settings`, one with an invalid `otp`, passkeys of three
/// algorithms and an entry with a history of three states, one with an attachment.
pub fn build() -> Fixture {
    let es256 = passkey(-7);
    let eddsa = passkey(-8);
    let rs256 = passkey(-257);
    let mut db = Database::new();
    let (work_id, bin_id);
    {
        let mut root = db.root_mut();
        root.add_entry().edit(|e| {
            e.set_unprotected(fields::TITLE, "Root entry");
            e.set_unprotected(fields::USERNAME, "root-user");
            e.set_protected(fields::PASSWORD, format!("{MARKER}-root"));
        });
        work_id = root
            .add_group()
            .edit(|g| {
                g.name = "Work".into();
                g.notes = Some("work stuff".into());
                g.set_icon_builtin(1);
                g.enable_searching = Some(false);
            })
            .id();
        bin_id = root
            .add_group()
            .edit(|g| g.name = "Recycle Bin".into())
            .id();
    }
    db.meta.recyclebin_uuid = Some(bin_id.uuid());
    let trashed_id;
    {
        let mut root = db.root_mut();
        let mut work = root.group_mut(work_id).expect("work");
        work.add_entry().edit(|e| {
            e.set_unprotected(fields::TITLE, "Mail");
            e.set_unprotected(fields::USERNAME, "alice");
            e.set_protected(fields::PASSWORD, format!("{MARKER}-mail"));
            e.set_unprotected(fields::URL, "https://mail.example.invalid");
            e.set_unprotected(fields::NOTES, "line one\nline two");
            e.set_unprotected("Security question", "first pet");
            e.set_protected("PIN", format!("{MARKER}-pin"));
            e.set_protected("otp", "JBSWY3DPEHPK3PXP");
            e.tags = vec!["work".into(), "mail".into()];
            e.times.creation = Some(Times::epoch());
            e.times.last_modification = Some(Times::epoch());
            e.times.expires = Some(true);
            e.times.expiry = Some(Times::epoch());
            e.foreground_color = Some(Color {
                r: 255,
                g: 0,
                b: 16,
            });
            e.background_color = Some(Color { r: 0, g: 32, b: 64 });
            e.override_url = Some("cmd://open {URL}".into());
            e.autotype = Some(AutoType {
                enabled: true,
                default_sequence: Some("{USERNAME}{TAB}{PASSWORD}{ENTER}".into()),
                data_transfer_obfuscation: DataTransferObfuscation::UseClipboard,
                associations: vec![AutoTypeAssociation {
                    window: "Firefox*".into(),
                    sequence: "{PASSWORD}".into(),
                }],
            });
            e.custom_data.insert(
                "plugin-key".into(),
                CustomDataItem {
                    value: Some(CustomDataValue::String("plugin value".into())),
                    last_modification_time: None,
                },
            );
            e.add_attachment(
                "readme.txt",
                Value::unprotected(b"hello attachment".to_vec()),
            );
            e.add_attachment("huge.bin", Value::unprotected(vec![7u8; 26 * 1024 * 1024]));
            e.set_icon_custom_new(b"\x89PNG-custom-icon".to_vec());
        });
        work.add_entry().edit(|e| {
            e.set_unprotected(fields::TITLE, "Seeded TOTP");
            e.set_protected("TOTP Seed", "JBSWY3DPEHPK3PXP");
            e.set_unprotected("TOTP Settings", "60;8");
        });
        work.add_entry().edit(|e| {
            e.set_unprotected(fields::TITLE, "Broken TOTP");
            e.set_protected("otp", "not a secret!");
            e.set_icon_builtin(999);
        });
        work.add_entry().edit(|e| {
            e.set_unprotected(fields::TITLE, "Passkeys");
            set_passkey(e, &es256, "es.example.invalid", b"credential-es256");
        });
        work.add_entry().edit(|e| {
            e.set_unprotected(fields::TITLE, "EdDSA passkey");
            set_passkey(e, &eddsa, "ed.example.invalid", b"credential-eddsa");
        });
        work.add_entry().edit(|e| {
            e.set_unprotected(fields::TITLE, "RSA passkey");
            set_passkey(e, &rs256, "rsa.example.invalid", b"credential-rs256");
        });
        work.add_entry().edit(|e| {
            e.set_unprotected(fields::TITLE, "Unreadable passkey");
            e.set_unprotected(
                "KPEX_PASSKEY_CREDENTIAL_ID",
                URL_SAFE_NO_PAD.encode(b"credential-bad"),
            );
            e.set_protected(
                "KPEX_PASSKEY_PRIVATE_KEY_PEM",
                "-----BEGIN PRIVATE KEY-----\n!!!\n-----END PRIVATE KEY-----",
            );
            e.set_unprotected("KPEX_PASSKEY_RELYING_PARTY", "bad.example.invalid");
        });
        let mut with_history = work.add_entry();
        with_history.edit(|e| {
            e.set_unprotected(fields::TITLE, "History v1");
            e.set_protected(fields::PASSWORD, format!("{MARKER}-v1"));
        });
        with_history.edit_tracking(|e| {
            e.set_protected(fields::PASSWORD, format!("{MARKER}-v2"));
            e.add_attachment(
                "state.txt",
                Value::unprotected(b"in a history state".to_vec()),
            );
        });
        with_history.edit_tracking(|e| e.set_unprotected(fields::TITLE, "History v3"));
        with_history.edit_tracking(|e| e.set_protected(fields::PASSWORD, format!("{MARKER}-v4")));
        let mut servers = work.add_group();
        servers.edit(|g| {
            g.name = "Servers".into();
            g.set_icon_custom_new(b"\x89PNG-group-icon".to_vec());
        });
        servers
            .add_entry()
            .edit(|e| e.set_unprotected(fields::TITLE, "Server entry"));
        trashed_id = work
            .add_entry()
            .edit(|e| {
                e.set_unprotected(fields::TITLE, "Trashed entry");
                e.set_protected(fields::PASSWORD, format!("{MARKER}-trashed"));
            })
            .id();
    }
    db.root_mut()
        .group_mut(work_id)
        .expect("work")
        .entry_mut(trashed_id)
        .expect("trashed")
        .move_to(bin_id)
        .expect("move to the bin");
    {
        let mut root = db.root_mut();
        let mut bin = root.group_mut(bin_id).expect("bin");
        let mut old = bin.add_group();
        old.edit(|g| g.name = "Old".into());
        old.add_entry()
            .edit(|e| e.set_unprotected(fields::TITLE, "Old trashed"));
    }
    let mut bytes = Vec::new();
    db.save(&mut bytes, key()).expect("save the fixture");
    Fixture {
        bytes,
        es256,
        eddsa,
        rs256,
    }
}
