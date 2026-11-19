//! Linux syscall boundary. No libc and no allocator.
use core::arch::asm;
use core::fmt;
use core::fmt::Write;
use zdot::PATH_CAP;

pub type Result<T> = core::result::Result<T, &'static str>;
pub const AT_FDCWD: i32 = -100;
pub const O_DIRECTORY: usize = 0x10000;
pub const O_NOFOLLOW: usize = 0x20000;
pub const O_CLOEXEC: usize = 0x80000;
pub const O_PATH: usize = 0x200000;
pub const O_CREAT: usize = 64;
pub const O_EXCL: usize = 128;
pub const ENOENT: isize = -2;
pub const EEXIST: isize = -17;

#[cfg(target_arch = "x86_64")]
mod nr {
    pub const READ: usize = 0;
    pub const WRITE: usize = 1;
    pub const CLOSE: usize = 3;
    pub const EXECVE: usize = 59;
    pub const EXIT: usize = 60;
    pub const FLOCK: usize = 73;
    pub const FSYNC: usize = 74;
    pub const OPENAT: usize = 257;
    pub const MKDIRAT: usize = 258;
    pub const UNLINKAT: usize = 263;
    pub const SYMLINKAT: usize = 266;
    pub const READLINKAT: usize = 267;
    pub const RENAMEAT2: usize = 316;
    pub const STATX: usize = 332;
}
#[cfg(target_arch = "aarch64")]
mod nr {
    pub const READ: usize = 63;
    pub const WRITE: usize = 64;
    pub const CLOSE: usize = 57;
    pub const EXECVE: usize = 221;
    pub const EXIT: usize = 93;
    pub const FLOCK: usize = 32;
    pub const FSYNC: usize = 82;
    pub const OPENAT: usize = 56;
    pub const MKDIRAT: usize = 34;
    pub const UNLINKAT: usize = 35;
    pub const SYMLINKAT: usize = 36;
    pub const READLINKAT: usize = 78;
    pub const RENAMEAT2: usize = 276;
    pub const STATX: usize = 291;
}

#[inline]
unsafe fn syscall(n: usize, a: usize, b: usize, c: usize, d: usize, e: usize, f: usize) -> isize {
    let result: isize;
    #[cfg(target_arch = "x86_64")]
    unsafe {
        asm!("syscall", inlateout("rax") n as isize => result,
             in("rdi") a, in("rsi") b, in("rdx") c, in("r10") d,
             in("r8") e, in("r9") f, lateout("rcx") _, lateout("r11") _,
             options(nostack));
    }
    #[cfg(target_arch = "aarch64")]
    unsafe {
        asm!("svc 0", in("x8") n, inlateout("x0") a as isize => result,
             in("x1") b, in("x2") c, in("x3") d, in("x4") e, in("x5") f,
             options(nostack));
    }
    result
}

pub fn exit(code: i32) -> ! {
    unsafe {
        syscall(nr::EXIT, code as usize, 0, 0, 0, 0, 0);
    }
    loop {
        core::hint::spin_loop();
    }
}

pub fn print(fd: i32, mut bytes: &[u8]) {
    while !bytes.is_empty() {
        let n = unsafe {
            syscall(
                nr::WRITE,
                fd as usize,
                bytes.as_ptr() as usize,
                bytes.len(),
                0,
                0,
                0,
            )
        };
        if n == -4 {
            continue;
        }
        if n <= 0 {
            return;
        }
        bytes = &bytes[n as usize..];
    }
}

pub struct Out(pub i32);
impl fmt::Write for Out {
    fn write_str(&mut self, text: &str) -> fmt::Result {
        print(self.0, text.as_bytes());
        Ok(())
    }
}

/// NUL-terminated UTF-8, with space for Linux's PATH_MAX.
pub struct Path {
    data: [u8; PATH_CAP],
    len: usize,
}
impl Path {
    pub const fn new() -> Self {
        Self {
            data: [0; PATH_CAP],
            len: 0,
        }
    }
    pub fn from(text: &str) -> Result<Self> {
        let mut result = Self::new();
        result.push(text)?;
        Ok(result)
    }
    pub fn push(&mut self, text: &str) -> Result<()> {
        if self.len + text.len() >= PATH_CAP || text.as_bytes().contains(&0) {
            return Err("path exceeds Linux PATH_MAX or contains NUL");
        }
        self.data[self.len..self.len + text.len()].copy_from_slice(text.as_bytes());
        self.len += text.len();
        self.data[self.len] = 0;
        Ok(())
    }
    pub fn join(&mut self, text: &str) -> Result<()> {
        if self.len > 0 && self.data[self.len - 1] != b'/' {
            self.push("/")?;
        }
        self.push(text)
    }
    pub fn as_ptr(&self) -> *const u8 {
        self.data.as_ptr()
    }
    pub fn as_str(&self) -> &str {
        // All constructors and readlink validate UTF-8.
        unsafe { core::str::from_utf8_unchecked(&self.data[..self.len]) }
    }
    pub fn bytes(&self) -> &[u8] {
        &self.data[..self.len]
    }
}
impl fmt::Write for Path {
    fn write_str(&mut self, text: &str) -> fmt::Result {
        self.push(text).map_err(|_| fmt::Error)
    }
}

