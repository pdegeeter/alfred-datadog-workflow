//! `alfred-dd` — Alfred workflow entry point.
//!
//! Subcommands, dispatched by Alfred:
//!   * `dashboards [query]` — script filter `ddd`: dashboards and notebooks.
//!   * `services [query]`   — script filter `dds`: APM services.
//!   * `refresh <kind> [env]` — background refresh of one cached source.
//!   * `run <arg>`          — Run Script action: perform the selected item.

use std::env;
use std::io;
use std::process::ExitCode;
use std::time::{SystemTime, UNIX_EPOCH};

use alfred_dd::action::{self, Action};
use alfred_dd::app::App;
use alfred_dd::cache::{Kind, Target};
use alfred_dd::config::Config;
use alfred_dd::filters::{self, Listing};
use alfred_dd::pup;
use alfred_dd::runner::SystemRunner;
use alfred_dd::sources;

fn main() -> ExitCode {
    let mut args = env::args().skip(1);
    let command = args.next().unwrap_or_default();
    let arg = args.next().unwrap_or_default();
    let extra = args.next();
    let runner = SystemRunner;

    let config = match Config::from_env() {
        Ok(config) => config,
        Err(err) => {
            eprintln!("{err:#}");

            return ExitCode::FAILURE;
        }
    };

    // Inherited by every pup child: no update check (a GitHub call), and the
    // configured site even when Alfred did not export it.
    env::set_var("PUP_NO_UPDATE_CHECK", "1");
    env::set_var("DD_SITE", &config.site);

    let exe = env::current_exe()
        .map(|path| path.to_string_lossy().into_owned())
        .unwrap_or_else(|_| "alfred-dd".to_owned());
    let app = App::new(&runner, config, pup::locate(), exe, now_ms());

    match command.as_str() {
        "dashboards" => output(filters::dashboards(&app, &arg)),
        "services" => output(filters::services(&app, &arg)),
        "refresh" => refresh(&app, &arg, extra),
        "run" => run(&app, &arg),
        other => {
            eprintln!("unknown command: `{other}` (expected dashboards, services, refresh or run)");

            ExitCode::FAILURE
        }
    }
}

fn now_ms() -> i64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|elapsed| elapsed.as_millis() as i64)
        .unwrap_or_default()
}

/// Print a script filter's items.
fn output(listing: Listing) -> ExitCode {
    match listing.output().write(io::stdout()) {
        Ok(()) => ExitCode::SUCCESS,
        Err(err) => {
            eprintln!("failed to write Alfred output: {err}");

            ExitCode::FAILURE
        }
    }
}

/// Background refresh started by a script filter serving a stale cache:
/// `refresh <kind> [env]`.
fn refresh(app: &App, kind: &str, env: Option<String>) -> ExitCode {
    let Some(kind) = Kind::parse(kind) else {
        eprintln!("unknown source: `{kind}`");

        return ExitCode::FAILURE;
    };
    let target = Target {
        kind,
        env: env.filter(|env| !env.is_empty()),
    };

    match sources::refresh(app, &target) {
        Ok(_) => ExitCode::SUCCESS,
        Err(err) => {
            eprintln!("refresh {kind}: {err}");

            ExitCode::FAILURE
        }
    }
}

/// Run Script action: decode the item's `arg` and perform it. Whatever is
/// printed becomes the text of the connected notification.
fn run(app: &App, arg: &str) -> ExitCode {
    let result = Action::parse(arg).and_then(|parsed| action::execute(app, &parsed));

    match result {
        Ok(message) => {
            if let Some(message) = message {
                println!("{message}");
            }

            ExitCode::SUCCESS
        }
        Err(err) => {
            println!("Datadog: {err}");
            eprintln!("{err:#}");

            ExitCode::FAILURE
        }
    }
}
