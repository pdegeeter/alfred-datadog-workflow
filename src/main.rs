//! `alfred-dd` — Alfred workflow entry point.
//!
//! Subcommands, dispatched by Alfred:
//!   * `run <arg>` — Run Script action: perform the selected item.

use std::env;
use std::process::ExitCode;
use std::time::{SystemTime, UNIX_EPOCH};

use alfred_dd::action::{self, Action};
use alfred_dd::app::App;
use alfred_dd::config::Config;
use alfred_dd::runner::SystemRunner;

fn main() -> ExitCode {
    let mut args = env::args().skip(1);
    let command = args.next().unwrap_or_default();
    let arg = args.next().unwrap_or_default();
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

    let app = App::new(&runner, config, now_ms());

    match command.as_str() {
        "run" => run(&app, &arg),
        other => {
            eprintln!("unknown command: `{other}` (expected run)");

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
