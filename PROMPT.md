# lilbug autonomous implementation prompt

You are working in `~/github.com/zapplebee/lilbug`.

Your goal is to implement as much of the already-decided `lilbug` rev1 architecture as possible without interactive guidance.

Assume the user is away for a long session and wants you to use the full available budget productively. Do not stop after the first working version. Keep going through the highest-value work that is already defined in the ADRs until you hit a true blocker or run out of meaningful ADR-backed work.

## Core rule

Only do work that is already defined, implied, or required by the accepted ADRs in `docs/adr/`.

Do not invent new product scope.
Do not add new transports, new UI surfaces, or new behaviors that are not already described in the ADRs.
If you encounter an ambiguity, choose the smallest reasonable implementation that stays within the ADRs.

## Working style

- Work autonomously.
- Prefer the smallest correct implementation first, then continue hardening and polishing within ADR scope.
- Do not stop at planning.
- Carry work through implementation, verification, documentation, and cleanup.
- Do not create git commits unless explicitly asked.
- Do not revert unrelated user changes.
- If there are several valid implementation options, choose the simplest one that keeps the design aligned with the ADRs.

## Non-interactive execution rule

Do not ask follow-up questions unless one of these is true:

- a required external credential or service is missing and there is no safe local fallback
- the repo contains conflicting user changes that block implementation
- a hard tooling limitation prevents progress

Otherwise, choose a sensible path and continue.

## ADRs to follow

Read and follow all relevant ADRs before making changes, especially:

- `docs/adr/0001-use-rust-and-esp-idf-hal.md`
- `docs/adr/0002-use-usb-cli-for-provisioning-and-config.md`
- `docs/adr/0004-use-a-native-only-software-emulator.md`
- `docs/adr/0006-add-a-debug-admin-interface-for-config-state-and-frame-access.md`
- `docs/adr/0007-support-local-render-and-streamed-frame-override-modes.md`
- `docs/adr/0008-share-inspection-and-admin-semantics-over-usb-and-wi-fi.md`
- `docs/adr/0009-use-onboard-persistent-storage-for-core-device-config.md`
- `docs/adr/0010-use-https-and-bearer-token-auth-for-rev1-control-plane.md`
- `docs/adr/0011-use-emulator-startup-modes-for-bootstrap-and-wifi-flows.md`
- `docs/adr/0012-define-the-rev1-http-api-surface.md`
- `docs/adr/0013-define-rev1-command-grammar.md`
- `docs/adr/0014-define-the-local-cli-config-file.md`
- `docs/adr/0015-separate-motion-and-face-command-lanes.md`

Also note:

- `ADR 0003` is superseded by `ADR 0010`
- `ADR 0005` is deferred and is not part of the rev1 implementation path

## Important constraints already decided

- project language: Rust
- firmware direction: `esp-idf-hal`
- rev1 normal control plane: HTTPS over Wi-Fi
- rev1 auth: Bearer token API key
- local CLI config: plaintext `~/.config/lilbug.json` during iteration
- first-contact and recovery path on real hardware: USB provisioning/config path
- emulator target: native only, not browser/wasm/webview
- emulator should use startup modes for bootstrap and Wi-Fi flows
- emulator should be scriptable and suitable for unattended testing
- emulator render target must match device pixel size where required by ADRs
- emulator must show a visible circular display boundary
- emulator must show `[FORWARD]` lower-left and `[BACKWARD]` lower-right, always visible and dimmed when inactive
- rev1 command grammar includes forms like `fwd:300`, `back:300`, `stop`, `brake`, `face:happy`
- motion and face are separate command lanes
- timed motion commands must auto-expire after their duration unless replaced by a newer motion command
- motion commands interrupt only motion
- face commands interrupt only face
- rev1 HTTP API surface is:
  - `POST /v1/init`
  - `GET /v1/state`
  - `GET /v1/config`
  - `POST /v1/config`
  - `POST /v1/cmd`
  - `GET /v1/frame.png`
- all non-bootstrap routes require Bearer auth
- `/v1/init` is bootstrap-only and should fully reset/reprovision the emulator or device when called in bootstrap mode
- core device config should be treated as persistent across restarts conceptually, and emulator work should move toward that where ADRs support it

## Known drift to fix

The current repo already contains some implementation work, but there is still significant drift between the ADRs and the code.

Treat the following as high-priority correction work for this run:

1. `init` reset semantics
   - `init` in bootstrap mode should fully reset and reprovision the emulator/device instead of failing because prior state exists.

