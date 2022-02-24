use libc::c_void;

use libunwind_rs::Cursor;

// TODO: need a better Debug implementation
#[derive(Clone, Debug)]
pub struct Frame {
    pub ip: usize,
    pub sp: usize,
    pub symbol_address: usize,
}

impl super::Frame for Frame {
    type S = backtrace::Symbol;

    fn resolve_symbol<F: FnMut(&Self::S)>(&self, cb: F) {
        backtrace::resolve(self.ip as *mut c_void, cb)
    }

    fn symbol_address(&self) -> *mut libc::c_void {
        self.symbol_address as *mut c_void
    }
}

pub fn trace<F: FnMut(&Frame) -> bool>(mut cb: F) {
    // TODO: come up with a better way to handle this error
    let _ = Cursor::local(|mut cursor| -> Result<(), libunwind_rs::Error> {
        loop {
            let mut symbol_address = 0;
            if let Ok(proc_info) = cursor.proc_info() {
                symbol_address = proc_info.start();
            }

            if cb(&Frame {
                ip: cursor.ip()?,
                sp: cursor.sp()?,
                symbol_address,
            }) {
                if cursor.step()? {
                    continue;
                }
            }

            break;
        }
        Ok(())
    });
}

pub use backtrace::Symbol;
