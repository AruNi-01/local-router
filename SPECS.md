# LocalRouter Specs

Status: Draft
Last updated: 2026-06-25

## Purpose

LocalRouter is a daemon-first local development control plane. It gives each
project workspace a supervised set of services, stable local URLs, automatic
port isolation, health state, logs, and a shared API for the CLI, dashboard, and
future agent/editor integrations.

This document is the working specification for the next product and technical
shape of LocalRouter. `PLAN.md` explains the broader product direction; this
file defines the concrete behavior the implementation should converge on.

## Core Goals

1. Run the same project in multiple workspaces or git worktrees without port
   conflicts.
2. Give every routable service a deterministic URL that identifies workspace,
   service, and project.
3. Keep application code free from LocalRouter-specific imports or hard-coded
   ports.
4. Expose one source of truth for routes, process state, health, logs,
   dependencies, and project topology.
5. Support common stacks through adapters while preserving a raw command escape
   hatch.

## Non-Goals

- Remote deployment or production hosting.
- Replacing language-specific package managers or task runners.
- Kubernetes-style orchestration.
- Requiring Docker.
- Requiring application source changes for basic routing.

## System Model

LocalRouter manages these entities:

- Project: a repository or local app root.
- Workspace: a concrete working context, usually a git checkout or worktree.
- Service: a runnable unit declared in `localrouter.yaml` or detected from
  project files.
- Instance: one running process for a service inside one workspace.
- Route: a hostname mapped to a running instance.
- Dependency: a relationship from one service to another service in the same
  project.

The daemon is the source of truth. The CLI and dashboard read and mutate daemon
state through the local API.

## Routing Contract

Default workspace-aware route:

```text
<workspace>.<service>.<project>.localhost
```

Short alias when a project has exactly one active workspace:

```text
<service>.<project>.localhost
```

Examples:

```text
main.web.myapp.localhost
feature-login.api.myapp.localhost
web.myapp.localhost
```

The daemon returns full URLs including the proxy port when needed:

```text
http://feature-login.web.myapp.localhost:9730
```

Clients must use daemon-provided URLs instead of deriving them independently.

### Hostname Rules

- Each label is slugified to DNS-safe lowercase text.
- Invalid route labels are rejected at manifest validation time.
- Route collisions are explicit state, not silent overwrites.
- Active running routes win over stale routes when a conflict must be resolved.
- Future public-domain support should consider a single-label option such as
  `<workspace>--<service>--<project>.example.com` for ordinary wildcard DNS and
  TLS compatibility.

## Runtime Contract

When starting a service instance, LocalRouter allocates an internal loopback
port and injects:

```text
PORT=<allocated-port>
HOST=127.0.0.1
PUBLIC_URL=<daemon-returned-service-url>
LOCALROUTER_PORT=<allocated-port>
LOCALROUTER_HOST=127.0.0.1
LOCALROUTER_PUBLIC_URL=<daemon-returned-service-url>
```

Application code should use its existing framework configuration path:

- Vite/React: `VITE_*` variables or dev server proxy config.
- Next.js: `NEXT_PUBLIC_*` variables or rewrites.
- Backend services: normal `PORT` and `HOST` handling.
- Custom commands: `${PORT}`, `${HOST}`, and `${PUBLIC_URL}` templates.

LocalRouter-specific variables are for scripts, tooling, and advanced
integration. Business code should not need to import or depend on LocalRouter.

## Manifest Contract

Project configuration lives at `localrouter.yaml`.

Minimal shape:

```yaml
project: myapp
workspace:
  strategy: git-worktree
services:
  web:
    command: npm run dev
    cwd: apps/web
    protocol: http
    adapter: vite
    route: web
    depends_on:
      - api
  api:
    command: npm run api
    cwd: apps/api
    protocol: http
    adapter: express
    route: api
```

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
- `enabled`
- `language`

`route: none` disables public routing for a service but still allows process
supervision, logs, and health state.

`enabled` and `disabled` are mutually exclusive aliases for whether a service
definition participates in runtime operations. Use one or the other; manifests
that set both must fail validation instead of applying precedence rules.

## Adapter Contract

Adapters are responsible for making common frameworks bind to the allocated
port without requiring project-specific code changes.

Current adapter behavior should remain:

- Vite-like tools receive `--host 127.0.0.1 --port <PORT> --strictPort`.
- Next.js receives `--hostname 127.0.0.1 --port <PORT>`.
- FastAPI/Starlette receive `--host 127.0.0.1 --port <PORT>`.
- Django receives `127.0.0.1:<PORT>`.
- Express/Fastify/Nest/Hono/Koa rely on injected `PORT`.

Adapter rewrites must strip conflicting user-provided host and port flags before
adding LocalRouter-owned values.

