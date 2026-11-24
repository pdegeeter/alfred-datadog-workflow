# Development

## Contributing

`main` is a protected branch:

- Changes land via pull requests — direct pushes are rejected (enforced for
  admins too).
- The CI checks `test` and `package` must be green before a PR can be merged.
- **All commits must be signed** (GitHub "require signed commits"). Configure
  SSH commit signing so your commits verify:

  ```sh
  git config gpg.format ssh
  git config user.signingkey ~/.ssh/<your-key>.pub
  git config commit.gpgsign true
  ```

  The signing key must be registered on your GitHub account as a *signing* key,
  and your commit email must be a verified GitHub email, otherwise the signature
  will not verify and the push/merge will be blocked.

## Prerequisites

- Rust (stable) — `rustup` recommended.
- [`powerpack-cli`](https://crates.io/crates/powerpack-cli): `cargo install powerpack-cli`.
- Alfred 5 with Powerpack, to actually run the workflow.

## Common tasks

```sh
cargo test --all                          # run the test suite (offline, no Datadog needed)
cargo clippy --all-targets -- -D warnings # lint (warnings are errors)
cargo fmt --all                           # format

./scripts/build.sh    # build release binary + package the .alfredworkflow
./scripts/deploy.sh   # build + update the installed workflow in place (imports it the first time)
./scripts/deploy.sh --import  # build + open the .alfredworkflow to import it
./scripts/icon.sh     # regenerate workflow/icon.png from a macOS system icon
```

The workflow icon (`workflow/icon.png`) is a committed asset converted from a
macOS system icon with `sips`. It is not regenerated at build time.

`scripts/deploy.sh` finds the installed workflow in Alfred's preferences
(`current` in `~/Library/Application Support/Alfred/prefs.json`, or
`$ALFRED_PREFERENCES`) by its bundle id and copies the new binary,
`info.plist` and icon over it; Alfred reloads the folder. When the workflow is
not installed yet, it opens the `.alfredworkflow` for import instead. It does
not symlink (`powerpack link`): Alfred 6 turns the link into a stale copy, and
re-importing over an existing install fails.

`scripts/build.sh` syncs `workflow/info.plist`'s `<version>` from `Cargo.toml`
before packaging, so the version shown in Alfred always matches the crate.

## Manual smoke test (without Alfred)

The binary speaks Alfred's JSON on stdout, so you can exercise it directly:

```sh
cargo build
B=./target/debug/alfred-dd

# Actions take the `arg` of an item (see docs/architecture.md):
$B run "open-url https://app.datadoghq.eu/logs"
$B run "copy checkout-api"
```

## Testing strategy

All logic is pure and unit-tested; every process spawn goes through the
`CommandRunner` trait and is replaced in tests by `FakeRunner`
(`tests/common/mod.rs`), which returns queued canned outputs per program and
records every call. The whole suite runs offline, without Datadog. Time is
injected (`common::NOW_MS`).

| Test file                | Covers |
| ------------------------ | ------ |
| `tests/config_test.rs`   | `DD_SITE` / `DD_ENV` defaults, blank values, trimming. |
| `tests/action_test.rs`   | `arg` round-trip of every action, invalid args, execution (open, clipboard, Alfred search). |
| `tests/plist_test.rs`    | `workflow/info.plist`: every keyword and the subcommand it runs, every modifier connected to the action, the readme, the `DD_SITE` / `DD_ENV` defaults (uses macOS `plutil`). |

## Project layout

```
.
├── src/                    Rust sources (see docs/architecture.md)
├── tests/                  Integration tests + shared FakeRunner
├── workflow/
│   ├── info.plist          Alfred workflow definition (the binary is added at build time)
│   └── icon.png            Workflow icon, converted from a macOS system icon
├── scripts/
│   ├── build.sh            Build + package the .alfredworkflow
│   ├── deploy.sh           Build + install into Alfred
│   └── icon.sh             Regenerate workflow/icon.png
├── docs/                   This documentation
├── .github/workflows/      CI and release-please pipelines
├── release-please-config.json
└── .release-please-manifest.json
```

## Editing the Alfred graph

`workflow/info.plist` defines one Script Filter per keyword, each connected —
for every modifier — to one Run Script, connected to a Post Notification. If
you edit the workflow in Alfred's UI, copy the updated `info.plist` from the
installed workflow folder back into `workflow/` before committing (the next
`./scripts/deploy.sh` overwrites the installed one). Validate it with:

```sh
plutil -lint workflow/info.plist
```
