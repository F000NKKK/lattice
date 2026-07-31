use net_lattice_core::Result;

use crate::DnsProvider;

/// Replaces the resolver configuration managed by a backend.
///
/// This is an official third-party backend extension contract. A successful
/// call must return the resolver view subsequently observed through
/// [`DnsProvider::dns_config`]. Backends must preserve input ordering where
/// their operating-system mechanism represents it, and must report platform,
/// permission, or manager-ownership failures through [`Result`] rather than
/// silently accepting a configuration they cannot apply.
///
/// The concrete system mechanism differs by platform. In particular, a
/// resolver manager may later replace configuration written by this operation;
/// callers that require persistence should re-read the observed state and use
/// a platform-specific manager integration where appropriate.
///
/// A failed replacement may have partially changed resolver state. This is
/// especially relevant when a platform applies global and per-interface
/// settings through separate native calls. After any error, callers must use
/// [`DnsProvider::dns_config`] to obtain the authoritative observed state;
/// rollback and atomicity are transaction-layer concerns, not guarantees of
/// this primitive.
pub trait DnsMutator: DnsProvider {
    /// The desired resolver configuration accepted by this backend.
    type NewDnsConfig;

    /// Replaces the resolver configuration and returns its resulting observed
    /// representation.
    fn set_dns_config(&self, config: Self::NewDnsConfig) -> Result<Self::DnsConfig>;
}
