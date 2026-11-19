# Design

zfetch gathers a small set of local values and formats one bounded output buffer. It does not discover every installed program or launch command-line helpers. This keeps the work per invocation small and makes unavailable data visible rather than triggering another subsystem.

| Field | Source | Interpretation |
| --- | --- | --- |
| OS | /etc/os-release, then /usr/lib/os-release | PRETTY_NAME, NAME or ID; quoted values are decoded without shell evaluation |
| Kernel and architecture | uname syscall | Release and machine fields only; host name is not copied |
| CPU | First 4096 bytes of /proc/cpuinfo | First model name, Hardware or Processor field; absent data remains unknown |
| Logical CPUs | /sys/devices/system/cpu/online | Count of validated, nonoverlapping online ranges |
| Memory | /proc/meminfo | Total bytes and total minus available bytes |
| Uptime | /proc/uptime | Whole seconds, with no floating-point arithmetic |
| Shell | SHELL | Configured shell's basename |
| Terminal | TERM_PROGRAM or TERM | Declared program name or terminal type |

## Fixed runtime

The collector reuses an 8 KiB read buffer. Individual text fields have explicit byte caps, and truncation keeps complete UTF-8 characters. Control characters are replaced before terminal display; JSON has its own escaping. No parser allocates, no background work remains, and the process writes its summary once.

The executable uses architecture-specific Linux startup and syscall conventions. Rust core provides parsing and formatting. The binary has no Cargo dependencies and links neither Rust std nor libc. Build scripts, developer tests, package tools and the compiler use their ordinary standard libraries; they are not installed as runtime dependencies.

## Checks and limits

| Claim | Evidence |
| --- | --- |
| Numeric parsing does not wrap | Overflow, invalid digit, CPU range and unit tests |
| Output does not inject terminal escapes | Unicode/control fixture and hostile environment CLI test |
| Kernel values are actually read | Live total-memory, CPU-count, kernel and uptime checks |
| Colors respect output context | PTY truecolor, NO_COLOR and redirected-output tests |
| No private identity fields are emitted | Exact JSON-key contract |
| No dynamic runtime is required | ELF program-header, dynamic-table and undefined-symbol checks |
| No child or network operation occurs | Scoped strace evidence |
| Linux ARM64 ABI works | Cross build and QEMU execution; official release also builds/tests on a native ARM64 runner |

The 4 KiB CPU cap can omit model labels that appear late in unusual cpuinfo files. CPU count comes from the separate online list, so it does not count truncated cpuinfo blocks. Mutable system values are a sequence of reads, not a globally atomic snapshot. Permissions or missing virtual files produce unavailable values.

Review: the purpose is a bounded system summary; each field has a documented source; parser and live-system tests support the claims; size and latency are measured within stated workloads; syscall-only collection keeps the runtime mechanism direct. No claim covers all fastfetch modules or every Linux system.
