# ATC Training Technical Design Document

## Purpose

This document translates the frozen v1 product design into an implementation-oriented technical plan.

It is intended to guide:

- Rust workspace structure
- crate and module boundaries
- server runtime design
- client UI/runtime design
- shared protocol and domain types
- phased delivery of the first playable version

This document is subordinate to:

- [atc-training-software-design.md](/Users/sasha/dev/atc-trainer/docs/atc-training-software-design.md)
- [atc-training-protocol-spec.md](/Users/sasha/dev/atc-trainer/docs/atc-training-protocol-spec.md)

## Technical Goals

The v1 implementation should achieve these concrete goals:

- run on Windows
- use Rust end-to-end
- use `axum` on the server
- use `iced` for the desktop client
- support up to `2` trainers and `1` student in one session
- keep the server authoritative for session and aircraft state
- keep the client simple, informative, and operationally dense
- support real-time synchronization over WebSocket
- keep all runtime state in memory
- write human-readable file logs for all accepted actions

## System Overview

The system is split into three Rust crates in one workspace:

```text
atc-trainer/
  Cargo.toml
  crates/
    atc-shared/
    atc-server/
    atc-client/
    atc-test-support/
```

High-level flow:

1. trainer authenticates with magic hash through the client
2. server creates a session and student position
3. student joins using session hash
4. client upgrades to WebSocket for live session state
5. trainers and student operate on one shared authoritative session state
6. server serializes accepted actions, updates session state, broadcasts results, and writes log lines

## Architecture Principles

- server authority over all canonical state
- deterministic per-session event ordering
- shared protocol/domain crate to prevent drift
- local UI state separated from synchronized session state
- small v1 surface area, no speculative subsystems
- replay-readiness in log and event structure, even without replay UI

## Workspace Design

### `atc-shared`

Responsibility:

- domain enums and identifiers
- DTOs for HTTP and WebSocket
- scenario file structures
- sector/scenery structures
- deterministic validation helpers

Must not contain:

- server runtime logic
- client UI code
- network connection code
- simulation loop code

Suggested modules:

- `ids`
- `role`
- `session`
- `aircraft`
- `sector`
- `scenario`
- `protocol`
- `validation`

### `atc-server`

Responsibility:

- `axum` HTTP API
- WebSocket session handling
- trainer/student authentication
- in-memory session runtime
- authoritative aircraft movement and state mutation
- scenario loading
- log writing

Suggested modules:

- `config`
- `app`
- `http`
- `ws`
- `auth`
- `sessions`
- `runtime`
- `aircraft`
- `scenario`
- `sector`
- `logging`
- `error`

### `atc-client`

Responsibility:

- `iced` application shell
- start screen and session screen
- sector rendering
- traffic manager UI
- trainer toolbox UI
- HTTP and WebSocket connectivity
- local view state such as focus, zoom, pan, and draft path editing

Suggested modules:

- `app`
- `screen`
- `network`
- `session`
- `canvas`
- `traffic_manager`
- `trainer_tools`
- `theme`
- `storage`
- `dialog`

### `atc-test-support`

Responsibility:

- headless protocol harness used by integration tests
- in-process server builder bound to an ephemeral port
- typed HTTP and WebSocket test client built on `atc-shared` DTOs
- scenario and METAR fixtures
- simulated-time helpers for movement ticks and cooldown windows
- temporary log sinks for per-test isolation

Must not contain:

- `iced` dependencies
- production runtime logic
- any `#[cfg(test)]`-only code that belongs inside server or client tests

Suggested modules:

- `server` — `TestServer` spawning an `axum::Router` on a random port
- `client` — `TestClient` wrapping `reqwest` and `tokio-tungstenite`
- `fixtures` — scenario and session presets
- `time` — simulated-time helpers
- `logs` — temp-directory log capture

## Server Design

### Runtime Model

Each session owns:

- one authoritative in-memory session state
- one serialized event-processing loop
- zero or more WebSocket subscribers
- one action log sink

The server should process all trainer and student actions for a session in one ordered stream.

That model is preferred for v1 because it:

- simplifies concurrency
- keeps multi-trainer ordering deterministic
- makes logging straightforward
- aligns naturally with replay in v2

### Session State

Each session should contain at least:

- session metadata
- session status: `draft | waiting_for_student | running | paused | ended`
- connected participants
- student position: `gnd | twr`
- active runway
- session METAR
- loaded scenario reference
- aircraft collection
- connection cooldown state for trainer disconnect handling

