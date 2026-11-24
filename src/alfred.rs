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
use crate::dashboards::{Board, BoardKind};
use crate::pup::{self, PupError};
use crate::services::Service;
use crate::time;
use crate::urls::{Urls, Window};

/// macOS system icons, so the workflow ships no artwork of its own.
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
