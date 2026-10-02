//! Revealing, copying and the TOTP code (spec 034, FR-003, FR-005, FR-006, research R7): the only
//! functions through which the value of a secret leaves the entry's row. The window gets a value
//! only by an explicit act of the user (`reveal`) or not at all (`copy_value` goes to the clipboard
//! in Rust, `totp_code` returns the code, never the secret).

use std::fmt;

use haex_crdt::rusqlite::params;
use zeroize::Zeroizing;

use super::items::otp_params_of;
use super::model::{CopyField, RevealedSecret, SecretField, SecretItem, SecretKeyValue, TotpCode};
use super::totp::{code_at, remaining_seconds};
use crate::error::{HolziError, Result};
use crate::storage::query::Query;

/// A value on its way to the clipboard. It is zeroed when dropped and prints no value.
pub struct Copied(Zeroizing<String>);

impl Copied {
    pub fn as_str(&self) -> &str {
        &self.0
    }

    pub fn into_inner(self) -> Zeroizing<String> {
        self.0
    }
}

impl fmt::Debug for Copied {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str("Copied(<redacted>)")
    }
}

/// One column of an entry as text; an empty cell reads as an empty string, a missing entry is
/// `PasswordsNotFound`.
fn column(q: &mut impl Query, item_id: &str, column: &'static str) -> Result<Zeroizing<String>> {
    // `column` is one of the literals below, never input.
    let sql = format!("SELECT {column} FROM haex_passwords_item_details WHERE id = ?1");
    let cell = q
        .query_row(&sql, params![item_id], |r| r.get::<_, Option<String>>(0))?
        .ok_or(HolziError::PasswordsNotFound)?;
    Ok(Zeroizing::new(cell.unwrap_or_default()))
}

fn key_value(q: &mut impl Query, item_id: &str, field_id: &str) -> Result<Zeroizing<String>> {
    let cell = q
        .query_row(
            "SELECT value FROM haex_passwords_item_key_values WHERE id = ?1 AND item_id = ?2",
            params![field_id, item_id],
            |r| r.get::<_, Option<String>>(0),
        )?
        .ok_or(HolziError::PasswordsNotFound)?;
    Ok(Zeroizing::new(cell.unwrap_or_default()))
}

/// The value of a secret field, for the moment the user asks to see it.
pub fn reveal(q: &mut impl Query, item_id: &str, field: &SecretField) -> Result<RevealedSecret> {
    let value = match field {
        SecretField::Password => column(q, item_id, "password")?,
        SecretField::OtpSecret => column(q, item_id, "otp_secret")?,
        SecretField::KeyValue { id } => key_value(q, item_id, id)?,
    };
    Ok(RevealedSecret { value })
}

/// The value to put on the clipboard; for `Totp` the current code of this moment.
pub fn copy_value(q: &mut impl Query, item_id: &str, field: &CopyField) -> Result<Copied> {
    let value = match field {
        CopyField::Username => column(q, item_id, "username")?,
        CopyField::Password => column(q, item_id, "password")?,
        CopyField::KeyValue { id } => key_value(q, item_id, id)?,
        CopyField::Totp => Zeroizing::new(totp_code(q, item_id, unix_now())?.code),
    };
    Ok(Copied(value))
}

/// The code of the entry at a Unix time with the seconds until it changes. `PasswordsNotFound` for
/// a missing entry, `InvalidInput` for a missing or invalid secret, digits, period or algorithm.
pub fn totp_code(q: &mut impl Query, item_id: &str, unix_seconds: u64) -> Result<TotpCode> {
    let params = otp_params_of(q, item_id)?;
    Ok(TotpCode {
        code: code_at(&params, unix_seconds),
        remaining_seconds: remaining_seconds(params.period, unix_seconds),
        period: params.period,
        digits: params.digits,
    })
}

/// The system time in whole seconds; a clock before 1970 reads as 0.
pub fn unix_now() -> u64 {
    std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map(|d| d.as_secs())
        .unwrap_or(0)
}

/// The whole entry with its secrets for the single-entry read of callers from outside; `None` for a
/// missing entry. The access rules decide before this is called.
pub fn secret_item(q: &mut impl Query, item_id: &str) -> Result<Option<SecretItem>> {
    let Some(mut item) = q.query_row(
        "SELECT id, title, username, password, note, url, expires_at, otp_secret, otp_digits, \
                otp_period, otp_algorithm FROM haex_passwords_item_details WHERE id = ?1",
        params![item_id],
        |r| {
            Ok(SecretItem {
                id: r.get(0)?,
                title: r.get(1)?,
                username: r.get(2)?,
                password: r.get(3)?,
                note: r.get(4)?,
                url: r.get(5)?,
                expires_at: r.get(6)?,
                otp_secret: r.get(7)?,
                otp_digits: r
                    .get::<_, Option<i64>>(8)?
                    .and_then(|v| u32::try_from(v).ok()),
                otp_period: r
                    .get::<_, Option<i64>>(9)?
                    .and_then(|v| u32::try_from(v).ok()),
                otp_algorithm: r.get(10)?,
                tags: Vec::new(),
                key_values: Vec::new(),
            })
        },
    )?
    else {
        return Ok(None);
    };
    item.tags = super::tags::names_of_item(q, item_id)?;
    item.key_values = q.query_map(
        "SELECT key, value FROM haex_passwords_item_key_values WHERE item_id = ?1 ORDER BY rowid",
        params![item_id],
        |r| {
            Ok(SecretKeyValue {
                key: r.get(0)?,
                value: r.get(1)?,
            })
        },
    )?;
    Ok(Some(item))
}
