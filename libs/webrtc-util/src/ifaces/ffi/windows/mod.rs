#![allow(unused, non_upper_case_globals)]

use winapi::shared::basetsd::{UINT32, UINT8, ULONG64};
use winapi::shared::guiddef::GUID;
use winapi::shared::minwindef::{BYTE, DWORD, PULONG, ULONG};
use winapi::shared::ws2def::SOCKET_ADDRESS;
use winapi::um::winnt::{PCHAR, PVOID, PWCHAR, WCHAR};

const MAX_ADAPTER_ADDRESS_LENGTH: usize = 8;
const ZONE_INDICES_LENGTH: usize = 16;
const MAX_DHCPV6_DUID_LENGTH: usize = 130;
const MAX_DNS_SUFFIX_STRING_LENGTH: usize = 256;

pub const IP_ADAPTER_IPV4_ENABLED: DWORD = 0x0080;
pub const IP_ADAPTER_IPV6_ENABLED: DWORD = 0x0100;

use std::net::{Ipv4Addr, Ipv6Addr, SocketAddr, SocketAddrV4, SocketAddrV6};
use std::{io, mem, ptr};

use winapi::shared::winerror::{
    ERROR_ADDRESS_NOT_ASSOCIATED, ERROR_BUFFER_OVERFLOW, ERROR_INVALID_PARAMETER,
    ERROR_NOT_ENOUGH_MEMORY, ERROR_NO_DATA, ERROR_SUCCESS,
};
use winapi::shared::ws2def::{AF_INET, AF_INET6, AF_UNSPEC, SOCKADDR_IN};
use winapi::shared::ws2ipdef::SOCKADDR_IN6;

const PREALLOC_ADAPTERS_LEN: usize = 15 * 1024;

use crate::ifaces::{Interface, Kind, NextHop};

#[link(name = "iphlpapi")]
extern "system" {
    pub fn GetAdaptersAddresses(
        family: ULONG,
        flags: ULONG,
        reserved: PVOID,
        addresses: *mut u8,
        size: PULONG,
    ) -> ULONG;
}

/// `IP_ADAPTER_ADDRESSES_LH` as the SDK lays it out: flat. Split into one struct per Windows
/// version, each part ended on its own alignment - the one ending at `oper_status` was padded to
/// eight bytes on x64 - and everything after it, `ipv6_if_index` first, was read from the wrong
/// offset.
#[repr(C)]
pub struct IpAdapterAddresses {
    pub length: ULONG,
    if_index: DWORD,
    pub next: *const IpAdapterAddresses,
    pub adapter_name: PCHAR,
    pub first_unicast_address: *const IpAdapterUnicastAddress,
    first_anycast_address: *const IpAdapterAnycastAddress,
    first_multicast_address: *const IpAdapterMulticastAddress,
    first_dns_server_address: *const IpAdapterDnsServerAddress,
    dns_suffix: PWCHAR,
    pub description: PWCHAR,
    friendly_name: PWCHAR,
    pub physical_address: [BYTE; MAX_ADAPTER_ADDRESS_LENGTH],
    pub physical_address_length: DWORD,
    pub flags: DWORD,
    mtu: DWORD,
    pub if_type: DWORD,
    oper_status: IfOperStatus,
    pub ipv6_if_index: DWORD,
    pub zone_indices: [DWORD; ZONE_INDICES_LENGTH],
    first_prefix: *const IpAdapterPrefix,
    transmit_link_speed: ULONG64,
    receive_link_speed: ULONG64,
    first_wins_server_address: *const IpAdapterWinsServerAddress,
    first_gateway_address: *const IpAdapterGatewayAddress,
    ipv4_metric: ULONG,
    ipv6_metric: ULONG,
    luid: IfLuid,
    dhcpv4_server: SOCKET_ADDRESS,
    compartment_id: UINT32,
    network_guid: GUID,
    connection_type: NetIfConnectionType,
    tunnel_type: TunnelType,
    dhcpv6_server: SOCKET_ADDRESS,
    dhcpv6_client_duid: [BYTE; MAX_DHCPV6_DUID_LENGTH],
    dhcpv6_client_duid_length: ULONG,
    dhcpv6_iaid: ULONG,
    first_dns_suffix: *const IpAdapterDnsSuffix,
}

