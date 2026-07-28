use lattice_core::Id;

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
}

#[cfg(test)]
mod tests {
    use super::*;
    use lattice_ip::{Ipv4Address, Ipv4Network, Ipv4PrefixLength};

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
            .with_metric(100);
        assert_eq!(route.gateway, Some(gateway));
        assert_eq!(route.metric, Some(100));
    }
}
