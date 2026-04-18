# ADR 0004: Use a native-only software emulator

## Status

Accepted

## Context

`lilbud` supports multiple non-embedded targets, but `lilbug` does not need that level of target spread for the first revision.

For `lilbug`, the emulator's job is narrower and more practical:

- provide a fast local development surface before hardware arrives
- render the face at the device's real pixel size
- exercise the same config path as the real device
- listen to the same runtime MQTT cues as the real device
- expose enough motor state to validate cue behavior visually

The emulator is for local development and iteration, not for shipping as a browser app or webview host.

## Decision

`lilbug` will have a single software emulator target, and it will be native-only.

It will not initially support browser, WASM, or webview targets.

The native emulator will:

- render at the real device resolution of `412x412`
- show a visible circular display boundary on screen
- accept configuration from the CLI through the same command model as the device
- listen to MQTT for runtime cues
- show motor direction indicators in the lower corners:
  - lower left: `[FORWARD]`
  - lower right: `[BACKWARD]`
- run as a separate long-running native process

## Rationale

Reasons for this decision:

- one emulator target is enough to unblock design and behavior work before hardware arrives
- native-only keeps scope down and reduces toolchain complexity
- matching the device resolution makes rendering issues visible earlier
- sharing the CLI config model helps keep provisioning behavior aligned between emulator and device
- listening to MQTT makes the emulator useful as a drop-in stand-in for the real bot
- explicit motion indicators make it easy to validate cue-to-motion behavior even before physical hardware exists

## Consequences

Positive:

- faster time to a usable development loop
- less platform and packaging work than a multi-target emulator stack
- better confidence that UI composition fits the real display
- easier end-to-end testing of CLI config and MQTT cue handling

Tradeoffs:

- no browser-based preview in rev1
- no direct reuse of `lilbud`'s broader desktop/wasm/webview target layout
- if a browser control surface is wanted later, it will be a separate addition rather than something built in from day one

## UI Requirements

The emulator must visibly model the physical display characteristics of the board.

Required display behavior:

- framebuffer size matches the device: `412x412`
- the visible scene includes a circular screen boundary
- the circular bound must be obvious on screen so out-of-bounds composition is easy to detect

Required motor indicators:

- show `[FORWARD]` in the lower-left corner when forward motion is active
- show `[BACKWARD]` in the lower-right corner when backward motion is active
- always show both labels
- dim inactive labels and emphasize the active label

These indicators are part of the emulator only. They are debugging aids, not a requirement for the physical device UI.

## Relationship to Device Interfaces

The emulator should behave like a software stand-in for the device where practical.

That means:

- it should consume the same runtime MQTT cue model
- it should support the same configuration concepts as the device
- it should be configurable from the CLI in the same way, even if the transport details differ

The important goal is command-model compatibility, not byte-for-byte transport identity.

For example:

- the hardware device may receive provisioning commands over USB serial
- the emulator will expose the same commands through a local socket/IPC control channel

In both cases, the command vocabulary and config semantics should stay aligned.

## Emulator Interface Decisions

The emulator interface choices for rev1 are:

- transport from CLI to emulator: local socket/IPC
- process model: separate long-running emulator process
- config persistence: memory only
- render direction: simple native 2D rendering rather than a larger GUI framework

These choices optimize for a fast development loop and a small implementation surface.

## Non-Goals

This ADR does not require:

- browser or WASM support
- a webview host
- exact hardware timing simulation
- exact USB transport emulation
- a fully photorealistic rendering of the physical enclosure
- persistent emulator config across restarts in rev1

## Follow-on Design Direction

The emulator should be treated as a development tool with these responsibilities:

- render face and animation state
- render debug-only motor state indicators
- receive and apply config commands
- connect to MQTT and react to cues like the device would

## Deferred Questions

- the exact socket or IPC mechanism to use locally
- whether memory-only config should later grow an optional file-backed mode for restart testing
