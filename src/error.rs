quick_error! {
    #[derive(Debug)]
    pub enum Error {
        NixError(err: nix::Error) {
            from()
            cause(err)
            description(err.description())
        }
        Running
        NotRunning
    }
}

pub type Result<T> = std::result::Result<T, Error>;
