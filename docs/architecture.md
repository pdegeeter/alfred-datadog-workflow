# Architecture

## Overview

The workflow is a single Rust binary, `alfred-dd`, with one subcommand per
object in the Alfred workflow graph:

| Subcommand               | Alfred object                | Keyword | Responsibility |
| ------------------------ | ---------------------------- | ------- | -------------- |
| `alfred-dd run <arg>`    | Run Script (`action.script`) | —       | Decode the selected item's `arg` and perform it. |

Every script filter is connected to the single Run Script, itself connected to
a Post Notification that shows whatever `run` prints (copies, refreshes,
errors).

The workflow is **read-only**: it never writes to Datadog.

## The `arg` protocol

Script filter items do not carry raw data: their `arg` — and the `arg` of each
modifier (`mods`) — is an encoded [`Action`](../src/action.rs):
`"<verb> <payload>"`. `run` decodes it with `Action::parse` and performs it
with `action::execute`.

| Verb       | Payload    | Effect |
| ---------- | ---------- | ------ |
| `open-url` | `<url>`    | Open the URL. |
| `copy`     | `<text>`   | Copy the text. |
| `search`   | `<query>`  | Reopen Alfred on that query (e.g. `ddd `); the trailing space is kept. |

Fields are ordered so that only the last one may contain spaces; URLs, keys
and kinds never do. A round-trip test covers every verb.

Each script filter declares connections for every modifier (none,
<kbd>⌘</kbd>, <kbd>⌥</kbd>, <kbd>⌃</kbd>, <kbd>⇧</kbd>, <kbd>⇧⌘</kbd>), all to
the same Run Script, so a modifier's `arg` always reaches `run`.

## Configuration

Alfred passes workflow variables to the binary as environment variables:

| Variable  | Default        | Used for |
| --------- | -------------- | -------- |
| `DD_SITE` | `datadoghq.eu` | Every link (`https://app.<DD_SITE>/…`). |
| `DD_ENV`  | `production`   | Default env of services, logs, traces and monitors. |

## Design decisions

- **Everything is a pure function except one trait.** The only side-effecting
  boundary — spawning `osascript` and `open` — is `runner::CommandRunner`
  (`SystemRunner` in prod, `FakeRunner` in tests). Its `spawn_detached` starts
  a process in its own process group with no stdio and returns at once.
- **Injected clock**: `App::now_ms` is read once per invocation; nothing else
  reads the system time.
- **No `uid` on items**: Alfred would re-sort by its own usage knowledge.
  Filtering and ordering happen in Rust (`alfredfiltersresults = false`).
- **Script filters never hard-fail**: errors become an invalid Alfred item.
- **No artwork**: item icons are macOS system icons (`alfred::icons`), the
  workflow icon is converted from one (`scripts/icon.sh`).
