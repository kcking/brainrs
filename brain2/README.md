# Brain-rs

> sparklemotion brain rust impl

## TODO

- implement ethernet
  - looks like we need to switch to esp-idf-svc (but can still run embassy from there if we want) [matrix chat](https://matrix.to/#/!YoLPkieCYHGzdjUhOK:matrix.org/$pcFvtrMFgvJH10Aq6UyI7J2C5-KZNANFy5rOng4e3fs?via=matrix.org&via=beeper.com&via=tchncs.de)

## Questions

- What happens when messageid overflows? it is an i16

## Random dev notes

- WIFI creds must be specified in env vars at compile (`WIFI_SSID` and `WIFI_PASSWORD`). One could add these to the end of their `~/export-esp.sh` script for convenience.

- if building for an xtensa board, `cargo install -f espup && espup install -v 1.88.0 && . ~/export-esp.sh`
  - if you see `error: linker`xtensa-esp32-elf-gcc`not found`, `. ~/export-esp.sh` is the answer :)
    `
- if rust-analyzer for toolchain `esp` is not found, just symlink the one from your host toolchain (replace with your host OS toolchain in the command below)

```
ln -sf ~/.rustup/toolchains/nightly-aarch64-apple-darwin/bin/rust-analyzer ~/.rustup/toolchains/esp/bin/rust-analyzer
```

- make sure to run `. ~/export-esp.sh` before trying to compile / opening vim etc. if building for an xtensa board
