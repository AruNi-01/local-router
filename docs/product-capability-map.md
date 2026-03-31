# LocalRouter Product Capability Map

Date: 2026-03-31
Status: Working analysis

## Purpose

This document captures what LocalRouter is as a product, which capability layers are already implemented, and how a user is expected to move through the system from project onboarding to day-to-day usage.

It is intended as a product-facing companion to the implementation and design docs in this repository.

## Product Summary

LocalRouter is a daemon-first local development control plane.

It is built for repositories that have:

- multiple runnable services
- unstable local ports across restarts
- multiple branches or git worktrees active at the same time
- fragmented process state, logs, routes, and health across terminals and browser tabs

Instead of asking developers to remember which service is on which port today, LocalRouter:

- detects or loads project service definitions
- starts and supervises local processes
- allocates loopback ports
- assigns stable local route names
- proxies HTTP and WebSocket traffic
- exposes a shared state model to both CLI and dashboard

## Product Surfaces

LocalRouter currently has four visible product surfaces:

- `localrouterd`
  - local daemon
  - API server
  - local proxy
  - embedded dashboard host
- `localrouter`
  - CLI for importing projects, managing instances, and inspecting state
- dashboard
  - visual UI for overview, projects, routes, logs, graph, and settings
- `localrouter-core`
  - shared runtime, registry, manifest, route, health, and persistence logic

## Capability Map

### 1. Project Onboarding

Problem this solves:

- users should be able to point LocalRouter at a repo and get a working draft quickly

Current capabilities:

- import a project from CLI or dashboard
- infer a manifest when `localrouter.yaml` is missing
- persist imported project state in local SQLite storage
- rescan a project and refresh detected services

Current implementation notes:

- project import is already wired through daemon API, CLI, and dashboard
- autodetect covers Node, Python, Go, Java, and Rust repositories
- autodetect distinguishes between likely runnable services and lower-confidence candidates

Maturity:

- solid MVP
- strong enough for real usage

### 2. Service Definition and Manifest Management

Problem this solves:

- users need a stable, explicit description of what to run and how to route it

Current capabilities:

- parse and validate `localrouter.yaml`
- validate service fields such as command, protocol, adapter, route, healthcheck, and dependencies
- resolve relative `cwd` values against the project root
- write updated manifest content back to disk
- edit manifests from the dashboard

Current implementation notes:

- manifest validation is relatively strict
- dashboard supports both structured editing and raw YAML editing
- service metadata includes command, adapter, protocol, route, env, dependencies, and enabled state

Maturity:

- usable and product-visible
- stronger in validation than in advanced editing UX

### 3. Runtime Supervision

Problem this solves:

- developers need one place to start, stop, and restart local services without hand-managing ports and terminals

Current capabilities:

- allocate free loopback ports per instance
- inject runtime environment values such as `PORT`, `HOST`, and `PUBLIC_URL`
- spawn child processes with service-specific working directories
- stop and restart instances
- track PID, uptime, CPU, memory, exit code, and status reason

Current implementation notes:

- runtime supervision is one of the most complete parts of the product
- service adapters already rewrite or augment commands for common frameworks in several cases

Maturity:

- strong MVP
- central product value is already present

### 4. Stable Routing and Local Proxy

Problem this solves:

- service URLs should stay stable even when internal ports change

Current capabilities:

- generate stable hostnames based on project, workspace, and service
- expose shorter aliases when only one workspace is active
- proxy HTTP traffic to active targets
- proxy WebSocket traffic
- detect route conflicts and mark route state as `active`, `stale`, or `conflict`

Current implementation notes:

- this is the most differentiating capability in the product
- route generation is deterministic and tied to daemon-managed state

Maturity:

- highly visible and already compelling

### 5. Health, Logs, and Operational Visibility

Problem this solves:

- it is not enough to know that a process exists; users need to know whether it is actually healthy and why it is failing

Current capabilities:

- perform periodic HTTP healthchecks
- classify instance status as `healthy`, `starting`, `unhealthy`, `stopped`, or `unknown`
- capture stdout and stderr as daemon-managed log entries
- surface status reason strings in API and UI
- provide aggregated views of instances, routes, and recent logs

Current implementation notes:

- health and logs are already integrated into dashboard and CLI workflows
- log retention is currently in-memory only

Maturity:

- useful for day-to-day development
- not yet a deep observability system

### 6. Topology and Visual Understanding

Problem this solves:

- multi-service local environments are hard to reason about without a graph of relationships

Current capabilities:

- build a graph snapshot of projects, workspaces, service instances, and routes
- expose graph state through the API
- render an interactive graph in the dashboard
- show service dependency edges where they are declared

Current implementation notes:

- graph output is already more than decorative because it is built from live daemon state
- current graph rendering is an exploratory visualization rather than a deeply analytical tool

Maturity:

- good first slice
- likely to expand later with filtering, grouping, and richer interactions

### 7. Shared Control Plane for CLI and UI

Problem this solves:

- state should not diverge between CLI and dashboard

Current capabilities:

- daemon is the single source of truth
- CLI operates as a thin client over the daemon API
- dashboard uses the same API and invalidates views on daemon events
- daemon exposes a WebSocket event stream

Current implementation notes:

- this shared-state model is already real, not aspirational
- dashboard and CLI are aligned around the same state objects

Maturity:

- strong architectural foundation

### 8. Distribution and Productization

Problem this solves:

- a local developer tool only becomes useful at scale when installation and release are straightforward

