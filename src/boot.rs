//! Read-only boot evidence. No commands, allocation or configuration evaluation.
use crate::{Text, decimal, json_string, value};
use core::fmt::{self, Write};

/// Native reads are bounded by the caller's buffer. Missing or unreadable data
/// remains unknown; probes never repair, execute or modify system configuration.
pub trait Probe {
    fn read<'a>(&self, path: &str, buffer: &'a mut [u8]) -> Option<&'a [u8]>;
    fn link<'a>(&self, path: &str, buffer: &'a mut [u8]) -> Option<&'a str>;
    fn exists(&self, path: &str) -> Option<bool>;
    fn env(&self, name: &str) -> Option<&str>;
    fn uid(&self) -> u32;
    fn entries(&self, path: &str, names: &mut [Text<128>]) -> usize;
}

#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum Status {
    Observed,
    Configured,
    Unavailable,
}
impl Status {
    fn label(self) -> &'static str {
        match self {
            Self::Observed => "observed",
            Self::Configured => "configured",
            Self::Unavailable => "unavailable",
        }
    }
}

struct File {
    path: Text<256>,
    present: Option<bool>,
}
struct Unit {
    name: Text<128>,
    status: Status,
}
pub struct Stage {
    id: &'static str,
    label: &'static str,
    name: Text<160>,
    status: Status,
    evidence: [Text<256>; 3],
    files: [File; 10],
    units: [Unit; 6],
    evidence_len: usize,
    files_len: usize,
    units_len: usize,
    truncated: bool,
}
impl Stage {
    fn new(id: &'static str, label: &'static str) -> Self {
        Self {
            id,
            label,
            name: Text::new(),
            status: Status::Unavailable,
            evidence: core::array::from_fn(|_| Text::new()),
            files: core::array::from_fn(|_| File {
                path: Text::new(),
                present: None,
            }),
            units: core::array::from_fn(|_| Unit {
                name: Text::new(),
                status: Status::Unavailable,
            }),
            evidence_len: 0,
            files_len: 0,
            units_len: 0,
            truncated: false,
        }
    }
    fn identify(&mut self, name: &str, status: Status) {
        self.name.set_clean(name);
        self.status = status;
    }
    fn evidence(&mut self, text: &str) {
        if self.evidence[..self.evidence_len]
            .iter()
            .any(|v| v.as_str() == text)
        {
            return;
        }
        if self.evidence_len == self.evidence.len() {
            self.truncated = true;
            return;
        }
        self.evidence[self.evidence_len].set_clean(text);
        self.evidence_len += 1;
    }
    fn unit(&mut self, name: &str, status: Status) {
        if !unit_name(name)
            || self.units[..self.units_len]
                .iter()
                .any(|v| v.name.as_str() == name)
        {
            return;
        }
        if self.units_len == self.units.len() {
            self.truncated = true;
            return;
        }
        self.units[self.units_len].name.set_clean(name);
        self.units[self.units_len].status = status;
        self.units_len += 1;
    }
    fn file(&mut self, probe: &impl Probe, path: &str) {
        let mut shown = Text::<256>::new();
        if let Some(home) = probe
            .env("HOME")
            .filter(|v| v.starts_with('/') && *v != "/")
            && let Some(suffix) = path.strip_prefix(home).filter(|v| v.starts_with('/'))
        {
            if write!(shown, "$HOME{suffix}").is_err() {
                self.truncated = true;
                return;
            }
        } else {
            if path.len() > 256 {
                self.truncated = true;
                return;
            }
            shown.set_clean(path);
        }
        if self.files[..self.files_len]
            .iter()
            .any(|v| v.path.as_str() == shown.as_str())
        {
            return;
        }
        if self.files_len == self.files.len() {
            self.truncated = true;
            return;
        }
        self.files[self.files_len].path = shown;
        self.files[self.files_len].present = probe.exists(path);
        self.files_len += 1;
    }
    fn present_file(&mut self, probe: &impl Probe, path: &str) -> bool {
        if probe.exists(path) != Some(true) {
            return false;
        }
        self.file(probe, path);
        true
    }
    fn conf_directory(&mut self, probe: &impl Probe, path: &str) {
        self.file(probe, path);
        let mut names = core::array::from_fn::<_, 32, _>(|_| Text::<128>::new());
        let count = probe.entries(path, &mut names).min(names.len());
        names[..count].sort_unstable_by(|a, b| a.as_str().cmp(b.as_str()));
        for name in &names[..count] {
            if name.as_str().ends_with(".conf") && basename(name.as_str()) {
                let mut full = Text::<256>::new();
                if write!(full, "{path}/{}", name.as_str()).is_ok() {
                    self.file(probe, full.as_str());
                }
            }
        }
        if count == names.len() {
            self.truncated = true;
        }
    }
}

