pub struct Validator {}
pub trait ValidatorImpl {
    fn addr_validate(addr: *const libc::c_void) -> bool;
}

pub fn addr_validate(addr: *const libc::c_void) -> bool {
    Validator::addr_validate(addr)
}
