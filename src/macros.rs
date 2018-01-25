use alloc::BumpAllocator;
use errors::{Error, Result};
pub use errors::NssStatus;
use interfaces::{AddressFamily, HostEntry, HostAddressList, NameService};
use libc::{AF_INET, AF_INET6, in_addr_t, in6_addr };
pub use libc::{c_char, c_int, c_void, ENOENT, hostent};
use std::{iter, mem, ptr};
use std::ffi::CStr;
use std::net::{IpAddr, Ipv4Addr, Ipv6Addr};


/// In C, the same type `T*` is used to mean both pointer-to-T and
/// pointer-to-array-of-T.
///
/// In Rust, those are two different things; use this function to convert
/// a pointer-to-array `*mut [T]` to the pointer of type `*mut T` that points
/// to element 0 of the array.  (That conversion is implicit and automatic in C,
/// but not in Rust.)
///
/// If the array has length 0, this is safe, but don't dereference the
/// resulting pointer. It's a one-past-the-end pointer, not a pointer to an
/// object of type T.
fn relax_array_ptr<T>(p: &mut [T]) -> *mut T {
    p as *mut [T] as *mut T
}

fn to_in_addr_t(ip: Ipv4Addr) -> in_addr_t {
    <Ipv4Addr as Into<u32>>::into(ip)
}

fn to_in6_addr(ipv6: Ipv6Addr) -> in6_addr {
    // in6_addr has, unfortunately, a private field, for alignment.
    // This means we can't construct it just by writing
    //     in6_addr { s6_addr: ipv6.octets() }
    // as we would like. Fortunately the private field is 0-size.
    unsafe {
        mem::transmute(ipv6.octets())
    }
}

impl<'a> HostEntry<'a> {
    fn write_to(
        &self,
        resultp: *mut hostent,
        buffer: *mut c_char,
        buflen: usize
    ) -> Result<()> {
        let mut allocator = unsafe { BumpAllocator::from_ptr(buffer, buflen) }?;

        let h_name = allocator.copy_c_str(&self.name)?.as_ptr() as *mut c_char;
        let h_aliases =
            if self.aliases.is_empty() {
                ptr::null_mut()
            } else {
                let copied_aliases: Result<Vec<*mut c_char>> =
                    self.aliases.iter()
                    .map(|alias| {
                        allocator.copy_c_str(alias)
                            .map(|cstr| cstr.as_ptr() as *mut c_char)
                    })
                    .collect();
                allocator.allocate_array(copied_aliases?.into_iter())?.as_mut_ptr()
            };

        let (h_addrtype, h_length, h_addr_list) =
            match self.addr_list {
                HostAddressList::V4(ref addrs) => {
                    const INADDRSZ: c_int = 4;
                    debug_assert_eq!(INADDRSZ, mem::size_of::<in_addr_t>() as i32);

                    // First, store all the addresses in the user's buffer.
                    // This API uses network byte order, hence the `.to_be()`.
                    let buf_addrs: &mut [in_addr_t] = allocator.allocate_array(
                        addrs.iter()
                            .map(|ip| to_in_addr_t(*ip).to_be())
                    )?;

                    // Make a null-terminated array of pointers to the elements of buf_addrs.
                    // Cast these pointers to `*mut c_char` because C doesn't have generics.
                    let addr_ptrs: &mut [*mut c_char] = allocator.allocate_array(
                        buf_addrs.iter_mut()
                            .map(|ip_ref| ip_ref as *mut in_addr_t as *mut c_char)
                            .chain(iter::once(ptr::null_mut()))
                    )?;
                    (AF_INET, INADDRSZ, relax_array_ptr(addr_ptrs))
                }
                HostAddressList::V6(ref addrs) => {
                    // See the V4 case for an explanation.
                    const IN6ADDRSZ: c_int = 16;
                    debug_assert_eq!(IN6ADDRSZ, mem::size_of::<in6_addr>() as i32);

                    let buf_addrs: &mut [in6_addr] = allocator.allocate_array(
                        addrs.iter().map(|ipv6| to_in6_addr(*ipv6))
                    )?;

                    let addr_ptrs: &mut [*mut c_char] = allocator.allocate_array(
                        buf_addrs.iter_mut()
                            .map(|ipv6_ref| ipv6_ref as *mut in6_addr as *mut c_char)
                            .chain(iter::once(ptr::null_mut()))
                    )?;
                    (AF_INET6, IN6ADDRSZ, relax_array_ptr(addr_ptrs))
                }
            };

        unsafe {
            *resultp = hostent { h_name, h_aliases, h_addrtype, h_length, h_addr_list };
        }
        Ok(())
    }
}

/// Store the result of a `gethostbyname2_r()` lookup in the four
/// out-parameters provided by the caller.
pub fn write_host_lookup_result(
    lookup_result: Result<Option<HostEntry>>,
    resultp: *mut hostent,
    buffer: *mut c_char,
    buflen: usize,
    errnop: *mut c_int,
    h_errnop: *mut c_int,
) -> NssStatus {
    match lookup_result {
        Err(err) => unsafe {
            err.report_with_host(errnop, h_errnop)
        },

        Ok(None) => unsafe {
            Error::with_errno(NssStatus::NotFound, ENOENT)
                .report_with_host(errnop, h_errnop)
        }

        Ok(Some(host)) => unsafe {
            match host.write_to(resultp, buffer, buflen) {
                Err(err) => err.report_with_host(errnop, h_errnop),
                Ok(()) => NssStatus::Success
            }
        }
    }
}