pub struct Report {
    stages: [Stage; 9],
}
impl Report {
    pub fn json(&self, out: &mut impl Write) -> fmt::Result {
        out.write_str("{\"schema\":\"zfetch.boot.v1\",\"ordering\":\"structural\",\"stages\":[")?;
        for (index, stage) in self.stages.iter().enumerate() {
            if index > 0 {
                out.write_char(',')?;
            }
            out.write_str("{\"stage\":")?;
            json_string(out, stage.id)?;
            out.write_str(",\"name\":")?;
            if stage.name.is_empty() {
                out.write_str("null")?;
            } else {
                json_string(out, stage.name.as_str())?;
            }
            out.write_str(",\"status\":")?;
            json_string(out, stage.status.label())?;
            out.write_str(",\"evidence\":[")?;
            for (i, evidence) in stage.evidence[..stage.evidence_len].iter().enumerate() {
                if i > 0 {
                    out.write_char(',')?;
                }
                json_string(out, evidence.as_str())?;
            }
            out.write_str("],\"units\":[")?;
            for (i, unit) in stage.units[..stage.units_len].iter().enumerate() {
                if i > 0 {
                    out.write_char(',')?;
                }
                out.write_str("{\"name\":")?;
                json_string(out, unit.name.as_str())?;
                out.write_str(",\"status\":")?;
                json_string(out, unit.status.label())?;
                out.write_char('}')?;
            }
            out.write_str("],\"files\":[")?;
            for (i, file) in stage.files[..stage.files_len].iter().enumerate() {
                if i > 0 {
                    out.write_char(',')?;
                }
                out.write_str("{\"path\":")?;
                json_string(out, file.path.as_str())?;
                out.write_str(",\"present\":")?;
                out.write_str(match file.present {
                    Some(true) => "true",
                    Some(false) => "false",
                    None => "null",
                })?;
                out.write_char('}')?;
            }
            write!(out, "],\"truncated\":{}}}", stage.truncated)?;
        }
        out.write_str("]}\n")
    }
    pub fn plain(&self, out: &mut impl Write) -> fmt::Result {
        out.write_str("\n  zfetch boot\n  Boot structure; services may start in parallel.\n\n")?;
        for (index, stage) in self.stages.iter().enumerate() {
            writeln!(
                out,
                "  {}  {}  {} [{}]",
                index + 1,
                stage.label,
                if stage.name.is_empty() {
                    "unavailable"
                } else {
                    stage.name.as_str()
                },
                stage.status.label()
            )?;
            for evidence in &stage.evidence[..stage.evidence_len] {
                writeln!(out, "     from  {}", evidence.as_str())?;
            }
            for unit in &stage.units[..stage.units_len] {
                writeln!(
                    out,
                    "     unit  {} [{}]",
                    unit.name.as_str(),
                    unit.status.label()
                )?;
            }
            for file in &stage.files[..stage.files_len] {
                writeln!(
                    out,
                    "     file  {} [{}]",
                    file.path.as_str(),
                    match file.present {
                        Some(true) => "present",
                        Some(false) => "not found",
                        None => "unreadable",
                    }
                )?;
            }
            if stage.truncated {
                writeln!(out, "     additional paths omitted at the bounded limit")?;
            }
            writeln!(out)?;
        }
        out.write_str("  observed = runtime evidence; configured = available configuration.\n  Unit state and timing are not queried. Private arguments and file contents are omitted.\n\n")
    }
}

fn text<'a>(probe: &impl Probe, path: &str, buffer: &'a mut [u8]) -> Option<&'a str> {
    core::str::from_utf8(probe.read(path, buffer)?).ok()
}
fn basename(name: &str) -> bool {
    !name.is_empty()
        && name.len() <= 128
        && name != "."
        && name != ".."
        && name
            .bytes()
            .all(|b| b.is_ascii_alphanumeric() || b"-_.@:+\\".contains(&b))
}
fn unit_name(name: &str) -> bool {
    basename(name)
        && [".service", ".target", ".scope"]
            .iter()
            .any(|suffix| name.ends_with(suffix))
}
fn filename(path: &str) -> &str {
    path.rsplit('/').next().unwrap_or("")
}
fn contains(text: &str, needle: &str) -> bool {
    text.as_bytes()
        .windows(needle.len())
        .any(|p| p.eq_ignore_ascii_case(needle.as_bytes()))
}
fn configured_unit(stage: &mut Stage, probe: &impl Probe, name: &str, user: bool) {
    let scope = if user { "user" } else { "system" };
    for base in [
        "/etc/systemd",
        "/run/systemd",
        "/usr/lib/systemd",
        "/lib/systemd",
    ] {
        let mut path = Text::<256>::new();
        if write!(path, "{base}/{scope}/{name}").is_ok() && stage.present_file(probe, path.as_str())
        {
            stage.unit(name, Status::Configured);
            return;
        }
    }
}
fn user_config(probe: &impl Probe, suffix: &str) -> Option<Text<256>> {
    let mut path = Text::new();
    if let Some(base) = probe.env("XDG_CONFIG_HOME").filter(|v| v.starts_with('/')) {
        write!(path, "{base}/{suffix}").ok()?;
    } else {
        let home = probe.env("HOME").filter(|v| v.starts_with('/'))?;
        write!(path, "{home}/.config/{suffix}").ok()?;
    }
    Some(path)
}

