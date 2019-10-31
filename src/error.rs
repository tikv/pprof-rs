quick_error! {
    #[derive(Debug)]
    pub enum Error {
        NixError(err: nix::Error) {
            from()
            cause(err)
            description(err.description())
        }
        IoError(err: std::io::Error) {
            from()
            cause(err)
            description(err.description())
        }
        CreatingError
        Running
        NotRunning
    }
}

pub type Result<T> = std::result::Result<T, Error>;
