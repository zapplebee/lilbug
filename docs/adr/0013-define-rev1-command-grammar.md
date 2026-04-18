# ADR 0013: Define the rev1 command grammar

## Status

Accepted

## Context

`lilbug` rev1 uses the CLI as the main operator surface and `POST /v1/cmd` as the main command endpoint.

We need a command grammar that is:

- short to type
- easy to parse
- easy to map onto JSON
- expressive enough for motion and face commands in rev1

## Decision

The rev1 CLI command grammar uses compact colon-separated tokens.

Examples:

- `fwd:300`
- `back:300`
- `stop`
- `brake`
- `face:happy`
- `face:blink`
- `face:surprised`

Where a duration is present for a motion command, it is interpreted as milliseconds.

## Rationale

Reasons for this decision:

- it keeps the CLI concise
- it is easy to explain and document
- it maps cleanly into structured API payloads
- it avoids introducing an overly large command surface in rev1

## Grammar Shape

The intended forms are:

- `<command>`
- `<command>:<value>`

Rev1 meanings:

- `fwd:<duration_ms>`
- `back:<duration_ms>`
- `stop`
- `brake`
- `face:<expression>`

## API Mapping

The CLI grammar is a convenience layer over `POST /v1/cmd`.

Expected request direction:

```json
{ "command": "forward", "duration_ms": 300 }
{ "command": "backward", "duration_ms": 300 }
{ "command": "stop" }
{ "command": "brake" }
{ "command": "face", "value": "happy" }
```

The HTTP API is the canonical structured form. The CLI token grammar is the ergonomic shorthand.

## Rev1 Command Set

Supported motion commands:

- `fwd:<duration_ms>`
- `back:<duration_ms>`
- `stop`
- `brake`

Supported face commands:

- `face:neutral`
- `face:happy`
- `face:blink`
- `face:surprised`

The exact supported expression list may grow later, but these should be the baseline set for rev1.

## Validation Rules

- duration values are integer milliseconds
- `fwd` and `back` require a duration value in rev1
- `stop` and `brake` do not take arguments
- `face` requires an expression value
- invalid commands should return a clear error from both CLI parsing and the HTTP API

## Non-Goals

This ADR does not define:

- speed or power-level syntax in rev1
- compound command chaining in a single token
- a scripting language or macro system

## Deferred Questions

- whether rev1 should later accept optional no-duration continuous motion commands
- whether aliases beyond `fwd` and `back` are worth supporting
- whether some face commands should also accept durations later