## Service Discovery Contract

`depends_on` should become runtime behavior, not only graph metadata.

For each service instance, LocalRouter should inject peer service URLs for
dependencies in the same workspace:

```text
LOCALROUTER_SERVICE_API_URL=http://feature-login.api.myapp.localhost:9730
LOCALROUTER_SERVICE_API_PORT=<api-port>
```

The short variable name is derived from the service name:

- uppercase
- non-alphanumeric runs become `_`
- leading and trailing `_` are removed

Example: `api-server` becomes `API_SERVER`.

If two dependencies for the same service normalize to the same variable name,
the manifest must fail validation. For example, depending on both `api-server`
and `api.server` would make `LOCALROUTER_SERVICE_API_SERVER_URL` ambiguous, so
LocalRouter rejects that dependency set instead of overwriting either value.

For compatibility with existing app conventions, the manifest should allow env
template mapping:

```yaml
services:
  web:
    command: npm run dev
    depends_on: [api]
    env:
      VITE_API_BASE_URL: ${LOCALROUTER_SERVICE_API_URL}
```

This keeps application code generic:

```ts
fetch(`${import.meta.env.VITE_API_BASE_URL}/users`)
```

### Dependency Startup

When starting a service:

1. Resolve `depends_on` within the same project.
2. Start dependencies first if they are not already running.
3. Wait until dependency health is `healthy` or until the dependency reaches a
   timeout from daemon config `dependencyReadyTimeout`; the default is 30
   seconds.
4. Start the requested service with peer URL and port variables injected.

Cycles in `depends_on` must fail validation.

## Proxy Contract

The proxy must support:

- host-based HTTP forwarding
- WebSocket forwarding
- route miss errors that do not fall through to the API server
- preservation of content headers
- removal of hop-by-hop headers

The current HTTP proxy buffers the full request body through `to_bytes` before
forwarding with `reqwest`. Before calling the proxy production-ready, evaluate
streaming behavior for:

- large request bodies
- server-sent events
- long-lived streaming responses
- framework hot-reload endpoints

If these cases are important, replace buffered forwarding with a streaming
Hyper/Tower proxy path.

## API Surface

The local daemon API should expose:

- health and config
- projects
- workspaces
- services
- instances
- routes
- logs
- graph
- event stream

The dashboard and CLI should remain thin clients over the same API. They should
not implement separate route derivation, process state, or manifest parsing.

## State and Persistence

Durable state is stored in SQLite under the LocalRouter data directory.

The persisted state should include:

- daemon config
- project records
- workspace records
- service definitions
- instance records
- route records
- manifest snapshots

Runtime-only process facts such as live PID health must be refreshed on daemon
startup and reconciled with persisted state.

## Security Boundary

LocalRouter is local-first.

Default binding:

- API: `127.0.0.1:<api_port>`
- Proxy: `127.0.0.1:<proxy_port>`

If LAN or public exposure is added later, it must be explicit and include:

- host allowlist behavior
- authentication for API routes
- clear distinction between daemon API protection and proxied app protection
- warnings when exposing dev services

## Implementation Plan

### Phase 1: Spec Alignment

- Keep `SPECS.md` as the behavior source of truth.
- Audit README and PLAN for conflicting route, env, and adapter claims.
- Add tests for current documented behavior where missing.

### Phase 2: Peer Service Runtime

- Normalize service names into env-safe identifiers.
- Inject `LOCALROUTER_SERVICE_<NAME>_URL` and `_PORT` for dependencies.
- Render manifest `env` values after peer variables are known.
- Add cycle detection for `depends_on`.
- Start dependencies before dependents.

### Phase 3: Proxy Hardening

- Add tests for SSE, large body forwarding, and hot-reload WebSocket behavior.
- Decide whether buffered `reqwest` forwarding is acceptable.
- If not acceptable, introduce streaming proxy internals.

### Phase 4: Workspace Multiplicity

- Support multiple active workspaces for the same project without replacing the
  previous workspace during project registration.
- Keep short aliases only when unambiguous.
- Make route conflicts visible and actionable in CLI and dashboard.

### Phase 5: Public URL Strategy

- Decide whether LocalRouter should keep multi-label local hostnames only or add
  a single-label public alias format.
- Add `publicBaseUrl` or equivalent only after the local proxy model is stable.

## Open Questions

1. Should peer service env variables be injected for all services in the same
   workspace or only declared `depends_on` services?
2. Should `depends_on` wait for healthy state, listening port, or process start?
3. Should `route` default to service name for all HTTP services, or should some
   detected backend services default to private?
4. Should LocalRouter support HTTPS locally, or leave that to a future
   certificate/trust workflow?
5. Should public-domain routing use multi-label hostnames or a single
   hash-capped label?
