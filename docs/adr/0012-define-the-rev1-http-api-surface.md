# ADR 0012: Define the rev1 HTTP API surface

## Status

Accepted

## Context

`lilbug` rev1 now uses HTTPS and Bearer token auth as its primary normal-operation control plane.

To keep the CLI, emulator, and future device firmware aligned, the core HTTP surface needs to be defined explicitly before further implementation expands in different directions.

The API must support:

- bootstrap / initialization of a fresh device or emulator
- config inspection
- state inspection
- config mutation
- command execution
- current frame retrieval as an image

## Decision

The rev1 HTTP API surface is:

- `POST /v1/init`
- `GET /v1/state`
- `GET /v1/config`
- `POST /v1/config`
- `POST /v1/cmd`
- `GET /v1/frame.png`

All non-bootstrap routes should require Bearer token authentication.

`/v1/init` is only available when the emulator or device is in bootstrap mode.

## Rationale

Reasons for this decision:

- this route set is small but covers the core operator workflows
- it matches the CLI-centered development approach
- it cleanly separates structured JSON endpoints from image retrieval
- it is sufficient for both emulator development and future hardware integration

## Route Summary

### `POST /v1/init`

Purpose:

- initialize an unprovisioned emulator or device
- install core config required for normal operation

Expected payload direction:

- Wi-Fi credentials
- nickname / device identity
- API key

Expected response direction:

- target base URL
- API key
- HTTPS certificate or certificate fingerprint for later trust pinning

Behavior:

- valid only in bootstrap mode
- in bootstrap mode, it may fully reset and reprovision the target rather than rejecting a repeat init
- for the emulator, bootstrap may use an insecure convenience path so the CLI can capture trust material for later HTTPS use
- for real hardware, first-contact provisioning is expected to happen over USB

### `GET /v1/state`

Purpose:

- return current runtime state

Expected response direction:

- current face / render mode summary
- motor state
- network or readiness summary as appropriate
- other current runtime data useful to the CLI

### `GET /v1/config`

Purpose:

- return current configuration suitable for operator inspection

### `POST /v1/config`

Purpose:

- mutate allowed configuration values after provisioning

### `POST /v1/cmd`

Purpose:

- execute high-level commands such as motion or face changes

Expected request direction:

- command name
- optional value or argument
- optional duration in milliseconds where relevant

### `GET /v1/frame.png`

Purpose:

- return the currently displayed frame as a PNG image

This endpoint exists specifically to support debugging, inspection, and multimodal tooling.

## Authentication

All normal-operation routes require:

```http
Authorization: Bearer <api_key>
```

`POST /v1/init` is the bootstrap exception and is only exposed in bootstrap mode.

## Response Direction

Expected rev1 response shape:

- JSON for all structured API routes
- PNG binary response for `GET /v1/frame.png`

Error responses should also use JSON with a clear machine-readable and human-readable error shape.

## Relationship to CLI

The CLI should map closely onto this surface.

Examples:

- `lilbug-cli init ...` -> `POST /v1/init`
- `lilbug-cli state --nickname <target>` -> `GET /v1/state`
- `lilbug-cli config get --nickname <target>` -> `GET /v1/config`
- `lilbug-cli config set --nickname <target> ...` -> `POST /v1/config`
- `lilbug-cli cmd --nickname <target> fwd:300` -> `POST /v1/cmd`
- `lilbug-cli frame --nickname <target> --out frame.png` -> `GET /v1/frame.png`

## Non-Goals

This ADR does not define:

- the exact JSON schema for every request and response body
- the exact internal config patch format used by `POST /v1/config`
- the exact command grammar used inside `POST /v1/cmd`
- any future streaming or websocket routes

## Deferred Questions

- exact JSON body shapes for each route
- whether `POST /v1/config` should be patch-like or command-like
- whether `GET /v1/state` should include condensed config references or only runtime data
- whether a future `GET /v1/health` endpoint is useful or unnecessary for rev1
