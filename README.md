# zfetch

A small native Linux system summary with a restrained copper and warm-white interface.

The production binary uses Rust `no_std`, direct Linux system calls and bounded buffers. It has no Cargo dependencies, allocator, libc, child processes, network access or resident service. Default output omits user names, host names, IP addresses and serial numbers.

```sh
cargo build --release --locked --offline
./target/release/zfetch
./target/release/zfetch --json
```

Linux x86_64 and aarch64. Missing fields are `null` in JSON and `unavailable` in text. `--no-color`, redirected output and `NO_COLOR` disable color. More usage, measurements and installation instructions are added alongside verified release evidence.

MIT.
