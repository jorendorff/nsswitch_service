//! Library for creating NSSwitch resolver libraries for Linux.

extern crate libc;

mod alloc;
mod errors;
mod interfaces;
#[macro_use] pub mod macros;

pub use interfaces::{AddressFamily, NameService, HostAddressList, HostEntry};
pub use errors::{Error, HostError, NssStatus, Result};
