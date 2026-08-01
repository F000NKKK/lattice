use net_lattice_core::Id;

use crate::address::{IpAddress, Network};

/// A single routing table entry.
///
/// `#[non_exhaustive]`: platforms carry different route fields (Linux adds
/// table/protocol/scope/type on top of destination/gateway/metric; Windows
/// and BSD expose a narrower set — see ARCHITECTURE.md's note on model
/// extensibility). Marking this non-exhaustive now means adding
/// platform-specific fields later is not a breaking change for consumers
/// who construct a `Route` via [`Route::new`] rather than a struct literal.
#[derive(Debug, Clone, PartialEq, Eq, Hash)]
#[non_exhaustive]
pub struct Route {
    pub id: RouteId,
    pub destination: Network,
    pub gateway: Option<IpAddress>,
    pub metric: Option<u32>,
    /// The outgoing interface, identified by its raw OS-level index.
    ///
    /// This is a raw `u32` rather than a typed `InterfaceId` because the
    /// `interface` domain module doesn't exist yet (it lands in Stage 0.4
    /// per ARCHITECTURE.md's Incremental Delivery Plan) — `Route` shouldn't
    /// block on it just to express what the kernel already gives us
    /// directly. Many routes are ambiguous or outright rejected by the
    /// kernel without an explicit output interface (on-link routes,
    /// multiple interfaces on the same subnet), so this isn't optional
    /// polish: without it, a meaningful fraction of real routes can't be
    /// added at all.
    pub interface_index: Option<u32>,
}

/// Identifies a [`Route`].
pub type RouteId = Id<Route>;

impl Route {
    pub fn new(id: RouteId, destination: Network) -> Self {
        Self {
            id,
            destination,
            gateway: None,
            metric: None,
            interface_index: None,
        }
    }

    pub fn with_gateway(mut self, gateway: IpAddress) -> Self {
        self.gateway = Some(gateway);
        self
    }

    pub fn with_metric(mut self, metric: u32) -> Self {
        self.metric = Some(metric);
        self
    }

    pub fn with_interface_index(mut self, interface_index: u32) -> Self {
        self.interface_index = Some(interface_index);
        self
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use net_lattice_ip::{
        Ipv4Address, Ipv4Network, Ipv4PrefixLength, Ipv6Address, Ipv6Network, Ipv6PrefixLength,
    };

    fn destination() -> Network {
        Network::from(Ipv4Network::new(
            Ipv4Address::new(10, 0, 0, 0),
            Ipv4PrefixLength::new(24).unwrap(),
        ))
    }

    #[test]
    fn new_route_has_no_gateway_or_metric() {
        let route = Route::new(RouteId::new(1), destination());
        assert!(route.gateway.is_none());
        assert!(route.metric.is_none());
    }

    #[test]
    fn builder_methods_set_optional_fields() {
        let gateway = IpAddress::from(Ipv4Address::new(10, 0, 0, 1));
        let route = Route::new(RouteId::new(1), destination())
            .with_gateway(gateway)
            .with_metric(100)
            .with_interface_index(2);
        assert_eq!(route.gateway, Some(gateway));
        assert_eq!(route.metric, Some(100));
        assert_eq!(route.interface_index, Some(2));
    }

    #[test]
    fn builder_methods_preserve_ipv6_route_fields() {
        let destination = Network::from(Ipv6Network::new(
            Ipv6Address::new([0x2001, 0xdb8, 0, 0x16, 0, 0, 0, 0]),
            Ipv6PrefixLength::new(64).expect("valid IPv6 prefix"),
        ));
        let gateway = IpAddress::from(Ipv6Address::new([0x2001, 0xdb8, 0, 0x16, 0, 0, 0, 1]));

        let route = Route::new(RouteId::new(16), destination)
            .with_gateway(gateway)
            .with_metric(42)
            .with_interface_index(7);

        assert_eq!(route.destination, destination);
        assert_eq!(route.gateway, Some(gateway));
        assert_eq!(route.metric, Some(42));
        assert_eq!(route.interface_index, Some(7));
    }
}
