# ADR 0015: Separate motion and face command lanes

## Status

Accepted

## Context

`lilbug` rev1 supports both motion commands and face commands through the shared command surface.

The remaining ambiguity is how these commands interact when they arrive close together:

- should a face command interrupt motion?
- should a motion command interrupt the current face?
- how should timed motion commands behave when replaced?

To keep behavior predictable, the system needs a clear concurrency model.

## Decision

`lilbug` will use separate command lanes for motion and face state.

Motion commands affect only motion state.
Face commands affect only face state.

They do not interrupt each other.

## Motion lane rules

Motion commands are:

- `fwd:<duration_ms>`
- `back:<duration_ms>`
- `stop`
- `brake`

Rules:

- `fwd:<duration_ms>` and `back:<duration_ms>` start motion immediately
- timed motion commands automatically end after their duration expires
- when a new motion command arrives, any currently active timed motion command is cancelled and replaced
- `stop` immediately cancels current motion and leaves the motor in `stop`
- `brake` immediately cancels current motion and leaves the motor in `brake`

## Face lane rules

Face commands are:

- `face:neutral`
- `face:happy`
- `face:blink`
- `face:surprised`

Rules:

- a face command updates only face state
- a later face command replaces the previous face state
- face commands do not cancel or modify motion state

## Rationale

Reasons for this decision:

- it matches the mental model of expression and movement happening concurrently
- it avoids surprising cross-effects between unrelated command types
- it creates a cleaner state model than treating all commands as a single serialized lane
- it fits the long-term shape of a more expressive character system

## Consequences

Positive:

- clearer runtime behavior
- simpler reasoning about command replacement rules
- better foundation for future animation and motion work

Tradeoffs:

- requires a more explicit state model than a single last-command field
- motion timing must be tracked independently from face state

## Design Direction

The implementation should move toward an explicit split between:

- current face state
- current motion state
- optional active motion expiry

This is preferable to encoding all behavior through a single last-command concept.

## Relationship to Existing Grammar

This ADR refines the semantics of ADR 0013 rather than replacing the command grammar itself.

The existing command syntax still applies.

What changes here is the concurrency and replacement behavior.

## Non-Goals

This ADR does not define:

- timed face commands
- compound multi-lane command batching in a single request
- exact scheduler implementation details

## Deferred Questions

- whether future face commands should optionally support duration-based temporary expressions
- how streamed-frame override mode should interact with locally managed face state later
