use net_lattice_core::Result;

/// Reads the system's current DNS resolver configuration.
///
/// Generic over an associated `DnsConfig` type rather than naming
/// `net_lattice_model::dns::DnsConfig` directly — `net-lattice-platform`
/// does not depend on `net-lattice-model` (see ARCHITECTURE.md). The facade
/// crate (`net-lattice`) is what constrains `DnsConfig` to the concrete
/// model type.
///
/// Read-only for now, like [`InterfaceProvider`](crate::InterfaceProvider):
/// writing DNS configuration is a separate concern (which resolver backend
/// owns `/etc/resolv.conf` vs. NetworkManager/systemd-resolved/Windows
/// per-adapter settings) deferred to a later stage.
pub trait DnsProvider {
    type DnsConfig;

    fn dns_config(&self) -> Result<Self::DnsConfig>;
}