2. Timed motion semantics
   - `fwd:<duration_ms>` and `back:<duration_ms>` must actually run for the requested time and then stop automatically.
   - a newer motion command must cancel the previously scheduled motion expiry.

3. Separate motion and face lanes
   - face commands must not interrupt motion commands.
   - motion commands must not interrupt face commands.
   - move away from an implicit single-command model if needed.

4. HTTPS identity for Wi-Fi targets
   - do not leave the implementation effectively limited to `localhost`.
   - the trust/certificate story should support the intended HTTPS-over-Wi-Fi direction rather than only a localhost emulator loop.

5. Emulator UI correctness
   - the `[BACKWARD]` indicator must be fully visible on screen.
   - do not leave documented emulator UI elements clipped or partially off-screen.

6. Verification drift
   - verification should cover the real user-facing flows implied by the ADRs, including timed motion behavior, reset/reprovision behavior, and the CLI flows.

## Main objective

Implement the CLI and emulator around the rev1 HTTPS control plane and the defined ADRs.

The CLI should become the primary operator tool.
The emulator should become the primary local test target.

## Deliverables

Implement as many of these as possible, in order, without leaving ADR scope:

1. Shared Rust types for rev1 config, state, command grammar, API requests, and API responses
2. A Rust CLI that supports the ADR-backed flows
3. A native Rust emulator with bootstrap and Wi-Fi modes
4. An HTTPS server in the emulator implementing the rev1 API surface
5. Bearer-token authentication on the emulator's normal-operation routes
6. Local CLI config handling in `~/.config/lilbug.json`
7. Frame retrieval as PNG for debug/multimodal tooling
8. Enough persistence in the emulator to make bootstrap-to-Wi-Fi flows meaningful across restarts if feasible within ADR scope
9. Automated tests and local verification coverage
10. Documentation updates reflecting the real implemented state

## Strong priority order

Work in this order unless you hit a blocker:

1. Replace any old MQTT- or local-IPC-centered emulator/CLI path with the ADR-defined HTTPS path
2. Correct the known ADR drift listed above before treating the implementation as complete
3. Implement the rev1 HTTP API surface in the emulator
4. Implement CLI target lookup and token/cert trust using `~/.config/lilbug.json`
5. Implement `init`, `state`, `config get`, `config set`, `cmd`, and `frame` CLI flows
6. Implement emulator startup modes for bootstrap and Wi-Fi flows
7. Implement frame PNG retrieval
8. Improve tests and verification
9. Fix defects, rough edges, and documentation gaps

Do not preserve outdated architecture just because it already exists in the repo.
If current code conflicts with the ADRs, update it to match the ADRs.

## CLI scope

Build or update `lilbug-cli` so it can at minimum:

- initialize a new emulator/device through the bootstrap path
- treat `init` in bootstrap mode as a full reset/reprovision operation, replacing prior persisted config and auth material for that target
- store local device records in `~/.config/lilbug.json`
- target a known device by nickname
- call `GET /v1/state`
- call `GET /v1/config`
- call `POST /v1/config`
- call `POST /v1/cmd`
- call `GET /v1/frame.png`
- surface useful errors

The CLI should follow the ADR-defined command grammar and the ADR-defined local config shape.

The CLI verification and docs should reflect the independent motion/face command semantics and the reset/reprovision behavior.

## Emulator scope

Build or update the emulator so it can at minimum:

- start in a bootstrap-oriented mode
- start in a Wi-Fi / normal-operation mode
- allow `POST /v1/init` in bootstrap mode to wipe and replace prior persisted emulator state cleanly
- render the required display surface and debug indicators
- expose the rev1 HTTPS API surface
- enforce Bearer auth on non-bootstrap routes
- return the current frame as PNG
- apply config and command changes through the HTTP API
- implement independent motion and face command lanes
- implement timed motion expiry and replacement behavior
- support inspection and control flows that a human or automation can run locally

The emulator is the test stand-in for the device. Keep it practical and scriptable.

## Implementation preferences

Use these defaults unless the codebase strongly suggests a better option that still fits the ADRs:

- Rust workspace or small multi-binary crate layout
- `clap` for CLI parsing
- `serde` and `serde_json` for structured payloads
- a lightweight Rust HTTP server/client stack
- a lightweight native rendering/window crate suitable for a fixed-size emulator window
- straightforward integration tests for parsing, auth, config handling, and HTTP route behavior

Prefer explicit state modeling over continuing to pile logic onto a single last-command field if that field no longer matches the ADRs.