### Aircraft Runtime State

Each aircraft should include:

- stable `aircraft_id`
- template/category
- callsign
- departure airfield
- destination airfield
- route text
- squawk mode
- assigned squawk code
- assigned runway
- assigned SID
- assigned altitude
- next waypoint
- current world position
- current speed
- current altitude
- current status
- assumption state
- active/draft path state

### Path Execution Model

Trainer-defined path execution in v1:

- trainer selects an aircraft
- trainer enters draft mode
- trainer places path points in world coordinates
- trainer may undo the last point only
- trainer presses `Finish pathing`
- trainer edits speed and altitude / `GND` per point
- trainer launches the path
- server moves the aircraft along straight-line segments between points

Important v1 rules:

- no protective validation beyond basic decoding for trusted trainer actions
- once launched, path geometry cannot be edited in place
- aircraft must be stopped on ground before a new path replaces the active one
- launched path visibility is trainer-only

### HTTP Responsibilities

HTTP is used only for:

- trainer authentication
- session creation
- student join
- scenario/scenery metadata bootstrap if needed

It should not carry live simulation updates.

### WebSocket Responsibilities

WebSocket is used for:

- initial full session snapshot
- trainer commands
- student aircraft updates
- incremental session events
- state resynchronization when required

### Logging

The logging subsystem should write one human-readable line per accepted event.

Required fields:

- timestamp
- session id
- actor
- aircraft id when applicable
- action
- concise result summary

Recommended style:

```text
2026-04-17T18:42:11Z session=s_01 actor=trainer:t_02 aircraft=ac_07 action=set_path result="3 points, launch pending"
2026-04-17T18:42:19Z session=s_01 actor=student:position_gnd aircraft=ac_07 action=update result="status=taxi runway=18 sid=east1a sqwk=4123 alt=A050"
```

## Client Design

### Screen Structure

The client should have exactly two primary screens:

- `StartScreen`
- `SessionScreen`

#### `StartScreen`

Contains:

- saved server profiles
- host field
- port field
- trainer magic-hash login section
- student session-hash join section
- connection / error area

#### `SessionScreen`

Contains:

- top status strip
- `SectorCanvas`
- `TrafficManagerPanel`
- `TrainerToolboxPanel` for trainer only

### State Separation

The client should separate:

- synchronized session state from server
- local UI state
- transient network state

#### Synchronized Session State

Includes:

- session metadata
- connected role state
- aircraft list and aircraft fields
- active runway
- session status
- student assumption state

#### Local UI State

Includes:

- focused aircraft id
- local zoom and pan
- trainer draft path state
- confirmation dialog state
- visible banners/errors

### Sector Canvas

Use one world coordinate space:

- center at airport reference point
- units in nautical miles
- `x` east
- `y` north

Canvas responsibilities:

- draw airport geometry
- draw CTR
- draw outer cardinal points `NORTH`, `EAST`, `SOUTH`, and `WEST`
- draw active-runway ILS
- draw aircraft markers
- draw labels
- draw one-minute airborne vectors
- support focus hit-testing
- support zoom and pan
- support trainer path drafting

Zoom behavior:

- local to each client
- zoom toward mouse position

Built-in sector geometry should include the four named outer points:

- `NORTH`
- `EAST`
- `SOUTH`
- `WEST`

Each point should be placed `10 NM` beyond the CTR boundary in its respective direction and used for simplified SID and next-waypoint visualization.

### Traffic Manager Panel

The traffic manager is contextual:

- empty when no aircraft is focused
- shows one focused aircraft only

Student edit rule:

- student may edit assumed traffic only

Trainer edit rule:

- trainer may edit any traffic

### Trainer Toolbox

The v1 toolbox should be permanently docked on the right side for trainer users.

Exact v1 actions:

1. `Create aircraft`
2. `Remove aircraft`
3. `Set active runway`
4. `Assign runway in work`
5. `Set speed`
6. `Draft path`
7. `Launch path`
8. `Stop ground aircraft`
9. `Go around`
10. `Rejected takeoff`
11. `Pause session`
12. `Resume session`

## Shared Domain And File Formats

### Scenario Files

Scenario files should use `TOML`.

Required top-level fields:

- `scenario_id`
- `name`
- `student_position_type`
- `initial_active_runway`
- `metar`
- `aircraft`

An example already exists:

- [scenario-gnd-36-medium.toml](/Users/sasha/dev/atc-trainer/docs/scenario-gnd-36-medium.toml)

### Local Client Settings

