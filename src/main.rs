#![no_std]
#![no_main]
#[cfg(not(all(
    target_os = "linux",
    any(target_arch = "x86_64", target_arch = "aarch64")
)))]
compile_error!("zfetch supports Linux x86_64 and aarch64");
mod sys;
use core::{ffi::CStr, fmt::Write};
use zfetch::{Text, json_string};
const VERSION: &str = env!("CARGO_PKG_VERSION");
const HELP: &str = "zfetch — a small native Linux system summary\n\nUsage: zfetch [--json | --format text|json] [--no-color]\n  --json       Emit zfetch.system.v1 JSON; unavailable fields are null\n  --format     Select text or json output (also --format=json)\n  --no-color   Disable ANSI colors (also disabled when output is redirected)\n  -h, --help   Show this help without probing the system\n  -V, --version Show the version\n\nNo network, child processes, dynamic libraries or background service.\nDefault output omits user names, host names, IP addresses and serial numbers.\nNO_COLOR is respected. Shell is the configured SHELL; terminal uses TERM_PROGRAM\nor TERM. Memory used is MemTotal minus MemAvailable. Linux x86_64 and aarch64.\n";

#[cfg(target_arch = "x86_64")]
core::arch::global_asm!(
    ".global _start",
    ".type _start,@function",
    "_start:",
    "xor rbp, rbp",
    "mov rdi, rsp",
    "and rsp, -16",
    "call zfetch_start",
    "ud2"
);
#[cfg(target_arch = "aarch64")]
core::arch::global_asm!(
    ".global _start",
    ".type _start,%function",
    "_start:",
    "mov x0, sp",
    "bl zfetch_start",
    "brk #0"
);

#[panic_handler]
fn panic(_: &core::panic::PanicInfo<'_>) -> ! {
    sys::write(2, b"zfetch: internal error\n");
    sys::exit(101)
}
fn fail(text: &str) -> ! {
    sys::write(2, text.as_bytes());
    sys::exit(2)
}

/// Keep stdout exclusively for the requested output, including help and version.
fn emit(bytes: &[u8]) -> ! {
    if sys::write(1, bytes) {
        sys::exit(0);
    }
    sys::write(2, b"zfetch: unable to write output\n");
    sys::exit(1)
}
fn json_format(value: &[u8]) -> bool {
    match value {
        b"json" => true,
        b"text" => false,
        _ => fail("zfetch: --format expects text or json\n"),
    }
}

#[unsafe(no_mangle)]
unsafe extern "C" fn zfetch_start(stack: *const usize) -> ! {
    let argc = unsafe { *stack };
    let argv = unsafe { stack.add(1) as *const *const u8 };
    let envp = unsafe { argv.add(argc + 1) };
    let mut json = false;
    let mut no_color = unsafe { sys::env(envp, "NO_COLOR") }.is_some()
        || unsafe { sys::env(envp, "TERM") } == Some("dumb");
    if argc > 32 {
        fail("zfetch: too many arguments\n");
    }
    let mut i = 1;
    while i < argc {
        let arg = unsafe { CStr::from_ptr((*argv.add(i)).cast()) }.to_bytes();
        match arg {
            b"--json" => json = true,
            b"--format" => {
                i += 1;
                if i == argc {
                    fail("zfetch: --format requires text or json\n");
                }
                json = json_format(unsafe { CStr::from_ptr((*argv.add(i)).cast()) }.to_bytes());
            }
            value if value.starts_with(b"--format=") => {
                json = json_format(&value[b"--format=".len()..]);
            }
            b"--no-color" => no_color = true,
            b"--help" | b"-h" => {
                emit(HELP.as_bytes());
            }
            b"--version" | b"-V" => {
                emit(concat!("zfetch ", env!("CARGO_PKG_VERSION"), "\n").as_bytes());
            }
            _ => fail("zfetch: unknown argument; run zfetch --help\n"),
        }
        i += 1;
    }
    let info = unsafe { collect(envp) };
    let mut out = Text::<8192>::new();
    let status = if json {
        info.json(&mut out)
    } else {
        info.display(&mut out, !no_color && sys::tty())
    };
    if status.is_err() {
        fail("zfetch: bounded output exceeded\n");
    }
    emit(out.as_str().as_bytes())
}

#[derive(Default)]
struct Info {
    os: Text<256>,
    kernel: Text<128>,
    architecture: Text<64>,
    cpu: Text<256>,
    logical_cpus: Option<u32>,
    memory_total: Option<u64>,
    memory_used: Option<u64>,
    uptime: Option<u64>,
    shell: Text<64>,
    terminal: Text<128>,
}
fn uname_field(buffer: &[u8; 390], field: usize) -> &str {
    let bytes = &buffer[field * 65..(field + 1) * 65];
    let len = bytes.iter().position(|&v| v == 0).unwrap_or(bytes.len());
    core::str::from_utf8(&bytes[..len]).unwrap_or("")
}
unsafe fn collect(envp: *const *const u8) -> Info {
    let mut info = Info::default();
    let mut buffer = [0u8; 8192];
    if let Some(text) = sys::read(c"/etc/os-release", &mut buffer[..8192]) {
        info.os = zfetch::os_name(text);
    }
    if info.os.is_empty()
        && let Some(text) = sys::read(c"/usr/lib/os-release", &mut buffer[..8192])
    {
        info.os = zfetch::os_name(text);
    }
    let mut uname = [0u8; 390];
    if sys::uname(&mut uname) {
        info.kernel.set_clean(uname_field(&uname, 2));
        info.architecture.set_clean(uname_field(&uname, 4));
    }
    if let Some(text) = sys::read(c"/proc/cpuinfo", &mut buffer[..4096]) {
        let model = zfetch::value(text, "model name", ':')
            .or_else(|| zfetch::value(text, "Hardware", ':'))
            .or_else(|| zfetch::value(text, "Processor", ':'))
            .unwrap_or("");
        info.cpu.set_clean(model);
    }
    if let Some(text) = sys::read(c"/sys/devices/system/cpu/online", &mut buffer[..8192]) {
        info.logical_cpus = zfetch::online_cpus(text);
    }
    if let Some(text) = sys::read(c"/proc/meminfo", &mut buffer[..8192]) {
        (info.memory_total, info.memory_used) = zfetch::memory(text);
    }
    if let Some(text) = sys::read(c"/proc/uptime", &mut buffer[..128]) {
        info.uptime = zfetch::uptime(text);
    }
    if let Some(shell) = unsafe { sys::env(envp, "SHELL") } {
        info.shell.set_clean(shell.rsplit('/').next().unwrap_or(""));
    }
    let terminal = unsafe { sys::env(envp, "TERM_PROGRAM") }
        .filter(|s| !s.is_empty())
        .or_else(|| unsafe { sys::env(envp, "TERM") })
        .unwrap_or("");
    info.terminal.set_clean(terminal);
    info
}

