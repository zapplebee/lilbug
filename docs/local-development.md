# Local Development

## Rev1 Architecture

The current local loop mirrors the accepted rev1 direction:

- bootstrap mode exposes only `POST /v1/init`
- Wi-Fi mode exposes the normal HTTPS API
- all non-bootstrap routes require Bearer auth
- the CLI stores known targets in `~/.config/lilbug.json`
- the emulator persists core config across restarts inside its storage directory
- re-running `init` in bootstrap mode replaces the prior persisted config for that target
- motion and face are independent command lanes

## Repeatable Verification Flow

The following is the exact flow used to verify the current implementation locally.

### 1. Build and test

```bash
cargo build
cargo test
```

### 2. Start the emulator in bootstrap mode

```bash
cargo run -p lilbug-emulator -- \
  --mode bootstrap \
  --https-addr 127.0.0.1:7443 \
  --wifi-base-url https://127.0.0.1:8443 \
  --storage-dir /tmp/lilbug-state
```

For unattended verification:

```bash
cargo run -p lilbug-emulator -- \
  --mode bootstrap \
  --https-addr 127.0.0.1:7443 \
  --wifi-base-url https://127.0.0.1:8443 \
  --storage-dir /tmp/lilbug-state \
  --headless
```

### 3. Initialize the target through the bootstrap route

```bash
cargo run -p lilbug-cli --bin lilbug -- \
  --config-path /tmp/lilbug.json \
  init \
  --nickname anthony \
  --bootstrap-url https://127.0.0.1:7443 \
  --wifi-ssid lab-net \
  --wifi-password secretpass
```

This proves:

- `POST /v1/init` works
- the CLI can create and save a device record
- the CLI stores API key and trust material for later Wi-Fi use

In the emulator, this bootstrap path is intentionally a convenience flow, not the final hardware provisioning transport. The real hardware equivalent is expected to happen over USB.

Re-run `init` with different values before switching to Wi-Fi mode to prove reset/reprovision semantics:

```bash
cargo run -p lilbug-cli --bin lilbug -- \
  --config-path /tmp/lilbug.json \
  init \
  --nickname anthony \
  --bootstrap-url https://127.0.0.1:7443 \
  --wifi-ssid lab-net-2 \
  --wifi-password newerpass \
  --api-key lb_second
```

This proves the bootstrap target now wipes and replaces its persisted config instead of rejecting the second init request.

### 4. Restart the emulator in Wi-Fi mode

```bash
cargo run -p lilbug-emulator -- \
  --mode wifi \
  --https-addr 127.0.0.1:8443 \
  --storage-dir /tmp/lilbug-state
```

This proves persisted config survives the bootstrap-to-Wi-Fi restart.

### 5. Exercise every implemented CLI flow

Get state:

```bash
cargo run -p lilbug-cli --bin lilbug -- --config-path /tmp/lilbug.json state --nickname anthony
```

Get config:

```bash
cargo run -p lilbug-cli --bin lilbug -- --config-path /tmp/lilbug.json config get --nickname anthony
```

Set config:

```bash
cargo run -p lilbug-cli --bin lilbug -- --config-path /tmp/lilbug.json config set --nickname anthony nickname bug-02
cargo run -p lilbug-cli --bin lilbug -- --config-path /tmp/lilbug.json config set --nickname bug-02 wifi.ssid lab-net-3
```

This also renames the local CLI record key from `anthony` to `bug-02`.

Send commands:

```bash
cargo run -p lilbug-cli --bin lilbug -- --config-path /tmp/lilbug.json cmd --nickname bug-02 fwd:300
cargo run -p lilbug-cli --bin lilbug -- --config-path /tmp/lilbug.json state --nickname bug-02
sleep 1
cargo run -p lilbug-cli --bin lilbug -- --config-path /tmp/lilbug.json state --nickname bug-02

cargo run -p lilbug-cli --bin lilbug -- --config-path /tmp/lilbug.json cmd --nickname bug-02 fwd:800
sleep 0.1
cargo run -p lilbug-cli --bin lilbug -- --config-path /tmp/lilbug.json cmd --nickname bug-02 back:300
cargo run -p lilbug-cli --bin lilbug -- --config-path /tmp/lilbug.json state --nickname bug-02
sleep 1
cargo run -p lilbug-cli --bin lilbug -- --config-path /tmp/lilbug.json state --nickname bug-02

cargo run -p lilbug-cli --bin lilbug -- --config-path /tmp/lilbug.json cmd --nickname bug-02 fwd:800
sleep 0.1
cargo run -p lilbug-cli --bin lilbug -- --config-path /tmp/lilbug.json cmd --nickname bug-02 face:happy
cargo run -p lilbug-cli --bin lilbug -- --config-path /tmp/lilbug.json state --nickname bug-02
sleep 1
cargo run -p lilbug-cli --bin lilbug -- --config-path /tmp/lilbug.json state --nickname bug-02

cargo run -p lilbug-cli --bin lilbug -- --config-path /tmp/lilbug.json cmd --nickname bug-02 stop
cargo run -p lilbug-cli --bin lilbug -- --config-path /tmp/lilbug.json cmd --nickname bug-02 brake
```

These command sequences prove:

- timed motion expires automatically
- a newer motion command replaces the older timed motion
- `face:happy` does not cancel active motion
- `stop` and `brake` only affect the motion lane

Retrieve frame:

```bash
cargo run -p lilbug-cli --bin lilbug -- --config-path /tmp/lilbug.json frame --nickname bug-02 --out /tmp/lilbug-frame.png
file /tmp/lilbug-frame.png
```

Expected result:

- `file` reports a PNG artifact
- the frame dimensions are `412 x 480`

### 6. Verify persistence after config mutation

After the config mutations above, restart Wi-Fi mode again and re-run `state --nickname bug-02`.
The returned config should still show `nickname: bug-02`, and the old nickname should no longer be the local lookup key.

## Manual Visual Checklist

When the native emulator window is open, verify:

- the upper display area is `412x412`
- the display boundary is visibly circular
- `[FORWARD]` is always visible in the lower-left
- `[BACKWARD]` is always visible in the lower-right
- inactive motion labels are dimmed
- the active motion label is emphasized when motion commands run

## HTTP Surface Summary

- `POST /v1/init`: bootstrap only
- `GET /v1/state`: authenticated
- `GET /v1/config`: authenticated
- `POST /v1/config`: authenticated
- `POST /v1/cmd`: authenticated
- `GET /v1/frame.png`: authenticated

## Implemented Vs Planned

Implemented now:

- shared rev1 types
- CLI config persistence in `~/.config/lilbug.json`
- HTTPS emulator server with Bearer auth
- emulator startup modes
- bootstrap reprovision reset behavior
- independent motion and face lanes with timed motion expiry
- PNG frame retrieval

Still planned later:

- real USB provisioning transport for hardware
- hardware firmware implementation on `esp-idf-hal`
- streamed-frame override transport work beyond the current config/state scaffolding
