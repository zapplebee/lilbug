# ADR 0001: Use Rust and `esp-idf-hal` for firmware

## Status

Accepted

## Context

`lilbug` is the next evolution of `lilbud`, but on different hardware:

- `lilbud` targets an `RP2040` board in Rust
- `lilbug` is planned around the `Waveshare ESP32-S3-Touch-LCD-1.46B`
- `lilbug` also needs built-in networking for remote cues and control

We want to preserve as much of the `lilbud` software shape as possible:

- shared face and animation logic
- simulator-friendly development
- thin platform-specific backends

At the same time, `lilbud`'s embedded implementation cannot be reused directly because it is tied to:

- `rp2040-hal`
- a different display path
- a different resolution and board pinout

The main open question is the firmware stack for the `ESP32-S3` target.

## Decision

`lilbug` will remain a Rust project.

For the embedded firmware target, we will use `esp-idf-hal` rather than `esp-hal` for the first revision.

## Rationale

We are choosing `esp-idf-hal` because rev1 needs Wi-Fi, and `esp-idf-hal` gives a more practical path to networking on `ESP32-S3`.

Reasons:

- Wi-Fi is a first-class requirement for `lilbug`
- `esp-idf-hal` sits on top of Espressif's `ESP-IDF`, which provides the mature networking stack we need
- this reduces bring-up risk compared with optimizing for the lowest-level HAL path first
- it keeps the project in Rust while avoiding unnecessary platform work early on
- it fits the goal of getting face rendering, cue transport, and motor control working together as soon as possible

## Consequences

Positive:

- keep a single language across shared logic, simulator work, and firmware
- preserve the broad `lilbud` architecture without copying the RP2040-specific code
- easier path to Wi-Fi-enabled firmware in rev1
- more time can go into robot behavior and interface design instead of platform bring-up

Tradeoffs:

- firmware will not be `no_std` in the same way as `lilbud`'s RP2040 target
- ESP32 firmware setup will depend on the `ESP-IDF` toolchain and its build workflow
- some low-level portability is traded away in favor of faster delivery on this board

## What Carries Over From lilbud

These ideas should carry over:

- shared face state and animation model
- simulator-first development
- a clean separation between shared logic and board-specific output
- multiple targets with thin backends

These implementation details do not carry over directly:

- `rp2040-hal`
- the current RP2040 display driver
- the `240x240` display assumptions

## Follow-on Design Direction

The expected shape is:

- shared Rust modules for face state, animation, and cue handling
- a simulator target for desktop or browser preview
- an `ESP32-S3` firmware target using `esp-idf-hal`
- a motor control module that can be mocked in the simulator and implemented on-device

## Deferred Questions

- how much face logic can be reused directly from `lilbud` versus adapted
- whether the simulator should be desktop-only at first or also include a browser target
- what transport to use first for cues: HTTP, WebSocket, MQTT, or something simpler
- whether rev1 should support PWM speed control or only direction control for the motor
