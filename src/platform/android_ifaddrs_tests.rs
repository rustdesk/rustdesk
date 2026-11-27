//! Tests for `android_ifaddrs.c`, the getifaddrs()/freeifaddrs() the Android
//! build links in place of bionic's. build.rs compiles the same file for Linux
//! with `RD_IFADDRS_TEST` and renamed functions, so it runs here against the
//! host kernel and is compared with the C library's own getifaddrs().

use hbb_common::libc;
use std::{
    collections::BTreeSet,
    ffi::CStr,
    io,
    net::{IpAddr, Ipv4Addr, Ipv6Addr},
};

extern "C" {
    fn rd_test_getifaddrs(ifap: *mut *mut libc::ifaddrs) -> libc::c_int;
    fn rd_test_freeifaddrs(ifa: *mut libc::ifaddrs);
    /// Nonzero: the shim's RTM_GETLINK request fails with this errno.
    static mut rd_ifaddrs_test_getlink_errno: libc::c_int;
}

/// (interface name, address) of every IPv4/IPv6 entry; other families are ignored.
type Entries = BTreeSet<(String, IpAddr)>;

unsafe fn collect(head: *const libc::ifaddrs) -> Entries {
    let mut out = Entries::new();
    let mut cur = head;
    while !cur.is_null() {
        let ifa = &*cur;
        cur = ifa.ifa_next;
        if ifa.ifa_addr.is_null() || ifa.ifa_name.is_null() {
            continue;
        }
        let name = CStr::from_ptr(ifa.ifa_name).to_string_lossy().into_owned();
        let addr = match (*ifa.ifa_addr).sa_family as libc::c_int {
            libc::AF_INET => {
                let sin = &*(ifa.ifa_addr as *const libc::sockaddr_in);
                IpAddr::V4(Ipv4Addr::from(u32::from_be(sin.sin_addr.s_addr)))
            }
            libc::AF_INET6 => {
                let sin6 = &*(ifa.ifa_addr as *const libc::sockaddr_in6);
                IpAddr::V6(Ipv6Addr::from(sin6.sin6_addr.s6_addr))
            }
            _ => continue,
        };
        out.insert((name, addr));
    }
    out
}

fn libc_entries() -> Entries {
    let mut head = std::ptr::null_mut();
    assert_eq!(
        unsafe { libc::getifaddrs(&mut head) },
        0,
        "{}",
        io::Error::last_os_error()
    );
    let out = unsafe { collect(head) };
    unsafe { libc::freeifaddrs(head) };
    out
}

/// Runs the shim with RTM_GETLINK failing with `getlink_errno` (0: not at all).
fn shim_entries(getlink_errno: libc::c_int) -> Result<Entries, i32> {
    unsafe { rd_ifaddrs_test_getlink_errno = getlink_errno };
    let mut head = std::ptr::null_mut();
    let rc = unsafe { rd_test_getifaddrs(&mut head) };
    let err = io::Error::last_os_error().raw_os_error().unwrap_or(0);
    unsafe { rd_ifaddrs_test_getlink_errno = 0 };
    if rc != 0 {
        assert!(head.is_null(), "a failed call must not hand out a list");
        return Err(err);
    }
    let out = unsafe { collect(head) };
    unsafe { rd_test_freeifaddrs(head) };
    Ok(out)
}

// One test function: the refusal switch is process-global.
#[test]
fn matches_libc_with_and_without_the_link_dump() {
    let want = libc_entries();
    assert!(
        !want.is_empty(),
        "the host has no IPv4/IPv6 addresses to compare"
    );

    assert_eq!(shim_entries(0), Ok(want.clone()), "link dump allowed");
    // Android 11+ refuses the link dump to apps targeting API 30+: the shim must
    // still report every address, named from the interface index.
    assert_eq!(
        shim_entries(libc::EACCES),
        Ok(want.clone()),
        "link dump refused (EACCES)"
    );
    assert_eq!(
        shim_entries(libc::EPERM),
        Ok(want),
        "link dump refused (EPERM)"
    );
    // Any other failure still fails the call rather than returning a partial list.
    assert_eq!(
        shim_entries(libc::EIO),
        Err(libc::EIO),
        "other link dump error"
    );
}
