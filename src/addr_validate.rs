use std::{
    mem::size_of,
    sync::atomic::{AtomicI32, Ordering},
};

use nix::{
    errno::Errno,
    unistd::{close, read, write},
};

struct Pipes {
    read_fd: AtomicI32,
    write_fd: AtomicI32,
}

static MEM_VALIDATE_PIPE: Pipes = Pipes {
    read_fd: AtomicI32::new(-1),
    write_fd: AtomicI32::new(-1),
};

#[inline]
#[cfg(target_os = "linux")]
fn create_pipe() -> nix::Result<(i32, i32)> {
    use nix::fcntl::OFlag;
    use nix::unistd::pipe2;

    pipe2(OFlag::O_CLOEXEC | OFlag::O_NONBLOCK)
}

#[inline]
#[cfg(target_os = "macos")]
fn create_pipe() -> nix::Result<(i32, i32)> {
    use nix::fcntl::{fcntl, FcntlArg, FdFlag, OFlag};
    use nix::unistd::pipe;
    use std::os::unix::io::RawFd;

    fn set_flags(fd: RawFd) -> nix::Result<()> {
        let mut flags = FdFlag::from_bits(fcntl(fd, FcntlArg::F_GETFD)?).unwrap();
        flags |= FdFlag::FD_CLOEXEC;
        fcntl(fd, FcntlArg::F_SETFD(flags))?;
        let mut flags = OFlag::from_bits(fcntl(fd, FcntlArg::F_GETFL)?).unwrap();
        flags |= OFlag::O_NONBLOCK;
        fcntl(fd, FcntlArg::F_SETFL(flags))?;
        Ok(())
    }

    let (read_fd, write_fd) = pipe()?;
    set_flags(read_fd)?;
    set_flags(write_fd)?;
    Ok((read_fd, write_fd))
}

fn open_pipe() -> nix::Result<()> {
    // ignore the result
    let _ = close(MEM_VALIDATE_PIPE.read_fd.load(Ordering::SeqCst));
    let _ = close(MEM_VALIDATE_PIPE.write_fd.load(Ordering::SeqCst));

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
pub fn validate(addr: *const libc::c_void) -> bool {
    const CHECK_LENGTH: usize = 2 * size_of::<*const libc::c_void>() / size_of::<u8>();

    // read data in the pipe
    let read_fd = MEM_VALIDATE_PIPE.read_fd.load(Ordering::SeqCst);
    let valid_read = loop {
        let mut buf = [0u8; CHECK_LENGTH];

        match read(read_fd, &mut buf) {
            Ok(bytes) => break bytes > 0,
            Err(_err @ Errno::EINTR) => continue,
            Err(_err @ Errno::EAGAIN) => break true,
            Err(_) => break false,
        }
    };

    if !valid_read && open_pipe().is_err() {
        return false;
    }

    let write_fd = MEM_VALIDATE_PIPE.write_fd.load(Ordering::SeqCst);
    loop {
        let buf = unsafe { std::slice::from_raw_parts(addr as *const u8, CHECK_LENGTH) };

        match write(write_fd, buf) {
            Ok(bytes) => break bytes > 0,
            Err(_err @ Errno::EINTR) => continue,
            Err(_) => break false,
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