Current capabilities:

- daemon serves an embedded dashboard build
- release artifacts exist in `dist/`
- install and release scripts are present
- Homebrew-related release assets exist

Current implementation notes:

- repository structure suggests this is already being treated as a distributable product
- versioned release outputs indicate an early but real release process

Maturity:

- early productization
- beyond internal prototype stage

## Current Maturity Assessment

Overall, LocalRouter appears to be beyond proof-of-concept and into early usable product territory.

What feels complete enough to use now:

- project import and manifest loading
- service start and stop flows
- stable local routing
- proxying
- health and route status visibility
- dashboard overview and core inspection views

What appears partially complete or still shallow:

- multi-workspace and multi-worktree management as a first-class user flow
- some daemon config fields that exist in models and UI but are not yet deeply wired into runtime behavior
- richer semantics for `enabled` and other manifest controls
- frontend test coverage
- advanced filtering and workflow polish

Practical maturity estimate:

- roughly early `v0.1` to pre-`v1`
- strong backend-centered MVP with meaningful user-facing functionality

## Typical User Path

This section describes the intended end-to-end product journey for a developer using LocalRouter.

### Stage 1. Enter a Project

The user is inside a repository that has one or more runnable services.

Typical entrypoints:

- `localrouter dev`
- `localrouter project add`
- dashboard import flow

Expected product behavior:

- daemon starts automatically if needed
- current project path is resolved
- project is registered or rescanned
- manifest is loaded from `localrouter.yaml` or generated by autodetect

### Stage 2. Discover Runnable Services

After import, the daemon converts manifest data into service definitions and instance rows.

Expected product behavior:

- project appears in the dashboard
- workspaces are shown
- services are visible with commands, adapter types, routes, and languages
- route definitions are generated even before all services are running

### Stage 3. Start the Local Environment

The user starts a whole project or selected services.

Typical entrypoints:

- `localrouter up`
- `localrouter dev`
- dashboard start button per instance

Expected product behavior:

- ports are allocated
- commands are rewritten when needed for supported adapters
- processes are spawned under supervision
- instances move into `starting`, then `healthy` or `unhealthy`
- routes become active when a running instance claims them

### Stage 4. Access Stable Local URLs

Once instances are running, the user stops caring about raw ports.

Expected product behavior:

- user opens a stable route URL from CLI or dashboard
- proxy forwards requests to the instance target
- WebSocket upgrades continue to work for compatible services
- if there is only one active workspace, shorter route aliases are available

### Stage 5. Observe and Diagnose

When something fails, the user should stay inside the LocalRouter workflow rather than dropping into ad-hoc shell scripts immediately.

Expected product behavior:

- dashboard overview shows global health and route counts
- project detail shows instances, ports, PID, URL, uptime, CPU, and memory
- logs page shows aggregated service logs
- route page shows route target and conflict state
- `localrouter doctor` gives a quick CLI summary of unhealthy instances or route conflicts

### Stage 6. Refine Configuration

When autodetect is not precise enough, the user edits project intent explicitly.

Expected product behavior:

- service definitions can be reviewed and refined in the dashboard
- raw YAML can be edited directly
- updates are validated and written back to `localrouter.yaml`
- subsequent rescans and runs use the saved manifest instead of relying only on autodetect

### Stage 7. Repeat Across Branches and Workspaces

This is the longer-term differentiator in the product story.

Expected ideal behavior:

- same project can be active in multiple worktrees
- each workspace gets isolated instances and route identities
- users can tell which branch or workspace owns which local URL

Current status:

- the model and naming strategy clearly aim at this
- implementation currently feels stronger on route naming and workspace identity capture than on full multi-worktree management workflows

## Gaps Between Product Intent and Current Implementation

### 1. Workspace Management Depth

Intent:

- treat workspaces as a first-class developer concept

Current gap:

- current code path mostly registers the current workspace context
- broader workspace switching and multi-worktree coordination remain less developed than the product plan suggests

### 2. Config Fields vs Runtime Behavior

Intent:

- global daemon settings should materially influence runtime behavior

Current gap:

- fields such as `autoDetect`, `hotReload`, `healthcheckInterval`, and `logLevel` are visible in config and settings UI
- some of these fields do not yet appear to drive much runtime logic

### 3. Manifest Semantics

Intent:

- service configuration should fully control what the daemon manages

Current gap:

- `enabled` and `disabled` exist in schema and UI
- runtime behavior does not yet appear to enforce every related semantic consistently

### 4. Frontend Confidence

Intent:

- dashboard should be a reliable operational surface

Current gap:

- dashboard is feature-rich
- automated frontend test coverage is still very thin relative to the amount of UI behavior already present

## Suggested Near-Term Product Priorities

If LocalRouter is moving toward a stronger `v1`, the highest-leverage product priorities appear to be:

1. complete the multi-workspace and multi-worktree user story
2. make daemon config fields fully effective in runtime behavior
3. tighten manifest-driven behavior, especially around service enablement and state transitions
4. improve dashboard testing and workflow polish
5. keep strengthening onboarding so autodetect gets users to a correct first run quickly

## Summary

LocalRouter is already a meaningful local developer platform, not just a route proxy or launch script.

Its clearest product strengths today are:

- daemon-centered shared state
- stable local routing
- supervised local service runtime
- visible health, logs, and topology

Its clearest unfinished areas are:

- deeper workspace lifecycle management
- stronger config-to-runtime closure
- broader polishing for reliability and product confidence
