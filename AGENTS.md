# AGENTS.md

## Cursor Cloud specific instructions

### Overview

LocalRouter is a Rust + TypeScript monorepo: a local control plane daemon (`localrouterd`), a CLI (`localrouter-cli`), a shared core library (`localrouter-core`), and a React dashboard (`apps/dashboard`). No external databases or Docker required — SQLite is bundled.

### Prerequisites

- **Rust 1.85+** (edition 2024). The VM ships with an older Rust; run `rustup update stable && rustup default stable` if `cargo build` fails with `feature edition2024 is required`.
- **Node.js 18+** and **npm** for the dashboard.
- **`just`** task runner (optional convenience; install via `curl -fsSL https://just.systems/install.sh | sudo bash -s -- --to /usr/local/bin`).

### Build order (important)

The daemon binary embeds the built dashboard via `include_dir!`. You must build the dashboard **before** building the Rust workspace:

```bash
cd apps/dashboard && npm run build   # produces apps/dashboard/dist/
cd /workspace && cargo build         # embeds dist/ into localrouterd
```

### Running services

| Service | Command | Default port |
|---------|---------|-------------|
| Daemon (`localrouterd`) | `cargo run -p localrouterd` | API: 9731, Proxy: 9730 |
| CLI | `cargo run -p localrouter-cli -- <args>` | — |
| Dashboard dev server | `cd apps/dashboard && npm run dev` | 5173 |

The daemon serves an embedded copy of the dashboard at `http://127.0.0.1:9731/`. The Vite dev server at port 5173 is only needed for frontend development and talks to the daemon API via `VITE_LOCALROUTER_API`.

### Testing

- **Rust**: `cargo test` (16 tests in `localrouter-core`)
- **Dashboard**: `npm run test` in `apps/dashboard` (vitest)
- **Lint**: `npm run lint` in `apps/dashboard` (ESLint; pre-existing warnings/errors exist)
- **Format**: `cargo fmt --all --check` (pre-existing formatting diffs exist)

See `justfile` for convenience recipes: `just test`, `just build`, `just fmt`, etc.

### Gotchas

- When auto-detecting this repo as a project, the daemon will register `localrouterd` as a service. Starting it via `localrouter up` will fail with "Address already in use" if the daemon is already running — this is expected.
- SQLite state is stored at `~/.local/share/localrouter/state.sqlite3`. Use `localrouter reset all` to clear it.
