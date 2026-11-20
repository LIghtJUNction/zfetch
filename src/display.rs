//! Terminal-only presentation. Redirected text and JSON keep their existing contracts.
use super::{Info, VERSION, duration, known, size};
use core::fmt::{self, Write};
use zfetch::Text;

struct Layout {
    columns: usize,
    margin: usize,
    accent: &'static str,
    muted: &'static str,
    reset: &'static str,
}

impl Layout {
    fn row(&self, out: &mut Text<8192>, label: &str, value: &str) -> fmt::Result {
        if self.columns >= 40 {
            spaces(out, self.margin)?;
            write!(out, "{}{label:<10}{}", self.muted, self.reset)?;
            flow(out, value, self.columns, self.margin + 10, false)
        } else {
            out.write_str(self.muted)?;
            let mut caption = Text::<24>::new();
            write!(caption, "{label}:")?;
            flow(out, caption.as_str(), self.columns, self.margin, true)?;
            out.write_str(self.reset)?;
            let indent = if self.columns >= 16 {
                self.margin + 2
            } else {
                0
            };
            flow(out, value, self.columns, indent, true)
        }
    }
}

fn spaces(out: &mut Text<8192>, n: usize) -> fmt::Result {
    for _ in 0..n {
        out.write_char(' ')?;
    }
    Ok(())
}

/// Reserve two cells for each non-ASCII scalar. This intentionally overestimates
/// combining and narrow Unicode glyphs, so the bounded renderer needs no width
/// database and can keep CJK text within the real terminal's width.
fn flow(
    out: &mut Text<8192>,
    mut text: &str,
    columns: usize,
    indent: usize,
    first_indent: bool,
) -> fmt::Result {
    let available = columns.saturating_sub(indent).max(1);
    if first_indent {
        spaces(out, indent)?;
    }
    while !text.is_empty() {
        let mut end = 0;
        let mut cells = 0;
        let mut space = None;
        for (offset, ch) in text.char_indices() {
            let width = if ch.is_ascii() { 1 } else { 2 };
            if cells + width > available {
                break;
            }
            cells += width;
            end = offset + ch.len_utf8();
            if ch == ' ' && offset != 0 {
                space = Some(offset);
            }
        }
        if end == 0 {
            // A wide glyph cannot fit in a one-column terminal.
            out.write_char('?')?;
            text = &text[text.chars().next().map_or(0, char::len_utf8)..];
        } else {
            if end < text.len()
                && let Some(boundary) = space
            {
                end = boundary;
            }
            out.write_str(&text[..end])?;
            text = text[end..].trim_start_matches(' ');
        }
        if !text.is_empty() {
            out.write_char('\n')?;
            spaces(out, indent)?;
        }
    }
    out.write_char('\n')
}

pub(super) fn terminal(
    info: &Info,
    out: &mut Text<8192>,
    color: bool,
    columns: u16,
) -> fmt::Result {
    let columns = usize::from(columns).max(1);
    let layout = Layout {
        columns,
        margin: if columns >= 24 { 2 } else { 0 },
        accent: if color { "\x1b[1;38;2;66;133;244m" } else { "" },
        muted: if color { "\x1b[2m" } else { "" },
        reset: if color { "\x1b[0m" } else { "" },
    };
    let mut header = Text::<64>::new();
    write!(header, "zfetch  {VERSION}")?;
    writeln!(out)?;
    out.write_str(layout.accent)?;
    flow(out, header.as_str(), columns, layout.margin, true)?;
    writeln!(out, "{}", layout.reset)?;

    layout.row(out, "OS", known(&info.os))?;
    layout.row(out, "Kernel", known(&info.kernel))?;
    layout.row(out, "Arch", known(&info.architecture))?;
    writeln!(out)?;
    layout.row(out, "CPU", known(&info.cpu))?;

    let mut threads = Text::<32>::new();
    if let Some(n) = info.logical_cpus {
        write!(threads, "{n} online")?;
    } else {
        threads.write_str("unavailable")?;
    }
    layout.row(out, "Threads", threads.as_str())?;

    let mut memory = Text::<128>::new();
    if let (Some(used), Some(total)) = (info.memory_used, info.memory_total) {
        size(&mut memory, used)?;
        memory.write_str(" / ")?;
        size(&mut memory, total)?;
        if total != 0
            && let Some(percent) = used.checked_mul(100).map(|n| n / total)
        {
            write!(memory, "  {percent}%")?;
        }
    } else {
        memory.write_str("unavailable")?;
    }
    layout.row(out, "Memory", memory.as_str())?;
    writeln!(out)?;

    let mut uptime = Text::<64>::new();
    duration(&mut uptime, info.uptime)?;
    layout.row(out, "Uptime", uptime.as_str())?;
    layout.row(out, "Shell", known(&info.shell))?;
    layout.row(out, "Terminal", known(&info.terminal))?;
    writeln!(out)
}