/// EFI variables have a four-byte attribute header followed by UTF-16LE data.
pub fn efi_text(bytes: &[u8]) -> Text<160> {
    let mut out = Text::new();
    if bytes.len() < 6 || !bytes.len().is_multiple_of(2) {
        return out;
    }
    let units = bytes[4..]
        .as_chunks::<2>()
        .0
        .iter()
        .map(|p| u16::from_le_bytes([p[0], p[1]]));
    let mut terminated = false;
    for ch in core::char::decode_utf16(units) {
        let Ok(ch) = ch else {
            return Text::new();
        };
        if ch == '\0' {
            terminated = true;
            break;
        }
        if out
            .write_char(if ch.is_control() { '?' } else { ch })
            .is_err()
        {
            return Text::new();
        }
    }
    if terminated { out } else { Text::new() }
}

/// Select one explicit kernel argument without exposing unrelated command-line data.
pub fn argument<'a>(line: &'a str, key: &str) -> Option<&'a str> {
    line.split_ascii_whitespace()
        .filter_map(|item| item.split_once('='))
        .filter(|(name, _)| *name == key)
        .map(|(_, value)| value.trim_matches('"'))
        .next_back()
}

#[derive(Default)]
struct Session {
    service: Text<96>,
    desktop: Text<128>,
    kind: Text<32>,
    scope: Text<128>,
    tty: Text<64>,
}
fn collect_session(probe: &impl Probe, stage: &mut Stage, buffer: &mut [u8]) -> Session {
    let mut session = Session::default();
    let id = probe.env("XDG_SESSION_ID").filter(|v| basename(v));
    if let Some(id) = id {
        let mut path = Text::<256>::new();
        if write!(path, "/run/systemd/sessions/{id}").is_ok()
            && let Some(body) = text(probe, path.as_str(), buffer)
            && value(body, "UID", '=')
                .and_then(decimal)
                .is_none_or(|uid| uid == u64::from(probe.uid()))
        {
            session
                .service
                .set_clean(value(body, "SERVICE", '=').unwrap_or(""));
            session
                .desktop
                .set_clean(value(body, "DESKTOP", '=').unwrap_or(""));
            session
                .kind
                .set_clean(value(body, "TYPE", '=').unwrap_or(""));
            session
                .scope
                .set_clean(value(body, "SCOPE", '=').unwrap_or(""));
            session.tty.set_clean(value(body, "TTY", '=').unwrap_or(""));
            stage.identify(
                if session.kind.is_empty() {
                    "logind session"
                } else {
                    session.kind.as_str()
                },
                Status::Observed,
            );
            stage.evidence(path.as_str());
            stage.file(probe, path.as_str());
            stage.unit(session.scope.as_str(), Status::Observed);
            if basename(session.service.as_str()) {
                let mut pam = Text::<256>::new();
                if write!(pam, "/etc/pam.d/{}", session.service.as_str()).is_ok() {
                    stage.file(probe, pam.as_str());
                }
            }
        }
    }
    if session.kind.is_empty() {
        session
            .kind
            .set_clean(probe.env("XDG_SESSION_TYPE").unwrap_or(""));
        if !session.kind.is_empty() {
            stage.identify(session.kind.as_str(), Status::Configured);
            stage.evidence("XDG_SESSION_TYPE");
        }
    }
    let mut runtime = Text::<256>::new();
    if write!(runtime, "/run/user/{}/systemd/private", probe.uid()).is_ok()
        && probe.exists(runtime.as_str()) == Some(true)
    {
        let mut unit = Text::<128>::new();
        if write!(unit, "user@{}.service", probe.uid()).is_ok() {
            stage.unit(unit.as_str(), Status::Observed);
        }
        stage.evidence("current user's systemd manager socket");
    }
    configured_unit(stage, probe, "systemd-logind.service", false);
    stage.file(probe, "/etc/systemd/logind.conf");
    stage.present_file(probe, "/etc/systemd/logind.conf.d");
    if let Some(path) = user_config(probe, "systemd/user") {
        stage.present_file(probe, path.as_str());
    }
    session
}

