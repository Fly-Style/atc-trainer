# Repository Guidelines

## Project Structure

- `crates/atc-client/` contains the GPUI desktop app. The sector viewer is in `src/sector_viewer.rs`, the Aurora parser is in `src/aurora.rs`, and the client state and network adapters are in `src/core/`.
- `crates/atc-server/` owns the Axum HTTP and WebSocket endpoints, authentication, and session state. Put protocol types shared by the client and server in `crates/atc-shared/`.
- `crates/atc-test-support/` provides reusable server, client, and fixture helpers. Integration tests live in `crates/atc-server/tests/` and `crates/atc-client/tests/`.
- `docs/` contains the [design specification](docs/superpowers/specs/2026-11-26-atc-trainer-design.md), screenshots, and `scenario-gnd-36-medium.toml`. The local `ukbu-aurora/` checkout contains sector data when available.

## Development Workflow

Use `karpathy-guidelines` for focused edits and verifiable outcomes. For design work, read the specification, use `grilling` to batch open questions, and apply relevant Superpowers workflows. Scope GitHub work as epic → story → task with observable acceptance criteria. When a separate critique is requested, apply actionable feedback and recheck the result.

## Build, Test, and Run

Run these commands from the repository root with the toolchain pinned in `rust-toolchain.toml`.

- `cargo build --workspace --locked` builds the workspace with the checked-in lockfile.
- `cargo run --locked -p atc-client` launches the desktop app. Append `-- ukbu-aurora/UKBU.isc` to open that local sector if available.
- `cargo run --locked -p atc-server` starts the standalone server using `server.toml`.
- `cargo test --workspace --locked` runs the workspace tests. Use `cargo test --locked -p atc-server --test student_admission` for the admission suite.

## Coding Style

Use four-space indentation and rustfmt defaults. Use `snake_case` for modules, functions, and variables, and `UpperCamelCase` for types and enum variants. Keep formatting edits scoped to the code you change. Run `cargo clippy --workspace --all-targets --locked` to lint the workspace.

Apply `rust-best-practices` to Rust changes and `rust-async-patterns` to Tokio work. Keep blocking work off Tokio workers, release locks before awaiting, and handle cancellation.

## Testing

Use Rust's `#[test]` for synchronous cases and `#[tokio::test]` for async integration tests. Name tests for observable behavior, such as `reducer_applies_session_state_snapshot`. For HTTP or WebSocket changes, reuse the loopback `TestServer` and `TestClient` helpers and cover successful requests and relevant rejection cases.

## Commits and Pull Requests

Use short, action-led subjects such as “Add GPUI Aurora sector viewer.” Recent squash commits include the PR number. Explain the behavior change, link the related issue, and list the checks you ran. Show UI changes with screenshots or a short demo video. Keep demo media on a separate branch and link it from the PR.

## Agent skills

### Issue tracker

Actionable work items are tracked in GitHub Issues (`Fly-Style/atc-trainer`) via the `gh` CLI using an **Epic → Story → Task** hierarchy; design specifications live in `docs/superpowers/specs/`. See `docs/agents/issue-tracker.md`.

### Triage labels

Uses `needs-triage`, `needs-info`, `ready-for-agent`, `ready-for-human`, and `bug` (no `wontfix` label). See `docs/agents/triage-labels.md`.

### Domain docs

All domain context, terminology, and architectural decisions live in `docs/superpowers/specs/` (no `CONTEXT.md` and no ADR pattern / `docs/adr/`). See `docs/agents/domain.md`.
