//! The single Run Script action every script filter item feeds into.
//!
//! Each item (and each of its modifiers) carries an `arg` of the form
//! `"<verb> <payload>"`. [`Action::parse`] turns it back into an [`Action`] and
//! [`execute`] performs it. Payload fields are ordered so that only the last
//! one may contain spaces (copied text, Alfred queries); keys, kinds and URLs
//! never do.

use anyhow::{anyhow, bail, Result};

use crate::app::App;
use crate::runner::CommandRunner;

/// Program used for clipboard writes and Alfred searches.
pub const OSASCRIPT: &str = "/usr/bin/osascript";

/// Program used to open URLs in the default browser.
pub const OPEN: &str = "/usr/bin/open";

const CLIPBOARD_SCRIPT: &str = "on run argv\nset the clipboard to (item 1 of argv)\nend run";

const ALFRED_SEARCH_SCRIPT: &str = "on run argv\ntell application id \"com.runningwithcrayons.Alfred\" to search (item 1 of argv)\nend run";

/// Something the user asked for by actioning an item.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Action {
    /// Open a URL, without touching frecency.
    OpenUrl { url: String },
    /// Copy text to the clipboard.
    Copy { text: String },
    /// Show Alfred with this query typed in (e.g. `ddd `).
    Search { query: String },
}

impl Action {
    /// Encode as the `arg` of an Alfred item.
    pub fn to_arg(&self) -> String {
        match self {
            Action::OpenUrl { url } => join("open-url", &[url]),
            Action::Copy { text } => join("copy", &[text]),
            Action::Search { query } => format!("search {query}"),
        }
    }

    /// Decode an item `arg` produced by [`Action::to_arg`].
    pub fn parse(arg: &str) -> Result<Action> {
        let (verb, payload) = arg.split_once(' ').unwrap_or((arg.trim(), ""));

        let action = match verb.trim() {
            "open-url" => Action::OpenUrl {
                url: required(payload.trim(), "url")?,
            },
            "copy" => Action::Copy {
                text: required(payload.trim(), "text")?,
            },
            "search" => Action::Search {
                query: required(payload.trim_start(), "query")?,
            },
            other => bail!("unknown action: {other:?}"),
        };

        Ok(action)
    }
}

fn join(verb: &str, fields: &[&str]) -> String {
    let mut arg = verb.to_owned();

    for field in fields {
        if !field.is_empty() {
            arg.push(' ');
            arg.push_str(field);
        }
    }

    arg
}

fn required(value: &str, what: &str) -> Result<String> {
    if value.is_empty() {
        return Err(anyhow!("missing {what}"));
    }

    Ok(value.to_owned())
}

/// Put `text` on the clipboard.
pub fn copy_to_clipboard(runner: &dyn CommandRunner, text: &str) -> Result<()> {
    runner
        .run(OSASCRIPT, &["-e", CLIPBOARD_SCRIPT, text])
        .map(|_| ())
}

/// Open `url` in the default browser.
pub fn open_url(runner: &dyn CommandRunner, url: &str) -> Result<()> {
    runner.run(OPEN, &[url]).map(|_| ())
}

/// Show Alfred with `query` typed in.
pub fn alfred_search(runner: &dyn CommandRunner, query: &str) -> Result<()> {
    runner
        .run(OSASCRIPT, &["-e", ALFRED_SEARCH_SCRIPT, query])
        .map(|_| ())
}

/// Perform `action`. Returns the text of the notification to show, if any.
pub fn execute(app: &App, action: &Action) -> Result<Option<String>> {
    match action {
        Action::OpenUrl { url } => {
            open_url(app.runner, url)?;

            Ok(None)
        }
        Action::Copy { text } => {
            copy_to_clipboard(app.runner, text)?;

            Ok(Some(format!("Copied {text}")))
        }
        Action::Search { query } => {
            alfred_search(app.runner, query)?;

            Ok(None)
        }
    }
}
