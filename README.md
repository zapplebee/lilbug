# lilbug

`lilbug` rev1 now centers on the ADR-defined HTTPS control plane:

- `lilbug-emulator`: a native Rust emulator with bootstrap and Wi-Fi startup modes
- `lilbug`: a CLI that initializes targets, stores trusted device records in `~/.config/lilbug.json`, and talks to the emulator over HTTPS
- `lilbug-core`: shared Rust types for rev1 config, state, commands, API payloads, and local CLI config

The old local TCP IPC and MQTT-first path has been replaced for rev1 work.

## Workspace

```text
lilbug/
├── Cargo.toml
├── crates/
│   ├── lilbug-cli/
│   ├── lilbug-core/
│   └── lilbug-emulator/
└── docs/
    ├── adr/
    └── local-development.md
```

## Implemented

- shared rev1 types for:
  - bootstrap init requests and responses
  - config and state payloads
  - command grammar parsing for `fwd:<ms>`, `back:<ms>`, `stop`, `brake`, and `face:<expression>`
  - JSON error responses
  - local CLI config records
- emulator startup modes:
  - `--mode bootstrap` for first-contact initialization
  - `--mode wifi` for normal authenticated operation
- HTTPS API routes:
  - `POST /v1/init`
  - `GET /v1/state`
  - `GET /v1/config`
  - `POST /v1/config`
  - `POST /v1/cmd`
  - `GET /v1/frame.png`
- Bearer-token auth on all non-bootstrap routes
- emulator persistence for nickname, Wi-Fi config, API key, and certificate material across restarts
- CLI target lookup via `~/.config/lilbug.json`
- PNG frame retrieval for debugging and multimodal tooling
- native emulator rendering with:
  - a `412x412` circular display area
  - a visible circular boundary ring
  - always-visible `[FORWARD]` and `[BACKWARD]` labels
  - dim inactive motion labels and highlighted active motion

## Start The Emulator

Bootstrap mode:

```bash
cargo run -p lilbug-emulator -- \
  --mode bootstrap \
  --https-addr 127.0.0.1:7443 \
  --wifi-base-url https://localhost:8443
```

Wi-Fi mode:

```bash
cargo run -p lilbug-emulator -- \
  --mode wifi \
  --https-addr 127.0.0.1:8443
```

Headless verification mode:

```bash
cargo run -p lilbug-emulator -- \
  --mode wifi \
  --https-addr 127.0.0.1:8443 \
  --headless \
  --run-for-ms 5000
```

Optional storage override:

```bash
cargo run -p lilbug-emulator -- --storage-dir /tmp/lilbug-state
```

## Use The CLI

Initialize a fresh bootstrap target and store a record in `~/.config/lilbug.json`:

```bash
cargo run -p lilbug-cli --bin lilbug -- \
  init anthony \
  --bootstrap-url https://localhost:7443 \
  --wifi-ssid lab-net \
  --wifi-password secretpass
```

Read state and config from a provisioned target:

```bash
cargo run -p lilbug-cli --bin lilbug -- state anthony
cargo run -p lilbug-cli --bin lilbug -- config get anthony
```

Mutate config over HTTPS:

```bash
cargo run -p lilbug-cli --bin lilbug -- config set anthony nickname bug-02
cargo run -p lilbug-cli --bin lilbug -- config set anthony wifi.ssid lab-net-2
```

Send rev1 commands:

```bash
cargo run -p lilbug-cli --bin lilbug -- cmd anthony fwd:300
cargo run -p lilbug-cli --bin lilbug -- cmd anthony back:300
cargo run -p lilbug-cli --bin lilbug -- cmd anthony stop
cargo run -p lilbug-cli --bin lilbug -- cmd anthony brake
cargo run -p lilbug-cli --bin lilbug -- cmd anthony face:happy
```

Retrieve the current frame as PNG:

```bash
cargo run -p lilbug-cli --bin lilbug -- frame anthony --out /tmp/lilbug-frame.png
```

Override the local config path during verification:

```bash
cargo run -p lilbug-cli --bin lilbug -- --config-path /tmp/lilbug.json state anthony
```

## Local CLI Config

The rev1 minimum file shape is:

```json
{
  "devices": {
    "anthony": {
      "base_url": "https://localhost:8443",
      "api_key": "lb_abcdef123456",
      "cert_fingerprint": "SHA256:..."
    }
  }
}
```

The current implementation also stores `cert_pem` so the CLI can trust the emulator's self-signed certificate directly while still recording the ADR-required fingerprint.

## HTTP API

Structured routes return JSON.
Frame retrieval returns PNG bytes.
Errors use JSON:

```json
{
  "code": "invalid_command",
  "message": "face command requires value"
}
```

All non-bootstrap routes require:

```http
Authorization: Bearer <api_key>
```

## Verification

Build and tests:

```bash
cargo build
cargo test
```

For a full repeatable bootstrap-to-Wi-Fi verification flow, including the exact CLI commands used to exercise each route, see `docs/local-development.md`.

## Implemented Vs Deferred

Implemented now:

- rev1 HTTPS emulator API
- CLI init/state/config/cmd/frame flows
- bearer auth
- persisted emulator core config
- PNG frame retrieval

Still deferred:

- real hardware USB provisioning transport
- firmware implementation on `esp-idf-hal`
- streamed-frame override transport details beyond the shared render-mode type
