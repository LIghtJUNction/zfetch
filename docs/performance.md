# Performance

This record compares zfetch 0.1.0 with fastfetch 2.71.0 on one x86_64 developer machine on 2026-11-20 (UTC+08). Other development work was active. Each command ran 100 times after three warmups, with round-robin ordering to reduce drift.

| Command | Median wall time | p95 wall time | Sampled RSS | Sampled PSS |
| --- | ---: | ---: | ---: | ---: |
| zfetch plain | 0.429 ms | 1.062 ms | 64 KiB | 60 KiB |
| zfetch JSON | 0.385 ms | 0.997 ms | 60 KiB | 56 KiB |
| fastfetch core JSON | 2.503 ms | 5.340 ms | 4,996 KiB | 1,915 KiB |
| fastfetch stock default | 15.184 ms | 26.604 ms | 12,508 KiB | 7,035 KiB |

Wall time includes Python's process creation, the executable and waiting. RSS/PSS come from three `smaps_rollup` samples after `wchan` confirms that the process is blocked on a prefilled output pipe. This is a reproducible resident sample after collection, not a whole-run peak or an idle-service measurement.

## Comparable scope

The core command asks fastfetch for `OS:Kernel:CPU:Memory:Uptime:Shell:Terminal`, disables its logo and user configuration, and emits JSON. zfetch's corresponding data additionally names architecture and online CPU count explicitly. The programs do not expose identical fields or use identical definitions for shell and terminal.

The separate stock-default result runs `fastfetch --config none` with its broader default modules. It is not an equal-feature comparison. The user configuration is disabled in both fastfetch workloads to avoid measuring unrelated commands or private customization.

## Size and build

| Executable | Size | Build |
| --- | ---: | --- |
| zfetch x86_64 | 23,288 bytes | Rust 1.100.0, opt-level z, fat LTO, stripped static ELF |
| zfetch aarch64 | 66,168 bytes | Cross build, default 64 KiB-compatible ELF layout; QEMU smoke tested |
| Installed fastfetch x86_64 | 2,000,920 bytes | Existing distribution executable; dynamic libraries excluded from this file size |

The production Cargo graph contains only zfetch. ELF checks reject a dynamic loader, undefined runtime symbols or shared-library dependencies. A process and network syscall trace recorded one entry execve, zero child processes and zero network calls for zfetch JSON.

[Raw timings and memory samples](benchmarks/linux-x86_64-0.1.0.json) include commands and the measured binary's SHA-256. [Syscall evidence](evidence/syscalls-linux-x86_64.json) states the trace scope. These measurements do not establish a universal ranking across hardware or other fetch tools.

Reproduce with `make benchmark`. Pass `--runs` and `--output` directly to `scripts/benchmark.py` to select sample count and destination.


## Official release artifacts

The official v0.1.0 binaries are 22,808 bytes on x86_64 and 66,168 bytes on aarch64. The x86_64 benchmark above used a locally linked 23,288-byte build of the same source; linker and platform differences affect ELF layout. The measurement record identifies that exact local binary and does not silently substitute the release file. Both official architecture jobs passed native tests, and both downloaded archives passed their published SHA-256 checks before AUR packaging.

See [release evidence](evidence/release-0.1.0.json).
