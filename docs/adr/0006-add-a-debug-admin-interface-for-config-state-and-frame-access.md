# ADR 0006: Add a debug/admin interface for config, state, and frame access

## Status

Accepted

## Context

`lilbug` needs to be inspectable and operable by tools, not just controlled through high-level cues.

There are at least three concrete needs:

- read config and other device data
- read current runtime state
- retrieve the current render output as an image that can be consumed by a multimodal model or other tooling

This applies both to the emulator and to real hardware.

MQTT is already the chosen runtime cue transport, but it is not the best fit for every request/response inspection task.

## Decision

`lilbug` will have a dedicated debug/admin interface for structured inspection and control tasks.

This interface will support at minimum:

- config retrieval
- state retrieval
- current frame retrieval as an image representation

The CLI will use this interface to inspect emulator or device instances.

## Rationale

Reasons for this decision:

- debugging and inspection are request/response workflows, which differ from MQTT's event-driven pub/sub model
- multimodal tooling needs a direct way to obtain the current rendered frame as an image artifact
- the CLI should be able to inspect config and state over whichever transport is currently available
- separating high-level cue transport from admin/debug access keeps responsibilities clearer

## Consequences

Positive:

- better observability of both emulator and hardware
- a direct path for multimodal tooling to inspect render output
- cleaner separation between runtime cues and administrative inspection
- makes the CLI more useful as a general-purpose operator tool

Tradeoffs:

- introduces another interface surface to design and maintain
- frame retrieval adds image encoding and transport work
- emulator and hardware backends need to expose comparable semantics

## Expected Capabilities

The interface should grow toward supporting at least:

- `config.get`
- `state.get`
- `frame.get`

The frame retrieval path should produce an image format suitable for tooling. PNG is the preferred default format unless a stronger reason emerges to choose another format.

## Relationship to MQTT

MQTT remains the runtime cue transport.

The debug/admin interface exists alongside MQTT and should not be treated as a replacement for runtime cue delivery.

Examples:

- use MQTT for `face=happy` or `action=forward`
- use the debug/admin path for `state.get`, `config.get`, or `frame.get`

## Non-Goals

This ADR does not require:

- a browser UI in rev1
- video streaming in rev1
- using MQTT itself as the primary mechanism for image retrieval

## Deferred Questions

- exact image encoding pipeline for `frame.get`
- how the CLI should output or store retrieved images locally
- whether the interface should eventually support config mutation in addition to config retrieval
