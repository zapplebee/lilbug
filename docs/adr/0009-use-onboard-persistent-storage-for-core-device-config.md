# ADR 0009: Use onboard persistent storage for core device config

## Status

Accepted

## Context

`lilbug` needs to preserve core configuration across restarts.

That includes at minimum:

- Wi-Fi credentials
- device nickname or identity
- HTTPS certificate material or trust material
- API key used for normal CLI interaction
- other small core configuration values needed for bring-up and normal operation

The board has removable storage options, but core device behavior should not depend on removable media being present.

## Decision

`lilbug` will use onboard persistent storage as the primary store for core device configuration.

The removable storage path, such as an SD or TF card, is not the primary configuration store for rev1.

## Rationale

Reasons for this decision:

- core config must survive restart even when no removable card is present
- Wi-Fi credentials and auth material are small and fit naturally in onboard persistent storage
- removable media should not be a prerequisite for basic device behavior
- this gives a cleaner bring-up and recovery story

## Consequences

Positive:

- the device remains self-contained for core operation
- restart persistence does not depend on card insertion or card health
- bootstrap and normal operation stay simpler

Tradeoffs:

- onboard persistent storage has tighter space limits than removable media
- config schema changes may need migration care later
- large assets still need a separate storage story if they arrive later

## Design Direction

Use onboard persistent storage for:

- Wi-Fi config
- nickname / device identity
- API key
- HTTPS certificate or trust material
- small runtime settings required for normal operation

Removable storage may be used later for:

- larger assets
- captured data
- media or frame caches if ever needed

## Non-Goals

This ADR does not define:

- the exact firmware storage backend API
- the exact format used to encode config in storage
- whether additional optional data also belongs in removable storage later

## Deferred Questions

- exact onboard persistence mechanism on the chosen ESP32 stack
- config migration strategy if the schema changes later
- whether certificate material should be stored as PEM, DER, or another internal representation
