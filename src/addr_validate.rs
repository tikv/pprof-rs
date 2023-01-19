use std::{cell::RefCell, mem::size_of};

use nix::{
    errno::Errno,
    unistd::{close, read, write},
};

thread_local! {
    static MEM_VALIDATE_PIPE: RefCell<[i32; 2]> = RefCell::new([-1, -1]);
}

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
    MEM_VALIDATE_PIPE.with(|pipes| {
        let mut pipes = pipes.borrow_mut();

        // ignore the result
        let _ = close(pipes[0]);
        let _ = close(pipes[1]);

        let (read_fd, write_fd) = create_pipe()?;

        pipes[0] = read_fd;
        pipes[1] = write_fd;

        Ok(())
    })
}

pub fn validate(addr: *const libc::c_void) -> bool {
    if addr.is_null() {
        return false;
    }

    const CHECK_LENGTH: usize = 2 * size_of::<*const libc::c_void>() / size_of::<u8>();

    // read data in the pipe
    let valid_read = MEM_VALIDATE_PIPE.with(|pipes| {
        let pipes = pipes.borrow();
        loop {
            let mut buf = [0u8; CHECK_LENGTH];

            match read(pipes[0], &mut buf) {
                Ok(bytes) => break bytes > 0,
                Err(_err @ Errno::EINTR) => continue,
                Err(_err @ Errno::EAGAIN) => break true,
                Err(_) => break false,
            }
        }
    });

    if !valid_read && open_pipe().is_err() {
        return false;
    }

    MEM_VALIDATE_PIPE.with(|pipes| {
        let pipes = pipes.borrow();
        loop {
            let buf = unsafe { std::slice::from_raw_parts(addr as *const u8, CHECK_LENGTH) };

            match write(pipes[1], buf) {
                Ok(bytes) => break bytes > 0,
                Err(_err @ Errno::EINTR) => continue,
                Err(_) => break false,
            }
        }
    })
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
