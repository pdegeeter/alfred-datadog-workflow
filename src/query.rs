//! The query syntax shared by every keyword.
//!
//! - free terms match names (see [`crate::ranking`]);
//! - `env:<value>` overrides `DD_ENV`;
//! - a last token such as `-15m`, `-1h`, `-4h`, `-24h`, `-7d` sets the time
//!   range (default: the last hour, live);
//! - any other `key:value` token (`service:x`, `team:y`, `nb:`, `@http.status_code:500`)
//!   is kept verbatim for logs / traces queries and available as a filter.

/// Minutes, hours and days in milliseconds.
const MINUTE_MS: i64 = 60_000;
const HOUR_MS: i64 = 60 * MINUTE_MS;
const DAY_MS: i64 = 24 * HOUR_MS;

/// Default time range: the last hour.
pub const DEFAULT_RANGE_MS: i64 = HOUR_MS;

/// A parsed query.
#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub struct Query {
    /// Free terms (no `key:value`), in order.
    pub terms: Vec<String>,
    /// `env:<value>`, if given (the last one wins).
    pub env: Option<String>,
    /// Length of the time range ending now, in milliseconds.
    pub range_ms: i64,
    /// `key:value` tokens other than `env:`, in order. The value may be empty
    /// while the user is typing (`service:`).
    pub filters: Vec<(String, String)>,
    /// Every token except `env:` and the time suffix, verbatim, in order.
    pub passthrough: Vec<String>,
}

impl Query {
    pub fn parse(input: &str) -> Query {
        let mut tokens: Vec<&str> = input.split_whitespace().collect();
        let mut range_ms = DEFAULT_RANGE_MS;

        if let Some(parsed) = tokens.last().and_then(|last| parse_range_ms(last)) {
            range_ms = parsed;
            tokens.pop();
        }

        let mut query = Query {
            range_ms,
            ..Query::default()
        };

        for token in tokens {
            match split_filter(token) {
                Some(("env", value)) if !value.is_empty() => {
                    query.env = Some(value.to_owned());
                }
                Some((key, value)) => {
                    query.filters.push((key.to_owned(), value.to_owned()));
                    query.passthrough.push(token.to_owned());
                }
                None => {
                    query.terms.push(token.to_owned());
                    query.passthrough.push(token.to_owned());
                }
            }
        }

        query
    }

    /// The env to use: `env:` from the query, else `default`.
    pub fn env_or<'a>(&'a self, default: &'a str) -> &'a str {
        self.env.as_deref().unwrap_or(default)
    }

    /// Values of the `key:` filters.
    pub fn filter_values(&self, key: &str) -> Vec<&str> {
        self.filters
            .iter()
            .filter(|(name, _)| name == key)
            .map(|(_, value)| value.as_str())
            .collect()
    }

    /// Whether a `key:` filter is present (with or without a value).
    pub fn has_filter(&self, key: &str) -> bool {
        self.filters.iter().any(|(name, _)| name == key)
    }
}

/// `key:value` with a plain key (letters, digits, `_`, `.`, `-`, `@`); URLs
/// (`https://…`) and times (`10:30`) are not filters.
fn split_filter(token: &str) -> Option<(&str, &str)> {
    let (key, value) = token.split_once(':')?;

    if key.is_empty() || value.starts_with("//") {
        return None;
    }

    let plain = key
        .chars()
        .all(|c| c.is_ascii_alphanumeric() || matches!(c, '_' | '.' | '-' | '@'));
    let starts_with_letter = key
        .chars()
        .next()
        .is_some_and(|c| c.is_ascii_alphabetic() || c == '@');

    if !plain || !starts_with_letter {
        return None;
    }

    Some((key, value))
}

/// `-15m`, `-1h`, `-7d` → milliseconds.
pub fn parse_range_ms(token: &str) -> Option<i64> {
    let rest = token.strip_prefix('-')?;
    let unit = rest.chars().last()?;
    let amount: i64 = rest[..rest.len() - unit.len_utf8()].parse().ok()?;

    if amount <= 0 {
        return None;
    }

    let unit_ms = match unit {
        'm' => MINUTE_MS,
        'h' => HOUR_MS,
        'd' => DAY_MS,
        _ => return None,
    };

    amount.checked_mul(unit_ms)
}
