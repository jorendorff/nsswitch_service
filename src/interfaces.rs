//! Safe interfaces to NSSwitch.

use std::borrow::Cow;
use std::ffi::CStr;
use std::net::{IpAddr, Ipv4Addr, Ipv6Addr};
use errors::Result;

pub enum AddressFamily {
    Ipv4,
    Ipv6
}

/// A list of addresses that are of the same address family (either all IPv4 or
/// all IPv6).
pub enum HostAddressList {
    V4(Vec<Ipv4Addr>),
    V6(Vec<Ipv6Addr>),
}

/// Information about a host, the type of record returned by `gethostbyname`
/// and friends.
pub struct HostEntry<'a> {
    pub name: Cow<'a, CStr>,
    pub aliases: Vec<Cow<'a, CStr>>,
    pub addr_list: HostAddressList,
}

pub trait Database {
    fn gethostbyname_r(name: &CStr) -> Result<Option<HostEntry>> {
        Self::gethostbyname2_r(name, AddressFamily::Ipv4)
    }

    /// Look up addresses for the hostname `name`.
    /// To intercept the `gethostbyname2_r` function, implement this method
    /// and use the `nssglue_gethostbyname2_r!(_nss_libraryname_gethostbyname2_r, DatabaseType);`
    /// macro.
    ///
    /// This method must cope with the fact that C users can pass strings that
    /// aren't valid UTF-8. The easiest way is to bail out in that case:
    ///
    /// ```
    /// // Convert the C null-terminated string `name` to a Rust &str.
    /// let name_str = match name.to_str() {
    ///     Err(_) => return Ok(None),  // `name` isn't UTF-8, so bail out.
    ///     Ok(s) => s,
    /// };
    /// ```
    ///
    /// The `gethostbyname2_r` method must return one of these:
    ///
    /// *   An ordinary C error, `Err(Error::with_errno(...))`;
    /// *   A `gethostbyname`-specific error, `Err(Error::with_h_errno(...))`;
    /// *   `Ok(None)` to indicate that no addresses exist for the name;
    /// *   `Ok(Some(HostEntry))`, a successful query result.
    ///
    fn gethostbyname2_r(name: &CStr, af: AddressFamily) -> Result<Option<HostEntry>>;

    fn gethostbyaddr_r(addr: &IpAddr) -> Result<Option<HostEntry>>;
}

