use std::{
    mem::size_of,
    sync::atomic::{AtomicI32, Ordering},
};

// This module talks to the OS through `libc` directly rather than through `nix`.
// `validate` runs inside the profiler's signal handler, so the calls here must be
// async-signal-safe (raw `read`/`write`/`pipe2`/`close` and reading `errno`
// qualify). Using `libc` also decouples this crate from the churn in `nix`'s
// file-descriptor API, letting `nix` be depended on with a broad version range.

struct Pipes {
    read_fd: AtomicI32,
    write_fd: AtomicI32,
}

static MEM_VALIDATE_PIPE: Pipes = Pipes {
    read_fd: AtomicI32::new(-1),
    write_fd: AtomicI32::new(-1),
};

/// Returns the current `errno`. Reading it is async-signal-safe and does not
/// allocate.
#[inline]
fn errno() -> i32 {
    std::io::Error::last_os_error().raw_os_error().unwrap_or(0)
}

#[inline]
#[cfg(any(target_os = "android", target_os = "linux"))]
fn create_pipe() -> Result<(i32, i32), i32> {
    let mut fds = [0 as libc::c_int; 2];
    // Safety: `fds` points to an array of two `c_int`s, as `pipe2` requires. The
    // fds are intentionally leaked (never wrapped in `OwnedFd`): these pipes live
    // for the whole program lifetime and their raw fds are stored in atomics so
    // they can be used from the signal handler.
    let res = unsafe { libc::pipe2(fds.as_mut_ptr(), libc::O_CLOEXEC | libc::O_NONBLOCK) };
    if res != 0 {
        return Err(errno());
    }
    Ok((fds[0], fds[1]))
}

#[inline]
#[cfg(any(target_os = "macos", target_os = "freebsd"))]
fn create_pipe() -> Result<(i32, i32), i32> {
    fn set_flags(fd: libc::c_int) -> Result<(), i32> {
        // Safety: `fd` is a valid fd returned by `pipe`.
        unsafe {
            let flags = libc::fcntl(fd, libc::F_GETFD);
            if flags < 0 || libc::fcntl(fd, libc::F_SETFD, flags | libc::FD_CLOEXEC) < 0 {
                return Err(errno());
            }
            let flags = libc::fcntl(fd, libc::F_GETFL);
            if flags < 0 || libc::fcntl(fd, libc::F_SETFL, flags | libc::O_NONBLOCK) < 0 {
                return Err(errno());
            }
        }
        Ok(())
    }

    let mut fds = [0 as libc::c_int; 2];
    // Safety: `fds` points to an array of two `c_int`s, as `pipe` requires. The
    // fds are intentionally leaked (see the Linux variant for rationale).
    let res = unsafe { libc::pipe(fds.as_mut_ptr()) };
    if res != 0 {
        return Err(errno());
    }
    set_flags(fds[0])?;
    set_flags(fds[1])?;
    Ok((fds[0], fds[1]))
}

fn open_pipe() -> Result<(), i32> {
    // Safety: closing the previously stored fds. They may be the -1 sentinel or
    // already-closed, in which case `close` fails with EBADF, which we ignore.
    unsafe {
        libc::close(MEM_VALIDATE_PIPE.read_fd.load(Ordering::SeqCst));
        libc::close(MEM_VALIDATE_PIPE.write_fd.load(Ordering::SeqCst));
    }

    let (read_fd, write_fd) = create_pipe()?;

    MEM_VALIDATE_PIPE.read_fd.store(read_fd, Ordering::SeqCst);
    MEM_VALIDATE_PIPE.write_fd.store(write_fd, Ordering::SeqCst);

    Ok(())
}

// validate whether the address `addr` is readable through `write()` to a pipe
//
// if the second argument of `write(ptr, buf)` is not a valid address, the
// `write()` will return an error the error number should be `EFAULT` in most
// cases, but we regard all errors (except EINTR) as a failure of validation
//
// `addr` is handed straight to `libc::write` and never dereferenced in Rust (in
// particular we deliberately avoid `slice::from_raw_parts`, which would be UB for
// an invalid `addr`), so keeping this a safe function is sound.
#[allow(clippy::not_unsafe_ptr_arg_deref)]
pub fn validate(addr: *const libc::c_void) -> bool {
    // it's a short circuit for null pointer, as it'll give an error in
    // `std::slice::from_raw_parts` if the pointer is null.
    if addr.is_null() {
        return false;
    }

    const CHECK_LENGTH: usize = 2 * size_of::<*const libc::c_void>() / size_of::<u8>();

    // read data in the pipe
    let read_fd = MEM_VALIDATE_PIPE.read_fd.load(Ordering::SeqCst);
    // The pipe is created lazily; a sentinel of -1 means it has not been set up
    // yet, which is treated as an invalid read so the pipe gets (re)opened below.
    let valid_read = read_fd >= 0
        && loop {
            let mut buf = [0u8; CHECK_LENGTH];

            // Safety: `read_fd` is a valid fd and `buf` is valid for writing
            // `CHECK_LENGTH` bytes.
            let ret = unsafe {
                libc::read(read_fd, buf.as_mut_ptr() as *mut libc::c_void, CHECK_LENGTH)
            };
            if ret >= 0 {
                break ret > 0;
            }
            match errno() {
                libc::EINTR => continue,
                libc::EAGAIN => break true,
                _ => break false,
            }
        };

    if !valid_read && open_pipe().is_err() {
        return false;
    }

    let write_fd = MEM_VALIDATE_PIPE.write_fd.load(Ordering::SeqCst);
    loop {
        // Safety: `write_fd` is a valid fd. `addr` is passed straight to `write`
        // as the source buffer: whether it is readable is exactly what we are
        // testing, so the kernel returns EFAULT instead of faulting the process.
        let ret = unsafe { libc::write(write_fd, addr, CHECK_LENGTH) };
        if ret >= 0 {
            break ret > 0;
        }
        match errno() {
            libc::EINTR => continue,
            _ => break false,
        }
    }
}

#[cfg(test)]
mod test {
    use super::*;

    #[test]
    fn validate_stack() {
        let i = 0;

        assert!(validate(&i as *const _ as *const libc::c_void));
    }

    #[test]
    fn validate_heap() {
        let vec = vec![0; 1000];

        for i in vec.iter() {
            assert!(validate(i as *const _ as *const libc::c_void));
        }
    }

    #[test]
    fn failed_validate() {
        assert!(!validate(std::ptr::null::<libc::c_void>()));
        assert!(!validate(-1_i32 as usize as *const libc::c_void))
    }
}
