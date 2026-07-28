//! Linux backend for Net Lattice: implements `net-lattice-platform`'s provider
//! traits via Netlink.
//!
//! Only ever compiled for `target_os = "linux"` — its dependencies
//! (`rtnetlink`, Linux-only) are gated the same way in `Cargo.toml`. See
//! ARCHITECTURE.md for how this crate binds `net-lattice-platform`'s generic
//! `RouteProvider::Route` associated type to the concrete
//! `net_lattice_model::route::Route`.

#![cfg(target_os = "linux")]

use std::hash::{Hash, Hasher};
use std::net::IpAddr;

use futures::TryStreamExt;
use net_lattice_core::{Error, PlatformErrorCode, Result};
use net_lattice_model::route::{Route, RouteId};
use net_lattice_model::{IpAddress, Network};
use net_lattice_platform::RouteProvider;
use rtnetlink::packet_route::route::{RouteAddress, RouteAttribute, RouteMessage};
use rtnetlink::{Handle, RouteMessageBuilder};

/// The Linux Netlink-backed implementation of Net Lattice's provider traits.
pub struct LinuxBackend {
    runtime: tokio::runtime::Runtime,
    handle: Handle,
}

impl LinuxBackend {
    pub fn new() -> Result<Self> {
        let runtime =
            tokio::runtime::Runtime::new().map_err(|err| Error::Platform(io_error_code(&err)))?;
        let (connection, handle, _) =
            rtnetlink::new_connection().map_err(|err| Error::Platform(io_error_code(&err)))?;
        runtime.spawn(connection);
        Ok(Self { runtime, handle })
    }
}

fn io_error_code(err: &std::io::Error) -> PlatformErrorCode {
    PlatformErrorCode::Linux(err.raw_os_error().unwrap_or(0))
}

fn rtnetlink_error_code(err: &rtnetlink::Error) -> PlatformErrorCode {
    match err {
        rtnetlink::Error::NetlinkError(message) => {
            PlatformErrorCode::Linux(message.code.map(i32::from).unwrap_or(0))
        }
        _ => PlatformErrorCode::Linux(0),
    }
}

/// Placeholder identity scheme: a route has no kernel-assigned numeric ID,
/// so this hashes its defining fields. See ARCHITECTURE.md's open Object
/// Identity question — this is not a long-term answer, only enough to give
/// `Stage 0.1` a `RouteId` to work with.
fn synthesize_route_id(message: &RouteMessage) -> RouteId {
    let mut hasher = std::collections::hash_map::DefaultHasher::new();
    message.header.destination_prefix_length.hash(&mut hasher);
    for attribute in &message.attributes {
        if let RouteAttribute::Destination(addr) = attribute {
            route_address_to_ip(addr).hash(&mut hasher);
        }
    }
    RouteId::new(hasher.finish())
}

fn route_address_to_ip(address: &RouteAddress) -> Option<IpAddr> {
    match address {
        RouteAddress::Inet(addr) => Some(IpAddr::V4(*addr)),
        RouteAddress::Inet6(addr) => Some(IpAddr::V6(*addr)),
        _ => None,
    }
}

fn std_ip_to_ip_address(addr: IpAddr) -> IpAddress {
    match addr {
        IpAddr::V4(addr) => IpAddress::from(net_lattice_ip::Ipv4Address::from(addr)),
        IpAddr::V6(addr) => IpAddress::from(net_lattice_ip::Ipv6Address::from(addr)),
    }
}

fn message_to_route(message: &RouteMessage) -> Option<Route> {
    let mut destination_addr = None;
    let mut gateway = None;
    let mut metric = None;

    for attribute in &message.attributes {
        match attribute {
            RouteAttribute::Destination(addr) => {
                destination_addr = route_address_to_ip(addr);
            }
            RouteAttribute::Gateway(addr) => {
                gateway = route_address_to_ip(addr).map(std_ip_to_ip_address);
            }
            RouteAttribute::Priority(priority) => {
                metric = Some(*priority);
            }
            _ => {}
        }
    }

    let destination_addr = destination_addr?;
    let prefix_len = message.header.destination_prefix_length;
    let destination = match destination_addr {
        IpAddr::V4(addr) => {
            let prefix = net_lattice_ip::Ipv4PrefixLength::new(prefix_len)?;
            Network::from(net_lattice_ip::Ipv4Network::new(addr.into(), prefix))
        }
        IpAddr::V6(addr) => {
            let prefix = net_lattice_ip::Ipv6PrefixLength::new(prefix_len)?;
            Network::from(net_lattice_ip::Ipv6Network::new(addr.into(), prefix))
        }
    };

    let mut route = Route::new(synthesize_route_id(message), destination);
    if let Some(gateway) = gateway {
        route = route.with_gateway(gateway);
    }
    if let Some(metric) = metric {
        route = route.with_metric(metric);
    }
    Some(route)
}

