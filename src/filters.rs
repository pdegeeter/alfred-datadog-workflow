//! One function per script filter: parse the query, load the sources, rank,
//! build the items. Never fails: problems become items.

use std::cmp::Ordering;
use std::time::Duration;

use powerpack::{Item, Output};

use crate::alfred;
use crate::app::App;
use crate::cache::{Kind, Loaded, Store, Target};
use crate::clipboard;
use crate::dashboards::{Board, BoardKind};
use crate::frecency::Frecency;
use crate::incidents::{self, Incident};
use crate::monitors::{self, Monitor};
use crate::query::Query;
use crate::ranking;
use crate::services::Service;
use crate::sources;
use crate::urls::Window;

/// How soon Alfred reruns a script filter while a background refresh runs.
pub const RERUN: Duration = Duration::from_millis(500);

/// What a script filter prints.
#[derive(Debug)]
pub struct Listing {
    pub items: Vec<Item>,
    /// A background refresh is running: ask Alfred to rerun shortly.
    pub rerun: bool,
}

impl Listing {
    /// The Alfred output.
    pub fn output(self) -> Output {
        let mut output = Output::new();
        output.items(self.items);

        if self.rerun {
            output.rerun(RERUN);
        }

        output
    }
}

fn frecency(app: &App) -> Frecency {
    Frecency::load(&Frecency::path(&app.config.data_dir))
}

/// Newest first, unknown dates last.
fn newest_first(a: Option<i64>, b: Option<i64>) -> Ordering {
    b.unwrap_or(i64::MIN).cmp(&a.unwrap_or(i64::MIN))
}

/// `dd`: what the query — or else the clipboard, read once — points at
/// (trace, monitor, incident, Datadog URL), then every keyword. The
/// clipboard is never logged.
pub fn home(app: &App, input: &str) -> Listing {
    let urls = app.urls();
    let mut clips = clipboard::detect(input, &urls);

    if clips.is_empty() {
        let pasted = app.runner.run(clipboard::PBPASTE, &[]).unwrap_or_default();
        clips = clipboard::detect(&pasted, &urls);
    }

    let carried = if clips.is_empty() { input } else { "" };
    let mut items: Vec<Item> = clips
        .iter()
        .map(|clip| alfred::clip_item(clip, &urls))
        .collect();
    items.extend(alfred::keyword_items(carried));

    Listing {
        items,
        rerun: false,
    }
}

/// `ddd`: dashboards and notebooks. `nb:` restricts to notebooks, `db:` to
/// dashboards (their value, if any, is a search term).
pub fn dashboards(app: &App, input: &str) -> Listing {
    let query = Query::parse(input);
    let only_notebooks = query.has_filter("nb");
    let only_dashboards = query.has_filter("db");
    let mut raw_terms = query.terms.clone();

    for key in ["nb", "db"] {
        raw_terms.extend(
            query
                .filter_values(key)
                .into_iter()
                .filter(|value| !value.is_empty())
                .map(str::to_owned),
        );
    }

    let mut targets = Vec::new();
    let mut loaded: Vec<(Kind, Loaded<Board>)> = Vec::new();

    if !only_notebooks {
        targets.push(Target::new(Kind::Dashboards));
        loaded.push((Kind::Dashboards, sources::load_dashboards(app)));
    }

    if !only_dashboards || only_notebooks {
        targets.push(Target::new(Kind::Notebooks));
        loaded.push((Kind::Notebooks, sources::load_notebooks(app)));
    }

    let rerun = loaded.iter().any(|(_, list)| list.rerun);
    let fetched_ms = loaded.iter().filter_map(|(_, list)| list.fetched_ms).min();
    let errors: Vec<Item> = loaded
        .iter()
        .filter_map(|(kind, list)| {
            list.error
                .as_ref()
                .map(|error| alfred::pup_error_item(*kind, error))
        })
        .collect();
    let boards: Vec<Board> = loaded
        .into_iter()
        .flat_map(|(_, list)| list.items)
        .collect();

    let frecency = frecency(app);
    let terms = ranking::terms(&raw_terms);
    let ranked = ranking::rank(
        boards,
        &terms,
        |board| board.title.as_str(),
        |board| frecency.score(&board.frecency_key(), app.now_ms),
        |a, b| {
            newest_first(a.modified_ms, b.modified_ms)
                .then_with(|| (a.kind == BoardKind::Notebook).cmp(&(b.kind == BoardKind::Notebook)))
        },
    );

    let urls = app.urls();
    let search = raw_terms.join(" ");
    let window = Window::last(query.range_ms, app.now_ms);
    let mut items = Vec::new();

    if ranked.is_empty() && errors.is_empty() && !search.is_empty() {
        items.push(alfred::no_board_item(&urls, &search));
    }

    items.extend(alfred::board_items(
        ranked, &urls, window, &search, app.now_ms,
    ));
    items.extend(errors);
    items.push(alfred::refresh_item(&targets, fetched_ms, app.now_ms));

    Listing { items, rerun }
}

