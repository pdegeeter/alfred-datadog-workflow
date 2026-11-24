//! Matching and ordering of named objects against the free terms of a query.
//!
//! Matching is case- and accent-insensitive and every term must match. The
//! match tier (exact title, title prefix, word prefixes, substring) comes
//! first, then a score (frecency), then a kind-specific tie-breaker.

use std::cmp::Ordering;

/// How well a name matches the terms; lower is better.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
pub enum Tier {
    /// The whole query is the name.
    Exact,
    /// The name starts with the whole query.
    Prefix,
    /// Every term starts a word of the name.
    WordPrefix,
    /// Every term appears somewhere in the name.
    Substring,
}

/// Lowercase, accents removed (Latin-1 and Latin Extended-A), so `Équipe`
/// matches `equipe`.
pub fn normalize(text: &str) -> String {
    text.chars().flat_map(fold).collect()
}

fn fold(c: char) -> impl Iterator<Item = char> {
    let folded: &str = match c {
        'À'..='Å' | 'à'..='å' | 'Ā' | 'ā' | 'Ă' | 'ă' | 'Ą' | 'ą' => "a",
        'Æ' | 'æ' => "ae",
        'Ç' | 'ç' | 'Ć' | 'ć' | 'Ĉ' | 'ĉ' | 'Ċ' | 'ċ' | 'Č' | 'č' => "c",
        'Ď' | 'ď' | 'Đ' | 'đ' => "d",
        'È'..='Ë' | 'è'..='ë' | 'Ē' | 'ē' | 'Ĕ' | 'ĕ' | 'Ė' | 'ė' | 'Ę' | 'ę' | 'Ě' | 'ě' => {
            "e"
        }
        'Ĝ' | 'ĝ' | 'Ğ' | 'ğ' | 'Ġ' | 'ġ' | 'Ģ' | 'ģ' => "g",
        'Ĥ' | 'ĥ' | 'Ħ' | 'ħ' => "h",
        'Ì'..='Ï' | 'ì'..='ï' | 'Ĩ' | 'ĩ' | 'Ī' | 'ī' | 'Ĭ' | 'ĭ' | 'Į' | 'į' | 'İ' | 'ı' => {
            "i"
        }
        'Ĵ' | 'ĵ' => "j",
        'Ķ' | 'ķ' => "k",
        'Ĺ' | 'ĺ' | 'Ļ' | 'ļ' | 'Ľ' | 'ľ' | 'Ŀ' | 'ŀ' | 'Ł' | 'ł' => "l",
        'Ñ' | 'ñ' | 'Ń' | 'ń' | 'Ņ' | 'ņ' | 'Ň' | 'ň' => "n",
        'Ò'..='Ö' | 'Ø' | 'ò'..='ö' | 'ø' | 'Ō' | 'ō' | 'Ŏ' | 'ŏ' | 'Ő' | 'ő' => "o",
        'Œ' | 'œ' => "oe",
        'Ŕ' | 'ŕ' | 'Ŗ' | 'ŗ' | 'Ř' | 'ř' => "r",
        'Ś' | 'ś' | 'Ŝ' | 'ŝ' | 'Ş' | 'ş' | 'Š' | 'š' => "s",
        'ß' => "ss",
        'Ţ' | 'ţ' | 'Ť' | 'ť' | 'Ŧ' | 'ŧ' => "t",
        'Ù'..='Ü'
        | 'ù'..='ü'
        | 'Ũ'
        | 'ũ'
        | 'Ū'
        | 'ū'
        | 'Ŭ'
        | 'ŭ'
        | 'Ů'
        | 'ů'
        | 'Ű'
        | 'ű'
        | 'Ų'
        | 'ų' => "u",
        'Ŵ' | 'ŵ' => "w",
        'Ý' | 'ý' | 'ÿ' | 'Ŷ' | 'ŷ' | 'Ÿ' => "y",
        'Ź' | 'ź' | 'Ż' | 'ż' | 'Ž' | 'ž' => "z",
        _ => "",
    };

    let replacement: Vec<char> = if folded.is_empty() {
        c.to_lowercase().collect()
    } else {
        folded.chars().collect()
    };

    replacement.into_iter()
}

/// The free terms of a query, normalized; empty terms dropped.
pub fn terms<S: AsRef<str>>(raw: &[S]) -> Vec<String> {
    raw.iter()
        .map(|term| normalize(term.as_ref()))
        .filter(|term| !term.is_empty())
        .collect()
}

fn is_word_start(name: &str, index: usize) -> bool {
    if index == 0 {
        return true;
    }

    name[..index]
        .chars()
        .next_back()
        .is_some_and(|previous| !previous.is_alphanumeric())
}

/// How `name` matches normalized `terms`, or `None` if a term is missing.
/// No terms match everything as [`Tier::Substring`].
pub fn tier(name: &str, terms: &[String]) -> Option<Tier> {
    let name = normalize(name);

    if !terms.iter().all(|term| name.contains(term.as_str())) {
        return None;
    }

    if terms.is_empty() {
        return Some(Tier::Substring);
    }

    let query = terms.join(" ");

    if name == query {
        return Some(Tier::Exact);
    }

    if name.starts_with(&query) {
        return Some(Tier::Prefix);
    }

    let word_prefixes = terms.iter().all(|term| {
        name.match_indices(term.as_str())
            .any(|(index, _)| is_word_start(&name, index))
    });

    if word_prefixes {
        return Some(Tier::WordPrefix);
    }

    Some(Tier::Substring)
}

/// Keep the items whose name matches `terms` and order them: tier, then
/// `score` (higher first), then `tie_break`. With no terms, every item is
/// kept and ordered by score, then `tie_break`.
pub fn rank<T>(
    items: Vec<T>,
    terms: &[String],
    name: impl Fn(&T) -> &str,
    score: impl Fn(&T) -> f64,
    tie_break: impl Fn(&T, &T) -> Ordering,
) -> Vec<T> {
    let mut matched: Vec<(Tier, f64, T)> = items
        .into_iter()
        .filter_map(|item| {
            let tier = tier(name(&item), terms)?;
            let score = score(&item);

            Some((tier, score, item))
        })
        .collect();

    matched.sort_by(|(tier_a, score_a, a), (tier_b, score_b, b)| {
        tier_a
            .cmp(tier_b)
            .then_with(|| score_b.partial_cmp(score_a).unwrap_or(Ordering::Equal))
            .then_with(|| tie_break(a, b))
    });

    matched.into_iter().map(|(_, _, item)| item).collect()
}