// Each field follows the last as the SDK has it, on every target: no part ends on an alignment
// of its own. Checked as the crate builds, with what a const allows on Rust 1.75.
const _: () = {
    macro_rules! offset {
        ($field:ident) => {{
            let u = std::mem::MaybeUninit::<IpAdapterAddresses>::uninit();
            let p = u.as_ptr();
            // SAFETY: the address of a field inside the allocation, and nothing is read.
            unsafe {
                std::ptr::addr_of!((*p).$field)
                    .cast::<u8>()
                    .offset_from(p.cast::<u8>()) as usize
            }
        }};
    }
    assert!(offset!(next) == 8);
    assert!(offset!(ipv6_if_index) == offset!(oper_status) + 4);
    assert!(offset!(first_prefix) == offset!(zone_indices) + 4 * ZONE_INDICES_LENGTH);
    assert!(offset!(transmit_link_speed) == offset!(first_prefix) + std::mem::size_of::<usize>());
};

#[repr(C)]
pub struct IpAdapterUnicastAddress {
    pub length: ULONG,
    flags: DWORD,
    pub next: *const IpAdapterUnicastAddress,
    pub address: SOCKET_ADDRESS,
    prefix_origin: IpPrefixOrigin,
    suffix_origin: IpSuffixOrigin,
    pub dad_state: IpDadState,
    valid_lifetime: ULONG,
    preferred_lifetime: ULONG,
    lease_lifetime: ULONG,
    on_link_prefix_length: UINT8,
}

#[repr(C)]
pub struct IpAdapterAnycastAddress {
    length: ULONG,
    flags: DWORD,
    next: *const IpAdapterAnycastAddress,
    address: SOCKET_ADDRESS,
}

#[repr(C)]
pub struct IpAdapterMulticastAddress {
    length: ULONG,
    flags: DWORD,
    next: *const IpAdapterMulticastAddress,
    address: SOCKET_ADDRESS,
}

#[repr(C)]
pub struct IpAdapterDnsServerAddress {
    length: ULONG,
    reserved: DWORD,
    next: *const IpAdapterDnsServerAddress,
    address: SOCKET_ADDRESS,
}

#[repr(C)]
pub struct IpAdapterPrefix {
    length: ULONG,
    flags: DWORD,
    next: *const IpAdapterPrefix,
    address: SOCKET_ADDRESS,
    prefix_length: ULONG,
}

#[repr(C)]
pub struct IpAdapterWinsServerAddress {
    length: ULONG,
    reserved: DWORD,
    next: *const IpAdapterWinsServerAddress,
    address: SOCKET_ADDRESS,
}

#[repr(C)]
pub struct IpAdapterGatewayAddress {
    length: ULONG,
    reserved: DWORD,
    next: *const IpAdapterGatewayAddress,
    address: SOCKET_ADDRESS,
}

#[repr(C)]
pub struct IpAdapterDnsSuffix {
    next: *const IpAdapterDnsSuffix,
    string: [WCHAR; MAX_DNS_SUFFIX_STRING_LENGTH],
}

bitflags! {
    struct IfLuid: ULONG64 {
        const Reserved = 0x0000000000FFFFFF;
        const NetLuidIndex = 0x0000FFFFFF000000;
        const IfType = 0xFFFF00000000000;
    }
}

#[repr(C)]
pub enum IpPrefixOrigin {
    IpPrefixOriginOther = 0,
    IpPrefixOriginManual,
    IpPrefixOriginWellKnown,
    IpPrefixOriginDhcp,
    IpPrefixOriginRouterAdvertisement,
    IpPrefixOriginUnchanged = 16,
}

#[repr(C)]
pub enum IpSuffixOrigin {
    IpSuffixOriginOther = 0,
    IpSuffixOriginManual,
    IpSuffixOriginWellKnown,
    IpSuffixOriginDhcp,
    IpSuffixOriginLinkLayerAddress,
    IpSuffixOriginRandom,
    IpSuffixOriginUnchanged = 16,
}

#[derive(PartialEq, Eq)]
#[repr(C)]
pub enum IpDadState {
    IpDadStateInvalid = 0,
    IpDadStateTentative,
    IpDadStateDuplicate,
    IpDadStateDeprecated,
    IpDadStatePreferred,
}

#[repr(C)]
pub enum IfOperStatus {
    IfOperStatusUp = 1,
    IfOperStatusDown = 2,
    IfOperStatusTesting = 3,
    IfOperStatusUnknown = 4,
    IfOperStatusDormant = 5,
    IfOperStatusNotPresent = 6,
    IfOperStatusLowerLayerDown = 7,
}

#[repr(C)]
pub enum NetIfConnectionType {
    NetIfConnectionDedicated = 1,
    NetIfConnectionPassive = 2,
    NetIfConnectionDemand = 3,
    NetIfConnectionMaximum = 4,
}

#[repr(C)]
pub enum TunnelType {
    TunnelTypeNone = 0,
    TunnelTypeOther = 1,
    TunnelTypeDirect = 2,
    TunnelType6To4 = 11,
    TunnelTypeIsatap = 13,
    TunnelTypeTeredo = 14,
    TunnelTypeIpHttps = 15,
}