fn known<const N: usize>(text: &Text<N>) -> &str {
    if text.is_empty() {
        "unavailable"
    } else {
        text.as_str()
    }
}
fn json_text<const N: usize>(out: &mut Text<8192>, key: &str, text: &Text<N>) -> core::fmt::Result {
    write!(out, ",\"{key}\":")?;
    if text.is_empty() {
        out.write_str("null")
    } else {
        json_string(out, text.as_str())
    }
}
fn json_number(out: &mut Text<8192>, key: &str, value: Option<u64>) -> core::fmt::Result {
    write!(out, ",\"{key}\":")?;
    if let Some(n) = value {
        write!(out, "{n}")
    } else {
        out.write_str("null")
    }
}
impl Info {
    fn json(&self, out: &mut Text<8192>) -> core::fmt::Result {
        out.write_str("{\"schema\":\"zfetch.system.v1\"")?;
        json_text(out, "os", &self.os)?;
        json_text(out, "kernel", &self.kernel)?;
        json_text(out, "architecture", &self.architecture)?;
        json_text(out, "cpu", &self.cpu)?;
        json_number(out, "logical_cpus", self.logical_cpus.map(u64::from))?;
        json_number(out, "memory_total_bytes", self.memory_total)?;
        json_number(out, "memory_used_bytes", self.memory_used)?;
        json_number(out, "uptime_seconds", self.uptime)?;
        json_text(out, "shell", &self.shell)?;
        json_text(out, "terminal", &self.terminal)?;
        out.write_str("}\n")
    }
    fn display(&self, out: &mut Text<8192>, color: bool) -> core::fmt::Result {
        let accent = if color { "\x1b[38;2;212;163;115m" } else { "" };
        let muted = if color { "\x1b[38;2;163;151;139m" } else { "" };
        let ink = if color { "\x1b[38;2;232;222;210m" } else { "" };
        let reset = if color { "\x1b[0m" } else { "" };
        writeln!(out, "\n  {accent}zfetch{reset}  {muted}{VERSION}{reset}\n")?;
        for (label, text) in [("os", known(&self.os)), ("kernel", known(&self.kernel))] {
            writeln!(out, "  {muted}{label:<10}{reset}{ink}{text}{reset}")?;
        }
        writeln!(
            out,
            "  {muted}{:<10}{reset}{ink}{}{reset}",
            "arch",
            known(&self.architecture)
        )?;
        write!(
            out,
            "  {muted}{:<10}{reset}{ink}{}",
            "cpu",
            known(&self.cpu)
        )?;
        if let Some(n) = self.logical_cpus {
            write!(out, "  {muted}· {n} threads")?;
        }
        writeln!(out, "{reset}")?;
        write!(out, "  {muted}{:<10}{reset}{ink}", "memory")?;
        if let (Some(used), Some(total)) = (self.memory_used, self.memory_total) {
            size(out, used)?;
            out.write_str(" / ")?;
            size(out, total)?;
        } else {
            out.write_str("unavailable")?;
        }
        writeln!(out, "{reset}")?;
        write!(out, "  {muted}{:<10}{reset}{ink}", "uptime")?;
        if let Some(seconds) = self.uptime {
            let days = seconds / 86400;
            let hours = seconds / 3600 % 24;
            let minutes = seconds / 60 % 60;
            if days > 0 {
                write!(out, "{days}d ")?;
            }
            if days > 0 || hours > 0 {
                write!(out, "{hours}h ")?;
            }
            if seconds < 60 {
                write!(out, "{seconds}s")?;
            } else {
                write!(out, "{minutes}m")?;
            }
        } else {
            out.write_str("unavailable")?;
        }
        writeln!(out, "{reset}")?;
        for (label, text) in [
            ("shell", known(&self.shell)),
            ("terminal", known(&self.terminal)),
        ] {
            writeln!(out, "  {muted}{label:<10}{reset}{ink}{text}{reset}")?;
        }
        writeln!(out)
    }
}
fn size(out: &mut Text<8192>, bytes: u64) -> core::fmt::Result {
    let (unit, label) = if bytes >= 1u64 << 40 {
        (1u64 << 40, "TiB")
    } else if bytes >= 1u64 << 30 {
        (1u64 << 30, "GiB")
    } else {
        (1u64 << 20, "MiB")
    };
    let tenths = bytes / unit * 10 + (bytes % unit * 10 + unit / 2) / unit;
    write!(out, "{}.{} {label}", tenths / 10, tenths % 10)
}