pub fn collect(probe: &impl Probe) -> Report {
    let mut report = Report {
        stages: [
            Stage::new("firmware", "Firmware"),
            Stage::new("bootloader", "Boot loader"),
            Stage::new("kernel", "Kernel"),
            Stage::new("initramfs", "Initial RAM filesystem"),
            Stage::new("system_manager", "System manager"),
            Stage::new("system_target", "System target"),
            Stage::new("login", "Login"),
            Stage::new("user_session", "User session"),
            Stage::new("desktop", "Desktop"),
        ],
    };
    let mut buffer = [0u8; 8192];
    let mut link = [0u8; 512];
    let firmware = &mut report.stages[0];
    if probe.exists("/sys/firmware/efi") == Some(true) {
        firmware.identify("UEFI", Status::Observed);
        firmware.evidence("/sys/firmware/efi");
    }
    firmware.file(probe, "/sys/firmware/efi");

    let loader = &mut report.stages[1];
    const LOADER_INFO: &str =
        "/sys/firmware/efi/efivars/LoaderInfo-4a67b082-0a4c-41cf-b6c7-440b29bb8c4f";
    if let Some(bytes) = probe.read(LOADER_INFO, &mut buffer) {
        let name = efi_text(bytes);
        if !name.is_empty() {
            loader.identify(name.as_str(), Status::Observed);
            loader.evidence(LOADER_INFO);
        }
    }
    let is_limine = contains(loader.name.as_str(), "limine");
    let is_grub = contains(loader.name.as_str(), "grub");
    let is_sdboot =
        contains(loader.name.as_str(), "systemd") || contains(loader.name.as_str(), "gummiboot");
    if is_limine || probe.exists("/etc/default/limine") == Some(true) {
        if loader.name.is_empty() {
            loader.identify("Limine configuration", Status::Configured);
        }
        loader.file(probe, "/etc/default/limine");
        for path in [
            "/boot/limine.conf",
            "/boot/limine/limine.conf",
            "/efi/limine.conf",
            "/efi/EFI/limine/limine.conf",
            "/efi/limine/limine.conf",
            "/boot/efi/limine.conf",
            "/etc/limine.conf",
        ] {
            if probe.exists(path) != Some(false) {
                loader.file(probe, path);
            }
        }
    }
    if is_grub || (loader.name.is_empty() && probe.exists("/boot/grub/grub.cfg") == Some(true)) {
        if loader.name.is_empty() {
            loader.identify("GRUB configuration", Status::Configured);
        }
        loader.file(probe, "/etc/default/grub");
        loader.file(probe, "/boot/grub/grub.cfg");
        loader.present_file(probe, "/etc/grub.d");
    }
    if is_sdboot
        || (loader.name.is_empty()
            && ["/boot/loader/loader.conf", "/efi/loader/loader.conf"]
                .iter()
                .any(|p| probe.exists(p) == Some(true)))
    {
        if loader.name.is_empty() {
            loader.identify("systemd-boot configuration", Status::Configured);
        }
        for path in [
            "/boot/loader/loader.conf",
            "/boot/loader/entries",
            "/efi/loader/loader.conf",
            "/efi/loader/entries",
            "/boot/efi/loader/loader.conf",
            "/boot/efi/loader/entries",
        ] {
            loader.present_file(probe, path);
        }
    }
    if loader.evidence_len == 0 {
        loader.evidence("current boot-loader identity is not exposed by readable EFI metadata");
    }

    let kernel = &mut report.stages[2];
    if let Some(release) = text(probe, "/proc/sys/kernel/osrelease", &mut buffer) {
        let mut name = Text::<160>::new();
        let _ = write!(name, "Linux {}", release.trim());
        kernel.identify(name.as_str(), Status::Observed);
        kernel.evidence("/proc/sys/kernel/osrelease");
    }
    kernel.file(probe, "/proc/cmdline");
    kernel.present_file(probe, "/etc/kernel/cmdline");
    kernel.present_file(probe, "/etc/kernel/cmdline.d");

    let initrd = &mut report.stages[3];
    if probe.exists("/run/initramfs") == Some(true) {
        initrd.identify("retained initramfs state", Status::Observed);
        initrd.evidence("/run/initramfs");
        initrd.file(probe, "/run/initramfs");
    }
    for (path, name) in [
        ("/etc/mkinitcpio.conf", "mkinitcpio configuration"),
        ("/etc/dracut.conf", "dracut configuration"),
        ("/etc/dracut.conf.d", "dracut configuration"),
        (
            "/etc/initramfs-tools/initramfs.conf",
            "initramfs-tools configuration",
        ),
    ] {
        if initrd.present_file(probe, path) && initrd.name.is_empty() {
            initrd.identify(name, Status::Configured);
        }
    }
    initrd.present_file(probe, "/etc/mkinitcpio.d");
    configured_unit(initrd, probe, "initrd.target", false);
    configured_unit(initrd, probe, "initrd-switch-root.service", false);

    let manager = &mut report.stages[4];
    let mut systemd = false;
    if let Some(name) = text(probe, "/proc/1/comm", &mut buffer) {
        manager.identify(name.trim(), Status::Observed);
        manager.evidence("/proc/1/comm");
        systemd = name.trim() == "systemd";
    }
    if systemd {
        manager.unit("init.scope", Status::Observed);
        manager.file(probe, "/etc/systemd/system.conf");
        manager.present_file(probe, "/etc/systemd/system.conf.d");
    }

    let target = &mut report.stages[5];
    if systemd {
        if let Some(line) = text(probe, "/proc/cmdline", &mut buffer)
            && let Some(name) = argument(line, "systemd.unit").filter(|v| unit_name(v))
        {
            target.identify(name, Status::Observed);
            target.evidence("systemd.unit= in /proc/cmdline");
            target.unit(name, Status::Configured);
        }
        for base in [
            "/etc/systemd/system",
            "/run/systemd/system",
            "/usr/lib/systemd/system",
            "/lib/systemd/system",
        ] {
            let mut path = Text::<256>::new();
            let _ = write!(path, "{base}/default.target");
            if let Some(resolved) = probe
                .link(path.as_str(), &mut link)
                .filter(|v| unit_name(filename(v)))
            {
                if target.name.is_empty() {
                    target.identify(filename(resolved), Status::Configured);
                    target.evidence(path.as_str());
                    target.unit(filename(resolved), Status::Configured);
                }
                target.file(probe, path.as_str());
                break;
            }
        }
        for unit in ["sysinit.target", "basic.target", "multi-user.target"] {
            configured_unit(target, probe, unit, false);
        }
        if !target.name.is_empty() {
            let mut path = Text::<256>::new();
            let _ = write!(path, "/etc/systemd/system/{}.wants", target.name.as_str());
            target.present_file(probe, path.as_str());
        }
    }

    let session = collect_session(probe, &mut report.stages[7], &mut buffer);
    let login = &mut report.stages[6];
    if systemd {
        let runtime_manager = match session.service.as_str() {
            "sddm" | "sddm-autologin" => Some("sddm.service"),
            "gdm" | "gdm-password" | "gdm-autologin" => Some("gdm.service"),
            "lightdm" | "lightdm-autologin" => Some("lightdm.service"),
            "greetd" => Some("greetd.service"),
            _ => None,
        };
        let tty_login = session.service.as_str() == "login" && basename(session.tty.as_str());
        if let Some(manager) = runtime_manager {
            login.identify(manager, Status::Observed);
            let mut evidence = Text::<256>::new();
            let _ = write!(
                evidence,
                "current logind session SERVICE={}",
                session.service.as_str()
            );
            login.evidence(evidence.as_str());
            configured_unit(login, probe, manager, false);
        } else if tty_login {
            login.identify("TTY login", Status::Observed);
            login.evidence("current logind session SERVICE=login");
        }
        for base in [
            "/etc/systemd/system",
            "/run/systemd/system",
            "/usr/lib/systemd/system",
            "/lib/systemd/system",
        ] {
            let mut path = Text::<256>::new();
            let _ = write!(path, "{base}/display-manager.service");
            if let Some(resolved) = probe
                .link(path.as_str(), &mut link)
                .filter(|v| unit_name(filename(v)))
            {
                let manager = filename(resolved);
                if login.name.is_empty() {
                    login.identify(manager, Status::Configured);
                    login.evidence(path.as_str());
                } else {
                    let mut evidence = Text::<256>::new();
                    let _ = write!(evidence, "configured display-manager.service -> {manager}");
                    login.evidence(evidence.as_str());
                }
                login.file(probe, path.as_str());
                if login.name.as_str() == manager {
                    login.unit(manager, Status::Configured);
                    if resolved.starts_with('/') {
                        login.file(probe, resolved);
                    }
                }
                break;
            }
        }
        if !login.name.is_empty() {
            configured_unit(login, probe, "systemd-user-sessions.service", false);
        }
        let is_sddm = login.name.as_str() == "sddm.service";
        let is_gdm = matches!(login.name.as_str(), "gdm.service" | "gdm3.service");
        let is_greetd = login.name.as_str() == "greetd.service";
        let is_lightdm = login.name.as_str() == "lightdm.service";
        if is_sddm {
            login.file(probe, "/etc/sddm.conf");
            login.conf_directory(probe, "/etc/sddm.conf.d");
        }
        if is_gdm {
            login.file(probe, "/etc/gdm/custom.conf");
            login.present_file(probe, "/etc/gdm3/custom.conf");
        }
        if is_greetd {
            login.file(probe, "/etc/greetd/config.toml");
        }
        if is_lightdm {
            login.file(probe, "/etc/lightdm/lightdm.conf");
            login.conf_directory(probe, "/etc/lightdm/lightdm.conf.d");
        }
        if tty_login {
            let mut unit = Text::<128>::new();
            let _ = write!(unit, "getty@{}.service", session.tty.as_str());
            login.unit(unit.as_str(), Status::Configured);
            let mut path = Text::<256>::new();
            let _ = write!(path, "/etc/systemd/system/{}.d", unit.as_str());
            login.conf_directory(probe, path.as_str());
            configured_unit(login, probe, "getty@.service", false);
            for name in [".profile", ".bash_profile", ".zprofile", ".xinitrc"] {
                if let Some(home) = probe.env("HOME").filter(|v| v.starts_with('/')) {
                    let mut path = Text::<256>::new();
                    if write!(path, "{home}/{name}").is_ok() {
                        login.present_file(probe, path.as_str());
                    }
                }
            }
        }
        if login.name.is_empty() && !session.service.is_empty() {
            login.identify("session login service", Status::Observed);
            let mut source = Text::<256>::new();
            let _ = write!(source, "logind SERVICE={}", session.service.as_str());
            login.evidence(source.as_str());
        }
    }

    let desktop = &mut report.stages[8];
    let reported = if !session.desktop.is_empty() {
        Some((
            session.desktop.as_str(),
            "current logind session DESKTOP",
            Status::Observed,
        ))
    } else {
        probe
            .env("XDG_CURRENT_DESKTOP")
            .filter(|v| !v.is_empty())
            .map(|v| (v, "XDG_CURRENT_DESKTOP", Status::Configured))
    };
    if let Some((name, source, status)) = reported {
        // An active session does not establish that inherited desktop metadata is current.
        desktop.identify(name, status);
        desktop.evidence(source);
    }
    if contains(desktop.name.as_str(), "hyprland") {
        if let Some(path) = user_config(probe, "hypr/hyprland.conf") {
            desktop.file(probe, path.as_str());
        }
        for suffix in ["hypr/hyprland.lua", "hypr/autostart.lua", "uwsm/env.d"] {
            if let Some(path) = user_config(probe, suffix) {
                desktop.present_file(probe, path.as_str());
            }
        }
        for path in [
            "/usr/share/wayland-sessions/hyprland.desktop",
            "/usr/share/wayland-sessions/hyprland-uwsm.desktop",
        ] {
            desktop.present_file(probe, path);
        }
    } else if contains(desktop.name.as_str(), "sway") {
        if let Some(path) = user_config(probe, "sway/config") {
            desktop.file(probe, path.as_str());
        }
        desktop.present_file(probe, "/etc/sway/config");
        desktop.present_file(probe, "/usr/share/wayland-sessions/sway.desktop");
    } else if contains(desktop.name.as_str(), "kde") || contains(desktop.name.as_str(), "plasma") {
        if let Some(path) = user_config(probe, "kwinrc") {
            desktop.file(probe, path.as_str());
        }
        desktop.present_file(probe, "/usr/share/wayland-sessions/plasma.desktop");
    } else if contains(desktop.name.as_str(), "gnome") {
        desktop.present_file(probe, "/usr/share/wayland-sessions/gnome.desktop");
        desktop.present_file(probe, "/etc/dconf/db");
    }
    if !desktop.name.is_empty() {
        configured_unit(desktop, probe, "graphical-session.target", true);
        if probe.exists("/usr/share/wayland-sessions/hyprland-uwsm.desktop") == Some(true)
            && contains(desktop.name.as_str(), "hyprland")
        {
            configured_unit(desktop, probe, "wayland-wm@.service", true);
            if let Some(path) = user_config(probe, "uwsm/env") {
                desktop.present_file(probe, path.as_str());
            }
            desktop.present_file(probe, "/etc/uwsm/env");
        }
    }
    report
}

