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

## Output contracts

Interactive text uses the terminal's window width. At 40 columns or more, labels
and values share aligned rows; narrower terminals put labels above values.
System, hardware and session rows are separated by whitespace. The title has
one blue accent, labels are dimmed, and values use the terminal's own foreground
color. `NO_COLOR` (including an empty value), `--no-color`, `TERM=dumb`, and
redirected stdout disable ANSI styling.

Wrapping keeps complete UTF-8 characters and reserves two cells for each
non-ASCII scalar. Combining characters and some narrow Unicode text may wrap
earlier than strictly necessary. A wide scalar in a one-column terminal is
displayed as `?`. The collector's bounded values remain unchanged in JSON.
If the terminal reports no width, interactive text uses 80 columns.

Redirected default text retains its existing compact, uncolored field layout.
It does not gain terminal line wrapping. Scripts should select `--json`,
`--format json`, or `--format=json`; `--format=text` explicitly selects text.
The last format option wins. JSON is one object followed by a newline, including
on a narrow terminal, with no color, heading, progress text, or stderr output on
success.

The `zfetch.system.v1` object always contains all eleven keys:

| Key | Type | Meaning |
| --- | --- | --- |
| `schema` | string | Exactly `zfetch.system.v1` |
| `os` | string or null | Decoded OS name |
| `kernel` | string or null | Kernel release |
| `architecture` | string or null | Machine architecture |
| `cpu` | string or null | Bounded CPU model |
| `logical_cpus` | integer or null | Online logical CPU count |
| `memory_total_bytes` | integer or null | Physical memory total, in bytes |
| `memory_used_bytes` | integer or null | Total minus available memory, in bytes |
| `uptime_seconds` | integer or null | Whole seconds since boot |
| `shell` | string or null | Configured shell basename |
| `terminal` | string or null | Terminal program or type |

Missing values stay null; they are never omitted or replaced by a human-readable
placeholder in JSON. Strings use JSON escaping after the same bounded
control-character sanitization used by the text collector.

Invalid options produce only a stderr diagnostic and status 2. Write failures,
including help and version output directed to `/dev/full`, produce a stderr
diagnostic and status 1. Closing a pipe early retains the normal Unix SIGPIPE
behavior (often status 141 in a shell); if SIGPIPE was inherited as ignored, the
write failure follows the status-1 path instead.

## Checks and limits

| Claim | Evidence |
| --- | --- |
| Numeric parsing does not wrap | Overflow, invalid digit, CPU range and unit tests |
| Output does not inject terminal escapes | Unicode/control fixture and hostile environment CLI test |
| Kernel values are actually read | Live total-memory, CPU-count, kernel and uptime checks |
| Colors respect output context | PTY color, NO_COLOR, --no-color, TERM=dumb and redirected-output tests |
| Narrow terminals stay bounded | Real PTY widths 1, 12, 24, 39, 40 and 80, including CJK input and a conflicting COLUMNS value |
| JSON is a stable pipe interface | Exact keys/types, explicit formats, quotes/backslashes/UTF-8, null fields and narrow-PTY JSON tests |
| Output failures are observable | /dev/full for all output paths, and closed pipes with default and ignored SIGPIPE |
| No private identity fields are emitted | Exact JSON-key contract |
| No dynamic runtime is required | ELF program-header, dynamic-table and undefined-symbol checks |
| No child or network operation occurs | Scoped strace evidence |
| Linux ARM64 ABI works | Cross build and QEMU execution; official release also builds/tests on a native ARM64 runner |

The 4 KiB CPU cap can omit model labels that appear late in unusual cpuinfo files. CPU count comes from the separate online list, so it does not count truncated cpuinfo blocks. Mutable system values are a sequence of reads, not a globally atomic snapshot. Permissions or missing virtual files produce unavailable values.

Review: the purpose is a bounded system summary with an explicit script interface; each field has a documented source; parser and live-system tests support the claims; size and latency are measured within stated workloads; syscall-only collection keeps the runtime mechanism direct. No claim covers all fastfetch modules or every Linux system.
