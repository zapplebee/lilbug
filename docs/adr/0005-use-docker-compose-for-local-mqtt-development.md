# ADR 0005: Use Docker Compose for local MQTT development

## Status

Deferred

## Note

This ADR remains a possible future direction if MQTT returns later, but it is no longer part of the chosen rev1 control path after ADR 0010.

## Context

`lilbug` uses MQTT for runtime cues. Development needs a repeatable local broker setup that is easy to start, inspect, and reset.

That setup should:

- work on a typical local development machine without custom broker installation steps
- be easy to document and reproduce
- preserve broker state across restarts when desired
- keep broker data visible on disk for inspection and cleanup

We explicitly do not want to hide broker persistence inside an opaque Docker named volume.

## Decision

Local MQTT development for `lilbug` will use Docker Compose.

Broker persistence will use a bind-mounted directory in the repo or project-adjacent filesystem location, not a Docker named volume.

## Rationale

Reasons for this decision:

- Docker Compose gives the project a standard one-command local broker workflow
- a bind-mounted directory makes broker state inspectable and easier to reason about
- bind mounts make cleanup, backup, and intentional reset simpler than named volumes
- visible on-disk data is better for debugging retained messages and broker persistence behavior
- this keeps local development environment setup explicit and documented

## Consequences

Positive:

- repeatable local MQTT setup
- easier troubleshooting of retained messages and persistent session behavior
- no hidden Docker-managed storage state
- easier to document exact local development steps

Tradeoffs:

- bind-mounted data directories need to be managed intentionally
- file ownership and permissions may need attention on some systems
- repo documentation needs to be clear about which directories are data and should be gitignored

## Expected Shape

The intended direction is:

- a committed `docker-compose.yml` or equivalent Compose file
- a documented broker config file if custom configuration is needed
- a bind-mounted local data directory such as `var/mqtt/` or similar
- that data directory ignored from version control

## Non-Goals

This ADR does not require:

- production deployment via Docker Compose
- a specific broker implementation beyond requiring MQTT compatibility
- elaborate multi-service orchestration in rev1

## Deferred Questions

- exact broker choice and image tag
- exact on-disk location for the bind-mounted persistence directory
- whether broker config should be minimal default config or a committed custom config file
