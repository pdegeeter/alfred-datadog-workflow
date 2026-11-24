//! Script filter items for every keyword.
//!
//! Every actionable item (and each of its modifiers) carries an
//! [`Action`] encoded in its `arg`; the one connected Run Script decodes and
//! performs it. No `uid` is set: Alfred would otherwise re-sort items by its
//! own usage knowledge and override the order computed here (match quality,
//! frecency, state).

use powerpack::{Item, Key, Modifier};

use crate::action::Action;
use crate::cache::{Kind, Target};
use crate::clipboard::Clip;
use crate::dashboards::{Board, BoardKind};
use crate::incidents::Incident;
use crate::monitors::Monitor;
use crate::pup::{self, PupError};
use crate::services::Service;
use crate::time;
use crate::urls::{Urls, Window};

/// Item icons: macOS system icons, so states (alert, warning, no data) stay
/// distinguishable from the workflow's own Datadog icon.
pub mod icons {
    const DIR: &str = "/System/Library/CoreServices/CoreTypes.bundle/Contents/Resources";

    fn system(name: &str) -> powerpack::Icon {
        powerpack::Icon::with_image(format!("{DIR}/{name}.icns"))
    }

    pub fn error() -> powerpack::Icon {
        system("AlertNoteIcon")
    }

    pub fn alert() -> powerpack::Icon {
        system("AlertStopIcon")
    }

    pub fn warning() -> powerpack::Icon {
        system("AlertCautionBadgeIcon")
    }

    pub fn unknown() -> powerpack::Icon {
        system("GenericQuestionMarkIcon")
    }

    pub fn dashboard() -> powerpack::Icon {
        system("GenericWindowIcon")
    }

    pub fn notebook() -> powerpack::Icon {
        system("GenericStationeryIcon")
    }

    pub fn service() -> powerpack::Icon {
        system("GenericNetworkIcon")
    }

    pub fn link() -> powerpack::Icon {
        system("GenericURLIcon")
    }

    pub fn document() -> powerpack::Icon {
        system("GenericDocumentIcon")
    }

    pub fn refresh() -> powerpack::Icon {
        system("Clock")
    }
}

/// Item shown in place of a list when something went wrong.
pub fn error_item(title: &str, message: &str) -> Item {
    Item::new(title)
        .subtitle(message)
        .valid(false)
        .icon(icons::error())
}

/// Item that runs `action` on <kbd>↩</kbd>.
pub fn action_item(title: impl Into<String>, subtitle: impl Into<String>, action: &Action) -> Item {
    Item::new(title).subtitle(subtitle).arg(action.to_arg())
}

/// Modifier that runs `action`.
pub fn modifier(keys: &[Key], subtitle: &str, action: &Action) -> Modifier {
    Modifier::new_multi(keys.iter().copied())
        .subtitle(subtitle)
        .arg(action.to_arg())
}

/// The item explaining why `kind` could not be loaded.
pub fn pup_error_item(kind: Kind, error: &PupError) -> Item {
    match error {
        PupError::Missing => error_item(
            "pup is not installed",
            &format!("Install it with: {}", pup::INSTALL_COMMAND),
        ),
        PupError::NotLoggedIn => error_item(
            &format!("Run `{}`", pup::LOGIN_COMMAND),
            "pup has no valid Datadog session (it may have expired)",
        ),
        PupError::Forbidden => error_item(
            &format!("Datadog refused access to {kind}"),
            &format!("Your pup login needs the {} permission", kind.scope()),
        ),
        PupError::RateLimited => error_item("Datadog rate limit reached", "Retry in a few seconds"),
        PupError::Other(message) => error_item(&format!("Cannot load {kind}"), message),
    }
}

/// Last item of a list: ↩ refreshes `targets` now.
pub fn refresh_item(targets: &[Target], fetched_ms: Option<i64>, now_ms: i64) -> Item {
    let names: Vec<String> = targets
        .iter()
        .map(|target| match &target.env {
            Some(env) => format!("{} ({env})", target.kind),
            None => target.kind.to_string(),
        })
        .collect();
    let names = names.join(" & ");
    let age = fetched_ms.map_or("never fetched".to_owned(), |fetched| {
        format!("updated {}", time::ago(fetched, now_ms))
    });

    action_item(
        format!("Refresh {names}"),
        format!("Cached list {age} · ↩ fetch it again now"),
        &Action::Refresh {
            targets: targets.to_vec(),
        },
    )
    .icon(icons::refresh())
}