#[cfg(test)]
mod tests {
    extern crate std;
    use super::*;
    use std::{
        collections::BTreeMap,
        string::{String, ToString},
        vec::Vec,
    };
    #[derive(Default)]
    struct Fixture {
        files: BTreeMap<String, Vec<u8>>,
        links: BTreeMap<String, String>,
        environment: BTreeMap<String, String>,
    }
    impl Fixture {
        fn file(&mut self, path: &str, value: &str) {
            self.files
                .insert(path.to_string(), value.as_bytes().to_vec());
        }
    }
    impl Probe for Fixture {
        fn read<'a>(&self, path: &str, buffer: &'a mut [u8]) -> Option<&'a [u8]> {
            let bytes = self.files.get(path)?;
            let n = bytes.len().min(buffer.len());
            buffer[..n].copy_from_slice(&bytes[..n]);
            Some(&buffer[..n])
        }
        fn link<'a>(&self, path: &str, buffer: &'a mut [u8]) -> Option<&'a str> {
            let bytes = self.links.get(path)?.as_bytes();
            if bytes.len() >= buffer.len() {
                return None;
            }
            buffer[..bytes.len()].copy_from_slice(bytes);
            core::str::from_utf8(&buffer[..bytes.len()]).ok()
        }
        fn exists(&self, path: &str) -> Option<bool> {
            Some(self.files.contains_key(path) || self.links.contains_key(path))
        }
        fn env(&self, name: &str) -> Option<&str> {
            self.environment.get(name).map(String::as_str)
        }
        fn uid(&self) -> u32 {
            1000
        }
        fn entries(&self, path: &str, names: &mut [Text<128>]) -> usize {
            let mut count = 0;
            for key in self.files.keys() {
                if let Some(name) = key
                    .strip_prefix(path)
                    .and_then(|v| v.strip_prefix('/'))
                    .filter(|v| !v.contains('/'))
                {
                    if count == names.len() {
                        break;
                    }
                    names[count].set_clean(name);
                    count += 1;
                }
            }
            count
        }
    }
    #[test]
    fn efi_identity_requires_complete_utf16_and_terminal_controls_are_cleaned() {
        let mut bytes = std::vec![7, 0, 0, 0];
        for unit in "Limine 12.9.6\0".encode_utf16() {
            bytes.extend_from_slice(&unit.to_le_bytes());
        }
        assert_eq!(efi_text(&bytes).as_str(), "Limine 12.9.6");
        assert!(efi_text(&bytes[..bytes.len() - 2]).is_empty());
        assert!(efi_text(&[7, 0, 0, 0, 0, 0xd8, 0, 0]).is_empty());
        assert_eq!(
            argument(
                "quiet secret=hidden systemd.unit=rescue.target",
                "systemd.unit"
            ),
            Some("rescue.target")
        );
    }
    #[test]
    fn limine_sddm_and_hyprland_chain_uses_runtime_evidence_and_correct_configs() {
        let mut f = Fixture::default();
        f.file("/sys/firmware/efi", "");
        let mut efi = std::vec![7, 0, 0, 0];
        for unit in "Limine 12.9.6\0".encode_utf16() {
            efi.extend_from_slice(&unit.to_le_bytes());
        }
        f.files.insert(
            "/sys/firmware/efi/efivars/LoaderInfo-4a67b082-0a4c-41cf-b6c7-440b29bb8c4f".to_string(),
            efi,
        );
        f.file("/etc/default/limine", "secret configuration content");
        f.file("/etc/greetd/config.toml", "obsolete");
        f.file("/proc/1/comm", "systemd\n");
        f.file("/proc/sys/kernel/osrelease", "6.19\n");
        f.file("/proc/cmdline", "secret=DO_NOT_PRINT");
        f.file("/etc/mkinitcpio.conf", "HOOKS=()");
        f.links.insert(
            "/usr/lib/systemd/system/default.target".to_string(),
            "graphical.target".to_string(),
        );
        f.links.insert(
            "/etc/systemd/system/display-manager.service".to_string(),
            "/usr/lib/systemd/system/sddm.service".to_string(),
        );
        f.file("/etc/sddm.conf.d/99-login.conf", "private");
        f.environment
            .insert("XDG_SESSION_ID".to_string(), "3".to_string());
        f.environment
            .insert("XDG_CURRENT_DESKTOP".to_string(), "Hyprland".to_string());
        f.environment
            .insert("DESKTOP_SESSION".to_string(), "stale-session".to_string());
        f.file(
            "/run/systemd/sessions/3",
            "UID=1000\nTYPE=wayland\nSERVICE=sddm\nACTIVE=1\nSCOPE=session-3.scope\nTTY=tty2\n",
        );
        f.file(
            "/usr/share/wayland-sessions/hyprland-uwsm.desktop",
            "Exec=uwsm start",
        );
        f.file("/usr/lib/systemd/user/wayland-wm@.service", "[Service]");
        let report = collect(&f);
        assert_eq!(report.stages[1].name.as_str(), "Limine 12.9.6");
        assert_eq!(report.stages[1].status, Status::Observed);
        assert_eq!(report.stages[5].name.as_str(), "graphical.target");
        assert_eq!(report.stages[6].name.as_str(), "sddm.service");
        assert_eq!(report.stages[6].status, Status::Observed);
        assert!(
            report.stages[6].units[..report.stages[6].units_len]
                .iter()
                .all(|u| u.status == Status::Configured)
        );
        assert_eq!(report.stages[8].name.as_str(), "Hyprland");
        assert_eq!(report.stages[8].status, Status::Configured);
        let mut json = Text::<65536>::new();
        report.json(&mut json).unwrap();
        for expected in [
            "/etc/default/limine",
            "/etc/sddm.conf.d/99-login.conf",
            "session-3.scope",
            "wayland-wm@.service",
        ] {
            assert!(json.as_str().contains(expected));
        }
        for omitted in [
            "DO_NOT_PRINT",
            "secret configuration content",
            "greetd",
            "stale-session",
        ] {
            assert!(!json.as_str().contains(omitted));
        }
        assert!(
            json.as_str()
                .contains("\"name\":\"wayland-wm@.service\",\"status\":\"configured\"")
        );
    }
    #[test]
    fn session_login_source_wins_over_a_changed_display_manager_alias() {
        let mut f = Fixture::default();
        f.file("/proc/1/comm", "systemd\n");
        f.environment
            .insert("XDG_SESSION_ID".to_string(), "3".to_string());
        f.file(
            "/run/systemd/sessions/3",
            "UID=1000\nTYPE=wayland\nSERVICE=sddm-autologin\n",
        );
        f.links.insert(
            "/etc/systemd/system/display-manager.service".to_string(),
            "/usr/lib/systemd/system/gdm.service".to_string(),
        );
        f.file("/etc/sddm.conf.d/99-login.conf", "private");
        f.file("/etc/gdm/custom.conf", "new configuration");
        let report = collect(&f);
        assert_eq!(report.stages[6].name.as_str(), "sddm.service");
        assert_eq!(report.stages[6].status, Status::Observed);
        let mut json = Text::<65536>::new();
        report.json(&mut json).unwrap();
        assert!(
            json.as_str()
                .contains("configured display-manager.service -> gdm.service")
        );
        assert!(json.as_str().contains("/etc/sddm.conf.d/99-login.conf"));
        assert!(!json.as_str().contains("/etc/gdm/custom.conf"));
    }
    #[test]
    fn logind_desktop_record_takes_precedence_over_inherited_desktop_environment() {
        let mut f = Fixture::default();
        f.environment
            .insert("XDG_SESSION_ID".to_string(), "3".to_string());
        f.environment.insert(
            "XDG_CURRENT_DESKTOP".to_string(),
            "stale-desktop".to_string(),
        );
        f.environment
            .insert("DESKTOP_SESSION".to_string(), "omarchy".to_string());
        f.file(
            "/run/systemd/sessions/3",
            "UID=1000\nTYPE=wayland\nDESKTOP=Hyprland\nACTIVE=1\n",
        );
        let report = collect(&f);
        assert_eq!(report.stages[8].name.as_str(), "Hyprland");
        assert_eq!(report.stages[8].status, Status::Observed);
        let mut json = Text::<65536>::new();
        report.json(&mut json).unwrap();
        assert!(!json.as_str().contains("stale-desktop"));
        assert!(!json.as_str().contains("omarchy"));
    }
    #[test]
    fn default_target_override_and_missing_system_are_explicit() {
        let mut f = Fixture::default();
        let report = collect(&f);
        let mut json = Text::<65536>::new();
        report.json(&mut json).unwrap();
        assert!(json.as_str().contains("\"schema\":\"zfetch.boot.v1\""));
        assert_eq!(report.stages.len(), 9);
        assert!(
            report
                .stages
                .iter()
                .all(|s| s.status == Status::Unavailable)
        );
        f.file("/proc/1/comm", "systemd\n");
        f.file("/proc/cmdline", "systemd.unit=rescue.target");
        f.links.insert(
            "/usr/lib/systemd/system/default.target".to_string(),
            "graphical.target".to_string(),
        );
        let report = collect(&f);
        assert_eq!(report.stages[5].name.as_str(), "rescue.target");
        let mut small = Text::<32>::new();
        assert!(report.json(&mut small).is_err());
    }
    #[test]
    fn grub_configuration_does_not_claim_an_observed_bootloader() {
        let mut f = Fixture::default();
        f.file("/boot/grub/grub.cfg", "");
        let report = collect(&f);
        assert_eq!(report.stages[1].status, Status::Configured);
        assert_eq!(report.stages[1].name.as_str(), "GRUB configuration");
    }
}
