# ADR 0010: Use HTTPS and Bearer token auth for the rev1 control plane

## Status

Accepted

## Context

We previously considered MQTT for runtime cues and a separate debug/admin interface.

After refining the operator workflow, a simpler rev1 design emerged:

- the device or emulator should expose a small HTTPS API over Wi-Fi
- the CLI should be the primary operator tool
- USB should be used for first-contact provisioning and recovery
- normal control, inspection, and frame retrieval should happen over HTTPS

The goal is to keep rev1 simple and scriptable while still providing encrypted transport on the local network.

## Decision

For rev1, `lilbug` will use a small HTTPS API over Wi-Fi as the primary control and inspection plane.

Authentication will use a Bearer token API key.

The CLI will store the API key in plaintext local config during iteration.

## Rationale

Reasons for this decision:

- a simple HTTPS API is a better fit than MQTT for request/response operations such as config, state, and frame retrieval
- the CLI can become a single front door for both setup and normal interaction
- HTTPS provides transport encryption on the local network
- Bearer token auth is simpler than a username/password or keychain-heavy design in rev1
- storing the token in plaintext locally is an acceptable iteration tradeoff for now

## Consequences

Positive:

- one primary normal-operation interface over Wi-Fi
- easier CLI design and operator ergonomics
- easier frame retrieval for multimodal tooling
- no MQTT broker is required for rev1 workflows

Tradeoffs:

- local config will temporarily contain plaintext API credentials
- rev1 does not attempt a more sophisticated auth scheme
- a local HTTPS server must be implemented on emulator and device paths

## Design Direction

The normal path after provisioning is:

- device or emulator serves HTTPS
- CLI authenticates with `Authorization: Bearer <api_key>`
- CLI reads config, state, and frame data through that API
- CLI sends commands such as motion or face updates through that API

The CLI's local config should store at minimum:

- device nickname
- last known base URL or host
- API key
- trusted certificate or certificate fingerprint

## Relationship to Provisioning

USB remains the first-contact and recovery path.

Provisioning installs at least:

- Wi-Fi credentials
- nickname
- certificate or trust material
- API key

Once provisioned, normal interaction moves to HTTPS over Wi-Fi.

## Superseded Decisions

This ADR supersedes the rev1 transport direction in:

- `ADR 0003: Use MQTT for runtime cues`

MQTT may still return later for other use cases, but it is no longer the chosen primary rev1 control plane.

## Non-Goals

This ADR does not define:

- the exact HTTP route set
- the exact local CLI config file schema
- long-term credential storage hardening beyond the current plaintext iteration model

## Deferred Questions

- exact HTTPS API route design
- exact certificate generation and trust bootstrap flow
- whether remote writes over Wi-Fi need additional safeguards later
