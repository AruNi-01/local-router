# Workspace Detection Design

## Goal

Replace the current file-scan autodetect flow with a workspace-aware detector that can:

- identify workspace roots across Node, Rust, Go, and Python ecosystems
- classify discovered members as `http_service`, `runnable_non_http`, `library`, or `tooling`
- emit high-confidence services into `localrouter.yaml`
- keep lower-confidence candidates available for future `needs review` UI

## Scope

First implementation slice:

- workspace roots
  - Node: `pnpm-workspace.yaml`, `package.json.workspaces`, `turbo.json`, `nx.json`, Bun workspaces
  - Rust: root `Cargo.toml` with `[workspace]`
  - Go: `go.work`, multi-`go.mod`
  - Python: `pyproject.toml` workspaces where detectable, plus multi-project roots
- runnable members
  - Web: Next.js, Nuxt, Astro, Remix, SvelteKit, Vite, Vue CLI / Vue SPA
  - Node HTTP backends: Nest, Fastify, Express, Hono, Koa
  - Rust HTTP backends: Axum, Actix, Warp, Rocket, Poem, Tide
  - Go HTTP backends: Gin, Fiber, Echo, Chi, stdlib `net/http`
  - Python HTTP backends: Django, FastAPI, Flask, Starlette
  - non-http runnable apps: Tauri, Electron, workers

## Detection Flow

1. Classify repository layout and package manager.
2. Discover candidate units from package manifests and entrypoints.
3. Score each candidate:
   - `high`: safe to emit directly
   - `review`: emit only for non-breaking display paths later
   - `low`: keep internal only
4. Convert only `http_service` and `runnable_non_http` candidates into manifest services.

## Runtime Mapping

- `http_service`
  - `protocol: http`
  - routable
- `runnable_non_http`
  - `protocol: none`
  - `route: none`
  - visible in UI, not proxied
- `library` and `tooling`
  - excluded from generated manifest

## Near-Term Constraints

- current dashboard does not yet expose `needs review`
- first slice should bias toward better precision on monorepos rather than maximal recall
- the detector should remain self-contained in `crates/localrouter-core/src/manifest.rs` until the model stabilizes
