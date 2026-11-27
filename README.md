# ATC Trainer

An instructor-led Air Traffic Control (ATC) trainer built in Rust with [GPUI](https://gpui.rs/), designed for learning controller workflows and practicing complex traffic situations without relying on external flight simulators or live ATC networks.

The student display is modeled on IVAO Aurora workflows, while main and co-instructors operate simulated traffic through a dedicated traffic-management interface.

## Documentation & Links

- **Design Specification**: [docs/superpowers/specs/design-doc.md](docs/superpowers/specs/design-doc.md) — product boundaries, architecture, data formats, delivery stages, and implementation phases.
- **Repository & Agent Guidelines**: [AGENTS.md](AGENTS.md) (`CLAUDE.md` symlinks here) — build commands, coding standards, testing conventions, and workflow.
- **Issue Tracker**: [GitHub Issues](https://github.com/Fly-Style/atc-trainer/issues) ([Epic → Story → Task guide](docs/agents/issue-tracker.md) | [triage labels](docs/agents/triage-labels.md) | [domain docs](docs/agents/domain.md)).
- **Screenshots**: [docs/screenshots/](docs/screenshots/) — current GPUI sector viewer and host/student admission captures.

## Implementation Phases

Each phase maps to a GitHub **Epic** and builds incrementally through the five capability stages (**Ground → Tower → Approach → Area Control → Weather**). See [Section 8 of the design specification](docs/superpowers/specs/design-doc.md#8-delivery-stages) for full details.

| Phase | Name | Status | Summary |
| --- | --- | --- | --- |
| **Phase 1** | Core app & session bootstrap | **Completed** | GPUI desktop app renders local IVAO Aurora sector packs, hosts an in-app HTTP server, verifies local sector identity, and admits a student. |
| **Phase 2** | Scenarios & traffic rendering | **Next** | Per-exercise scenario format referencing a specific FIR sector file (initial aircraft, flight plans, separate physical vs. student states), host-to-client state sync, and aircraft/label rendering. |
| **Phase 3** | Ground anchors & instructor ground movement | Planned | Per-airport extended sector format for ground anchors (`TAXI`, `PUSHBACK_FACING_XXX`, `15 kt`/`30 kt` taxi speeds), visible instructor anchor-route selection, `move`/`stop` commands, finite accel/decel, and occupied-anchor auto-stop. |
| **Phase 4** | Student ground workflow & transfers | Planned | Decoupled student informational marks, right-click label control (`free`/`assumed`/`transferred`), Aurora-style departure/arrival lists, instructor read-only mark visibility, and artificial-neighbor transfers. |
| **Phase 5** | Multi-instructor & direct-IP session hardening | Planned | Co-instructor join strings and permissions, server last-write-wins conflict handling, disconnect and slot takeover, and direct-IP remote hosting readiness. *(Completes live Stage 1 — Ground.)* |
| **Phase 6** | Tower operations & core airborne simulation | Planned | `.lnmperf` performance engine (embedded A320, B737, C172 + local import + wake-turbulence fallback; `GS = TAS`), runway ops, airborne heading/altitude/speed commands, and VFR traffic patterns. *(Stage 2 — Tower.)* |
| **Phase 7** | Approach & procedure flying | Planned | Parsing and flying IVAO sector SIDs and STARs, approach procedures, terminal radar layers, and approach workflows. *(Stage 3 — Approach.)* |
| **Phase 8** | Area control | Planned | En-route FIR/airway rendering, high-altitude cruise and flight levels, sector handoffs, and larger-traffic ergonomics. *(Stage 4 — Area Control.)* |
| **Phase 9** | Persistence, recovery & movement replay | Planned | Low-priority durable movement history and session persistence, crash recovery on host restart, preserved recordings after session end, and movement replay. |
| **Phase 10 (Interlude)** | Dedicated self-hostable server & observers | Planned *(parallelizable)* | Standalone self-hostable server binary (joining without exposing an instructor's IP) and up to four observers. Can be executed in parallel with other phases once live sessions are established. |
| **Phase 11** | Weather influence | Planned | Wind (`GS ≠ TAS`, drift, runway wind components), QNH/atmosphere, and visibility/METAR effects layered onto all positions. *(Stage 5 — Weather.)* |

## Workspace Structure

- [`crates/atc-client`](crates/atc-client/) — GPUI desktop application (Aurora sector parser, 2D sector viewer, in-app host, client state and network adapters).
- [`crates/atc-server`](crates/atc-server/) — Axum HTTP/WebSocket session server, authentication, and authoritative simulation state.
- [`crates/atc-shared`](crates/atc-shared/) — Protocol and domain types shared by client and server.
- [`crates/atc-test-support`](crates/atc-test-support/) — Reusable loopback `TestServer`, `TestClient`, and test fixtures.

## Quick Start

Requires the Rust toolchain pinned in [rust-toolchain.toml](rust-toolchain.toml) (`1.100.0`).

```bash
# Build the workspace
cargo build --workspace --locked

# Launch the GPUI desktop app (optionally pass a local Aurora .isc sector file)
cargo run --locked -p atc-client
cargo run --locked -p atc-client -- ukbu-aurora/UKBU.isc

# Run tests and lints
cargo test --workspace --locked
cargo clippy --workspace --all-targets --locked
```

## License

Dual-licensed under MIT or Apache-2.0 (see [Cargo.toml](Cargo.toml)).
