//! Адреса сетевых интерфейсов (`getifaddrs`): общие для macOS и iOS.

use std::ffi::CStr;
use std::net::{IpAddr, Ipv4Addr};

/// Адрес сетевого интерфейса.
#[derive(Clone, Debug, PartialEq, Eq, PartialOrd, Ord)]
pub struct IfAddr {
    pub name: String,
    pub addr: IpAddr,
    pub up: bool,
}

/// Все адреса всех интерфейсов (`getifaddrs`).
pub fn interfaces() -> Vec<IfAddr> {
    let mut out = Vec::new();
    unsafe {
        let mut ifap: *mut libc::ifaddrs = std::ptr::null_mut();
        if libc::getifaddrs(&mut ifap) != 0 {
            return out;
        }
        let mut p = ifap;
        while !p.is_null() {
            let ifa = &*p;
            p = ifa.ifa_next;
            if ifa.ifa_addr.is_null() || ifa.ifa_name.is_null() {
                continue;
            }
            let addr = match i32::from((*ifa.ifa_addr).sa_family) {
                libc::AF_INET => {
                    let sin = &*(ifa.ifa_addr as *const libc::sockaddr_in);
                    IpAddr::V4(Ipv4Addr::from(u32::from_be(sin.sin_addr.s_addr)))
                }
                libc::AF_INET6 => {
                    let sin6 = &*(ifa.ifa_addr as *const libc::sockaddr_in6);
                    IpAddr::V6(std::net::Ipv6Addr::from(sin6.sin6_addr.s6_addr))
                }
                _ => continue,
            };
            let flags = ifa.ifa_flags as libc::c_int;
            let up = flags & libc::IFF_UP != 0 && flags & libc::IFF_RUNNING != 0;
            out.push(IfAddr { name: CStr::from_ptr(ifa.ifa_name).to_string_lossy().into_owned(), addr, up });
        }
        libc::freeifaddrs(ifap);
    }
    out
}
