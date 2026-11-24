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

- **Script Filters**, one per keyword: `home` (`dd`), `dashboards` (`ddd`), `services`
  (`dds`), `monitors` (`ddm`), `logs` (`ddl`, no API call), `incidents`
  (`ddi`). They print Alfred JSON (`filters.rs`), with `rerun` while a
  background refresh runs.
- `alfred-dd refresh <kind> [env]`, spawned detached by a script filter
  serving a stale cache (`env` only for services).
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

# Exercise the binary directly (it speaks Alfred JSON on stdout; needs pup logged in):
cargo build && DD_ENV=prod ./target/debug/alfred-dd dashboards checkout | python3 -m json.tool
```

`powerpack-cli` is required for build/deploy/packaging: `cargo install powerpack-cli`.

## Architecture

**Everything is a pure function except one trait.** Every process spawn —
`pup` (Datadog), `osascript` (clipboard write, Alfred), `/usr/bin/pbpaste`
(clipboard read) and `/usr/bin/open` —
goes through
`runner::CommandRunner` (`SystemRunner` in prod). Its `spawn_detached` starts a
background process without waiting for it. Tests inject `FakeRunner`
(`tests/common/mod.rs`), which returns queued canned stdout per program and
records invocations (detached ones flagged), so the suite runs offline. Keep
new logic pure and route any new spawn through it.

- `config.rs` — `DD_SITE` (default `datadoghq.eu`) and `DD_ENV` (default
  `production`) from Alfred's workflow variables; cache and data directories.
- `app.rs` — `App`: runner + config + `now_ms`, injected everywhere so time is
  deterministic in tests.
- `pup.rs` — every Datadog read is `pup --no-envelope api <endpoint>` (raw
  GET). pup's own subcommands (`dashboards list`, `monitors search`…) answer
  403 with an OAuth token `pup api` accepts: do not use them. pup is resolved
  by absolute path (`/opt/homebrew/bin/pup`, then `/usr/local/bin/pup`; Alfred
  has no shell `PATH`). Errors are classified from pup's stderr
  (`pup::classify`); a 401 or expired session runs `pup auth refresh` once and
  retries once.
- `cache.rs` — stale-while-revalidate JSON cache per `Kind` (dashboards,
  notebooks, services 1 h; monitors 2 min; incidents 60 s) keyed by site (and
  env for services). Stale ⇒ served + detached `alfred-dd refresh <kind>` +
  Alfred `rerun`; missing ⇒ synchronous fetch; corrupted or other key ⇒
  missing. A lock file per kind prevents concurrent refreshes; a `.failed`
  marker stops a failing refresh from respawning on every rerun.
- `frecency.rs` — count + last open per `<kind>:<id>` in
  `$alfred_workflow_data/frecency.json`, decayed with a one-week half-life.
- `ranking.rs` — accent/case-insensitive matching (every term must match),
  tiers exact > prefix > word prefix > substring, then score, then a
  tie-breaker.
- `time.rs` / `percent.rs` — RFC 3339 parsing and "3d ago"; query-string
  encoding. No date or URL crate.
- `query.rs` — shared query syntax: free terms, `env:`, last-token time suffix
  (`-15m`, `-4h`, `-7d`; default 1 h), other `key:value` kept verbatim.
- `urls.rs` — every Datadog URL, pure. Logs / dashboards / notebooks / error
  tracking take `from_ts`/`to_ts`/`live`; traces and the service page take
  `start`/`end`/`paused`. Incidents by numeric public id.
- `dashboards.rs` / `notebooks.rs` — parsing into `Board`; `services.rs` —
  APM names + catalog definitions merged into `Service`; `monitors.rs` —
  `Monitor`, state order, env scope (`env:` tag or none); `incidents.rs` —
  `Incident`, commander from the `commander` facet, severity order;
  `sources.rs` — fetch
  (pagination), cache key, `load` / `refresh` per `cache::Target` (kind + env
  for services); `filters.rs` — one function per script filter.
- `clipboard.rs` — pure detection of trace / monitor / incident ids and
  Datadog URLs (ambiguous decimal ⇒ trace and monitor). `dd` checks its query
  first, then reads the clipboard once; never log the clipboard.
- `action.rs` — the `arg` protocol (`Action::to_arg` / `Action::parse`) and
  `execute`.
- `alfred.rs` — item builders; items set no `icon`, so every one shows the workflow's Datadog icon.

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
- **Icons**: `workflow/icon.png` is the Datadog logo, unmodified, fetched from
  the DataDog GitHub organization's avatar by `scripts/icon.sh` (committed, not
  rebuilt). Keep it unmodified and keep the trademark notice in `README.md`.
  Items set no `icon` (every item, errors included, shows the workflow icon;
  `tests/alfred_test.rs` checks it): states live in the subtitle.
- **Public repo**: fixtures and docs use neutral names (`checkout-api`,
  `team:payments`, `alice@example.com`); never commit real service, team,
  dashboard or person names.
- **Fixtures** (`tests/fixtures/pup/*.json`) are real `pup --no-envelope api`
  responses, trimmed and anonymized with every key kept. When a source changes,
  capture a new response and anonymize it the same way; never hand-write a
  shape.
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