fn ip_address_to_std(address: IpAddress) -> IpAddr {
    match address {
        IpAddress::V4(addr) => IpAddr::V4(addr.into()),
        IpAddress::V6(addr) => IpAddr::V6(addr.into()),
    }
}

fn network_to_std(network: Network) -> (IpAddr, u8) {
    match network {
        Network::V4(net) => (IpAddr::V4(net.address().into()), net.prefix().value()),
        Network::V6(net) => (IpAddr::V6(net.address().into()), net.prefix().value()),
    }
}

impl RouteProvider for LinuxBackend {
    type Route = Route;

    fn routes(&self) -> Result<Vec<Self::Route>> {
        self.runtime.block_on(async {
            let route_handle = self.handle.route();

            let mut v4 = route_handle
                .get(RouteMessageBuilder::<std::net::Ipv4Addr>::new().build())
                .execute();
            let mut v6 = route_handle
                .get(RouteMessageBuilder::<std::net::Ipv6Addr>::new().build())
                .execute();

            let mut routes = Vec::new();
            while let Some(message) = v4
                .try_next()
                .await
                .map_err(|err| Error::Platform(rtnetlink_error_code(&err)))?
            {
                routes.extend(message_to_route(&message));
            }
            while let Some(message) = v6
                .try_next()
                .await
                .map_err(|err| Error::Platform(rtnetlink_error_code(&err)))?
            {
                routes.extend(message_to_route(&message));
            }
            Ok(routes)
        })
    }

    fn add_route(&self, route: Self::Route) -> Result<()> {
        self.runtime.block_on(async {
            let (destination, prefix_len) = network_to_std(route.destination);
            let message = match destination {
                IpAddr::V4(addr) => {
                    let mut builder = RouteMessageBuilder::<std::net::Ipv4Addr>::new()
                        .destination_prefix(addr, prefix_len);
                    if let Some(IpAddr::V4(gateway)) = route.gateway.map(ip_address_to_std) {
                        builder = builder.gateway(gateway);
                    }
                    if let Some(metric) = route.metric {
                        builder = builder.priority(metric);
                    }
                    builder.build()
                }
                IpAddr::V6(addr) => {
                    let mut builder = RouteMessageBuilder::<std::net::Ipv6Addr>::new()
                        .destination_prefix(addr, prefix_len);
                    if let Some(IpAddr::V6(gateway)) = route.gateway.map(ip_address_to_std) {
                        builder = builder.gateway(gateway);
                    }
                    if let Some(metric) = route.metric {
                        builder = builder.priority(metric);
                    }
                    builder.build()
                }
            };

            self.handle
                .route()
                .add(message)
                .execute()
                .await
                .map_err(|err| Error::Platform(rtnetlink_error_code(&err)))
        })
    }

    fn remove_route(&self, route: Self::Route) -> Result<()> {
        self.runtime.block_on(async {
            let (destination, prefix_len) = network_to_std(route.destination);
            let message = match destination {
                IpAddr::V4(addr) => RouteMessageBuilder::<std::net::Ipv4Addr>::new()
                    .destination_prefix(addr, prefix_len)
                    .build(),
                IpAddr::V6(addr) => RouteMessageBuilder::<std::net::Ipv6Addr>::new()
                    .destination_prefix(addr, prefix_len)
                    .build(),
            };

            self.handle
                .route()
                .del(message)
                .execute()
                .await
                .map_err(|err| Error::Platform(rtnetlink_error_code(&err)))
        })
    }
}
