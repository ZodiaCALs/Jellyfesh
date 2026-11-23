//! Which server addresses are on the local network, by Android's definition
//! (developer.android.com/privacy-and-security/local-network-definition):
//! apps targeting Android SDK 37 need `ACCESS_LOCAL_NETWORK` to reach them,
//! so the apps ask for it before a source uses such a server.

use std::net::{IpAddr, Ipv4Addr, Ipv6Addr};

/// Whether `address` (a URL such as `http://192.168.1.20:4533`, or a bare
/// host with an optional port) names a server on the local network:
///
/// * IPv4 `10/8`, `172.16/12`, `192.168/16`, `100.64/10` (shared address
///   space, CGNAT) and link-local `169.254/16`;
/// * IPv6 link-local `fe80::/10` and unique local `fc00::/7` (and IPv4
///   addresses mapped into IPv6);
/// * multicast `224/4` and `ff00::/8`, and the broadcast `255.255.255.255`;
/// * multicast DNS names ending in `.local`.
///
/// Loopback is not the local network. Other host names cannot be judged
/// without resolving them, which this does not do: they count as not local
/// (a name a home router resolves to a private address is such a case; the
/// apps then ask when Android refuses the connection).
pub fn address_is_local(address: &str) -> bool {
    let Some(host) = host_of(address) else {
        return false;
    };
    match host.parse::<IpAddr>() {
        Ok(IpAddr::V4(ip)) => local_v4(ip),
        Ok(IpAddr::V6(ip)) => local_v6(ip),
        Err(_) => {
            let name = host.trim_end_matches('.').to_ascii_lowercase();
            name.ends_with(".local") && name.len() > ".local".len()
        }
    }
}

/// The host of a URL or of `host[:port]`, without brackets around an IPv6
/// literal.
fn host_of(address: &str) -> Option<String> {
    let address = address.trim();
    if address.is_empty() {
        return None;
    }
    let mut address = address;
    if let Some((_, rest)) = address.split_once("://") {
        if let Ok(url) = url::Url::parse(address) {
            return match url.host()? {
                url::Host::Domain(name) => Some(name.to_owned()),
                url::Host::Ipv4(ip) => Some(ip.to_string()),
                url::Host::Ipv6(ip) => Some(ip.to_string()),
            };
        }
        // Not a URL the parser takes (an IPv6 zone, "%25eth0"): the part after the scheme.
        address = rest.rsplit('@').next().unwrap_or(rest);
    }
    // A bare host: "[fe80::1]:8096", "192.168.1.2:4533", "nas.local", "fe80::1".
    let without_path = address.split(['/', '?', '#']).next().unwrap_or_default();
    if let Some(rest) = without_path.strip_prefix('[') {
        // Without a zone ("fe80::1%25eth0", "fe80::1%eth0").
        return rest
            .split(']')
            .next()
            .and_then(|ip| ip.split('%').next())
            .map(str::to_owned);
    }
    if let Some((ip, _zone)) = without_path.split_once('%')
        && ip.parse::<Ipv6Addr>().is_ok()
    {
        return Some(ip.to_owned());
    }
    if without_path.parse::<Ipv6Addr>().is_ok() {
        return Some(without_path.to_owned());
    }
    let host = match without_path.rsplit_once(':') {
        Some((host, port)) if port.bytes().all(|b| b.is_ascii_digit()) => host,
        _ => without_path,
    };
    (!host.is_empty()).then(|| host.to_owned())
}

fn local_v4(ip: Ipv4Addr) -> bool {
    let [a, b, ..] = ip.octets();
    ip.is_private()
        || ip.is_link_local()
        // 100.64.0.0/10, the shared address space of carrier-grade NAT.
        || (a == 100 && (64..128).contains(&b))
        || ip.is_multicast()
        || ip.is_broadcast()
}

fn local_v6(ip: Ipv6Addr) -> bool {
    if let Some(v4) = ip.to_ipv4_mapped() {
        return local_v4(v4);
    }
    let first = ip.segments()[0];
    // fe80::/10 link-local, fc00::/7 unique local, ff00::/8 multicast.
    (first & 0xffc0) == 0xfe80 || (first & 0xfe00) == 0xfc00 || ip.is_multicast()
}

#[cfg(test)]
mod tests {
    use super::*;

    /// The table: Android's local network definition, row by row.
    #[test]
    fn addresses_follow_androids_local_network_definition() {
        let table: &[(&str, bool)] = &[
            // Private IPv4 ranges.
            ("http://10.0.0.5:8096", true),
            ("10.255.255.255", true),
            ("http://172.16.0.1", true),
            ("172.31.255.254:4533", true),
            ("172.32.0.1", false),
            ("172.15.255.255", false),
            ("https://192.168.1.20:4533/navidrome", true),
            ("192.169.0.1", false),
            // Shared address space (CGNAT).
            ("http://100.64.0.1", true),
            ("100.127.255.255", true),
            ("100.63.255.255", false),
            ("100.128.0.0", false),
            // Link-local.
            ("169.254.10.20", true),
            ("http://[fe80::1%25eth0]:8096", true),
            ("[fe80::abcd]:8096", true),
            ("fe80::1", true),
            ("febf::1", true),
            ("fec0::1", false),
            // Unique local IPv6.
            ("http://[fd12:3456:789a::1]:4533", true),
            ("[fc00::1]", true),
            ("fe00::1", false),
            // Multicast and broadcast.
            ("224.0.0.251", true),
            ("239.255.255.250", true),
            ("255.255.255.255", true),
            ("[ff02::fb]", true),
            // IPv4 mapped into IPv6.
            ("[::ffff:192.168.0.10]", true),
            ("[::ffff:8.8.8.8]", false),
            // mDNS names.
            ("http://jellyfin.local:8096", true),
            ("NAS.LOCAL.", true),
            ("local", false),
            (".local", false),
            // Loopback is not the local network.
            ("http://127.0.0.1:4533", false),
            ("localhost", false),
            ("[::1]", false),
            // Public addresses and names that would need a lookup.
            ("https://demo.navidrome.org", false),
            ("8.8.8.8", false),
            ("[2001:db8::1]", false),
            ("nas.home.arpa", false),
            ("", false),
            ("   ", false),
        ];
        for (address, local) in table {
            assert_eq!(address_is_local(address), *local, "{address:?}");
        }
    }
}
