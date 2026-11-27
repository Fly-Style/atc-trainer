# Domain Docs

**Source of truth:** `docs/superpowers/specs/` (shared with Superpowers workflows).

This repo **does not use the ADR pattern** (`docs/adr/`) and **does not use separate `CONTEXT.md` / `CONTEXT-MAP.md` files**. All domain context, terminology, architectural decisions, trade-offs, and deferred scope live in `docs/superpowers/specs/`.

## Before exploring, read these

- **`docs/superpowers/specs/`**: read the top-level design specification (`docs/superpowers/specs/design-doc.md`) and any phase or feature sub-specs relevant to the area you are working on.

Do not create `CONTEXT.md`, `CONTEXT-MAP.md`, or `docs/adr/` — even if a skill (such as `/domain-modeling`, `/grill-with-docs`, or `/improve-codebase-architecture`) normally offers to create them.

## Cooperating with Superpowers skills

- **Design & domain modeling**: Superpowers skills (e.g., `brainstorming`, design specs, implementation plans) and engineering skills (`/grilling`, `/domain-modeling`, `/research`, `/prototype`, `/design-an-interface`) share `docs/superpowers/specs/` as the single source of truth.
- **Recording new terms or decisions**: When `/grilling`, `/domain-modeling`, or `/improve-codebase-architecture` resolves a domain term, trade-off, or architectural decision, update the relevant spec in `docs/superpowers/specs/` directly instead of creating a `CONTEXT.md` or ADR file.
- **Slicing & tracking work**: Skills like `/to-tickets`, `/to-spec`, `/triage`, and `/wayfinder` read specs from `docs/superpowers/specs/` and reference them from GitHub issues rather than duplicating decisions into ADRs or standalone context files.

## Use the specs' vocabulary

When your output names a domain concept (in an issue title, refactor proposal, hypothesis, or test name), use the terms defined in `docs/superpowers/specs/`. Don't drift to synonyms the specs avoid.

If a concept you need isn't defined in the relevant spec yet, flag the gap and clarify it with the user rather than inventing terminology silently.

## Flag spec conflicts

If your output (a ticket, refactor proposal, or implementation plan) contradicts a recorded decision in `docs/superpowers/specs/`, surface it explicitly rather than silently overriding:

> _Contradicts `docs/superpowers/specs/design-doc.md` (Section X), but worth reopening because…_
