//! Minimal Linux syscall boundary for x86_64 and aarch64.
use core::{arch::asm, ffi::CStr};
#[cfg(target_arch = "x86_64")]
mod nr {
    pub const READ: usize = 0;
    pub const WRITE: usize = 1;
    pub const CLOSE: usize = 3;
    pub const IOCTL: usize = 16;
    pub const UNAME: usize = 63;
    pub const OPENAT: usize = 257;
    pub const EXIT: usize = 60;
}
#[cfg(target_arch = "aarch64")]
mod nr {
    pub const READ: usize = 63;
    pub const WRITE: usize = 64;
    pub const CLOSE: usize = 57;
    pub const IOCTL: usize = 29;
    pub const UNAME: usize = 160;
    pub const OPENAT: usize = 56;
    pub const EXIT: usize = 93;
}

#[inline]
unsafe fn syscall(n: usize, a: usize, b: usize, c: usize, d: usize) -> isize {
    let result: isize;
    #[cfg(target_arch = "x86_64")]
    unsafe {
        asm!("syscall", inlateout("rax") n as isize => result,
        in("rdi") a, in("rsi") b, in("rdx") c, in("r10") d,
        lateout("rcx") _, lateout("r11") _, options(nostack));
    }
    #[cfg(target_arch = "aarch64")]
    unsafe {
        asm!("svc 0", in("x8") n, inlateout("x0") a as isize => result,
        in("x1") b, in("x2") c, in("x3") d, options(nostack));
    }
    result
}
pub fn exit(code: i32) -> ! {
    unsafe {
        syscall(nr::EXIT, code as usize, 0, 0, 0);
    }
    loop {
        core::hint::spin_loop();
    }
}
pub fn write(fd: i32, mut bytes: &[u8]) -> bool {
    while !bytes.is_empty() {
        let n = unsafe {
            syscall(
                nr::WRITE,
                fd as usize,
                bytes.as_ptr() as usize,
                bytes.len(),
                0,
            )
        };
        if n == -4 {
            continue;
        }
        if n <= 0 {
            return false;
        }
        bytes = &bytes[n as usize..];
    }
    true
}
pub fn read<'a>(path: &CStr, buffer: &'a mut [u8]) -> Option<&'a str> {
    let fd = unsafe {
        syscall(
            nr::OPENAT,
            -100isize as usize,
            path.as_ptr() as usize,
            0x80000,
            0,
        )
    };
    if fd < 0 {
        return None;
    }
    let mut len = 0;
    let mut success = true;
    while len < buffer.len() {
        let n = unsafe {
            syscall(
                nr::READ,
                fd as usize,
                buffer[len..].as_mut_ptr() as usize,
                buffer.len() - len,
                0,
            )
        };
        if n == -4 {
            continue;
        }
        if n < 0 {
            success = false;
            break;
        }
        if n == 0 {
            break;
        }
        len += n as usize;
    }
    unsafe {
        syscall(nr::CLOSE, fd as usize, 0, 0, 0);
    }
    if !success {
        return None;
    }
    // Kernel text can be truncated in the middle of UTF-8 at the fixed limit.
    match core::str::from_utf8(&buffer[..len]) {
        Ok(text) => Some(text),
        Err(error) if error.error_len().is_none() => {
            core::str::from_utf8(&buffer[..error.valid_up_to()]).ok()
        }
        Err(_) => None,
    }
}
pub fn uname(buffer: &mut [u8; 390]) -> bool {
    unsafe { syscall(nr::UNAME, buffer.as_mut_ptr() as usize, 0, 0, 0) == 0 }
}
pub fn tty() -> bool {
    let mut termios = [0u8; 64];
    unsafe { syscall(nr::IOCTL, 1, 0x5401, termios.as_mut_ptr() as usize, 0) == 0 }
}

/// Startup environment access; values are borrowed and never executed.
pub unsafe fn env<'a>(mut envp: *const *const u8, name: &str) -> Option<&'a str> {
    loop {
        let value = unsafe { *envp };
        if value.is_null() {
            return None;
        }
        if let Ok(text) = unsafe { CStr::from_ptr(value.cast()) }.to_str()
            && let Some(value) = text.strip_prefix(name).and_then(|v| v.strip_prefix('='))
        {
            return Some(value);
        }
        envp = unsafe { envp.add(1) };
    }
}

// LLVM may emit these even when the program has no libc or allocator.
use core::ffi::{c_char, c_void};
#[unsafe(no_mangle)]
unsafe extern "C" fn memcpy(dst: *mut c_void, src: *const c_void, n: usize) -> *mut c_void {
    let (d, s) = (dst.cast::<u8>(), src.cast::<u8>());
    for i in 0..n {
        unsafe {
            d.add(i).write_volatile(s.add(i).read_volatile());
        }
    }
    dst
}
#[unsafe(no_mangle)]
unsafe extern "C" fn memset(dst: *mut c_void, c: i32, n: usize) -> *mut c_void {
    let d = dst.cast::<u8>();
    for i in 0..n {
        unsafe {
            d.add(i).write_volatile(c as u8);
        }
    }
    dst
}
#[unsafe(no_mangle)]
unsafe extern "C" fn memmove(dst: *mut c_void, src: *const c_void, n: usize) -> *mut c_void {
    let (d, s) = (dst.cast::<u8>(), src.cast::<u8>());
    if (dst as usize) < (src as usize) {
        for i in 0..n {
            unsafe {
                d.add(i).write_volatile(s.add(i).read_volatile());
            }
        }
    } else {
        for i in (0..n).rev() {
            unsafe {
                d.add(i).write_volatile(s.add(i).read_volatile());
            }
        }
    }
    dst
}
#[unsafe(no_mangle)]
unsafe extern "C" fn memcmp(a: *const c_void, b: *const c_void, n: usize) -> i32 {
    let (a, b) = (a.cast::<u8>(), b.cast::<u8>());
    for i in 0..n {
        let (x, y) = unsafe { (a.add(i).read_volatile(), b.add(i).read_volatile()) };
        if x != y {
            return i32::from(x) - i32::from(y);
        }
    }
    0
}
#[unsafe(no_mangle)]
unsafe extern "C" fn bcmp(a: *const c_void, b: *const c_void, n: usize) -> i32 {
    unsafe { memcmp(a, b, n) }
}
#[unsafe(no_mangle)]
unsafe extern "C" fn strlen(s: *const c_char) -> usize {
    let mut n = 0;
    while unsafe { s.add(n).read_volatile() } != 0 {
        n += 1;
    }
    n
}
