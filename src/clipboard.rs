//! What a copied text points at in Datadog. Pure: the caller reads the
//! clipboard (`pbpaste`) and never logs it.

use crate::urls::{is_datadog_app_host, Urls};

/// Program that prints the clipboard.
pub const PBPASTE: &str = "/usr/bin/pbpaste";

/// Longer texts are not IDs or URLs worth checking.
pub const MAX_LENGTH: usize = 2048;

/// Something the clipboard points at.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Clip {
    /// A trace id: 32 hex chars (128-bit) or decimal (64-bit).
    Trace(String),
    Monitor(u64),
    /// An incident, by public id.
    Incident(u64),
    /// A Datadog app URL, moved to the configured site.
    Url(String),
}

/// Every reading of `text`, best first. A 1–20 digit number is ambiguous: a
/// trace id and a monitor id.
pub fn detect(text: &str, urls: &Urls) -> Vec<Clip> {
    let text = text.trim();

    let spaced = text.contains(char::is_whitespace);

    if text.is_empty() || text.len() > MAX_LENGTH || (spaced && !is_labelled(text)) {
        return Vec::new();
    }

    if let Some(clip) = datadog_url(text, urls) {
        return vec![clip];
    }

    if let Some(id) = text.strip_prefix('#').and_then(number) {
        return vec![Clip::Incident(id)];
    }

    if let Some(id) = labelled(text, "monitor") {
        return vec![Clip::Monitor(id)];
    }

    if let Some(id) = labelled(text, "incident") {
        return vec![Clip::Incident(id)];
    }

    if text.len() == 32 && text.chars().all(|c| c.is_ascii_hexdigit()) {
        return vec![Clip::Trace(text.to_ascii_lowercase())];
    }

    if (1..=20).contains(&text.len()) && text.chars().all(|c| c.is_ascii_digit()) {
        let mut clips = vec![Clip::Trace(text.to_owned())];
        clips.extend(number(text).map(Clip::Monitor));

        return clips;
    }

    Vec::new()
}

fn number(text: &str) -> Option<u64> {
    if text.is_empty() || !text.chars().all(|c| c.is_ascii_digit()) {
        return None;
    }

    text.parse().ok()
}

/// `monitor 123`, `monitor:123`, `Monitor #123`, `monitor_id: 123`…
fn is_labelled(text: &str) -> bool {
    labelled(text, "monitor").is_some() || labelled(text, "incident").is_some()
}

fn labelled(text: &str, label: &str) -> Option<u64> {
    let lower = text.to_ascii_lowercase();
    let rest = lower.strip_prefix(label)?;
    let rest = rest.strip_prefix("_id").unwrap_or(rest);
    let rest = rest.trim_start_matches([' ', ':', '#', '-', '=']);

    number(rest)
}

/// A Datadog app URL: a monitor or an incident page by id, else the URL
/// itself on the configured site.
fn datadog_url(text: &str, urls: &Urls) -> Option<Clip> {
    let rest = text
        .strip_prefix("https://")
        .or_else(|| text.strip_prefix("http://"))?;
    let (host, path) = rest.split_at(rest.find(['/', '?', '#']).unwrap_or(rest.len()));

    if !is_datadog_app_host(host) {
        return None;
    }

    let path = path.split(['?', '#']).next().unwrap_or_default();
    let segments: Vec<&str> = path
        .split('/')
        .filter(|segment| !segment.is_empty())
        .collect();

    match segments.as_slice() {
        ["monitors", id, ..] => {
            if let Some(id) = number(id) {
                return Some(Clip::Monitor(id));
            }
        }
        ["incidents", id, ..] => {
            if let Some(id) = number(id) {
                return Some(Clip::Incident(id));
            }
        }
        _ => {}
    }

    urls.on_this_site(text).map(Clip::Url)
}
