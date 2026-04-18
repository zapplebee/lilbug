# ADR 0011: Use emulator startup modes for bootstrap and Wi-Fi flows

## Status

Accepted

## Context

The emulator needs to support first-contact provisioning flows and normal post-provisioning behavior.

One possible approach was a UI button or similar emulator control to simulate the transition into a setup path.

That is less useful for unattended testing and agent-driven workflows than a deterministic startup mode.

## Decision

The emulator will use explicit startup modes rather than a UI button for provisioning flow transitions.

The expected direction is:

- a bootstrap-oriented mode for first-contact setup
- a Wi-Fi / normal-operation mode for post-provisioning behavior

The exact flag spelling can be refined, but the behavior should be selected from the command line, such as `--mode bootstrap` and `--mode wifi`.

## Rationale

Reasons for this decision:

- command-line modes are easier to script than UI interactions
- unattended verification and agent-driven testing become much simpler
- the emulator better models lifecycle state in a deterministic way
- test flows can explicitly cover fresh-device and provisioned-device behavior

## Consequences

Positive:

- better automation and repeatability
- simpler integration with the CLI and future end-to-end tests
- no need for ad hoc emulator-only UI control paths

Tradeoffs:

- mode transitions may require restart between phases unless later designed otherwise
- emulator startup behavior becomes more stateful and must load persisted local state carefully when that arrives

## Design Direction

Bootstrap mode should behave like a new or unprovisioned device.

Wi-Fi mode should behave like a provisioned device that loads persisted config and exposes its normal HTTPS interface.

This should allow a deterministic flow such as:

1. start emulator in bootstrap mode
2. run `lilbug-cli init ...`
3. persist configuration
4. start emulator in Wi-Fi mode
5. run normal CLI commands against the HTTPS interface

## Relationship to Persistence

This ADR assumes the emulator and device will eventually have a persistence model that makes the mode switch meaningful across restarts.

For the emulator, that persistence mechanism can evolve separately from the physical device backend.

## Non-Goals

This ADR does not define:

- the exact emulator persistence file format
- whether mode switching can happen without process restart
- the exact naming of every startup mode

## Deferred Questions

- whether the emulator should also support a combined convenience mode later
- whether bootstrap mode should expose only the init path or a broader diagnostic surface
