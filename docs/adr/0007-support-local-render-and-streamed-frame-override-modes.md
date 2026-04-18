# ADR 0007: Support local render and streamed-frame override modes

## Status

Accepted

## Context

`lilbug` needs to support more than one rendering model.

There are two real use cases:

- the device renders its own face locally from a simple internal state machine
- a host system pushes fully rendered frames over Wi-Fi and the device displays them directly

The second mode is important for experimentation, remote rendering, and multimodal-host-driven behavior.

At the same time, the device should retain a simple local fallback behavior so it is not completely dependent on streamed frames.

## Decision

`lilbug` will support two render modes:

- local render mode
- streamed-frame override mode

In local render mode, the device runs a simple local idle/render loop.

In streamed-frame override mode, a host can push whole frames over Wi-Fi for display.

## Rationale

Reasons for this decision:

- local rendering provides graceful degradation when there is no active remote renderer
- streamed-frame mode enables richer host-driven and multimodal workflows
- explicit modes prevent the architecture from becoming ambiguous or half-coupled
- this keeps the device useful both as a self-contained character and as a remote render target

## Consequences

Positive:

- supports autonomous device behavior and host-driven experimentation
- provides a clear model for fallback behavior
- gives a direct path for advanced rendering without overloading the on-device logic

Tradeoffs:

- more system complexity than a single rendering mode
- streamed frames introduce bandwidth and latency sensitivity
- mode transitions need to be defined clearly

## Mode Definitions

### Local render mode

The device is responsible for:

- running its own simple idle loop
- rendering its own default face or expression state
- remaining functional when no remote renderer is active

### Streamed-frame override mode

An external system is allowed to:

- push complete frames over Wi-Fi
- temporarily override the device's locally generated render output

The device's job in this mode is intentionally simpler: receive, validate, and display the incoming frames while maintaining basic control and fallback behavior.

## Design Direction

The system should treat streamed frames as an explicit override, not as the only rendering path.

That means:

- local rendering remains available
- loss of streamed frames should not leave the device with undefined behavior
- timeout or disconnect behavior should return the device to local rendering or another defined fallback state

## Relationship to Debug/Admin Access

The debug/admin path should be able to inspect the currently active render state regardless of which render mode is active.

That includes the ability to retrieve the current displayed frame as an image.

## Non-Goals

This ADR does not define:

- the exact frame transport format
- the exact Wi-Fi transport used for streamed frames
- the exact timeout values for override expiration

## Deferred Questions

- exact wire format for streamed frames
- whether streamed frames should be compressed or raw
- how long override mode remains active without new frames
- whether emulator support for streamed-frame mode should be implemented at the same time as device support
