esp-idf-svc version of the brain. synthesizes [examples](https://github.com/esp-rs/esp-idf-svc/blob/e0d9c76e83122ac991526a6c6f296b12cf698258/examples/tcp_async.rs) and some chatter in the esp-rs matrix room.

The main reason we need esp-idf-svc is for RMII ethernet support.

- should we use same ID for both wifi and eth? (MAC addresses are different between the two interfaces).

Run

```
# debug mode causes a stack overflow
cargo run --release
```
