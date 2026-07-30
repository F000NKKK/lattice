use crate::IpAddress;

/// The system's current DNS resolver configuration.
///
/// `#[non_exhaustive]`: platforms surface DNS configuration differently
/// (a flat `/etc/resolv.conf` on Linux/BSD/macOS vs. per-adapter DNS server
/// lists plus a global suffix on Windows) — this carries the subset that
/// maps cleanly across all of them. Marking it non-exhaustive now means
/// adding platform-specific fields later (e.g. per-interface resolvers) is
/// not a breaking change for consumers who construct a `DnsConfig` via
/// [`DnsConfig::new`] rather than a struct literal — see ARCHITECTURE.md's
/// note on model extensibility.
#[derive(Debug, Clone, Default, PartialEq, Eq, Hash)]
#[non_exhaustive]
pub struct DnsConfig {
    /// Nameserver addresses, in resolution order.
    pub nameservers: Vec<IpAddress>,
    /// Search-list domains appended to unqualified lookups, in order.
    pub search_domains: Vec<String>,
}

impl DnsConfig {
    pub fn new() -> Self {
        Self::default()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use net_lattice_ip::Ipv4Address;

    #[test]
    fn new_config_has_no_entries() {
        let config = DnsConfig::new();
        assert!(config.nameservers.is_empty());
        assert!(config.search_domains.is_empty());
    }

    #[test]
    fn config_carries_nameservers_and_search_domains() {
        let config = DnsConfig {
            nameservers: vec![IpAddress::V4(Ipv4Address::new(1, 1, 1, 1))],
            search_domains: vec!["example.com".to_string()],
        };
        assert_eq!(config.nameservers.len(), 1);
        assert_eq!(config.search_domains, vec!["example.com".to_string()]);
    }
}
