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
    use nix::fcntl::{fcntl, FcntlArg, FdFlag};
    use nix::unistd::pipe;

    let (read_fd, write_fd) = pipe()?;

    let mut flags = FdFlag::from_bits(fcntl(read_fd, FcntlArg::F_GETFL)?).unwrap();
    flags |= FdFlag::FD_CLOEXEC | FdFlag::O_NONBLOCK;
    fcntl(read_fd, FcntlArg::F_SETFD(flags))?;

    let mut flags = FdFlag::from_bits(fcntl(write_fd, FcntlArg::F_GETFL)?).unwrap();
    flags |= FdFlag::FD_CLOEXEC | FdFlag::O_NONBLOCK;
    fcntl(write_fd, FcntlArg::F_SETFD(flags))?;

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

        assert_eq!(validate(&i as *const _ as *const libc::c_void), true);
    }

    #[test]
    fn validate_heap() {
        let vec = vec![0; 1000];

        for i in vec.iter() {
            assert_eq!(validate(i as *const _ as *const libc::c_void), true);
        }
    }

    #[test]
    fn failed_validate() {
        assert_eq!(validate(0 as *const libc::c_void), false);
        assert_eq!(validate((-1 as i32) as usize as *const libc::c_void), false)
    }
}
