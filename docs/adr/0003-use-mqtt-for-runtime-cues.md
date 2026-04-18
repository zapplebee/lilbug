# ADR 0003: Use MQTT for runtime cues

## Status

Superseded by ADR 0010

## Superseded By

- `ADR 0010: Use HTTPS and Bearer token auth for the rev1 control plane`

## Context

`lilbug` needs a runtime network transport for cues such as:

- face changes
- motion commands
- short-lived actions
- state publication back to tools or services

The likely producers are not limited to a single UI:

- a local CLI
- small automation scripts
- future services
- possibly multiple `lilbug` devices

We considered using a direct WebSocket connection instead.

## Decision

`lilbug` will use MQTT as the primary runtime cue transport over Wi-Fi.

WebSocket is explicitly deferred and may be added later for browser-facing tooling or live debug surfaces, but it is not the primary cue transport for rev1.

## Rationale

Reasons for choosing MQTT:

- `lilbug` is a small event-driven device, which fits pub/sub well
- multiple producers can publish cues without owning a long-lived direct connection to the device
- topic-based routing is a natural fit for per-device control and fleet growth later
- presence and retained state are easier to model with broker features
- it separates runtime control from the USB provisioning path cleanly

## Consequences

Positive:

- clean integration point for CLI tools, scripts, and future services
- natural support for more than one device
- topic structure can keep commands, config, state, and presence separate
- broker features such as retained messages and last-will can support better device behavior

Tradeoffs:

- requires an MQTT broker in the environment
- introduces broker and topic configuration work in provisioning
- browser-first tooling is less direct than a raw WebSocket approach

## Expected Topic Shape

The exact names may change, but the intended structure is:

- `lilbug/<id>/cue`
- `lilbug/<id>/config`
- `lilbug/<id>/state`
- `lilbug/<id>/presence`

Example cue payloads:

```json
{ "face": "happy" }
{ "action": "forward", "durationMs": 500 }
{ "face": "blink", "action": "stop" }
```

## Why Not WebSocket First

WebSocket is a reasonable choice for a single live control surface, especially in a browser.

We are not choosing it first because:

- it is less natural when there are multiple independent cue producers
- it pushes more connection ownership into the controller side
- it does not provide the same built-in topic and presence model as MQTT

WebSocket remains a possible later addition for:

- browser control panels
- live visualization
- debugging and inspection tools

## Relationship to USB CLI

MQTT is the runtime transport.

USB CLI remains the provisioning and maintenance path for tasks such as:

- setting Wi-Fi credentials
- setting MQTT broker and topic config
- reading device status
- recovering from bad network config

## Deferred Questions

- exact QoS choices for cue and state topics
- whether some state should be retained
- how much acknowledgement behavior the device should provide for action cues
- whether browser-facing tools should speak MQTT directly or go through a bridge later