The client should persist local settings in `TOML` under the Windows user config directory.

Suggested contents:

- saved server profiles
- window size
- last zoom level

Suggested path:

- `%APPDATA%/atc-trainer/client.toml`

## Cross-Platform And Build Strategy

The codebase is Windows-first, but development may occur on non-Windows systems.

Recommended build strategy:

- keep platform-specific code isolated
- prefer native Windows CI builds for final binaries
- keep cross-compilation optional for developer convenience

Likely Windows targets:

- `x86_64-pc-windows-msvc`
- `x86_64-pc-windows-gnu`

Preferred release strategy:

- build release artifacts on Windows CI
- distribute as a portable folder in v1

## Testing Strategy

Integration tests are written alongside the implementation of each phase, not bolted on at the end. They drive the protocol contract for `atc-shared`, catch regressions before they reach the `iced` client, and keep Phase 4 packaging honest.

### Principles

- target the protocol, not the `iced` UI
- exercise the real `atc-server` through a headless protocol client
- one server per test; no shared session state across tests
- use simulated time for movement ticks and cooldown windows
- reuse `atc-shared` DTOs directly so protocol drift is a compile error, not a runtime surprise

### Test Harness

Tests use an in-process server and a headless `TestClient`, both provided by `atc-test-support`.

- build the `axum::Router` directly and bind to an ephemeral port with `TcpListener`
- run on the `tokio::test` runtime
- HTTP calls go through `reqwest`
- WebSocket calls go through `tokio-tungstenite`
- time-sensitive behavior advances through `tokio::time::pause` and `tokio::time::advance`
- log output routes to a per-test `tempfile::tempdir`

Tradeoff: the in-process harness bypasses the real binary entrypoint and config loader. That gap is closed by one subprocess smoke test in Phase 4, not by duplicating every integration test at the binary level.

### Unit Tests

#### `atc-shared`

- serialization round-trip for every DTO
- validation helpers
- scenario `TOML` parsing, including malformed fixtures

#### `atc-server`

- runway-switch SID conversion logic
- log line formatting
- per-session event ordering under concurrent input

#### `atc-client`

- reducer and message-handler logic where practical
- manual validation for `iced` canvas interactions, zoom, pan, focus, and path drafting

### Integration Tests

Integration tests live in `crates/atc-server/tests/` and depend on `atc-test-support`. They are grouped by phase and added with the code that makes them pass. Pre-staging later-phase tests before their prerequisites rots fixtures and is explicitly avoided.

#### Phase 1 — HTTP and WS handshake

- trainer login: valid magic hash returns `200`; invalid returns `401`
- session creation returns `session_id` and `session_hash`
- student position creation supports `GND` and `TWR`
- join by session hash: valid accepted; wrong hash rejected; unknown session returns `404`
- start session transitions `draft` → `waiting_for_student` → `running`
- WebSocket connect with valid auth delivers one full `session_state` snapshot
- WebSocket connect without auth is rejected
- scenario `TOML` loads and populates the session aircraft collection

#### Phase 2 — Multi-client sync

- two WebSocket clients on one session both receive the initial snapshot
- a server-side state change broadcasts identical events to both clients

#### Phase 3 — Operational commands

- trainer `create_aircraft` broadcasts `aircraft_created`
- aircraft limit enforced: the `31`st creation is rejected
- draft path → `Finish pathing` → edit speed and altitude → `launch_path` emits position updates along straight-line segments under simulated time
- a launched path cannot be replaced until the aircraft is stopped on the ground
- student `update_aircraft` on assumed traffic is accepted; on unassumed traffic is rejected
- trainer may edit any traffic
- active runway switch auto-reassigns all ground aircraft and converts any assigned SID to the paired variant, for example `east1a` ↔ `east1b`
- `GND`: `assigned_runway` is cleared on assumption
- `TWR`: `assigned_runway` is cleared when the aircraft vacates the runway
- `go-around` sets runway heading only, then returns control to the trainer
- `rejected takeoff` accepted only in an on-runway departure state
- pause freezes aircraft movement; metadata edits remain accepted
- resume continues aircraft motion from the frozen pose
- two trainers sending conflicting commands on the same aircraft produce one deterministic serialized order, and both observers see identical final state

#### Phase 4 — Lifecycle and logging

- student disconnect does not end the session
- all trainers disconnect begins a cooldown
- any trainer reconnect within `3` minutes keeps the session alive
- no trainer reconnect within `3` minutes auto-closes the session
- the log file contains one line per accepted action with `timestamp`, `session`, `actor`, `aircraft`, `action`, and `result`

