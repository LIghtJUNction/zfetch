//! Bounded parsing for the native system summary. No allocator or dependencies.
#![no_std]
#![forbid(unsafe_code)]
use core::fmt;

/// UTF-8 text held in a fixed buffer.
pub struct Text<const N: usize> {
    bytes: [u8; N],
    len: usize,
}
impl<const N: usize> Default for Text<N> {
    fn default() -> Self {
        Self::new()
    }
}
impl<const N: usize> Text<N> {
    pub const fn new() -> Self {
        Self {
            bytes: [0; N],
            len: 0,
        }
    }
    pub fn as_str(&self) -> &str {
        core::str::from_utf8(&self.bytes[..self.len]).unwrap_or("")
    }
    pub fn is_empty(&self) -> bool {
        self.len == 0
    }
    /// Replace terminal control characters and truncate only at UTF-8 boundaries.
    pub fn set_clean(&mut self, text: &str) {
        self.len = 0;
        for ch in text.chars() {
            let ch = if ch.is_control() { '?' } else { ch };
            let mut bytes = [0; 4];
            let encoded = ch.encode_utf8(&mut bytes);
            if self.len + encoded.len() > N {
                break;
            }
            self.bytes[self.len..self.len + encoded.len()].copy_from_slice(encoded.as_bytes());
            self.len += encoded.len();
        }
    }
}
impl<const N: usize> fmt::Write for Text<N> {
    fn write_str(&mut self, text: &str) -> fmt::Result {
        if self.len + text.len() > N {
            return Err(fmt::Error);
        }
        self.bytes[self.len..self.len + text.len()].copy_from_slice(text.as_bytes());
        self.len += text.len();
        Ok(())
    }
}

/// Parse a nonempty decimal integer without wrapping.
pub fn decimal(text: &str) -> Option<u64> {
    if text.is_empty() {
        return None;
    }
    let mut n = 0u64;
    for digit in text.bytes() {
        if !digit.is_ascii_digit() {
            return None;
        }
        n = n.checked_mul(10)?.checked_add(u64::from(digit - b'0'))?;
    }
    Some(n)
}

/// Read an exact key from a line-oriented file.
pub fn value<'a>(text: &'a str, name: &str, delimiter: char) -> Option<&'a str> {
    text.lines().find_map(|line| {
        let (key, value) = line.split_once(delimiter)?;
        (key.trim() == name).then_some(value.trim())
    })
}

/// Parse Linux online CPU lists, rejecting malformed or overlapping ranges.
pub fn online_cpus(text: &str) -> Option<u32> {
    let mut count = 0u64;
    let mut previous = None;
    for range in text.trim().split(',') {
        let (start, end) = if let Some((a, b)) = range.split_once('-') {
            (decimal(a)?, decimal(b)?)
        } else {
            let n = decimal(range)?;
            (n, n)
        };
        if start > end || previous.is_some_and(|p| start <= p) {
            return None;
        }
        count = count.checked_add(end.checked_sub(start)?.checked_add(1)?)?;
        previous = Some(end);
    }
    u32::try_from(count).ok()
}

/// Total and used bytes; used is total minus Linux MemAvailable.
pub fn memory(text: &str) -> (Option<u64>, Option<u64>) {
    fn bytes(text: &str, name: &str) -> Option<u64> {
        let mut parts = value(text, name, ':')?.split_whitespace();
        let n = decimal(parts.next()?)?;
        if parts.next()? != "kB" || parts.next().is_some() {
            return None;
        }
        n.checked_mul(1024)
    }
    let total = bytes(text, "MemTotal");
    let available = bytes(text, "MemAvailable");
    let used = total
        .zip(available)
        .map(|(all, free)| all.saturating_sub(free));
    (total, used)
}

/// Whole uptime seconds, without rounding or floating point.
pub fn uptime(text: &str) -> Option<u64> {
    let first = text.split_whitespace().next()?;
    if let Some((whole, fraction)) = first.split_once('.') {
        decimal(fraction)?;
        decimal(whole)
    } else {
        decimal(first)
    }
}

