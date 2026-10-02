//! Time stamps of the password manager (spec 034, data-model.md): RFC 3339 in UTC with
//! milliseconds, written by Rust so that two writes within a second still differ for the conflict
//! check (research R15) and so that the age of a record compares through `datetime(...)`.

use std::time::{SystemTime, UNIX_EPOCH};

/// The current time as `2026-10-02T09:41:45.123Z`.
pub fn now() -> String {
    format_millis(unix_millis(SystemTime::now()))
}

/// Milliseconds since the Unix epoch; a clock before 1970 reads as 0.
pub fn unix_millis(time: SystemTime) -> i64 {
    time.duration_since(UNIX_EPOCH)
        .map(|d| i64::try_from(d.as_millis()).unwrap_or(i64::MAX))
        .unwrap_or(0)
}

/// `2026-10-02T09:41:45.123Z` for milliseconds since the Unix epoch (proleptic Gregorian, UTC).
pub fn format_millis(millis: i64) -> String {
    let seconds = millis.div_euclid(1000);
    let ms = millis.rem_euclid(1000);
    let days = seconds.div_euclid(86_400);
    let secs_of_day = seconds.rem_euclid(86_400);
    let (year, month, day) = civil_from_days(days);
    format!(
        "{year:04}-{month:02}-{day:02}T{:02}:{:02}:{:02}.{ms:03}Z",
        secs_of_day / 3600,
        secs_of_day % 3600 / 60,
        secs_of_day % 60,
    )
}

/// Days since 1970-01-01 to a calendar date (Howard Hinnant's `civil_from_days`).
fn civil_from_days(days: i64) -> (i64, i64, i64) {
    let z = days + 719_468;
    let era = z.div_euclid(146_097);
    let doe = z.rem_euclid(146_097);
    let yoe = (doe - doe / 1460 + doe / 36_524 - doe / 146_096) / 365;
    let year = yoe + era * 400;
    let doy = doe - (365 * yoe + yoe / 4 - yoe / 100);
    let mp = (5 * doy + 2) / 153;
    let day = doy - (153 * mp + 2) / 5 + 1;
    let month = if mp < 10 { mp + 3 } else { mp - 9 };
    (if month <= 2 { year + 1 } else { year }, month, day)
}

/// Milliseconds since the epoch for a text like `now()` writes; `None` for anything else.
pub fn parse_millis(text: &str) -> Option<i64> {
    let bytes = text.as_bytes();
    if bytes.len() != 24
        || bytes[4] != b'-'
        || bytes[7] != b'-'
        || bytes[10] != b'T'
        || bytes[13] != b':'
        || bytes[16] != b':'
        || bytes[19] != b'.'
        || bytes[23] != b'Z'
    {
        return None;
    }
    let number = |from: usize, to: usize| text.get(from..to)?.parse::<i64>().ok();
    let (year, month, day) = (number(0, 4)?, number(5, 7)?, number(8, 10)?);
    let (hour, minute, second, ms) = (
        number(11, 13)?,
        number(14, 16)?,
        number(17, 19)?,
        number(20, 23)?,
    );
    if !(1..=12).contains(&month)
        || !(1..=31).contains(&day)
        || hour > 23
        || minute > 59
        || second > 60
    {
        return None;
    }
    let days = days_from_civil(year, month, day);
    Some(((days * 24 + hour) * 60 + minute) * 60_000 + second * 1000 + ms)
}

/// Milliseconds since the epoch for an ISO 8601 time as the exporters of other products write it:
/// `2024-01-05T10:20:30Z`, with a fraction of any length (`.1234567`) and with `Z`, an offset
/// (`+02:00`, `+0200`) or none (read as UTC); a space may stand for the `T`. `None` for anything else.
pub fn parse_iso_millis(text: &str) -> Option<i64> {
    let text = text.trim();
    let (date, rest) = text.split_once(['T', ' '])?;
    let mut date_parts = date.split('-');
    let year = date_parts.next()?.parse::<i64>().ok()?;
    let month = date_parts.next()?.parse::<i64>().ok()?;
    let day = date_parts.next()?.parse::<i64>().ok()?;
    if date_parts.next().is_some() || !(1..=12).contains(&month) || !(1..=31).contains(&day) {
        return None;
    }
    let zone_at = rest.find(['Z', 'z', '+', '-']).unwrap_or(rest.len());
    let (clock, zone) = rest.split_at(zone_at);
    let (hms, fraction) = clock.split_once('.').unwrap_or((clock, ""));
    let mut time_parts = hms.split(':');
    let hour = time_parts.next()?.parse::<i64>().ok()?;
    let minute = time_parts.next()?.parse::<i64>().ok()?;
    let second = time_parts
        .next()
        .map_or(Some(0), |p| p.parse::<i64>().ok())?;
    if time_parts.next().is_some() || hour > 23 || minute > 59 || second > 60 {
        return None;
    }
    let ms = if fraction.is_empty() {
        0
    } else if fraction.bytes().all(|b| b.is_ascii_digit()) {
        format!("{:0<3}", &fraction[..fraction.len().min(3)])
            .parse::<i64>()
            .ok()?
    } else {
        return None;
    };
    let offset_minutes = match zone {
        "" | "Z" | "z" => 0,
        z => {
            let sign = if z.starts_with('-') { -1 } else { 1 };
            let digits: String = z[1..].chars().filter(char::is_ascii_digit).collect();
            if digits.len() != 4 {
                return None;
            }
            sign * (digits[..2].parse::<i64>().ok()? * 60 + digits[2..].parse::<i64>().ok()?)
        }
    };
    let days = days_from_civil(year, month, day);
    Some(((days * 24 + hour) * 60 + minute) * 60_000 + second * 1000 + ms - offset_minutes * 60_000)
}

/// Calendar date to days since 1970-01-01 (the inverse of `civil_from_days`).
fn days_from_civil(year: i64, month: i64, day: i64) -> i64 {
    let y = if month <= 2 { year - 1 } else { year };
    let era = y.div_euclid(400);
    let yoe = y - era * 400;
    let mp = if month > 2 { month - 3 } else { month + 9 };
    let doy = (153 * mp + 2) / 5 + day - 1;
    let doe = yoe * 365 + yoe / 4 - yoe / 100 + doy;
    era * 146_097 + doe - 719_468
}

/// The current time, but never at or before `previous`: two writes within one millisecond still
/// get different tokens (research R15).
pub fn now_after(previous: Option<&str>) -> String {
    let current = unix_millis(SystemTime::now());
    let floor = previous.and_then(parse_millis).map_or(i64::MIN, |p| p + 1);
    format_millis(current.max(floor))
}
