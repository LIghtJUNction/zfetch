//! Native filesystem adapter for the opt-in boot report.
use crate::sys;
use core::ffi::CStr;
use zfetch::{Text, boot::Probe};

struct Native {
    environment: *const *const u8,
}
fn path<'a>(text: &str, buffer: &'a mut [u8; 1024]) -> Option<&'a CStr> {
    if text.len() >= buffer.len() || text.as_bytes().contains(&0) {
        return None;
    }
    buffer[..text.len()].copy_from_slice(text.as_bytes());
    buffer[text.len()] = 0;
    CStr::from_bytes_with_nul(&buffer[..text.len() + 1]).ok()
}
impl Probe for Native {
    fn read<'a>(&self, name: &str, buffer: &'a mut [u8]) -> Option<&'a [u8]> {
        let mut storage = [0u8; 1024];
        sys::read_bytes(path(name, &mut storage)?, buffer)
    }
    fn link<'a>(&self, name: &str, buffer: &'a mut [u8]) -> Option<&'a str> {
        let mut storage = [0u8; 1024];
        sys::readlink(path(name, &mut storage)?, buffer)
    }
    fn exists(&self, name: &str) -> Option<bool> {
        let mut storage = [0u8; 1024];
        sys::exists(path(name, &mut storage)?)
    }
    fn env(&self, name: &str) -> Option<&str> {
        // The process entrypoint owns the immutable startup environment until exit.
        unsafe { sys::env(self.environment, name) }
    }
    fn uid(&self) -> u32 {
        sys::uid()
    }
    fn entries(&self, name: &str, names: &mut [Text<128>]) -> usize {
        let mut storage = [0u8; 1024];
        path(name, &mut storage).map_or(0, |path| sys::entries(path, names))
    }
}

/// Keep the larger boot buffer off the normal summary and help paths.
#[inline(never)]
pub unsafe fn emit(environment: *const *const u8, json: bool) -> ! {
    let report = zfetch::boot::collect(&Native { environment });
    let mut out = Text::<65536>::new();
    let result = if json {
        report.json(&mut out)
    } else {
        report.plain(&mut out)
    };
    if result.is_err() {
        crate::fail("zfetch: bounded boot output exceeded\n");
    }
    crate::emit(out.as_str().as_bytes())
}
