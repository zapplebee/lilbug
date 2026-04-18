# AGENTS

This file captures project-specific working norms for agents operating in this repo.

## Purpose

`lilbug` is intentionally a Rust project.

The user is often a TypeScript developer and often uses `bun` in other projects, but this repo exists in part to flex and deepen Rust skills. When choosing between a Rust implementation and bringing in another stack for convenience, prefer Rust unless an accepted ADR clearly requires otherwise.

## Source of truth

Before making architectural changes, read the ADRs in `docs/adr/`.

- accepted ADRs define the current intended architecture
- superseded ADRs provide history but should not drive new implementation
- deferred ADRs are not active implementation targets unless revived explicitly

When code and ADRs diverge, prefer bringing the code back in line with the ADRs unless the user explicitly asks to revisit the decision.

## Temp docs and notes

Use `.scratch/` for temporary notes, draft docs, rough plans, and disposable working material.

- `.scratch/` is intentionally gitignored
- do not leave important decisions only in `.scratch/`
- move durable decisions into ADRs, README updates, or real docs before finishing

## Git and GitHub conventions

Use `git` normally for local history and use the `gh` CLI for GitHub interactions.

Use `gh` for:

- creating and viewing issues
- creating and viewing pull requests
- reading PR comments and review state
- checking GitHub-side metadata when needed

Do not treat the GitHub web UI as the primary automation surface when `gh` can do the job.

## Commit conventions

Prefer conventional-commit style messages when making commits.

Examples:

- `feat: add bootstrap HTTPS init flow`
- `fix: correct backward indicator placement in emulator`
- `docs: update ADR references in README`
- `refactor: simplify command parsing around rev1 grammar`

Keep commit messages concise and accurate.

## Documentation validation

After every meaningful change, validate the documentation.

At minimum, check whether changes require updates to:

- `README.md`
- `docs/local-development.md`
- any relevant ADR status or supersession markers
- CLI examples and run commands

Do not leave docs silently stale after code changes.

If a change alters the implemented architecture or invalidates a documented flow, update the docs in the same pass.

## Rust style preferences

Prefer Rust that is clear, explicit, and teachable.

The goal is not just to make the code work, but to make the codebase a good Rust-learning surface.

### Prefer declarative code

Prefer declarative, readable code over clever condensed control flow.

Good:

- explicit data structures
- clear enums for protocol and mode modeling
- obvious transformation steps
- readable matching and branching

Avoid:

- code-golf style syntax
- dense chains that hide intent
- unnecessary cleverness in iterator pipelines when a clearer form exists
- compact but opaque control flow

If the more declarative version is a little longer but significantly easier to read, prefer it.

### Comments

Comments should be inline and useful.

Prefer comments that explain:

- why a pattern exists
- why a branch or workaround is needed
- non-obvious Rust ownership or borrowing behavior
- complex syntax or crate-specific behavior that would otherwise slow a reader down

Do not add filler comments that restate obvious code.

Examples of useful comment targets:

- borrow/lifetime-sensitive code
- state transitions
- request/response translation layers
- image/frame encoding steps
- mode handling or auth boundaries

## Architectural bias

Do not default to only adding more code on top of the current shape.

When implementing a new feature, first evaluate whether the better change is an architectural improvement that creates a cleaner layer of abstraction for future work.

Examples of good architectural thinking:

- replacing duplicated request handling with a clearer shared API model
- separating transport concerns from command semantics
- introducing a better state model instead of scattering flags
- moving from ad hoc branching to explicit enums and typed transitions

Do not churn the design gratuitously, but do not be afraid to make a larger structural improvement when it clearly produces a better long-term abstraction and remains consistent with the ADRs.

## Preferred implementation posture

When extending the system:

1. check the ADRs
2. check whether the current code shape still matches them
3. decide whether the next change should be a small feature addition or a cleaner structural change first
4. implement the smallest good version of the improved shape
5. update docs immediately after

## Validation posture

When you make a change, prefer verifying the real user-facing flow, not just compilation.

Examples:

- run the CLI flow, not just the parser tests
- retrieve the frame image, not just the HTTP route unit test
- exercise bootstrap and Wi-Fi mode transitions, not just isolated helpers

The project favors end-to-end confidence where practical.