#### Phase 4 — Binary smoke test

- one subprocess test launches the real server binary with the built-in scenario and runs a minimal trainer-login, session-create, and WebSocket-snapshot flow

### Sequencing

- Phase 1 tests land with the HTTP routes and WebSocket skeleton
- Phase 2 tests land with the client shell and sync layer
- Phase 3 tests land with the runtime command loop
- Phase 4 tests land with pause, cooldown, logging, and packaging

## Risks And Mitigations

### `iced` Canvas Interaction Complexity

Risk:

- custom hit-testing, zoom/pan, and path authoring can become fragile

Mitigation:

- implement canvas features incrementally
- keep v1 interactions explicit and simple
- avoid over-abstracted input systems early

### Multi-Trainer Conflicts

Risk:

- two trainers may act on the same aircraft simultaneously

Mitigation:

- rely on serialized per-session ordering
- log all accepted actions
- keep UI transparent about last-applied state

### Pathing Scope Creep

Risk:

- freeform pathing can expand into a general simulation engine

Mitigation:

- keep straight-line segments only
- keep point parameters minimal
- keep trainer trust model explicit

### Windows Packaging Friction

Risk:

- desktop dependencies and linker differences can slow delivery

Mitigation:

- keep dependencies conservative
- validate Windows builds early
- do not wait until the end to test packaging

## Phased Implementation Plan

### Phase 1: Foundation And Server Core

Goal:

- establish the workspace, shared domain model, scenario parsing, and server session runtime

Deliverables:

- Rust workspace with `atc-shared`, `atc-server`, `atc-client`, and `atc-test-support`
- shared ids, enums, DTOs, scenario types, and sector/scenery structures
- scenario `TOML` parsing
- trainer auth endpoint
- session creation and student join endpoints
- in-memory session registry
- serialized per-session event loop
- WebSocket handshake and initial snapshot support
- baseline file logging
- Phase 1 integration tests as defined in the Testing Strategy

Exit criteria:

- workspace builds cleanly
- example scenario parses successfully
- trainer can create a session
- student can join by session hash
- both can connect to the same session over WebSocket

### Phase 2: Client Shell And Shared Session View

Goal:

- deliver a connected client with the basic live session UI and sector rendering

Deliverables:

- `StartScreen`
- saved server profiles and local settings
- trainer login and student join flows
- `SessionScreen` shell
- top status strip
- `SectorCanvas`
- airport, CTR, and ILS rendering
- aircraft markers and labels
- local zoom/pan
- focus hit-testing
- WebSocket-driven live session synchronization
- Phase 2 integration tests as defined in the Testing Strategy

Exit criteria:

- trainer and student can connect from the client
- both users see the same live aircraft state
- sector view is usable with zoom and pan
- aircraft focus from radar labels works

### Phase 3: Operational Workflow

Goal:

- implement the core training loop for both student and trainer roles

Deliverables:

- focused aircraft traffic manager
- assumption flow
- student editing for status, runway, SID, squawk, altitude, and handoff
- trainer toolbox panel
- manual aircraft creation and removal
- active runway switching
- path drafting, `Finish pathing`, point parameter editing, and path launch
- stop ground aircraft
- go-around and rejected takeoff
- trainer-only path visibility
- Phase 3 integration tests as defined in the Testing Strategy

Exit criteria:

- student can manage assumed aircraft
- trainer can create and move aircraft live
- both clients reflect operational changes in real time
- the full session workflow is playable end-to-end

### Phase 4: Hardening, Packaging, And Playtest

Goal:

- stabilize session control, logging, and Windows delivery for real use

Deliverables:

- pause and resume
- disconnect cooldown handling
- finalized human-readable log format
- full log coverage for accepted actions
- Windows release build
- portable folder packaging
- trainer/student playtest runbook
- final defect triage and cleanup
- Phase 4 integration tests and the binary smoke test as defined in the Testing Strategy

Exit criteria:

- session survives expected disconnect scenarios
- logs are complete and readable
- two remote users can run a full training session on Windows
- packaging is repeatable and suitable for v1 distribution

## Recommended Immediate Next Steps

1. Create the Rust workspace with `atc-shared`, `atc-server`, and `atc-client`.
2. Implement Phase 1 shared domain and scenario parsing.
3. Stand up the server session core and WebSocket skeleton before investing heavily in `iced`.

That order keeps the protocol and runtime stable before the client grows complicated.
