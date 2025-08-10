# SparkleMotion Brain in Rust

This is the esp-idf-\{hal,svc\} version of the brain. It synthesizes
[examples](https://github.com/esp-rs/esp-idf-svc/blob/e0d9c76e83122ac991526a6c6f296b12cf698258/examples/tcp_async.rs)
and some chatter in the esp-rs matrix room.

The main reason we need esp-idf-svc is for RMII ethernet support.

## Supported Boards

This version of the brain only supports ESP32 as it requires RMII for Ethernet.

## Features

- Ethernet or WiFi (WiFi is enabled with `--no-default-features -F wifi`).
- Check in with Pinky
- Render PixelShader
- Re-send BrainHello when we haven't heard from Pinky in 5s
- Handle fragmented messages
- Handle Mapping messages
- Gamma Correction

## Creating Image for OTA

```
# Build for ethernet (default features)
cargo build --release
# Or WiFi (uncomment)
# cargo build --release --no-default-features -F wifi
# Create image
espflash save-image --chip esp32 target/xtensa-esp32-espidf/release/brainidf target/xtensa-esp32-espidf/release/brainidf.bin
# Copy to sparklemotion serving directory
VER=`git describe --always --tags`
COUNT=`git rev-list --count HEAD`
cp target/xtensa-esp32-espidf/release/brainidf.bin ~/sparklemotion/fw/rust-${COUNT}-${VER}.bin
# Restart sparklemotion so it is discovered
```

## TODO

- OTA firmware updates

### Development Setup

```bash
cargo install --locked espup espflash cargo-espflash && espup install -v 1.88.0 && . ~/export-esp.sh
```

### Run

```
cargo run --release
```

### WiFi Build

```
# Set SSID/PASSWORD in your environment vars somehow
cargo run --release --no-default-features -F wifi
```

### IDE Support

I recommend using VSCode to develop. First open a terminal and export the esp
env vars with `. ~/export-esp.sh`, then run `code .` in the same terminal so it
inherits the configuration.
