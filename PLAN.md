# LocalRouter Plan

Date: 2026-03-08
Status: Draft
Product Shape: Daemon-first local development platform

## 1. Product Summary

LocalRouter is a local development runtime for parallel worktrees and multi-service projects.

It solves:

- stable local URLs for every service and worktree
- automatic port isolation across projects, branches, and workspaces
- cross-language service startup and supervision
- shared service discovery for humans, CLIs, IDEs, and AI agents
- visual understanding of projects, workspaces, services, routes, and dependencies

LocalRouter is not only a reverse proxy. It is a daemon-managed local control plane with:

- a background daemon
- a CLI
- a local reverse proxy
- a service runner and adapter system
- a Web UI dashboard
- a graph model for project and workspace topology

## 2. Product Goals

### Primary goals

- Make parallel worktree development safe and boring.
- Remove manual port management.
- Give every service a stable, named local URL.
- Support heterogeneous stacks: Node.js, Python, Go, Rust, Java, and custom commands.
- Make the current local environment visible at a glance.

### Secondary goals

- Provide machine-readable service metadata for agents and editor integrations.
- Make logs, health, routes, and service relationships easy to inspect.
- Support team conventions without requiring project-specific task runners.

### Non-goals for v1

- Remote deployment
- Cloud-hosted service registry
- Kubernetes-like orchestration
- Full IDE plugin suite before core daemon is stable

## 3. Core Concepts

- Project: a repository or local app root
- Workspace: a concrete working context, usually a git worktree or checkout
- Service: a runnable unit such as web, api, worker, db proxy, docs, or cron
- Route: a stable local URL mapped to a service instance
- Instance: one running process for one service inside one workspace
- Graph: the relationship model between projects, workspaces, services, and routes
- Adapter: logic that knows how to launch or parameterize a service kind

## 4. Product Surface

### Daemon

The daemon is the source of truth. It maintains:

- process registry
- route registry
- port allocation state
- workspace identity
- health state
- logs metadata
- graph state

It exposes:

- local HTTP API
- local WebSocket stream for live events
- optional local Unix socket for trusted CLI traffic

### CLI

The CLI is the operator interface.

Core commands:

- `localrouter up`
- `localrouter down`
- `localrouter ps`
- `localrouter logs <service>`
- `localrouter open <service>`
- `localrouter doctor`
- `localrouter graph`
- `localrouter project add`
- `localrouter workspace use`

The CLI should work even when the user does not use npm scripts, just, make, cargo aliases, or custom wrappers.

### Proxy

The proxy provides stable local domains such as:

- `web.myapp.localhost`
- `api.myapp.localhost`
- `docs.myapp.localhost`
- `feat-login.api.myapp.localhost`

Responsibilities:

- route registration and removal
- HTTP and WebSocket forwarding
- host-based routing
- optional TLS for local HTTPS later

### Web UI

The dashboard is the main visual product surface.

Primary views:

- Projects list
- Project detail
- Workspace detail
- Service instance detail
- Route inspector
- Logs viewer
- Graph view

Graph requirements:

- show Project -> Workspace -> Service hierarchy
- show service-to-service dependencies
- show route bindings
- show runtime health and process status
- support filtering by project, workspace, route, health, and language

## 5. Configuration Model

LocalRouter should support two layers:

### A. Auto-detection

Used for onboarding and best-effort setup.

Detect from:

- `package.json`
- `Cargo.toml`
- `go.mod`
- `pyproject.toml`
- `pom.xml` / `build.gradle`
- Docker Compose files

Auto-detection is helpful but not authoritative.

### B. Explicit manifest

Final source of project intent.

Proposed file:

- `localrouter.yaml`

Example shape:

```yaml
project: myapp

workspaces:
  strategy: git-worktree

services:
  web:
    command: next dev --port ${PORT}
    protocol: http
    route: web
    healthcheck: http://127.0.0.1:${PORT}

  api:
    command: cargo run --bin api -- --port ${PORT}
    protocol: http
    route: api
    healthcheck: http://127.0.0.1:${PORT}/healthz

  worker:
    command: python worker.py
    route: none
```

Key design rule:

- no project-specific task runner is required
- custom commands are always supported
- adapters only reduce boilerplate

## 6. Adapter Strategy

Adapters exist because not every ecosystem respects a generic `PORT` env var.

Adapter responsibilities:

