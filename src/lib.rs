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

nssglue_gethostbyname_r!(_nss_example_gethostbyname_r, ExampleResolver);
nssglue_gethostbyname2_r!(_nss_example_gethostbyname2_r, ExampleResolver);
nssglue_gethostbyaddr_r!(_nss_example_gethostbyaddr_r, ExampleResolver);