/// `ddd`: one item per dashboard or notebook.
///
/// ↩ open · ⌘ open on `window` · ⌥ copy URL · ⇧ search the list for `search`.
pub fn board_items(
    boards: Vec<Board>,
    urls: &Urls,
    window: Window,
    search: &str,
    now_ms: i64,
) -> Vec<Item> {
    boards
        .into_iter()
        .map(|board| {
            let url = board.url(urls);
            let key = board.frecency_key();
            let (label, icon, list_url) = match board.kind {
                BoardKind::Dashboard => (
                    "Dashboard",
                    icons::dashboard(),
                    urls.dashboard_search(search),
                ),
                BoardKind::Notebook => {
                    ("Notebook", icons::notebook(), urls.notebook_search(search))
                }
            };
            let label = match &board.detail {
                Some(detail) => format!("{label} ({detail})"),
                None => label.to_owned(),
            };
            let mut subtitle: Vec<String> = Vec::new();

            if !board.author.is_empty() {
                subtitle.push(board.author.clone());
            }

            subtitle.push(label);

            if let Some(modified) = board.modified_ms {
                subtitle.push(format!("modified {}", time::ago(modified, now_ms)));
            }

            Item::new(board.title.clone())
                .subtitle(subtitle.join(" · "))
                .arg(
                    Action::Open {
                        key: key.clone(),
                        url: url.clone(),
                    }
                    .to_arg(),
                )
                .icon(icon)
                .copy_text(url.clone())
                .modifier(modifier(
                    &[Key::Command],
                    "Open on the query's time range",
                    &Action::Open {
                        key,
                        url: board.url_at(urls, window),
                    },
                ))
                .modifier(modifier(
                    &[Key::Option],
                    "Copy the URL",
                    &Action::Copy { text: url },
                ))
                .modifier(modifier(
                    &[Key::Shift],
                    "Search the list in Datadog",
                    &Action::OpenUrl { url: list_url },
                ))
        })
        .collect()
}

/// `dds`: one item per service, every link in `env` on `window`.
///
/// ↩ service page · ⌘ logs · ⌥ traces · ⌃ error tracking · ⇧ monitors ·
/// ⇧⌘ copy the name.
pub fn service_items(services: Vec<Service>, urls: &Urls, env: &str, window: Window) -> Vec<Item> {
    services
        .into_iter()
        .map(|service| {
            let name = service.name.clone();
            let scope = format!("service:{name} env:{env}");
            let mut subtitle: Vec<String> = Vec::new();

            if let Some(team) = &service.team {
                subtitle.push(team.clone());
            }

            let mut stack = service.languages.clone();
            stack.extend(service.kind.clone());

            if !stack.is_empty() {
                subtitle.push(stack.join("/"));
            }

            if service.traced {
                subtitle.push(env.to_owned());
            } else {
                subtitle.push(format!("no traces in {env}"));
            }

            Item::new(name.clone())
                .subtitle(subtitle.join(" · "))
                .arg(
                    Action::Open {
                        key: service.frecency_key(),
                        url: urls.service(&name, env, window),
                    }
                    .to_arg(),
                )
                .icon(icons::service())
                .copy_text(name.clone())
                .modifier(modifier(
                    &[Key::Command],
                    &format!("Logs of {name} in {env}"),
                    &Action::OpenUrl {
                        url: urls.logs(&scope, window),
                    },
                ))
                .modifier(modifier(
                    &[Key::Option],
                    &format!("Traces of {name} in {env}"),
                    &Action::OpenUrl {
                        url: urls.traces(&scope, window),
                    },
                ))
                .modifier(modifier(
                    &[Key::Control],
                    &format!("Error Tracking for {name} in {env}"),
                    &Action::OpenUrl {
                        url: urls.error_tracking(&scope, window),
                    },
                ))
                .modifier(modifier(
                    &[Key::Shift],
                    &format!("Monitors of {name}"),
                    &Action::OpenUrl {
                        url: urls.monitor_search(&format!("service:{name}")),
                    },
                ))
                .modifier(modifier(
                    &[Key::Command, Key::Shift],
                    "Copy the service name",
                    &Action::Copy { text: name },
                ))
        })
        .collect()
}

