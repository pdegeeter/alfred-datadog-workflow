//! Shared test helpers: a deterministic, offline [`CommandRunner`] double,
//! anonymized `pup` fixtures and a throwaway [`App`].
//!
//! Included by multiple test crates; each uses a different subset, so unused
//! helpers are expected per-crate.
#![allow(dead_code)]

use std::cell::RefCell;
use std::collections::{HashMap, VecDeque};
use std::path::Path;

use alfred_dd::app::App;
use alfred_dd::config::Config;
use alfred_dd::runner::CommandRunner;
use anyhow::{bail, Result};

/// 2026-11-24T12:00:00Z, the "now" of every test.
pub const NOW_MS: i64 = 1_795_521_600_000;

/// A single recorded invocation of [`FakeRunner`].
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Call {
    pub program: String,
    pub args: Vec<String>,
    pub detached: bool,
}

#[derive(Debug, Clone)]
enum Response {
    Success(String),
    Failure(String),
}

/// A [`CommandRunner`] that returns canned output keyed by program name and
/// records every call, so tests stay offline and can assert on invocations.
///
/// Responses queued for a program are consumed in order; the last one keeps
/// answering once the queue is down to it. Detached spawns always succeed.
pub struct FakeRunner {
    responses: RefCell<HashMap<String, VecDeque<Response>>>,
    pub calls: RefCell<Vec<Call>>,
}

impl FakeRunner {
    pub fn new() -> Self {
        Self {
            responses: RefCell::new(HashMap::new()),
            calls: RefCell::new(Vec::new()),
        }
    }

    /// Queue a successful run of `program` printing `stdout`.
    pub fn with_success(self, program: &str, stdout: &str) -> Self {
        self.push(program, Response::Success(stdout.to_owned()));

        self
    }

    /// Queue a failed run of `program`.
    pub fn with_failure(self, program: &str, message: &str) -> Self {
        self.push(program, Response::Failure(message.to_owned()));

        self
    }

    /// Every recorded call.
    pub fn calls(&self) -> Vec<Call> {
        self.calls.borrow().clone()
    }

    /// The arguments of every call, in order (detached spawns included).
    pub fn argvs(&self) -> Vec<Vec<String>> {
        self.calls().into_iter().map(|call| call.args).collect()
    }

    /// The detached spawns only.
    pub fn detached(&self) -> Vec<Call> {
        self.calls()
            .into_iter()
            .filter(|call| call.detached)
            .collect()
    }

    fn push(&self, program: &str, response: Response) {
        self.responses
            .borrow_mut()
            .entry(program.to_owned())
            .or_default()
            .push_back(response);
    }

    fn record(&self, program: &str, args: &[&str], detached: bool) {
        self.calls.borrow_mut().push(Call {
            program: program.to_owned(),
            args: args.iter().map(|arg| arg.to_string()).collect(),
            detached,
        });
    }
}

impl CommandRunner for FakeRunner {
    fn run(&self, program: &str, args: &[&str]) -> Result<String> {
        self.record(program, args, false);

        let mut responses = self.responses.borrow_mut();
        let Some(queue) = responses.get_mut(program) else {
            bail!("failed to spawn `{program}`: No such file or directory (os error 2)");
        };

        let response = if queue.len() > 1 {
            queue.pop_front().expect("queue is not empty")
        } else {
            queue.front().cloned().expect("queue is not empty")
        };

        match response {
            Response::Success(stdout) => Ok(stdout),
            Response::Failure(message) => bail!("{message}"),
        }
    }

    fn spawn_detached(&self, program: &str, args: &[&str]) -> Result<()> {
        self.record(program, args, true);

        Ok(())
    }
}

/// Contents of `tests/fixtures/pup/<name>.json`: real responses, anonymized.
pub fn fixture(name: &str) -> String {
    let path = Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("tests/fixtures/pup")
        .join(format!("{name}.json"));

    std::fs::read_to_string(&path).unwrap_or_else(|err| panic!("{}: {err}", path.display()))
}

/// A configuration on `site` / `env` whose directories live under `dir`.
pub fn config(dir: &Path, site: &str, env: &str) -> Config {
    Config {
        site: site.to_owned(),
        env: env.to_owned(),
        cache_dir: dir.join("cache"),
        data_dir: dir.join("data"),
    }
}

/// Where the tests pretend pup is installed.
pub const PUP: &str = "/opt/homebrew/bin/pup";

/// Where the tests pretend the workflow binary is.
pub const EXE: &str = "/workflow/alfred-dd";

/// An [`App`] on `datadoghq.eu` / `production` at [`NOW_MS`], pup installed.
pub fn app<'a>(runner: &'a FakeRunner, dir: &Path) -> App<'a> {
    App::new(
        runner,
        config(dir, "datadoghq.eu", "production"),
        Some(PUP.to_owned()),
        EXE,
        NOW_MS,
    )
}

/// The pup argv of `GET endpoint`.
pub fn api(endpoint: &str) -> Vec<String> {
    vec![
        "--no-envelope".to_owned(),
        "api".to_owned(),
        endpoint.to_owned(),
    ]
}

/// The JSON Alfred receives for these items.
pub fn items_json(items: Vec<powerpack::Item>) -> serde_json::Value {
    let mut output = powerpack::Output::new();
    output.items(items);

    serde_json::to_value(&output).expect("items serialize")["items"].clone()
}
