# LocalRouter

LocalRouter is a local control plane for development services.

It runs a local daemon, supervises project processes, assigns stable local routes, and exposes the same state to both a dashboard and a CLI.

The daemon now serves the dashboard itself, so end users do not need to run the frontend source separately.

Current scope:

- macOS and Linux
- local daemon only
- local HTTP API on `127.0.0.1`
- local proxy routes on `.localhost`

## Monorepo Layout

```text
.
├── apps/
│   ├── dashboard/         # React dashboard
│   ├── localrouter-cli/   # CLI client
│   └── localrouterd/      # daemon binary
└── crates/
    └── localrouter-core/  # shared Rust backend/core logic
```

`localrouterd` is the source of truth.

- The dashboard reads and mutates daemon state through the local API.
- The CLI is a thin client over the same API.
- The core package owns manifest parsing, persistence, process supervision, route generation, health checks, logs, and graph state.

## Requirements

- Rust toolchain
- Node.js 18+
- npm
- `git` if you want branch/workspace names detected automatically

## Quick Start

### Fastest path

If you already have `localrouter` and `localrouterd` installed, go to your project directory and run:

```bash
localrouter dev
```

This will:

- start `localrouterd` automatically if needed
- register or rescan the current project
- start the project's instances
- open the embedded dashboard unless you pass `--no-open`

The embedded dashboard is served from the daemon at:

```text
http://127.0.0.1:9731/
```

### Install without source code

Install the latest release binaries:

```bash
curl -fsSL https://raw.githubusercontent.com/AruNi-01/local-router/main/scripts/install/install.sh | sh
```

Then:

```bash
localrouter dev
```

The installer downloads the matching release archive and verifies its SHA-256 checksum before installing.

Maintainers should follow [RELEASING.md](./RELEASING.md) for tag, asset, and tap update steps.

### 1. Build the workspace

This is only needed if you are developing from source.

```bash
cargo build
```

### 2. Import a project

From the project directory you want to proxy:

```bash
./target/debug/localrouter project add
```

Or import an explicit path:

```bash
./target/debug/localrouter project add /absolute/path/to/your/project
```

If the daemon is not running yet, the CLI will start it automatically.

Default daemon endpoints:

- API server: `http://127.0.0.1:9731/v1`
- Proxy server: `http://127.0.0.1:9730`

If you want to manage the daemon explicitly:

```bash
./target/debug/localrouter daemon start
./target/debug/localrouter daemon status
./target/debug/localrouter daemon stop
```

### 3. Start a project or service

Start all matching instances:

```bash
./target/debug/localrouter up my-project
```

Start a specific service:

```bash
./target/debug/localrouter up dashboard
```

Inspect running instances:

```bash
./target/debug/localrouter ps
./target/debug/localrouter routes
./target/debug/localrouter logs dashboard
```

Open a routable service in the browser:

```bash
./target/debug/localrouter open dashboard
```

Stop or restart:

```bash
./target/debug/localrouter down my-project
./target/debug/localrouter restart dashboard
```

### 4. Start the dashboard

For end users, this step is not required because the daemon already serves the built dashboard.

```bash
cd apps/dashboard
npm install
npm run dev
```

By default the dashboard talks to `http://127.0.0.1:9731/v1`.

If your daemon API is on another address:

```bash
VITE_LOCALROUTER_API=http://127.0.0.1:9731/v1 npm run dev
```

If the project has a `localrouter.yaml`, that file is used.

If not, LocalRouter will auto-detect a manifest from common project files and store the generated manifest in daemon state until you save a real one.

## How Routing Works

When a service has a route, the daemon gives it:

- an internal process port
- a stable public URL
- one or more local hostnames through the proxy

Default route pattern:

```text
<workspace>.<service>.<project>.localhost
```

If there is only one active workspace for the project, LocalRouter also adds a short alias:

```text
<service>.<project>.localhost
```

The daemon returns the final URL, including proxy port when needed, for example:

```text
http://main.dashboard.local-router.localhost:9730
http://dashboard.local-router.localhost:9730
```

The dashboard and CLI consume this value directly. They do not derive URLs themselves.

## Project Manifest

Project configuration lives in `localrouter.yaml` at the project root.

Minimal example:

```yaml
project: local-router
workspace:
  strategy: git-worktree
services:
  dashboard:
    command: npm run dev -- --host ${HOST} --port ${PORT}
    cwd: apps/dashboard
    protocol: http
    adapter: vite
    route: dashboard
    healthcheck: http://127.0.0.1:${PORT}
```

