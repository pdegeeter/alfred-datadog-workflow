# Alfred Datadog Workflow

An [Alfred](https://www.alfredapp.com/) workflow, written in Rust, to navigate
[Datadog](https://www.datadoghq.com/) from Alfred. It is **read-only**: every
action opens a URL, copies text or refreshes a local cache; nothing is ever
written to Datadog.

## Configuration

Set these in the workflow configuration (Alfred Preferences → Workflows →
Datadog → Configure Workflow):

| Variable  | Default        | Meaning |
| --------- | -------------- | ------- |
| `DD_SITE` | `datadoghq.eu` | Site of your Datadog organization (`datadoghq.com`, `us3.datadoghq.com`, …). |
| `DD_ENV`  | `production`   | Default environment for services, logs, traces and monitors. |

## Install

1. Download `alfred-dd.alfredworkflow` from the
   [latest release](https://github.com/pdegeeter/alfred-datadog-workflow/releases/latest).
2. Double-click it to import into Alfred.

Requires Alfred 5 with the Powerpack, on Apple Silicon (arm64).

## Development

See [docs/development.md](docs/development.md) for building, testing and
deploying locally, [docs/architecture.md](docs/architecture.md) for how it
works, and [docs/release.md](docs/release.md) for the release process.

## License

MIT
