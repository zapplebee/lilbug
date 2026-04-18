# ADR 0014: Define the local CLI config file

## Status

Accepted

## Context

`lilbug-cli` needs a local store for remembering provisioned or known devices.

The CLI must be able to retain enough information to:

- look up a device by nickname
- connect to its last known HTTPS endpoint
- authenticate with its Bearer token
- trust its self-signed certificate or certificate fingerprint

For rev1, we are explicitly willing to store this material in plaintext while iterating.

## Decision

The CLI will store local device records in `~/.config/lilbug.json`.

The file will contain a top-level `devices` object keyed by nickname.

Each device record should include at minimum:

- `base_url`
- `api_key`
- `cert_fingerprint`

## Rationale

Reasons for this decision:

- a single JSON file is easy to inspect and edit during iteration
- nickname-keyed lookup makes the CLI ergonomic
- storing HTTPS target, token, and trust material is enough for normal operation
- plaintext is acceptable for rev1 speed and simplicity

## Expected Shape

The intended file shape is:

```json
{
  "devices": {
    "anthony": {
      "base_url": "https://192.168.1.42",
      "api_key": "lb_abcdef123456",
      "cert_fingerprint": "SHA256:..."
    }
  }
}
```

The exact schema may grow, but this is the rev1 minimum.

## Design Direction

The CLI should:

- create the file if it does not exist
- update or replace a device record when reprovisioning the same nickname intentionally
- use nickname as the primary human-facing target selector

Examples:

- `lilbug-cli state anthony`
- `lilbug-cli cmd anthony fwd:300`
- `lilbug-cli frame anthony --out frame.png`

## Consequences

Positive:

- simple local operator experience
- easy inspection while iterating
- no additional system integration such as keychain required in rev1

Tradeoffs:

- local credentials are stored in plaintext
- hostname or IP drift may require updates if devices move on the network
- rev1 does not yet attempt stronger local secret handling

## Non-Goals

This ADR does not define:

- keychain integration
- encrypted local storage
- sync or sharing of config across machines
- a full schema versioning system for the config file

## Deferred Questions

- whether to store full certificate PEM instead of only a fingerprint
- whether to keep additional metadata such as nickname aliases, last seen IP, or last seen time
- whether a schema version field should be added earlier rather than later