#[inline]
pub unsafe fn call_gethostbyname_r<T: NameService>(
    name: *const c_char,
    result: *mut hostent,
    buffer: *mut c_char,
    buflen: usize,
    errnop: *mut c_int,
    h_errnop: *mut c_int,
) -> NssStatus {
    let lookup_result = T::gethostbyname_r(CStr::from_ptr(name));
    write_host_lookup_result(lookup_result, result, buffer, buflen, errnop, h_errnop)
}

#[macro_export]
macro_rules! nssglue_gethostbyname_r {
    ($name:ident, $t:ty) => {
        #[no_mangle]
        pub unsafe extern "C" fn $name(
            name: *const $crate::macros::c_char,
            result: *mut $crate::macros::hostent,
            buffer: *mut $crate::macros::c_char,
            buflen: usize,
            errnop: *mut $crate::macros::c_int,
            h_errnop: *mut $crate::macros::c_int,
        ) -> $crate::macros::NssStatus {
            $crate::macros::call_gethostbyname_r::<$t>(
                name,
                result,
                buffer,
                buflen,
                errnop,
                h_errnop
            )
        }
    }
}

#[inline]
pub unsafe fn call_gethostbyname2_r<T: NameService>(
    name: *const c_char,
    af: c_int,
    result: *mut hostent,
    buffer: *mut c_char,
    buflen: usize,
    errnop: *mut c_int,
    h_errnop: *mut c_int,
) -> NssStatus {
    let lookup_result = T::gethostbyname2_r(
        CStr::from_ptr(name),
        match af {
            AF_INET => AddressFamily::Ipv4,
            AF_INET6 => AddressFamily::Ipv6,
            _ => return Error::invalid_args().report_with_host(errnop, h_errnop)
        },
    );
    write_host_lookup_result(lookup_result, result, buffer, buflen, errnop, h_errnop)
}

/// This macro defines a function that implements `gethostbyname2_r` in a way
/// that NSSwitch can find and use.
///
/// The way this works in practice is like this:
///
/// *   A process calls `gethostbyname2_r()`.
///
/// *   `gethostbyname2_r` consults `/etc/nsswitch.conf` which (once you've configured it)
///     tells it to load your library, `/lib/libnss_YOURLIBNAME.so.2`.
///
/// *   So `gethostbyname2_r` loads your library, finds the
///     `_nss_YOURLIBNAME_gethostbyname2_r` function defined by this macro,
///     and calls it.
///
/// *   The macro-defined function is a minimal wrapper that delegates all the
///     actual work to the `gethostbyname2_r` method of `$t`, a `NameService`
///     implementation that you provide.
///
/// `$name` must be of the form `_nss_YOURLIBNAME_gethostbyname2_r`,
/// where `YOURLIBNAME` is the actual library name.
#[macro_export]
macro_rules! nssglue_gethostbyname2_r {
    ($name:ident, $t:ty) => {
        #[no_mangle]
        pub unsafe extern "C" fn $name(
            name: *const $crate::macros::c_char,
            af: $crate::macros::c_int,
            result: *mut $crate::macros::hostent,
            buffer: *mut $crate::macros::c_char,
            buflen: usize,
            errnop: *mut $crate::macros::c_int,
            h_errnop: *mut $crate::macros::c_int,
        ) -> $crate::macros::NssStatus {
            $crate::macros::call_gethostbyname2_r::<$t>(
                name,
                af,
                result,
                buffer,
                buflen,
                errnop,
                h_errnop
            )
        }
    }
}

#[inline]
pub unsafe fn call_gethostbyaddr_r<T: NameService>(
    addr: *const c_void,
    len: c_int,
    af: c_int,
    result: *mut hostent,
    buffer: *mut c_char,
    buflen: usize,
    errnop: *mut c_int,
    h_errnop: *mut c_int,
) -> NssStatus {
    let addr: IpAddr = match af {
        AF_INET => {
            if len as usize != mem::size_of::<u32>() {
                return Error::invalid_args().report_with_host(errnop, h_errnop);
            }
            let inaddr: in_addr_t = *(addr as *const u32);
            IpAddr::from(Ipv4Addr::from(inaddr))
        }
        AF_INET6 => {
            if len as usize != mem::size_of::<[u8; 16]>() {
                return Error::invalid_args().report_with_host(errnop, h_errnop);
            }
            let octets: [u8; 16] = *(addr as *const [u8; 16]);
            IpAddr::from(Ipv6Addr::from(octets))
        }
        _ => return Error::invalid_args().report_with_host(errnop, h_errnop)
    };
    let lookup_result = T::gethostbyaddr_r(&addr);
    write_host_lookup_result(lookup_result, result, buffer, buflen, errnop, h_errnop)
}

#[macro_export]
macro_rules! nssglue_gethostbyaddr_r {
    ($name:ident, $t:ty) => {
        #[no_mangle]
        pub unsafe extern "C" fn $name(
            addr: *const $crate::macros::c_void,
            len: $crate::macros::c_int,
            af: $crate::macros::c_int,
            result: *mut $crate::macros::hostent,
            buffer: *mut $crate::macros::c_char,
            buflen: usize,
            errnop: *mut $crate::macros::c_int,
            h_errnop: *mut $crate::macros::c_int,
        ) -> $crate::macros::NssStatus {
            $crate::macros::call_gethostbyaddr_r::<$t>(
                addr,
                len,
                af,
                result,
                buffer,
                buflen,
                errnop,
                h_errnop
            )
        }
    }
}
