# ADR 0002: Use a USB CLI for provisioning and device config

## Status

Accepted

## Context

`lilbug` needs an initial setup and maintenance path that works before Wi-Fi is configured and continues to work even when network access is unavailable.

That setup path needs to cover at least:

- setting Wi-Fi credentials
- setting device identity and local config
- inspecting current config and connection state
- recovering a device whose network config is wrong

There is already a local pattern for this in `~/github.com/zapplebee/codex-monitor`:

- USB CDC serial
- host-side serial discovery and reconnect
- line-delimited JSON messages over USB

That implementation is a good model for a simple, reliable tethered control path.

## Decision

`lilbug` will expose a USB CLI for provisioning and configuration.

The CLI transport will use USB serial, and the wire format will be line-delimited JSON commands and responses.

This path is for setup, inspection, and maintenance rather than normal runtime cue delivery.

## Rationale

Reasons for this decision:

- it works before Wi-Fi exists
- it provides a recovery path when Wi-Fi config is broken
- it matches an existing implementation pattern already used in `codex-monitor`
- it supports a local command-line tool without requiring a custom GUI first
- it keeps provisioning concerns separate from the runtime cue transport

## Consequences

Positive:

- reliable first-boot and recovery workflow
- simple host tooling for local setup over USB
- no dependency on a captive portal or temporary access point flow in rev1
- easy to script from a local CLI

Tradeoffs:

- requires a cable for initial setup and some config changes
- requires a small host-side serial tool
- introduces a second protocol surface alongside the network cue transport

## Wire Direction

The expected shape is:

- device exposes USB CDC serial
- host CLI discovers and opens the serial device
- commands are sent as newline-delimited JSON
- responses and logs are returned over the same link

Example commands:

```json
{ "cmd": "wifi.set", "ssid": "my-net", "password": "secret" }
{ "cmd": "wifi.connect" }
{ "cmd": "config.set", "name": "lilbug-01" }
{ "cmd": "config.get" }
{ "cmd": "state.get" }
{ "cmd": "mqtt.set", "host": "mqtt://broker.local", "topic_prefix": "lilbug/lilbug-01" }
```

## Non-Goals

This USB CLI is not the primary runtime cue channel.

It is not intended to be:

- the normal control path for expression and motion during operation
- a full remote control protocol over USB
- a replacement for the network transport

## Deferred Questions

- whether secrets should be stored in NVS, a file, or another board-specific config store
- whether responses should be plain JSON only or support a friendlier human-readable mode in the CLI
- whether firmware logs and command responses should share a single stream or use a structured framing convention