/// The logs / traces query of `query`: its tokens verbatim (free terms and
/// `key:value`), then `env:<env>`.
pub fn search_query(query: &Query, env: &str) -> String {
    let mut tokens = query.passthrough.clone();
    tokens.push(format!("env:{env}"));

    tokens.join(" ")
}

/// Services whose name starts with the `service:` value being typed as the
/// last token, from the services cache only (never fetched here).
fn service_suggestions(app: &App, query: &Query, env: &str) -> Vec<String> {
    let Some(last) = query.passthrough.last() else {
        return Vec::new();
    };
    let Some(prefix) = last.strip_prefix("service:") else {
        return Vec::new();
    };

    let store = Store::new(&app.config.cache_dir);
    let key = sources::key(app, &Target::services(env));
    let Some(entry) = store.read::<Service>(Kind::Services, &key) else {
        return Vec::new();
    };

    if entry.items.iter().any(|service| service.name == prefix) {
        return Vec::new();
    }

    let prefix = ranking::normalize(prefix);
    let frecency = frecency(app);
    let matches: Vec<Service> = entry
        .items
        .into_iter()
        .filter(|service| ranking::normalize(&service.name).contains(&prefix))
        .collect();

    ranking::rank(
        matches,
        &ranking::terms(&[prefix.as_str()]),
        |service| service.name.as_str(),
        |service| frecency.score(&service.frecency_key(), app.now_ms),
        |a, b| a.name.cmp(&b.name),
    )
    .into_iter()
    .take(MAX_SUGGESTIONS)
    .map(|service| service.name)
    .collect()
}

/// At most this many `service:` completions.
pub const MAX_SUGGESTIONS: usize = 9;

/// `ddl`: a Log Explorer link built from the query, no API call. Typing
/// `service:<prefix>` as the last token lists matching services from the
/// services cache as completions.
pub fn logs(app: &App, input: &str) -> Listing {
    let query = Query::parse(input);
    let env = query.env_or(&app.config.env).to_owned();
    let search = search_query(&query, &env);
    let window = Window::last(query.range_ms, app.now_ms);
    let mut items = vec![alfred::logs_item(
        &app.urls(),
        &search,
        window,
        query.range_ms,
    )];

    let tokens: Vec<&str> = input.split_whitespace().collect();
    let typed = query
        .passthrough
        .last()
        .and_then(|last| tokens.iter().rposition(|token| token == last));

    for name in service_suggestions(app, &query, &env) {
        let Some(position) = typed else {
            break;
        };
        let mut completion: Vec<String> = tokens.iter().map(|token| token.to_string()).collect();
        completion[position] = format!("service:{name}");

        items.push(alfred::completion_item(
            &name,
            &format!("{} ", completion.join(" ")),
        ));
    }

    Listing {
        items,
        rerun: false,
    }
}

/// `dds`: APM services of the query's env (`env:` or `DD_ENV`); `team:`
/// keeps the services of that team.
pub fn services(app: &App, input: &str) -> Listing {
    let query = Query::parse(input);
    let env = query.env_or(&app.config.env).to_owned();
    let loaded = sources::load_services(app, &env);
    let teams: Vec<String> = query
        .filter_values("team")
        .into_iter()
        .filter(|team| !team.is_empty())
        .map(ranking::normalize)
        .collect();

    let services = loaded
        .items
        .into_iter()
        .filter(|service| {
            teams.is_empty()
                || service
                    .team
                    .as_deref()
                    .is_some_and(|team| teams.contains(&ranking::normalize(team)))
        })
        .collect();

    let frecency = frecency(app);
    let ranked = ranking::rank(
        services,
        &ranking::terms(&query.terms),
        |service| service.name.as_str(),
        |service| frecency.score(&service.frecency_key(), app.now_ms),
        |a, b| b.traced.cmp(&a.traced).then_with(|| a.name.cmp(&b.name)),
    );

    let mut items = Vec::new();

    if let Some(error) = &loaded.error {
        items.push(alfred::pup_error_item(Kind::Services, error));
    } else if ranked.is_empty() {
        items.push(alfred::error_item(
            "No matching service",
            &format!("No APM or catalog service matches in {env}"),
        ));
    }

    let window = Window::last(query.range_ms, app.now_ms);
    items.extend(alfred::service_items(ranked, &app.urls(), &env, window));
    items.push(alfred::refresh_item(
        &[Target::services(&env)],
        loaded.fetched_ms,
        app.now_ms,
    ));

    Listing {
        items,
        rerun: loaded.rerun,
    }
}

