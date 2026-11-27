//! Tests for `android_ifaddrs.c`, the getifaddrs()/freeifaddrs() the Android
//! build links in place of bionic's. build.rs compiles the same file for Linux
//! debug builds with `RD_IFADDRS_TEST` and renamed functions, so it runs here
//! against the host kernel and is compared with the C library's getifaddrs().
//!
//! The test hook fails the RTM_GETLINK request before it is sent. On Android
//! the refusal arrives from sendto() or as an NLMSG_ERROR reply; both reach the
//! same errno check in the shim, but only the hook's path is exercised here.

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

/// One IPv4/IPv6 entry: link name, address, netmask, and the low 16 interface
/// flags (all the fallback path can read, through SIOCGIFFLAGS).
#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord)]
struct Entry {
    name: String,
    addr: IpAddr,
    netmask: Option<IpAddr>,
    flags: u32,
}

type Entries = BTreeSet<Entry>;

unsafe fn ip_of(sa: *const libc::sockaddr) -> Option<IpAddr> {
    if sa.is_null() {
        return None;
    }
    match (*sa).sa_family as libc::c_int {
        libc::AF_INET => {
            let sin = &*(sa as *const libc::sockaddr_in);
            Some(IpAddr::V4(Ipv4Addr::from(u32::from_be(
                sin.sin_addr.s_addr,
            ))))
        }
        libc::AF_INET6 => {
            let sin6 = &*(sa as *const libc::sockaddr_in6);
            Some(IpAddr::V6(Ipv6Addr::from(sin6.sin6_addr.s6_addr)))
        }
        _ => None,
    }
}

unsafe fn collect(head: *const libc::ifaddrs) -> Entries {
    let mut out = Entries::new();
    let mut cur = head;
    while !cur.is_null() {
        let ifa = &*cur;
        cur = ifa.ifa_next;
        if ifa.ifa_name.is_null() {
            continue;
        }
        let Some(addr) = ip_of(ifa.ifa_addr) else {
            continue;
        };
        // glibc names a labelled IPv4 alias "eth0:1" (IFA_LABEL); the shim
        // reports the link name, which is the part before the colon.
        let name = CStr::from_ptr(ifa.ifa_name).to_string_lossy();
        let name = name.split(':').next().unwrap_or_default().to_owned();
        out.insert(Entry {
            name,
            addr,
            netmask: ip_of(ifa.ifa_netmask),
            flags: ifa.ifa_flags & 0xffff,
        });
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

/// The shim's view in each mode, taken while the C library's view stays the same
/// before and after, so an address that comes or goes mid-test is retried
/// rather than reported as a mismatch.
fn stable_run() -> Option<(Entries, [Result<Entries, i32>; 4])> {
    for _ in 0..5 {
        let before = libc_entries();
        let runs = [
            shim_entries(0),
            shim_entries(libc::EACCES),
            shim_entries(libc::EPERM),
            shim_entries(libc::EIO),
        ];
        if libc_entries() == before {
            return Some((before, runs));
        }
    }
    None
}

// One test function: the refusal switch is process-global.
#[test]
fn matches_libc_with_and_without_the_link_dump() {
    let Some((want, [allowed, eacces, eperm, eio])) = stable_run() else {
        eprintln!("skipped: the host's addresses kept changing; nothing stable to compare");
        return;
    };
    if want.is_empty() {
        // e.g. a network-less build sandbox, where even lo has no address.
        eprintln!("skipped: the host has no IPv4/IPv6 addresses to compare");
        return;
    }
    assert_eq!(allowed, Ok(want.clone()), "link dump allowed");
    // Android 11+ refuses the link dump to apps targeting API 30+: the shim must
    // still report every address, named and flagged from the interface index.
    assert_eq!(eacces, Ok(want.clone()), "link dump refused (EACCES)");
    assert_eq!(eperm, Ok(want), "link dump refused (EPERM)");
    // Any other failure still fails the call rather than returning a partial list.
    assert_eq!(eio, Err(libc::EIO), "other link dump error");
}
