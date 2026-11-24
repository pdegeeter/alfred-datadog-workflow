//! One function per script filter: parse the query, load the sources, rank,
//! build the items. Never fails: problems become items.

use std::cmp::Ordering;
use std::time::Duration;

use powerpack::{Item, Output};

use crate::alfred;
use crate::app::App;
use crate::cache::{Kind, Loaded, Target};
use crate::dashboards::{Board, BoardKind};
use crate::frecency::Frecency;
use crate::query::Query;
use crate::ranking;
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