/// Whether `values` (normalized `key:` filter values) all appear among the
/// monitor's `key:` tags. No values: no constraint.
fn has_tags(monitor: &Monitor, key: &str, values: &[String]) -> bool {
    values.iter().all(|value| {
        monitor
            .tag_values(key)
            .any(|tag| ranking::normalize(tag) == *value)
    })
}

/// `ddm`: monitors of the env (`env:` or `DD_ENV`; monitors without an `env:`
/// tag always count), filtered by name terms and `team:` / `service:` tags,
/// alerting first, then frecency, then name.
pub fn monitors(app: &App, input: &str) -> Listing {
    let query = Query::parse(input);
    let env = query.env_or(&app.config.env).to_owned();
    let loaded = sources::load_monitors(app);
    let terms = ranking::terms(&query.terms);
    let filters: Vec<(&str, Vec<String>)> = ["team", "service"]
        .into_iter()
        .map(|key| {
            let values = query
                .filter_values(key)
                .into_iter()
                .filter(|value| !value.is_empty())
                .map(ranking::normalize)
                .collect();

            (key, values)
        })
        .collect();

    let frecency = frecency(app);
    let mut matched: Vec<(f64, Monitor)> = loaded
        .items
        .into_iter()
        .filter(|monitor| monitor.in_env(&env))
        .filter(|monitor| {
            filters
                .iter()
                .all(|(key, values)| has_tags(monitor, key, values))
        })
        .filter(|monitor| ranking::tier(&monitor.name, &terms).is_some())
        .map(|monitor| (frecency.score(&monitor.frecency_key(), app.now_ms), monitor))
        .collect();

    matched.sort_by(|(score_a, a), (score_b, b)| {
        monitors::status_rank(&a.status)
            .cmp(&monitors::status_rank(&b.status))
            .then_with(|| score_b.partial_cmp(score_a).unwrap_or(Ordering::Equal))
            .then_with(|| ranking::normalize(&a.name).cmp(&ranking::normalize(&b.name)))
    });

    let monitors: Vec<Monitor> = matched.into_iter().map(|(_, monitor)| monitor).collect();
    let mut items = Vec::new();

    if let Some(error) = &loaded.error {
        items.push(alfred::pup_error_item(Kind::Monitors, error));
    } else if monitors.is_empty() {
        items.push(alfred::error_item(
            "No matching monitor",
            &format!("No monitor matches in {env}"),
        ));
    }

    let search = query.passthrough.join(" ");
    items.extend(alfred::monitor_items(
        monitors,
        &app.urls(),
        &search,
        app.now_ms,
    ));
    items.push(alfred::refresh_item(
        &[Target::new(Kind::Monitors)],
        loaded.fetched_ms,
        app.now_ms,
    ));

    Listing {
        items,
        rerun: loaded.rerun,
    }
}

/// `ddi`: active and stable incidents, most severe first, then newest;
/// free terms filter the titles.
pub fn incidents(app: &App, input: &str) -> Listing {
    let query = Query::parse(input);
    let loaded = sources::load_incidents(app);
    let terms = ranking::terms(&query.terms);

    let mut incidents: Vec<Incident> = loaded
        .items
        .into_iter()
        .filter(|incident| ranking::tier(&incident.title, &terms).is_some())
        .collect();

    incidents.sort_by(|a, b| {
        incidents::severity_rank(a.severity.as_deref())
            .cmp(&incidents::severity_rank(b.severity.as_deref()))
            .then_with(|| newest_first(a.created_ms, b.created_ms))
    });

    let mut items = Vec::new();

    if let Some(error) = &loaded.error {
        items.push(alfred::pup_error_item(Kind::Incidents, error));
    } else if incidents.is_empty() {
        items.push(alfred::error_item(
            "No active incident",
            "No active or stable incident matches",
        ));
    }

    items.extend(alfred::incident_items(incidents, &app.urls(), app.now_ms));
    items.push(alfred::refresh_item(
        &[Target::new(Kind::Incidents)],
        loaded.fetched_ms,
        app.now_ms,
    ));

    Listing {
        items,
        rerun: loaded.rerun,
    }
}