/// `ddm`: one item per monitor.
///
/// ↩ open · ⌘ the monitor list searched for `search` · ⌥ copy URL.
pub fn monitor_items(monitors: Vec<Monitor>, urls: &Urls, search: &str, now_ms: i64) -> Vec<Item> {
    let list_url = urls.monitor_search(search);

    monitors
        .into_iter()
        .map(|monitor| {
            let url = urls.monitor(monitor.id);
            let mut subtitle = vec![monitor.status.clone(), monitor.kind.clone()];
            let tags: Vec<String> = ["service", "team"]
                .into_iter()
                .flat_map(|key| {
                    monitor
                        .tag_values(key)
                        .map(move |value| format!("{key}:{value}"))
                })
                .collect();

            if !tags.is_empty() {
                subtitle.push(tags.join(", "));
            }

            if let Some(triggered) = monitor.last_triggered_ms {
                subtitle.push(format!("triggered {}", time::ago(triggered, now_ms)));
            }

            subtitle.retain(|part| !part.is_empty());

            let icon = match monitor.status.as_str() {
                "Alert" => icons::alert(),
                "Warn" => icons::warning(),
                "No Data" => icons::unknown(),
                _ => icons::document(),
            };

            Item::new(monitor.name.clone())
                .subtitle(subtitle.join(" · "))
                .arg(
                    Action::Open {
                        key: monitor.frecency_key(),
                        url: url.clone(),
                    }
                    .to_arg(),
                )
                .icon(icon)
                .copy_text(url.clone())
                .modifier(modifier(
                    &[Key::Command],
                    "Search the monitor list in Datadog",
                    &Action::OpenUrl {
                        url: list_url.clone(),
                    },
                ))
                .modifier(modifier(
                    &[Key::Option],
                    "Copy the URL",
                    &Action::Copy { text: url },
                ))
        })
        .collect()
}

/// `ddi`: one item per incident. ↩ open · ⌥ copy URL.
pub fn incident_items(incidents: Vec<Incident>, urls: &Urls, now_ms: i64) -> Vec<Item> {
    incidents
        .into_iter()
        .map(|incident| {
            let url = urls.incident(incident.public_id);
            let mut subtitle = vec![
                incident
                    .severity
                    .clone()
                    .unwrap_or_else(|| "UNKNOWN".to_owned()),
                incident.state.clone(),
            ];

            if let Some(commander) = &incident.commander {
                subtitle.push(commander.clone());
            }

            if let Some(created) = incident.created_ms {
                subtitle.push(time::ago(created, now_ms));
            }

            subtitle.retain(|part| !part.is_empty());

            Item::new(format!("#{} {}", incident.public_id, incident.title))
                .subtitle(subtitle.join(" · "))
                .arg(
                    Action::Open {
                        key: incident.frecency_key(),
                        url: url.clone(),
                    }
                    .to_arg(),
                )
                .icon(icons::alert())
                .copy_text(url.clone())
                .modifier(modifier(
                    &[Key::Option],
                    "Copy the URL",
                    &Action::Copy { text: url },
                ))
        })
        .collect()
}

