use libc::{self, c_int, EINVAL, ERANGE};
use std::result;

#[repr(C)]
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum NssStatus {
    TryAgain = -2,
    Unavailable = -1,
    NotFound = 0,
    Success = 1,
}

#[derive(Clone, Debug)]
pub struct Error {
    status: NssStatus,
    errno: c_int,
    h_errno: c_int,
}

pub type Result<T> = result::Result<T, Error>;

pub(crate) const NETDB_INTERNAL: c_int = -1;  // see errno
const NETDB_SUCCESS: c_int = 0;    // no problem

/// Constant 
pub const HOST_NOT_FOUND: c_int = 1;   // Authoritative Answer Host not found
pub const TRY_AGAIN: c_int = 2;        // Non-Authoritative not found, or SERVFAIL
pub const NO_RECOVERY: c_int = 3;      // Non-Recoverable: FORMERR, REFUSED, NOTIMP
pub const NO_DATA: c_int = 4;          // Valid name, no data for requested type

macro_rules! abort {
    ($($message: expr),*) => {
        eprintln!($($message),*);
        unsafe {
            libc::abort();
        }
    }
}

impl Error {
    pub(crate) fn buffer_too_small() -> Error {
        Error {
            status: NssStatus::TryAgain,
            errno: ERANGE,
            h_errno: NETDB_INTERNAL,
        }
    }

    pub(crate) fn invalid_args() -> Error {
        Error::new(NssStatus::Unavailable, EINVAL, NETDB_INTERNAL)
    }

    pub fn with_errno(status: NssStatus, errno: c_int) -> Error {
        Error::new(status, errno, NETDB_INTERNAL)
    }

    pub fn with_host(status: NssStatus, errno: c_int, h_errno: c_int) -> Error {
        Error::new(status, errno, h_errno)
    }

    fn new(status: NssStatus, errno: c_int, h_errno: c_int) -> Error {
        // Check for invalid combinations. Don't allow nsswitch resolvers to
        // fail while claiming success, as that would lead to undefined
        // behavior (the out-parameters are left uninitialized on error, but
        // users will think they are populated).
        if status == NssStatus::Success {
            abort!("nsswitch resolver: internal error reporting an error: status == NSS_STATUS_SUCCESS");
        }
        if h_errno == NETDB_SUCCESS {
            abort!("nsswitch resolver: internal error reporting an error: h_errno == 0");
        }
        if h_errno == NETDB_INTERNAL && errno == 0 {
            abort!("nsswitch resolver: internal error reporting an error: errno == 0");
        }
        if status == NssStatus::TryAgain && errno == ERANGE {
            // The NSSwitch documentation reserves this combination of error
            // codes for complaining that the user-provided buffer is not large
            // enough. Since we never let safe Rust code see `buflen`, safe
            // Rust can't legitimately use this combination.
            abort!("nsswitch resolver: internal error reporting an error: errno == ERANGE is reserved");
        }

        Error { status, errno, h_errno }
    }

/*
    pub(crate) unsafe fn report(self, errnop: *mut c_int) -> NssStatus {
        if self.h_errno != NETDB_INTERNAL {
            eprintln!("nsswitch resolver: internal error reporting an error: host errors not supported for this function");
            libc::abort();
        }
        *errnop = self.errno;
        self.status
    }
*/

    pub(crate) unsafe fn report_with_host(self, errnop: *mut c_int, h_errnop: *mut c_int) -> NssStatus {
        *h_errnop = self.h_errno;
        if self.h_errno == NETDB_INTERNAL {
            *errnop = self.errno;
        }
        self.status
    }
}