- inject port and host values
- rewrite or append known flags
- generate public URL env vars
- define healthcheck defaults
- parse structured startup success when possible

Initial adapter targets:

- Next.js / Vite / generic Node
- Uvicorn / FastAPI / generic Python
- Rust custom command
- Spring Boot / JVM app
- Go custom command

Fallback path:

- raw command adapter with `${PORT}`, `${HOST}`, `${PUBLIC_URL}` interpolation

## 7. Naming and Routing Strategy

Default route format:

- `<service>.<project>.localhost`

Worktree-aware route format:

- `<workspace>.<service>.<project>.localhost`

Examples:

- `main.web.atmos.localhost`
- `feat-auth.api.atmos.localhost`
- `docs.localrouter.localhost`

Naming rules:

- deterministic
- human-readable
- collision-safe
- overridable

## 8. Runtime Architecture

### Internal components

- Registry service
- Port allocator
- Process supervisor
- Proxy manager
- Workspace resolver
- Healthcheck engine
- Event bus
- Graph builder
- Persistence layer

### Persistence

Store local durable state for:

- known projects
- known workspaces
- last route assignments
- service definitions
- daemon state

Do not persist:

- long-lived logs in v1
- full terminal recordings

## 9. Web UI Information Architecture

### Home dashboard

- all active projects
- active workspaces
- running services
- unhealthy services
- route conflicts

### Project page

- project metadata
- workspace list
- service catalog
- default graph

### Workspace page

- branch/worktree identity
- running services
- route list
- logs and health
- workspace graph slice

### Service page

- command
- adapter
- assigned port
- public URL
- healthcheck
- dependencies
- logs
- restart controls

### Graph page

- force-directed or layered graph
- nodes: project, workspace, service, route
- edges: contains, depends_on, exposes, proxies_to

## 10. API Design

Daemon API should be local-first and event-driven.

Minimum API groups:

- `/projects`
- `/workspaces`
- `/services`
- `/instances`
- `/routes`
- `/graph`
- `/logs`
- `/health`
- `/events`

Event stream should include:

- service_started
- service_stopped
- service_failed
- health_changed
- route_registered
- route_removed
- workspace_detected

## 11. Security Model

This is a local tool, but it still needs guardrails.

- daemon only binds to localhost by default
- explicit trust boundary between CLI/UI and daemon
- no remote access by default
- no shell interpolation surprises
- manifest parsing must avoid arbitrary code execution
- route names must be sanitized

## 12. Implementation Phases

### Phase 1: Daemon core

- daemon process
- registry
- port allocator
- process supervisor
- raw command execution
- local persistence

### Phase 2: Proxy and routing

- host-based local proxy
- route registration
- HTTP and WebSocket forwarding
- stable local URL generation

### Phase 3: Manifest and adapters

- `localrouter.yaml`
- project detection
- adapter API
- first-party adapters for major stacks

### Phase 4: CLI completeness

- lifecycle commands
- logs
- open
- doctor
- graph export

### Phase 5: Web UI

- dashboard
- project/workspace/service views
- live updates over WebSocket
- graph explorer

### Phase 6: Graph intelligence

- dependency edges
- route-to-service edges
- workspace comparison
- conflict detection

### Phase 7: Agent and editor integration

- machine-readable context endpoint
- CLI output modes for agents
- editor deep links

## 13. MVP Cut vs Final Shape

Even though the product is being designed for the final daemon-first form, the first shippable cut should still be narrower:

- daemon
- proxy
- raw command runner
- manifest
- CLI basics
- minimal dashboard

The full graph intelligence, deep adapters, and editor integrations should be built on top of the same stable daemon primitives.

## 14. Risks

- adapter explosion across frameworks
- users expecting zero-config for custom stacks
- keeping route naming deterministic across worktrees
- process lifecycle edge cases on macOS and Linux
- graph complexity becoming noisy instead of useful

## 15. Success Criteria

- A developer can run two or more worktrees of the same project with no manual port selection.
- A developer can open every service via named local domains.
- A mixed-language stack can be started from a manifest without project-specific task runners.
- The dashboard accurately reflects project/workspace/service state in real time.
- An agent can query the daemon and unambiguously find the correct local service URL.

## 16. Recommended Next Deliverables

- architecture decision record for daemon/process model
- `localrouter.yaml` schema draft
- CLI command spec
- daemon API spec
- Web UI wireframes for dashboard and graph
- adapter contract spec
