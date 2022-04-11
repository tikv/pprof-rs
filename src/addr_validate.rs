use std::cell::RefCell;

use nix::{
    errno::Errno,
    fcntl::OFlag,
    unistd::{close, pipe2, read, write},
};

thread_local! {
    static MEM_VALIDATE_PIPE: RefCell<[i32; 2]> = RefCell::new([-1, -1]);
}

fn open_pipe() -> nix::Result<()> {
    MEM_VALIDATE_PIPE.with(|pipes| {
        let mut pipes = pipes.borrow_mut();

        // ignore the result
        let _ = close(pipes[0]);
        let _ = close(pipes[1]);

        let (read_fd, write_fd) = pipe2(OFlag::O_CLOEXEC | OFlag::O_NONBLOCK)?;

        pipes[0] = read_fd;
        pipes[1] = write_fd;

        Ok(())
    })
}

pub fn validate(addr: *const libc::c_void) -> bool {
    // read data in the pipe
    let valid_read = MEM_VALIDATE_PIPE.with(|pipes| {
        let pipes = pipes.borrow();
        loop {
            let mut buf = [0u8];

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
            let buf = unsafe { std::slice::from_raw_parts(addr as *const u8, 1) };

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
