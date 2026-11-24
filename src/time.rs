//! Timestamps: RFC 3339 parsing (Datadog's `modified_at`, `created`…) and
//! "3d ago" formatting, without a date crate.

const SECOND_MS: i64 = 1_000;
const MINUTE_MS: i64 = 60 * SECOND_MS;
const HOUR_MS: i64 = 60 * MINUTE_MS;
const DAY_MS: i64 = 24 * HOUR_MS;

/// Milliseconds since the Unix epoch of an RFC 3339 timestamp such as
/// `2026-11-18T09:45:07.234445+00:00` or `2026-10-29T12:09:14Z`.
pub fn parse_rfc3339_ms(value: &str) -> Option<i64> {
    let value = value.trim();
    let (date, rest) = value.split_once(['T', 't', ' '])?;
    let mut date_parts = date.splitn(3, '-');
    let year: i64 = date_parts.next()?.parse().ok()?;
    let month: i64 = date_parts.next()?.parse().ok()?;
    let day: i64 = date_parts.next()?.parse().ok()?;

    let offset_at = rest
        .rfind(['Z', 'z', '+', '-'])
        .filter(|&index| index >= 8)
        .unwrap_or(rest.len());
    let (clock, offset) = rest.split_at(offset_at);

    let mut clock_parts = clock.splitn(3, ':');
    let hour: i64 = clock_parts.next()?.parse().ok()?;
    let minute: i64 = clock_parts.next()?.parse().ok()?;
    let seconds = clock_parts.next().unwrap_or("0");
    let (whole, fraction) = seconds.split_once('.').unwrap_or((seconds, ""));
    let second: i64 = whole.parse().ok()?;
    let millis: i64 = format!("{:0<3}", fraction.get(..3).unwrap_or(fraction))
        .parse()
        .ok()?;

    if !(1..=12).contains(&month) || !(1..=31).contains(&day) || hour > 23 || minute > 59 {
        return None;
    }

    let offset_ms = parse_offset_ms(offset)?;
    let days = days_from_civil(year, month, day);

    Some(
        days * DAY_MS + hour * HOUR_MS + minute * MINUTE_MS + second * SECOND_MS + millis
            - offset_ms,
    )
}

fn parse_offset_ms(offset: &str) -> Option<i64> {
    if offset.is_empty() || offset.eq_ignore_ascii_case("z") {
        return Some(0);
    }

    let sign = if offset.starts_with('-') { -1 } else { 1 };
    let (hours, minutes) = offset[1..].split_once(':').unwrap_or((&offset[1..], "0"));
    let hours: i64 = hours.parse().ok()?;
    let minutes: i64 = minutes.parse().ok()?;

    Some(sign * (hours * HOUR_MS + minutes * MINUTE_MS))
}

/// Days since 1970-01-01 of a proleptic Gregorian date (Howard Hinnant's
/// `days_from_civil`).
fn days_from_civil(year: i64, month: i64, day: i64) -> i64 {
    let year = if month <= 2 { year - 1 } else { year };
    let era = year.div_euclid(400);
    let year_of_era = year - era * 400;
    let month_index = (month + 9) % 12;
    let day_of_year = (153 * month_index + 2) / 5 + day - 1;
    let day_of_era = year_of_era * 365 + year_of_era / 4 - year_of_era / 100 + day_of_year;

    era * 146_097 + day_of_era - 719_468
}

/// How long ago `then_ms` was, as of `now_ms`: `just now`, `5m ago`,
/// `3h ago`, `2d ago`, `4mo ago`, `2y ago`.
pub fn ago(then_ms: i64, now_ms: i64) -> String {
    let elapsed = (now_ms - then_ms).max(0);

    if elapsed < MINUTE_MS {
        return "just now".to_owned();
    }

    let (amount, unit) = if elapsed < HOUR_MS {
        (elapsed / MINUTE_MS, "m")
    } else if elapsed < DAY_MS {
        (elapsed / HOUR_MS, "h")
    } else if elapsed < 30 * DAY_MS {
        (elapsed / DAY_MS, "d")
    } else if elapsed < 365 * DAY_MS {
        (elapsed / (30 * DAY_MS), "mo")
    } else {
        (elapsed / (365 * DAY_MS), "y")
    };

    format!("{amount}{unit} ago")
}
