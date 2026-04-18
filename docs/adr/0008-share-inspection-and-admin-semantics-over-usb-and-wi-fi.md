# ADR 0008: Share inspection and admin semantics over USB and Wi-Fi

## Status

Accepted

## Context

`lilbug` needs to be inspectable and configurable through more than one transport.

The system already assumes:

- USB is the safe provisioning and recovery path
- Wi-Fi is available during normal operation

We want to be able to read config and other runtime data from `lilbug` whether it is connected over USB or reachable over Wi-Fi.

At the same time, transport differences should not force every tool to learn a completely different command vocabulary.

## Decision

`lilbug` will share the same inspection and admin command semantics over USB and Wi-Fi where practical.

The transport implementations may differ, but the command vocabulary and response model should remain aligned.

This applies to at least:

- config retrieval
- state retrieval
- frame retrieval
- other closely related inspection and admin operations

## Rationale

Reasons for this decision:

- the CLI should be able to target emulator, USB-connected device, or Wi-Fi-connected device with minimal conceptual drift
- transport-specific behavior should not leak into every tool command
- shared semantics reduce duplication and long-term protocol drift
- USB remains the fallback path, while Wi-Fi becomes the normal convenience path when available

## Consequences

Positive:

- one command model across emulator and device access paths
- easier CLI design and documentation
- easier testing of shared command semantics independent of transport
- better recovery story because USB and Wi-Fi access patterns remain conceptually aligned

Tradeoffs:

- some transport-specific capabilities may need careful mapping
- exact wire framing may differ across transports even when semantics are shared
- transport-specific security or authentication concerns may need later differentiation

## Design Direction

The important invariant is semantic compatibility, not byte-for-byte transport identity.

That means:

- USB may use newline-delimited JSON over serial
- Wi-Fi may use a local TCP, HTTP, or other request/response transport
- the CLI should still expose the same logical operations and expect the same conceptual responses

## Relationship to Provisioning

USB remains the required path for first-boot provisioning and network recovery.

Wi-Fi access is an additional convenience and operational surface, not a replacement for the tethered recovery path.

## Non-Goals

This ADR does not define:

- the exact Wi-Fi admin transport
- security and authentication policy for remote admin access
- whether every write operation available over USB must also be exposed over Wi-Fi in rev1

## Deferred Questions

- exact Wi-Fi transport for admin access
- whether Wi-Fi admin access should be read-only at first
- how remote admin authentication should work
