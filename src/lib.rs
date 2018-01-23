//! Library for creating NSSwitch resolver libraries for Linux.

extern crate libc;

mod alloc;
pub mod errors;
pub mod interfaces;
#[macro_use] pub mod macros;

use std::ffi::CStr;
use std::net::{IpAddr, Ipv4Addr, Ipv6Addr};

use interfaces::{AddressFamily, HostAddressList, HostEntry, Switcheroo};
use errors::Result;

/*
extern "C" {
    fn inet_pton(af: c_int, src: *const c_char, dst: *mut c_void) -> c_int;

    fn inet_addr(cp: *const c_char) -> in_addr_t;
}
*/
    
#[repr(C)]
#[allow(non_camel_case_types)]
pub enum nss_status {
    NSS_STATUS_TRYAGAIN = -2,
    NSS_STATUS_UNAVAIL = -1,
    NSS_STATUS_NOTFOUND = 0,
    NSS_STATUS_SUCCESS = 1,
}

/*
fn make_hostent(
    name: *const c_char,
    af: c_int,
    length: c_int,
    addr: *mut c_char,
) -> hostent {
    let addr_list: [*mut c_char; 2] = [addr, null_mut()];
    hostent {
        h_name: strdup(name),
        h_aliases: into_malloc_heap(null_mut()),
        h_addrtype: af,
        h_length: length,
        h_addr_list: relax_array_ptr(into_malloc_heap(addr_list)),
    }
}
 */

struct ExampleResolver;

impl Switcheroo for ExampleResolver {
    fn gethostbyname2_r(name: &CStr, af: AddressFamily) -> Result<Option<HostEntry>> {
        use std::borrow::Cow;

        // Convert the C null-terminated string `name` to a Rust string.
        let name_str = match name.to_str() {
            Err(_) => return Ok(None),  // `name` isn't UTF-8, so bail out.
            Ok(s) => s,
        };

        let name_tld = match name_str.rfind('.') {
            None => return Ok(None),
            Some(index) => &name_str[index + 1..],
        };

        let domains = std::env::var("DEV_TLD_DOMAINS").unwrap_or_else(|_| "dev".to_string());
        for domain in domains.split(',') {
            if name_tld.eq_ignore_ascii_case(domain) {
                return Ok(Some(HostEntry {
                    name: Cow::Borrowed(name),
                    aliases: vec![],
                    addr_list: match af {
                        AddressFamily::Ipv4 => HostAddressList::V4(vec![
                            Ipv4Addr::new(127, 0, 0, 1)
                        ]),
                        AddressFamily::Ipv6 => HostAddressList::V6(vec![
                            Ipv6Addr::new(0, 0, 0, 0, 0, 0, 0, 1)
                        ]),
                    }
                }));
            }
        }

        Ok(None)
    }

    fn gethostbyaddr_r(_addr: &IpAddr) -> Result<Option<HostEntry>> {
        Ok(None)
    }
}

nssglue_gethostbyname2_r!(_nss_example_gethostbyname2_r, ExampleResolver);
nssglue_gethostbyaddr_r!(_nss_example_gethostbyaddr_r, ExampleResolver);


// stuff i want to throw away
/*
pub unsafe extern "C" fn _nss_dev_tld_gethostbyname_r(
    name: *const c_char,
    result: *mut hostent,
    buffer: *mut c_char,
    buflen: usize,
    errnop: *mut c_int,
    h_errnop: *mut c_int,
) -> nss_status {
    return _nss_dev_tld_gethostbyname2_r(name, AF_INET, result, buffer, buflen, errnop, h_errnop);
}

unsafe fn dev_tld_fill_hostent(
    name: *const c_char,
    af: c_int,
    result: *mut hostent,
) -> nss_status {
    const INADDRSZ: c_int = 4;
    const IN6ADDRSZ: c_int = 16;

    *result = match af {
        AF_INET => make_hostent(name, AF_INET, INADDRSZ, {
            let addr: in_addr_t = inet_addr(b"127.0.0.1\0" as *const u8 as *const c_char);
            addr as *mut c_char
        }),
        AF_INET6 => make_hostent(name, AF_INET6, IN6ADDRSZ, {
            let mut addr6: in6_addr = std::mem::uninitialized();
            inet_pton(
                AF_INET6,
                b"::1\0" as *const u8 as *const c_char,
                &mut addr6 as *mut in6_addr as *mut c_void,
            );
            into_malloc_heap(addr6) as *mut c_char
        }),
        _ => return nss_status::NSS_STATUS_NOTFOUND,
    };

    nss_status::NSS_STATUS_SUCCESS
}

*/