Supported top-level keys:

- `project`
- `workspace.strategy`
- `proxy.disabled`
- `services`

Supported service fields:

- `command`
- `cwd`
- `protocol`
- `adapter`
- `route`
- `healthcheck`
- `env`
- `depends_on`
- `disabled`
- `language`

Runtime substitutions available in `command`:

- `${PORT}`: allocated internal port
- `${HOST}`: currently `127.0.0.1`
- `${PUBLIC_URL}`: final external URL returned by the daemon

Behavior notes:

- `route: none` disables public routing for a service
- HTTP services default to `healthcheck: http://127.0.0.1:${PORT}` when omitted
- `disabled: true` keeps the service definition but marks it disabled in config
- adapter defaults are inferred from the service name and command

## Auto-Detection

If `localrouter.yaml` is missing, LocalRouter tries to generate one:

- `package.json` with a `dev` script inside the project tree
- root `Cargo.toml`
- fallback generic Python HTTP server

Current adapter inference covers:

- `vite`
- `nextjs`
- `uvicorn`
- `cargo-bin`
- `generic`
- `worker`

Auto-detection is a bootstrap path. For stable behavior, save a real `localrouter.yaml`.

## Global Daemon Config

The daemon exposes editable global config through the dashboard Settings page and the `/v1/config` API.

Important fields:

- `apiPort`: default `9731`
- `proxyPort`: default `9730`
- `dnsSuffix`: default `.localhost`
- `logLevel`: default `info`
- `healthcheckInterval`: default `10`
- `autoDetect`: default `true`
- `hotReload`: default `false`

If you change ports or DNS suffix, restart the daemon and refresh the dashboard.

## CLI Reference

Human-readable output:

```bash
./target/debug/localrouter daemon start|stop|status
./target/debug/localrouter project add <path>
./target/debug/localrouter project list
./target/debug/localrouter project remove <id|name|path>
./target/debug/localrouter ps
./target/debug/localrouter up [target]
./target/debug/localrouter down [target]
./target/debug/localrouter restart <target>
./target/debug/localrouter logs <target>
./target/debug/localrouter routes
./target/debug/localrouter open <target>
./target/debug/localrouter doctor
./target/debug/localrouter graph
./target/debug/localrouter dev [path] [--no-open]
```

JSON output:

```bash
./target/debug/localrouter --json ps
./target/debug/localrouter --json routes
./target/debug/localrouter --json graph
```

Target matching in CLI commands is fuzzy across:

- project id or name
- service id or name
- workspace id or name
- instance id
- instance URL substring

## Dashboard Workflow

The dashboard covers the same daemon state as the CLI:

- `Overview`: instance and route status
- `Projects`: import, rescan, remove
- `Routes`: inspect, filter, copy, open
- `Logs`: aggregated daemon-managed service logs
- `Graph`: topology snapshot
- `Settings`: global daemon config and per-project manifest editing

## Persistence

Daemon state is stored in a local SQLite file.

Typical locations:

- macOS: `~/Library/Application Support/localrouter/state.sqlite3`
- Linux: `~/.local/share/localrouter/state.sqlite3`

The daemon also writes a PID file in the same local data directory.

Persisted state includes:

- projects
- workspaces
- service definitions
- instance summaries
- routes
- manifest snapshots
- daemon config

Logs are kept in memory only.

## Troubleshooting

### Daemon is not reachable

Check:

```bash
./target/debug/localrouter daemon status
curl http://127.0.0.1:9731/v1/health
```

### Route exists but browser cannot connect

Check:

```bash
./target/debug/localrouter ps
./target/debug/localrouter routes
./target/debug/localrouter logs <service>
```

Common causes:

- process exited immediately
- healthcheck is failing
- route is in `conflict`
- service command is not honoring `${PORT}`

### CLI is talking to the wrong daemon

Override the API base:

```bash
LOCALROUTER_API=http://127.0.0.1:9731/v1 ./target/debug/localrouter ps
```

### Project import looks wrong

Rescan after adding or fixing `localrouter.yaml`:

```bash
./target/debug/localrouter project list
./target/debug/localrouter project remove /absolute/path/to/project
./target/debug/localrouter project add /absolute/path/to/project
```

## Development Notes

- This is a monorepo. Do not put daemon/core code into `apps/dashboard`.
- `apps/localrouterd` is the daemon binary.
- `apps/localrouter-cli` is the API client.
- `crates/localrouter-core` is the shared backend implementation.
- The dashboard should consume daemon data, not invent process or route state on the client.