## What not to build

- do not reintroduce MQTT as the primary rev1 path
- do not add browser targets
- do not add webview targets
- do not add keychain integration
- do not add encrypted local secret storage yet
- do not invent extra HTTP routes beyond what ADRs require unless necessary to satisfy the defined routes cleanly
- do not implement frame streaming unless it is directly needed by an accepted ADR-backed task already in scope

## Acceptance criteria

The work is only done when all of the following are true, or when you hit a real external blocker:

1. The repo builds successfully.
2. The CLI and emulator reflect the ADR-defined rev1 HTTPS control plane rather than the old MQTT/local-IPC direction.
3. There is a documented command to start the emulator in bootstrap mode.
4. There is a documented command to start the emulator in Wi-Fi mode.
5. There is a documented command to run the CLI.
6. The CLI can initialize a fresh emulator/device target in the defined bootstrap flow.
7. Re-running `init` against a bootstrap-mode target fully resets and reprovisions it instead of failing because prior state exists.
8. The CLI can store and reuse device records from `~/.config/lilbug.json`.
9. The CLI can get config and state from a provisioned target.
10. The CLI can send rev1 commands like `fwd:300`, `back:300`, `stop`, `brake`, and `face:happy`.
11. Timed motion commands actually expire automatically after their duration unless replaced by a newer motion command.
12. Motion and face commands behave as separate lanes and do not cancel each other.
13. Every implemented CLI flow is actually exercised during verification, not just compiled.
14. The verification evidence includes the real commands used for each CLI flow.
15. The emulator visibly renders:
   - the device-sized display area required by the ADRs
   - a circular visible display boundary
   - lower-left `[FORWARD]`
   - lower-right `[BACKWARD]`
   - dim inactive motion labels
16. The emulator can return the current frame as PNG.
17. The HTTPS trust story and implementation are not effectively limited to `localhost` only.
18. There are automated tests for the shared command/config logic and as much API/auth/config handling as is practical.
19. There is at least one repeatable documented end-to-end verification flow a human can run locally.
20. README/docs reflect the actual implemented architecture.

## Exhaustion rule

Do not stop after satisfying the minimum acceptance criteria if there is still meaningful ADR-backed work left.

After the main flow works, continue spending the remaining session on, in this order:

1. fixing defects or architectural drift from the ADRs
2. improving API and CLI error handling
3. improving tests
4. improving bootstrap and restart verification
5. removing obsolete code paths that conflict with the current ADRs
6. tightening docs and examples
7. polishing emulator layout issues and obvious UX defects that violate the defined requirements

Only stop when:

- the remaining work would require new product decisions not covered by the ADRs
- or you are blocked by tooling/environment limits

## Verification expectations

Run the relevant tests and build commands yourself.

At minimum, verify:

- build passes
- tests pass
- CLI bootstrap flow works as far as practical locally
- re-running `init` in bootstrap mode replaces prior persisted target state cleanly
- CLI can talk to the emulator over the implemented HTTPS path
- CLI can retrieve state/config and send commands
- timed motion commands expire on their own
- a newer motion command replaces the prior motion command cleanly
- face commands do not cancel active motion
- frame retrieval works and produces a real image artifact
- emulator startup modes behave as documented

Exercise every implemented CLI flow with real commands.

That includes, if implemented:

- `init`
- `state`
- `config get`
- `config set`
- `cmd` with representative motion and face commands
- `frame`

Use real command sequences that prove the lane semantics, such as:

- a timed motion command that later returns to stop on its own
- a timed motion command replaced by a newer motion command
- a face command issued while motion is active without cancelling the motion
- re-running `init` to prove reset/reprovision behavior

Do not claim a CLI flow works unless you actually ran it successfully or clearly document the exact external blocker that prevented running it.

When practical, actually run the emulator and CLI locally rather than only compiling them.

If some verification remains manual, document exactly what remains and why.

## Documentation updates

Update the repo docs to reflect what now exists.

At minimum, update documentation for:

- project structure
- how to start the emulator in each mode
- how to run the CLI
- the local config file shape
- the HTTP API surface
- the command grammar
- the motion/face lane semantics
- the reset/reprovision behavior of `init`
- what is implemented versus still planned

If `README.md` no longer reflects reality, update it.

## Final response requirements

When done, provide:

- what you implemented
- what commands you ran to verify it
- what passed
- what remains blocked or intentionally deferred

List the CLI verification commands explicitly and map each one to the flow it proved.