unsafe fn v4_socket_from_adapter(unicast_addr: &IpAdapterUnicastAddress) -> SocketAddrV4 {
    let socket_addr = &unicast_addr.address;

    let in_addr: SOCKADDR_IN = mem::transmute(*socket_addr.lpSockaddr);
    let sin_addr = in_addr.sin_addr.S_un;

    let v4_addr = Ipv4Addr::new(
        *sin_addr.S_addr() as u8,
        (*sin_addr.S_addr() >> 8) as u8,
        (*sin_addr.S_addr() >> 16) as u8,
        (*sin_addr.S_addr() >> 24) as u8,
    );

    SocketAddrV4::new(v4_addr, 0)
}

unsafe fn v6_socket_from_adapter(unicast_addr: &IpAdapterUnicastAddress) -> SocketAddrV6 {
    let socket_addr = &unicast_addr.address;

    let sock_addr6: *const SOCKADDR_IN6 = socket_addr.lpSockaddr as *const SOCKADDR_IN6;
    let in6_addr: SOCKADDR_IN6 = *sock_addr6;

    // `Word()` reinterprets the on-wire (network order) bytes as `[u16; 8]`, so on a
    // little-endian host every group is read byte-swapped and `Ipv6Addr::from` then takes
    // those swapped values as address segments. `Byte()` keeps the on-wire order, which is
    // what `Ipv6Addr::from::<[u8; 16]>` expects - the same reasoning the IPv4 path above
    // already applies to `S_addr`.
    let v6_addr = Ipv6Addr::from(*in6_addr.sin6_addr.u.Byte());

    SocketAddrV6::new(
        v6_addr,
        0,
        in6_addr.sin6_flowinfo,
        *in6_addr.u.sin6_scope_id(),
    )
}

unsafe fn local_ifaces_with_buffer(buffer: &mut Vec<u8>) -> io::Result<()> {
    let mut length = buffer.capacity() as u32;

    let ret_code = GetAdaptersAddresses(
        AF_UNSPEC as u32,
        0,
        ptr::null_mut(),
        buffer.as_mut_ptr(),
        &mut length,
    );
    match ret_code {
        ERROR_SUCCESS => Ok(()),
        ERROR_ADDRESS_NOT_ASSOCIATED => Err(io::Error::new(
            io::ErrorKind::AddrNotAvailable,
            "An address has not yet been associated with the network endpoint.",
        )),
        ERROR_BUFFER_OVERFLOW => {
            buffer.reserve_exact(length as usize);

            local_ifaces_with_buffer(buffer)
        }
        ERROR_INVALID_PARAMETER => Err(io::Error::new(
            io::ErrorKind::InvalidInput,
            "One of the parameters is invalid.",
        )),
        ERROR_NOT_ENOUGH_MEMORY => Err(io::Error::new(
            io::ErrorKind::Other,
            "Insufficient memory resources are available to complete the operation.",
        )),
        ERROR_NO_DATA => Err(io::Error::new(
            io::ErrorKind::AddrNotAvailable,
            "No addresses were found for the requested parameters.",
        )),
        _ => Err(io::Error::new(
            io::ErrorKind::Other,
            "Some Other Error Occurred.",
        )),
    }
}

unsafe fn map_adapter_addresses(mut adapter_addr: *const IpAdapterAddresses) -> Vec<Interface> {
    let mut adapter_addresses = Vec::new();

    while !adapter_addr.is_null() {
        let curr_adapter_addr = &*adapter_addr;
        let name = adapter_name(curr_adapter_addr);

        let mut unicast_addr = curr_adapter_addr.first_unicast_address;
        while !unicast_addr.is_null() {
            let curr_unicast_addr = &*unicast_addr;

            // For some reason, some IpDadState::IpDadStateDeprecated addresses are return
            // These contain BOGUS interface indices and will cause problesm if used
            if curr_unicast_addr.dad_state != IpDadState::IpDadStateDeprecated {
                if is_ipv4_enabled(curr_unicast_addr) {
                    adapter_addresses.push(Interface {
                        name: name.clone(),
                        kind: Kind::Ipv4,
                        addr: Some(SocketAddr::V4(v4_socket_from_adapter(curr_unicast_addr))),
                        mask: Some(v4_mask(curr_unicast_addr.on_link_prefix_length)),
                        hop: None,
                    });
                } else if is_ipv6_enabled(curr_unicast_addr) {
                    let mut v6_sock = v6_socket_from_adapter(curr_unicast_addr);
                    // Make sure the scope id is set for ALL interfaces, not just link-local
                    v6_sock.set_scope_id(curr_adapter_addr.ipv6_if_index);
                    adapter_addresses.push(Interface {
                        name: name.clone(),
                        kind: Kind::Ipv6,
                        addr: Some(SocketAddr::V6(v6_sock)),
                        mask: Some(v6_mask(curr_unicast_addr.on_link_prefix_length)),
                        hop: None,
                    });
                }
            }

            unicast_addr = curr_unicast_addr.next;
        }

        adapter_addr = curr_adapter_addr.next;
    }

    adapter_addresses
}