/// `dd`: the item for something found in the clipboard (or the query).
/// ↩ open · ⌥ copy URL.
pub fn clip_item(clip: &Clip, urls: &Urls) -> Item {
    let (title, key, url, icon) = match clip {
        Clip::Trace(id) => (format!("Trace {id}"), None, urls.trace(id), icons::link()),
        Clip::Monitor(id) => (
            format!("Monitor #{id}"),
            Some(format!("monitor:{id}")),
            urls.monitor(*id),
            icons::document(),
        ),
        Clip::Incident(id) => (
            format!("Incident #{id}"),
            Some(format!("incident:{id}")),
            urls.incident(*id),
            icons::alert(),
        ),
        Clip::Url(url) => ("Datadog link".to_owned(), None, url.clone(), icons::link()),
    };
    let action = match key {
        Some(key) => Action::Open {
            key,
            url: url.clone(),
        },
        None => Action::OpenUrl { url: url.clone() },
    };

    Item::new(title)
        .subtitle(format!("From the clipboard · {url}"))
        .arg(action.to_arg())
        .icon(icon)
        .copy_text(url.clone())
        .modifier(modifier(
            &[Key::Option],
            "Copy the URL",
            &Action::Copy { text: url },
        ))
}

/// `dd`: one item per keyword; ↩ reopens Alfred on it, carrying `query`.
pub fn keyword_items(query: &str) -> Vec<Item> {
    const KEYWORDS: [(&str, &str, &str); 5] = [
        (
            "ddd",
            "Dashboards & notebooks",
            "nb: / db: to restrict · -4h time range",
        ),
        (
            "dds",
            "APM services",
            "Service page, logs, traces, errors, monitors · env:<env>",
        ),
        ("ddm", "Monitors", "Alerting first · team:<t> service:<s>"),
        (
            "ddl",
            "Logs",
            "Log Explorer query · service:<prefix> completes",
        ),
        ("ddi", "Incidents", "Active incidents, most severe first"),
    ];

    KEYWORDS
        .into_iter()
        .map(|(keyword, title, subtitle)| {
            let typed = format!("{keyword} {}", query.trim());
            let typed = if query.trim().is_empty() {
                format!("{keyword} ")
            } else {
                format!("{} ", typed.trim_end())
            };

            action_item(
                format!("{keyword} — {title}"),
                subtitle,
                &Action::Search { query: typed },
            )
        })
        .collect()
}

/// `ddl`: the Log Explorer on `search`.
///
/// ↩ Log Explorer · ⌘ Traces Explorer with the same query · ⌥ copy the URL.
pub fn logs_item(urls: &Urls, search: &str, window: Window, range_ms: i64) -> Item {
    let url = urls.logs(search, window);

    Item::new(format!("Logs: {search}"))
        .subtitle(format!(
            "Last {} · ↩ Log Explorer · ⌘ Traces Explorer · ⌥ copy URL",
            duration(range_ms)
        ))
        .arg(Action::OpenUrl { url: url.clone() }.to_arg())
        .icon(icons::document())
        .copy_text(url.clone())
        .modifier(modifier(
            &[Key::Command],
            "Open the same query in the Traces Explorer",
            &Action::OpenUrl {
                url: urls.traces(search, window),
            },
        ))
        .modifier(modifier(
            &[Key::Option],
            "Copy the Log Explorer URL",
            &Action::Copy { text: url },
        ))
}

/// A suggestion that completes the query to `completion` (⇥ or ↩).
pub fn completion_item(title: &str, completion: &str) -> Item {
    Item::new(title)
        .subtitle("↩ or ⇥ to complete the query")
        .autocomplete(completion)
        .valid(false)
        .icon(icons::service())
}

/// `15m`, `1h`, `7d`, `90m`…
pub fn duration(range_ms: i64) -> String {
    const MINUTE_MS: i64 = 60_000;
    const HOUR_MS: i64 = 60 * MINUTE_MS;
    const DAY_MS: i64 = 24 * HOUR_MS;

    if range_ms % DAY_MS == 0 {
        return format!("{}d", range_ms / DAY_MS);
    }

    if range_ms % HOUR_MS == 0 {
        return format!("{}h", range_ms / HOUR_MS);
    }

    format!("{}m", range_ms / MINUTE_MS)
}

/// `ddd` with no match: search the dashboard list in Datadog.
pub fn no_board_item(urls: &Urls, search: &str) -> Item {
    action_item(
        format!("Search Datadog dashboards for “{search}”"),
        "No matching dashboard or notebook in the cache",
        &Action::OpenUrl {
            url: urls.dashboard_search(search),
        },
    )
    .icon(icons::dashboard())
}