/// Decode os-release quoting without evaluating shell syntax.
pub fn os_name(text: &str) -> Text<256> {
    use core::fmt::Write;
    let mut result = Text::new();
    let Some(raw) = value(text, "PRETTY_NAME", '=')
        .or_else(|| value(text, "NAME", '='))
        .or_else(|| value(text, "ID", '='))
    else {
        return result;
    };
    let quote = raw.as_bytes().first().copied();
    let body = if matches!(quote, Some(b'"' | b'\'')) {
        if raw.len() < 2 || raw.as_bytes().last().copied() != quote {
            return result;
        }
        &raw[1..raw.len() - 1]
    } else {
        raw
    };
    let mut decoded = Text::<256>::new();
    let mut chars = body.chars().peekable();
    while let Some(ch) = chars.next() {
        if quote == Some(b'"')
            && ch == '\\'
            && let Some(&next) = chars.peek()
            && (matches!(next, '\\' | '"' | '$') || next == char::from(96))
        {
            let Some(next) = chars.next() else {
                break;
            };
            if decoded.write_char(next).is_err() {
                break;
            }
            continue;
        }
        if decoded.write_char(ch).is_err() {
            break;
        }
    }
    result.set_clean(decoded.as_str());
    result
}

/// Escape a JSON string, including arbitrary control characters.
pub fn json_string<W: fmt::Write>(out: &mut W, value: &str) -> fmt::Result {
    out.write_char('"')?;
    for ch in value.chars() {
        match ch {
            '"' => out.write_str("\\\"")?,
            '\\' => out.write_str("\\\\")?,
            '\n' => out.write_str("\\n")?,
            '\r' => out.write_str("\\r")?,
            '\t' => out.write_str("\\t")?,
            ch if ch < ' ' => write!(out, "\\u{:04x}", ch as u32)?,
            ch => out.write_char(ch)?,
        }
    }
    out.write_char('"')
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn decimal_is_bounded() {
        assert_eq!(decimal("18446744073709551615"), Some(u64::MAX));
        for s in ["", "-1", "1a", "18446744073709551616"] {
            assert_eq!(decimal(s), None);
        }
    }
    #[test]
    fn cpu_ranges_match_online_contract() {
        assert_eq!(online_cpus("0-3,8,10-11\n"), Some(7));
        for s in ["", "3-1", "0-2,2-4", "1,0", "0,", "4294967296-8589934591"] {
            assert_eq!(online_cpus(s), None);
        }
    }
    #[test]
    fn memory_uses_available_and_checked_units() {
        assert_eq!(
            memory("MemTotal: 2048 kB\nMemAvailable: 512 kB\n"),
            (Some(2097152), Some(1572864))
        );
        assert_eq!(
            memory("MemTotal: 10 MB\nMemAvailable: 1 kB\n"),
            (None, None)
        );
        assert_eq!(memory("MemTotal: 10 kB\n"), (Some(10240), None));
    }
    #[test]
    fn uptime_is_exact_integer_seconds() {
        assert_eq!(uptime("123.99 456.00\n"), Some(123));
        for s in ["-1", "x", "1.bad", "1.2.3", "1."] {
            assert_eq!(uptime(s), None);
        }
    }
    #[test]
    fn os_release_decodes_but_never_evaluates() {
        assert_eq!(
            os_name("PRETTY_NAME=\"Example \\\"Linux\\\" \\$HOME\"\n").as_str(),
            "Example \"Linux\" $HOME"
        );
        assert_eq!(os_name("NAME='A Linux'\n").as_str(), "A Linux");
        assert_eq!(os_name("PRETTY_NAME=\"broken\n").as_str(), "");
        assert_eq!(os_name("NAME=Plain\n").as_str(), "Plain");
    }
    #[test]
    fn text_cannot_inject_terminal_controls() {
        let mut text = Text::<7>::new();
        text.set_clean("a\u{1b}[b你好");
        assert_eq!(text.as_str(), "a?[b你");
        let mut out = Text::<64>::new();
        json_string(&mut out, "\"\\\n\r\t\u{1}你好").unwrap();
        assert_eq!(out.as_str(), "\"\\\"\\\\\\n\\r\\t\\u0001你好\"");
    }
}
