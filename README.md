# zfetch

A small native Linux system summary. Warm white, copper, quiet spacing.

The official v0.1.0 x86_64 executable is **22,808 bytes** (about 22.3 KiB). It uses Rust `no_std`, fixed buffers and direct Linux system calls: no Cargo dependencies, allocator, libc, external commands, network requests or background service. Default output omits user names, host names, IP addresses and serial numbers.

[Releases](https://github.com/LIghtJUNction/zfetch/releases) · [Measurements](docs/performance.md) · [Design](docs/design.md) · [oh-my-ztty](https://github.com/LIghtJUNction/oh-my-ztty)

## Use

```sh
zfetch
zfetch --json
zfetch --no-color
```

The default view shows OS, kernel, architecture, CPU, online thread count, memory, uptime, configured shell and terminal identifier. Missing fields are `unavailable` in text and `null` in JSON. Redirecting output or setting `NO_COLOR` disables ANSI color. No patched font is required.

## Install

On Arch Linux, install [zfetch-project-z-bin](https://aur.archlinux.org/packages/zfetch-project-z-bin) for the official binary, or [zfetch-project-z](https://aur.archlinux.org/packages/zfetch-project-z) to build from source:

```sh
yay -S zfetch-project-z-bin
zfetch
```

Both packages install the command `zfetch`. AUR already has an unrelated older project named `zfetch`; the package names distinguish Project Z and declare a conflict so one installation cannot overwrite the other.

Download the archive for Linux x86_64 or aarch64 and `SHA256SUMS` from [Releases](https://github.com/LIghtJUNction/zfetch/releases), verify the checksum, then extract it. Install the executable to a user-owned command directory:

```sh
sha256sum --check --ignore-missing SHA256SUMS
tar -xzf zfetch-0.1.0-linux-x86_64.tar.gz
install -Dm755 zfetch-0.1.0-linux-x86_64/bin/zfetch "$HOME/.local/bin/zfetch"
```

The archive also includes a manual, zsh completion and license. To build from source with Rust 1.100 or newer:

```sh
git clone https://github.com/LIghtJUNction/zfetch.git
cd zfetch
make check
make install PREFIX="$HOME/.local"
```

`make uninstall PREFIX="$HOME/.local"` removes the files installed by that target. zfetch has no state, cache, daemon or login-shell hook to remove.

## Performance with its scope

In 100 interleaved warm runs on one x86_64 developer machine, plain output took **0.429 ms median** and JSON took **0.385 ms median**, including Python process creation and waiting. fastfetch 2.71.0's same core module group took **2.503 ms median** in JSON. Their fields are not identical: fastfetch offers more probes and a wider feature set.

The JSON memory sample was **60 KiB RSS** while blocked on its output pipe, compared with **4,996 KiB RSS** for the fastfetch core-module command. These are resident samples at a known point, not peak memory guarantees. The separate stock-default fastfetch result, commands, p95 values and raw records are in [Performance](docs/performance.md).

## Boundaries

Linux x86_64 and aarch64 are supported. Shell means the `SHELL` environment variable's basename; terminal means `TERM_PROGRAM`, falling back to `TERM`. CPU model information is bounded to the first 4 KiB of `/proc/cpuinfo`. Memory used is `MemTotal - MemAvailable`. The summary intentionally does not enumerate packages, disks, GPUs, displays, sensors or desktop themes.

This is part of Project Z. It can be bundled as a pinned Git submodule and used without starting an agent or opening a connection. MIT.
