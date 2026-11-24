# Alfred Datadog Workflow

An [Alfred](https://www.alfredapp.com/) workflow, written in Rust, to navigate
[Datadog](https://www.datadoghq.com/) from Alfred. It is **read-only**: every
action opens a URL, copies text or refreshes a local cache; nothing is ever
written to Datadog.

```
ddd checkout -4h
 ┌──────────────────────────────────────────────────────────┐
 │ Checkout API overview                                      │
 │   alice@example.com · Dashboard · modified 4d ago          │
 │ Checkout incident review                                   │
 │   Alice Martin · Notebook (investigation) · modified 6d ago│
 └──────────────────────────────────────────────────────────┘
```

## Usage

### Query syntax

Every keyword shares the same syntax:

- **free terms** match names, case- and accent-insensitively; every term must
  match. Exact names first, then names starting with the query, then words
  starting with each term, then anything containing them; then the objects you
  open most often (and most recently), then the most recently modified.
- **`env:<value>`** overrides `DD_ENV` for that query.
- **a time suffix** as the last token — `-15m`, `-1h`, `-4h`, `-24h`, `-7d`
  (any `-<n>m|h|d`) — sets the time range of the links (default: the last
  hour, live).
- other **`key:value`** tokens are kept verbatim for logs / traces queries.

### Dashboards and notebooks — `ddd`

Lists your dashboards and notebooks. `nb:` restricts to notebooks, `db:` to
dashboards (`ddd nb:checkout`).

- <kbd>↩</kbd> open it
- <kbd>⌘</kbd><kbd>↩</kbd> open it on the query's time range (default the last
  hour, live)
- <kbd>⌥</kbd><kbd>↩</kbd> copy its URL
- <kbd>⇧</kbd><kbd>↩</kbd> open the dashboard (or notebook) list in Datadog,
  searched for the query

With no match, the first item searches the dashboard list in Datadog.

### APM services — `dds`

Lists the services APM saw in the env over the last day, plus the services
declared in the Software Catalog (marked "no traces in <env>"), with their
team and, when the catalog has them, languages and type. `env:<value>` picks
another env; `team:<name>` keeps that team's services. Every link uses the
env and the time range of the query.

- <kbd>↩</kbd> service page
- <kbd>⌘</kbd><kbd>↩</kbd> its logs (`service:<name> env:<env>`)
- <kbd>⌥</kbd><kbd>↩</kbd> its traces in the Traces Explorer
- <kbd>⌃</kbd><kbd>↩</kbd> its Error Tracking issues
- <kbd>⇧</kbd><kbd>↩</kbd> its monitors
- <kbd>⇧</kbd><kbd>⌘</kbd><kbd>↩</kbd> copy the service name

### Cache and refresh

Lists are cached locally and served instantly; when a list is older than its
lifetime (dashboards, notebooks, services: 1 hour; monitors: 2 minutes;
incidents: 1 minute) it is shown at once and refreshed in the background,
and Alfred updates the results when the refresh lands. The last item of each
list, **Refresh …**, fetches it again right away.

## Requirements

- [pup](https://github.com/DataDog/pup), Datadog's CLI, logged in:

  ```sh
  brew install datadog-labs/pack/pup
  DD_SITE=datadoghq.eu pup auth login
  ```

  The workflow reads Datadog through `pup api` with your OAuth session; it
  refreshes the session when it expires and tells you to run
  `pup auth login` when that is not enough.

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
