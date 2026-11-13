//! Name resolution and address classification for the `http` import.
//!
//! The host resolves a name once per request (and per redirect hop), checks
//! every address of the answer, and hands only those addresses to the
//! connector, so a DNS answer cannot change between the check and the
//! connection (DNS rebinding). A [`Resolve`] implementation can be injected
//! ([`crate::HostOptions::resolver`]); tests use one to simulate hostile DNS.

use std::fmt;
use std::io;
use std::net::{IpAddr, Ipv4Addr, Ipv6Addr, ToSocketAddrs};

/// Resolves host names. Implementations must be cheap to call concurrently.
pub trait Resolve: Send + Sync + fmt::Debug {
    /// The addresses of `host` (never an IP literal; the host handles those).
    fn lookup(&self, host: &str, port: u16) -> io::Result<Vec<IpAddr>>;
}

/// The operating system's resolver (`getaddrinfo`).
#[derive(Debug, Default, Clone, Copy)]
pub struct SystemResolver;

impl Resolve for SystemResolver {
    fn lookup(&self, host: &str, port: u16) -> io::Result<Vec<IpAddr>> {
        Ok((host, port).to_socket_addrs()?.map(|addr| addr.ip()).collect())
    }
}

/// What an address may be used for.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum AddressClass {
    /// A globally routable address: any allowlisted origin may use it.
    /// (Documentation ranges count as public: they are not routable, so
    /// allowing them grants nothing.)
    Public,
    /// Private, loopback, link-local, unique-local, shared (CGNAT) and
    /// similar: only the person's own configured server may use them.
    Local,
    /// Unspecified, multicast, broadcast or reserved: never used.
    Forbidden,
}

/// Classifies an address. IPv4 addresses embedded in IPv6 (mapped,
/// compatible, NAT64, 6to4) are classified by the embedded address.
pub fn classify(ip: IpAddr) -> AddressClass {
    match ip {
        IpAddr::V4(v4) => classify_v4(v4),
        IpAddr::V6(v6) => classify_v6(v6),
    }
}

fn classify_v4(ip: Ipv4Addr) -> AddressClass {
    let [a, b, c, _] = ip.octets();
    if ip.is_unspecified() || ip.is_broadcast() || ip.is_multicast() || a >= 240 {
        return AddressClass::Forbidden;
    }
    let local = a == 0 // "this network"
        || a == 10
        || a == 127
        || (a == 100 && (64..128).contains(&b)) // shared address space (CGNAT)
        || (a == 169 && b == 254) // link-local, cloud metadata
        || (a == 172 && (16..32).contains(&b))
        || (a == 192 && b == 168)
        || (a == 192 && b == 0 && c == 0) // IETF protocol assignments
        || (a == 198 && (18..20).contains(&b)); // benchmarking
    if local {
        AddressClass::Local
    } else {
        AddressClass::Public
    }
}

fn classify_v6(ip: Ipv6Addr) -> AddressClass {
    if ip.is_unspecified() || ip.is_multicast() {
        return AddressClass::Forbidden;
    }
    if ip.is_loopback() {
        return AddressClass::Local;
    }
    let segments = ip.segments();
    // IPv4-mapped (::ffff:a.b.c.d) and the deprecated IPv4-compatible (::a.b.c.d).
    if segments[..5] == [0; 5] && (segments[5] == 0xffff || segments[5] == 0) {
        let [.., a, b, c, d] = ip.octets();
        return classify_v4(Ipv4Addr::new(a, b, c, d));
    }
    // NAT64 well-known prefix 64:ff9b::/96 embeds an IPv4 address.
    if segments[..6] == [0x64, 0xff9b, 0, 0, 0, 0] {
        let [.., a, b, c, d] = ip.octets();
        return classify_v4(Ipv4Addr::new(a, b, c, d));
    }
    // 6to4 (2002::/16) embeds an IPv4 address in bits 16–48.
    if segments[0] == 0x2002 {
        let o = ip.octets();
        return classify_v4(Ipv4Addr::new(o[2], o[3], o[4], o[5]));
    }
    let local = (segments[0] & 0xfe00) == 0xfc00 // unique local fc00::/7
        || (segments[0] & 0xffc0) == 0xfe80 // link-local fe80::/10
        || (segments[0] & 0xffc0) == 0xfec0 // site-local (deprecated)
        || segments[..3] == [0x64, 0xff9b, 1] // local-use NAT64 64:ff9b:1::/48
        || segments[..2] == [0x2001, 0] // Teredo tunnels
        || segments[..4] == [0x100, 0, 0, 0]; // discard-only 100::/64
    if local {
        AddressClass::Local
    } else {
        AddressClass::Public
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn class(text: &str) -> AddressClass {
        classify(text.parse().unwrap())
    }

    #[test]
    fn classifies_ipv4() {
        for public in [
            "8.8.8.8",
            "1.1.1.1",
            "203.0.113.7",
            "93.184.216.34",
            "100.128.0.1",
            "172.32.0.1",
        ] {
            assert_eq!(class(public), AddressClass::Public, "{public}");
        }
        for local in [
            "10.0.0.1",
            "127.0.0.1",
            "127.1.2.3",
            "169.254.169.254",
            "172.16.0.1",
            "172.31.255.255",
            "192.168.1.1",
            "100.64.0.1",
            "0.1.2.3",
            "192.0.0.8",
            "198.18.0.1",
        ] {
            assert_eq!(class(local), AddressClass::Local, "{local}");
        }
        for forbidden in ["0.0.0.0", "255.255.255.255", "224.0.0.1", "240.0.0.1"] {
            assert_eq!(class(forbidden), AddressClass::Forbidden, "{forbidden}");
        }
    }

    #[test]
    fn classifies_ipv6_and_embedded_ipv4() {
        for public in [
            "2606:4700::1111",
            "2001:4860:4860::8888",
            "64:ff9b::808:808",
            "2002:808:808::1",
        ] {
            assert_eq!(class(public), AddressClass::Public, "{public}");
        }
        for local in [
            "::1",
            "fd00::1",
            "fe80::1",
            "::ffff:127.0.0.1",
            "::ffff:10.1.2.3",
            "::127.0.0.1",
            "64:ff9b::7f00:1",
            "2002:7f00:1::1",
            "2001:0:4136:e378::1",
            "64:ff9b:1::1",
        ] {
            assert_eq!(class(local), AddressClass::Local, "{local}");
        }
        for forbidden in ["::", "ff02::1", "::ffff:0.0.0.0"] {
            assert_eq!(class(forbidden), AddressClass::Forbidden, "{forbidden}");
        }
    }
}