pub struct Fd(pub i32);
impl Drop for Fd {
    fn drop(&mut self) {
        unsafe {
            syscall(nr::CLOSE, self.0 as usize, 0, 0, 0, 0, 0);
        }
    }
}
pub fn openat(base: i32, path: &str, flags: usize, mode: usize) -> core::result::Result<Fd, isize> {
    let c = Path::from(path).map_err(|_| -36isize)?;
    let n = unsafe {
        syscall(
            nr::OPENAT,
            base as usize,
            c.as_ptr() as usize,
            flags | O_CLOEXEC,
            mode,
            0,
            0,
        )
    };
    if n < 0 { Err(n) } else { Ok(Fd(n as i32)) }
}
pub fn directory(path: &str) -> Result<Fd> {
    openat(AT_FDCWD, path, O_DIRECTORY | O_NOFOLLOW, 0)
        .map_err(|_| "cannot open directory (it must exist and not be a symlink)")
}
pub fn read_file(base: i32, path: &str, buffer: &mut [u8]) -> Result<usize> {
    let fd = openat(base, path, O_NOFOLLOW, 0).map_err(|_| "cannot open regular file")?;
    if kind(fd.0)? != 0o100000 {
        return Err("expected a regular file");
    }
    let mut length = 0;
    loop {
        let n = unsafe {
            syscall(
                nr::READ,
                fd.0 as usize,
                buffer[length..].as_mut_ptr() as usize,
                buffer.len() - length,
                0,
                0,
                0,
            )
        };
        if n == -4 {
            continue;
        }
        if n < 0 {
            return Err("file read failed");
        }
        if n == 0 {
            break;
        }
        length += n as usize;
        if length == buffer.len() {
            let mut extra = [0u8; 1];
            let more = unsafe {
                syscall(
                    nr::READ,
                    fd.0 as usize,
                    extra.as_mut_ptr() as usize,
                    1,
                    0,
                    0,
                    0,
                )
            };
            if more != 0 {
                return Err("file exceeds bounded input limit");
            }
            break;
        }
    }
    Ok(length)
}
pub fn create_file(base: i32, path: &str, bytes: &[u8]) -> Result<()> {
    let fd = openat(base, path, 1 | O_CREAT | O_EXCL | O_NOFOLLOW, 0o600)
        .map_err(|_| "cannot exclusively create metadata file")?;
    let mut remaining = bytes;
    while !remaining.is_empty() {
        let n = unsafe {
            syscall(
                nr::WRITE,
                fd.0 as usize,
                remaining.as_ptr() as usize,
                remaining.len(),
                0,
                0,
                0,
            )
        };
        if n == -4 {
            continue;
        }
        if n <= 0 {
            return Err("metadata write failed");
        }
        remaining = &remaining[n as usize..];
    }
    fsync(fd.0)
}
pub fn fsync(fd: i32) -> Result<()> {
    if unsafe { syscall(nr::FSYNC, fd as usize, 0, 0, 0, 0, 0) } < 0 {
        Err("filesystem synchronization failed")
    } else {
        Ok(())
    }
}
pub fn mkdir(base: i32, path: &str, mode: usize) -> core::result::Result<(), isize> {
    let c = Path::from(path).map_err(|_| -36isize)?;
    let n = unsafe {
        syscall(
            nr::MKDIRAT,
            base as usize,
            c.as_ptr() as usize,
            mode,
            0,
            0,
            0,
        )
    };
    if n < 0 { Err(n) } else { Ok(()) }
}
pub fn unlink(base: i32, path: &str, directory: bool) -> Result<()> {
    let c = Path::from(path)?;
    let n = unsafe {
        syscall(
            nr::UNLINKAT,
            base as usize,
            c.as_ptr() as usize,
            if directory { 0x200 } else { 0 },
            0,
            0,
            0,
        )
    };
    if n < 0 && n != ENOENT {
        Err("cannot remove managed entry")
    } else {
        Ok(())
    }
}
pub fn symlink(source: &str, parent: i32, name: &str) -> Result<()> {
    let from = Path::from(source)?;
    let to = Path::from(name)?;
    let n = unsafe {
        syscall(
            nr::SYMLINKAT,
            from.as_ptr() as usize,
            parent as usize,
            to.as_ptr() as usize,
            0,
            0,
            0,
        )
    };
    if n < 0 {
        Err("cannot create symlink (a competing entry may have appeared)")
    } else {
        Ok(())
    }
}
pub fn rename(from_fd: i32, from: &str, to_fd: i32, to: &str) -> Result<()> {
    let a = Path::from(from)?;
    let b = Path::from(to)?;
    let n = unsafe {
        syscall(
            nr::RENAMEAT2,
            from_fd as usize,
            a.as_ptr() as usize,
            to_fd as usize,
            b.as_ptr() as usize,
            1, /* RENAME_NOREPLACE */
            0,
        )
    };
    if n < 0 {
        Err("cannot move entry without replacing another entry")
    } else {
        Ok(())
    }
}
pub fn readlink(base: i32, name: &str) -> core::result::Result<Path, isize> {
    let c = Path::from(name).map_err(|_| -36isize)?;
    let mut out = Path::new();
    let n = unsafe {
        syscall(
            nr::READLINKAT,
            base as usize,
            c.as_ptr() as usize,
            out.data.as_mut_ptr() as usize,
            PATH_CAP - 1,
            0,
            0,
        )
    };
    if n < 0 {
        return Err(n);
    }
    out.len = n as usize;
    if out.len == PATH_CAP - 1 || core::str::from_utf8(&out.data[..out.len]).is_err() {
        return Err(-36);
    }
    Ok(out)
}
pub fn canonical(fd: i32) -> Result<Path> {
    let mut path = Path::new();
    write!(&mut path, "/proc/self/fd/{fd}").map_err(|_| "path too long")?;
    let value = readlink(AT_FDCWD, path.as_str())
        .map_err(|_| "Linux /proc is required to resolve open directories")?;
    if !value.as_str().starts_with('/')
        || value.as_str().ends_with(" (deleted)")
        || value.bytes().iter().any(|b| *b < 32 || *b == 127)
    {
        return Err("directory has an unsupported canonical path");
    }
    Ok(value)
}
pub fn kind(fd: i32) -> Result<u16> {
    #[repr(align(8))]
    struct Statx([u8; 256]);
    let mut info = Statx([0; 256]);
    let n = unsafe {
        syscall(
            nr::STATX,
            fd as usize,
            c"".as_ptr() as usize,
            0x1000 | 0x100, /* AT_EMPTY_PATH | AT_SYMLINK_NOFOLLOW */
            1,
            info.0.as_mut_ptr() as usize,
            0,
        )
    };
    if n < 0 {
        return Err("statx failed (Linux 4.11 or newer is required)");
    }
    Ok(u16::from_ne_bytes([info.0[28], info.0[29]]) & 0o170000)
}
pub fn exists(base: i32, name: &str) -> Result<bool> {
    match openat(base, name, O_PATH | O_NOFOLLOW, 0) {
        Ok(_) => Ok(true),
        Err(ENOENT) => Ok(false),
        Err(_) => Err("cannot inspect entry"),
    }
}
pub fn lock(target: i32) -> Result<Fd> {
    let fd = openat(target, ".zdot.lock", O_CREAT | O_NOFOLLOW, 0o600)
        .map_err(|_| "cannot open deployment lock")?;
    if kind(fd.0)? != 0o100000 {
        return Err("deployment lock is not a regular file");
    }
    if unsafe {
        syscall(
            nr::FLOCK,
            fd.0 as usize,
            2 | 4, /* LOCK_EX | LOCK_NB */
            0,
            0,
            0,
            0,
        )
    } < 0
    {
        return Err("another zdot operation holds the deployment lock");
    }
    Ok(fd)
}
pub unsafe fn exec_git(args: &[*const u8], envp: *const *const u8) -> Result<()> {
    for path in [c"/usr/bin/git", c"/bin/git"] {
        let n = unsafe {
            syscall(
                nr::EXECVE,
                path.as_ptr() as usize,
                args.as_ptr() as usize,
                envp as usize,
                0,
                0,
                0,
            )
        };
        if n != ENOENT {
            return Err("could not execute Git");
        }
    }
    Err("Git was not found at /usr/bin/git or /bin/git")
}

// LLVM can lower slice copies and array initialization to these symbols.
// Volatile accesses prevent these definitions being replaced by recursive calls.
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
            return x as i32 - y as i32;
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