/// Query the local system for all interface addresses.
pub fn ifaces() -> Result<Vec<Interface>, ::std::io::Error> {
    let mut adapters_list = Vec::with_capacity(PREALLOC_ADAPTERS_LEN);
    unsafe {
        local_ifaces_with_buffer(&mut adapters_list)?;

        Ok(map_adapter_addresses(
            adapters_list.as_ptr() as *const IpAdapterAddresses
        ))
    }
}

/// The adapter's own name, so that `Net` keeps one adapter's addresses apart from another's:
/// ICE takes one IPv6 address per interface and prefix, and every adapter named "" folded
/// Ethernet and Wi-Fi on the same LAN into a single choice.
unsafe fn adapter_name(adapter: &IpAdapterAddresses) -> String {
    let name = adapter.adapter_name;
    if name.is_null() {
        return adapter.ipv6_if_index.to_string();
    }
    std::ffi::CStr::from_ptr(name)
        .to_string_lossy()
        .into_owned()
}

/// The on-link prefix as a netmask, the form `Interface::convert` takes a prefix in.
fn v4_mask(prefix_len: u8) -> SocketAddr {
    let bits = match prefix_len.min(32) {
        0 => 0,
        n => u32::MAX << (32 - n as u32),
    };
    SocketAddr::V4(SocketAddrV4::new(Ipv4Addr::from(bits), 0))
}

fn v6_mask(prefix_len: u8) -> SocketAddr {
    let bits = match prefix_len.min(128) {
        0 => 0,
        n => u128::MAX << (128 - n as u32),
    };
    SocketAddr::V6(SocketAddrV6::new(Ipv6Addr::from(bits), 0, 0, 0))
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::vnet::interface::Interface as VnetInterface;
    use std::net::IpAddr;

    fn prefix_len(addr: &str, mask: SocketAddr) -> u8 {
        let addr: SocketAddr = addr.parse().unwrap();
        VnetInterface::convert(addr, Some(mask))
            .unwrap()
            .prefix_len()
    }

    // The mask is the on-link prefix length written out, and `Interface::convert` reads the
    // length back off it - both ends of the range included, where the shift would overflow.
    #[test]
    fn masks_round_trip_the_prefix_length() {
        assert_eq!(v6_mask(0).ip(), "::".parse::<IpAddr>().unwrap());
        assert_eq!(
            v6_mask(64).ip(),
            "ffff:ffff:ffff:ffff::".parse::<IpAddr>().unwrap()
        );
        assert_eq!(
            v6_mask(127).ip(),
            "ffff:ffff:ffff:ffff:ffff:ffff:ffff:fffe"
                .parse::<IpAddr>()
                .unwrap()
        );
        assert_eq!(v6_mask(128).ip(), Ipv6Addr::from(u128::MAX));
        assert_eq!(v4_mask(0).ip(), Ipv4Addr::new(0, 0, 0, 0));
        assert_eq!(v4_mask(24).ip(), Ipv4Addr::new(255, 255, 255, 0));
        assert_eq!(v4_mask(32).ip(), Ipv4Addr::new(255, 255, 255, 255));
        for len in [0u8, 1, 48, 64, 127, 128] {
            assert_eq!(prefix_len("[2001:db8::1]:0", v6_mask(len)), len);
        }
        for len in [0u8, 8, 24, 32] {
            assert_eq!(prefix_len("192.0.2.1:0", v4_mask(len)), len);
        }
    }
}

unsafe fn is_ipv4_enabled(unicast_addr: &IpAdapterUnicastAddress) -> bool {
    if unicast_addr.length != 0 {
        let socket_addr = &unicast_addr.address;
        let sa_family = (*socket_addr.lpSockaddr).sa_family;

        sa_family == AF_INET as u16
    } else {
        false
    }
}

unsafe fn is_ipv6_enabled(unicast_addr: &IpAdapterUnicastAddress) -> bool {
    if unicast_addr.length != 0 {
        let socket_addr = &unicast_addr.address;
        let sa_family = (*socket_addr.lpSockaddr).sa_family;

        sa_family == AF_INET6 as u16
    } else {
        false
    }
}
