use std::fmt;

/// The single error type surfaced across the Lattice workspace.
///
/// Provider trait methods return `Result<T, Error>` — never a raw OS error
/// type (`std::io::Error`, a bare `errno`, a Windows `DWORD`). See
/// ARCHITECTURE.md's Error Model for why.
#[derive(Debug)]
pub enum Error {
    /// The operation requires privileges the caller does not have (e.g.
    /// `CAP_NET_ADMIN` on Linux, Administrator on Windows).
    PermissionDenied,
    /// The referenced object does not exist.
    NotFound,
    /// An object with the same identity already exists.
    AlreadyExists,
    /// The operation has no meaning on this backend at all, as opposed to
    /// a `Capability` being merely absent at runtime.
    Unsupported,
    /// The operation is not valid given the object's current state.
    InvalidState,
    /// Escape hatch preserving the raw backend-specific error for
    /// diagnostics. Not the primary way consumers are expected to match on
    /// failures.
    Platform(PlatformErrorCode),
}

/// A platform-tagged raw error code.
///
/// Linux errno is a signed `i32`, Windows error codes are an unsigned
/// `DWORD` (`u32`); collapsing both into one untyped integer would either
/// truncate one of them or imply the two are comparable, which they are
/// not.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum PlatformErrorCode {
    Linux(i32),
    Windows(u32),
    Darwin(i32),
}

impl fmt::Display for Error {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Error::PermissionDenied => write!(f, "permission denied"),
            Error::NotFound => write!(f, "not found"),
            Error::AlreadyExists => write!(f, "already exists"),
            Error::Unsupported => write!(f, "unsupported operation"),
            Error::InvalidState => write!(f, "invalid state"),
            Error::Platform(code) => write!(f, "platform error: {code:?}"),
        }
    }
}

impl std::error::Error for Error {}
