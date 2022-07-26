use crate::validator::{Validator, ValidatorImpl};

impl ValidatorImpl for Validator {
    fn addr_validate(_: *const libc::c_void) -> bool {
        unimplemented!()
    }
}
