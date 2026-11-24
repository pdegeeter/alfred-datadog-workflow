# CLAUDE.md

This file provides guidance to Claude Code (claude.ai/code) when working with code in this repository.

## Keep everything in sync (always)

Every code change must update its tests, the docs under `docs/`, the `README.md`,
and this `CLAUDE.md` in the **same** change — never as a follow-up. If you change
behaviour (a keyword, a modifier, an action verb, a pup call, a URL format, a
cache TTL, the plist, the release flow), find and fix every reference to the old
behaviour across `README.md`, `docs/`, `CLAUDE.md`, `workflow/info.plist`'s
`readme`, and doc comments before committing. `cargo test --all`,
`cargo clippy --all-targets -- -D warnings`, and `cargo fmt --all` must all pass.

## What this is

A read-only Alfred (macOS) workflow written in Rust to navigate Datadog. It
never writes to Datadog: every action opens a URL, copies text or refreshes a
local cache. A single binary, `alfred-dd`, backs every object of the Alfred
graph:

- **Script Filters**, one per keyword (none yet). They print Alfred JSON.
- One **Run Script**, `alfred-dd run <arg>`, connected to every script filter
  for every modifier (none, ⌘, ⌥, ⌃, ⇧, ⇧⌘). `arg` is an encoded
  `action::Action` (`"<verb> <payload>"`, see `docs/architecture.md`).
  Whatever it prints feeds a Post Notification.

The binary ships inside `workflow/` (with `info.plist`) and is packaged into a
`.alfredworkflow` file for distribution.

## Commands

```sh
cargo test --all                          # full suite (offline, no Datadog needed)
cargo test --test action_test             # one test file
cargo clippy --all-targets -- -D warnings # lint; warnings are errors (CI gate)
cargo fmt --all                           # format (CI checks with --check)

./scripts/build.sh            # sync plist version + build release + package .alfredworkflow
./scripts/deploy.sh           # build + update the installed workflow in place (imports it the first time)
./scripts/deploy.sh --import  # build + open the .alfredworkflow to import it
```

`powerpack-cli` is required for build/deploy/packaging: `cargo install powerpack-cli`.

## Architecture

**Everything is a pure function except one trait.** Every process spawn —
`osascript` (clipboard write, Alfred) and `/usr/bin/open` — goes through
`runner::CommandRunner` (`SystemRunner` in prod). Its `spawn_detached` starts a
background process without waiting for it. Tests inject `FakeRunner`
(`tests/common/mod.rs`), which returns queued canned stdout per program and
records invocations (detached ones flagged), so the suite runs offline. Keep
new logic pure and route any new spawn through it.

- `config.rs` — `DD_SITE` (default `datadoghq.eu`) and `DD_ENV` (default
  `production`) from Alfred's workflow variables; cache and data directories.
- `app.rs` — `App`: runner + config + `now_ms`, injected everywhere so time is
  deterministic in tests.
- `action.rs` — the `arg` protocol (`Action::to_arg` / `Action::parse`) and
  `execute`.
- `alfred.rs` — item builders; item icons are macOS system icons.

## Conventions that matter here

- **Read-only**: never add a call that writes to Datadog (no mute, downtime,
  incident update).
- **The `arg` protocol**: only the last payload field may contain spaces.
  Adding a verb means updating `to_arg`, `parse`, `execute`, the round-trip test
  in `tests/action_test.rs` and the table in `docs/architecture.md`.
- **Modifier connections**: every script filter has one connection per
  modifier (none, ⌘, ⌥, ⌃, ⇧, ⇧⌘) to the Run Script. Adding a new modifier
  combination means adding its connection in `workflow/info.plist` and to
  `MODIFIERS` in `tests/plist_test.rs`.
- **No `uid` on Alfred items** — Alfred would re-sort them. Filtering and
  ordering are done in Rust (`alfredfiltersresults = false`).
- **Script filters never hard-fail**: errors become an invalid Alfred item.
- **Keywords are fixed**; `tests/plist_test.rs` pins the keyword → subcommand
  map, the modifier connections, the readme and the `DD_SITE` / `DD_ENV`
  defaults: update it with any keyword, modifier or configuration change.
- **No artwork, no logo**: item icons and `workflow/icon.png` come from macOS
  system icons (`scripts/icon.sh`). Never add the Datadog logo.
- **Public repo**: fixtures and docs use neutral names (`checkout-api`,
  `team:payments`, `alice@example.com`); never commit real service, team,
  dashboard or person names.
- **Deploying**: `scripts/deploy.sh` copies the build over the installed
  workflow found by bundle id (never `powerpack link`: Alfred 6 turns symlinks
  into stale copies and fails to re-import over an install).
- **Editing `workflow/info.plist`**: if you edit the workflow in Alfred's UI,
  copy the updated `info.plist` from the installed folder back into `workflow/`
  before committing (the next deploy overwrites it), and validate with
  `plutil -lint workflow/info.plist`. The binary in `workflow/` is a build
  artifact (gitignored); `info.plist` and `icon.png` are tracked.
- **Protected `main`**: work via PRs only (enforced for admins). CI `test` and
  `package` must be green, and **all commits must be signed** (SSH signing) to
  merge. See `docs/development.md`.

## Releases

Automated via release-please (`release-type: rust`). Land Conventional Commits
on `main` (`feat:`/`fix:`/`chore:`…); release-please opens a Release PR that
bumps `Cargo.toml`/`Cargo.lock` and `CHANGELOG.md`. Merging it tags `vX.Y.Z`,
creates the GitHub Release, and the `build-and-upload` job attaches the
`.alfredworkflow` (arm64) as a release asset. Do not bump versions by hand.
See `docs/release.md`.

Target platform is **macOS arm64 only**; CI and release build on `macos-latest`.
