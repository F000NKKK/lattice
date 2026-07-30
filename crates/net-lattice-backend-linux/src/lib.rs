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

use futures::{StreamExt, TryStreamExt};
use net_lattice_core::{Error, Id, PlatformErrorCode, Result};
use net_lattice_model::dns::DnsConfig;
use net_lattice_model::event::{ChangeKind, Event};
use net_lattice_model::ifaddr::{InterfaceAddress, InterfaceAddressId};
use net_lattice_model::interface::{AdminState, Interface, InterfaceKind, OperationalState};
use net_lattice_model::mac::MacAddress;
use net_lattice_model::neighbor::{NeighborEntry, NeighborId, NeighborState};
use net_lattice_model::route::{Route, RouteId};
use net_lattice_model::{IpAddress, Network};
use net_lattice_platform::{
    AddressProvider, Capability, CapabilityProvider, DnsProvider, EventProvider, EventReceiver,
    InterfaceProvider, NeighborProvider, RouteProvider,
};
use rtnetlink::packet_route::RouteNetlinkMessage;
use rtnetlink::packet_route::address::{AddressAttribute, AddressMessage};
use rtnetlink::packet_route::link::{LinkAttribute, LinkLayerType, LinkMessage, State};
use rtnetlink::packet_route::neighbour::{
    NeighbourAddress, NeighbourAttribute, NeighbourMessage, NeighbourState as RtNeighbourState,
};
use rtnetlink::packet_route::route::{RouteAddress, RouteAttribute, RouteMessage};
use rtnetlink::{Handle, MulticastGroup, RouteMessageBuilder};

/// The Linux Netlink-backed implementation of Net Lattice's provider traits.
pub struct LinuxBackend {
    runtime: tokio::runtime::Runtime,
    handle: Handle,
}

struct LinuxWatch {
    connection: tokio::task::JoinHandle<()>,
    events: tokio::task::JoinHandle<()>,
}

impl Drop for LinuxWatch {
    fn drop(&mut self) {
        self.events.abort();
        self.connection.abort();
    }
}

impl LinuxBackend {
    pub fn new() -> Result<Self> {
        let runtime =
            tokio::runtime::Runtime::new().map_err(|err| Error::Platform(io_error_code(&err)))?;
        // `rtnetlink::new_connection` constructs a `tokio::io::unix::AsyncFd`
        // socket synchronously, which requires an active Tokio reactor
        // context at construction time (not just when later polled) -
        // without `_guard`, this panics with "there is no reactor running"
        // the moment `add_route`/`remove_route`/`routes` first drives it.
        let _guard = runtime.enter();
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

/// Placeholder identity scheme, same rationale as `synthesize_route_id`: an
/// interface address has no kernel-assigned numeric ID, so this hashes its
/// interface and network together (the pair `RTM_GETADDR` itself keys
/// entries by).
fn synthesize_interface_address_id(interface_index: u32, network: &Network) -> InterfaceAddressId {
    let mut hasher = std::collections::hash_map::DefaultHasher::new();
    interface_index.hash(&mut hasher);
    network.hash(&mut hasher);
    InterfaceAddressId::new(hasher.finish())
}

fn message_to_interface_address(message: &AddressMessage) -> Option<InterfaceAddress> {
    let interface_index = message.header.index;
    let prefix_len = message.header.prefix_len;

    let mut address_addr = None;
    let mut broadcast = None;
    for attribute in &message.attributes {
        match attribute {
            // `IFA_LOCAL`, when present, is the actual assigned address —
            // `IFA_ADDRESS` alone (no `IFA_LOCAL`) names the *peer* address
            // on a point-to-point link, so prefer `Local` and only fall
            // back to `Address` when there is no separate local entry.
            AddressAttribute::Local(addr) => address_addr = Some(*addr),
            AddressAttribute::Address(addr) if address_addr.is_none() => {
                address_addr = Some(*addr);
            }
            AddressAttribute::Broadcast(addr) => {
                broadcast = Some(std_ip_to_ip_address(IpAddr::V4(*addr)));
            }
            _ => {}
        }
    }

    let network = match address_addr? {
        IpAddr::V4(addr) => {
            let prefix = net_lattice_ip::Ipv4PrefixLength::new(prefix_len)?;
            Network::from(net_lattice_ip::Ipv4Network::new(addr.into(), prefix))
        }
        IpAddr::V6(addr) => {
            let prefix = net_lattice_ip::Ipv6PrefixLength::new(prefix_len)?;
            Network::from(net_lattice_ip::Ipv6Network::new(addr.into(), prefix))
        }
    };

    let mut entry = InterfaceAddress::new(
        synthesize_interface_address_id(interface_index, &network),
        interface_index,
        network,
    );
    if let Some(broadcast) = broadcast {
        entry = entry.with_broadcast(broadcast);
    }
    Some(entry)
}

impl AddressProvider for LinuxBackend {
    type InterfaceAddress = InterfaceAddress;

    fn addresses(&self) -> Result<Vec<Self::InterfaceAddress>> {
        self.runtime.block_on(async {
            let mut messages = self.handle.address().get().execute();
            let mut addresses = Vec::new();
            while let Some(message) = messages
                .try_next()
                .await
                .map_err(|err| Error::Platform(rtnetlink_error_code(&err)))?
            {
                addresses.extend(message_to_interface_address(&message));
            }
            Ok(addresses)
        })
    }
}

/// Placeholder identity scheme: a route has no kernel-assigned numeric ID,
/// so this hashes its defining fields. See ARCHITECTURE.md's open Object
/// Identity question — this is not a long-term answer, only enough to give
/// `Stage 0.1` a `RouteId` to work with.
///
/// Hashes destination, gateway, and outgoing interface together so that
/// two routes to the same destination that differ only in gateway or
/// interface (a common case with multiple default routes, or ECMP-like
/// setups) don't collide on the same `RouteId`.
fn synthesize_route_id(message: &RouteMessage) -> RouteId {
    let mut hasher = std::collections::hash_map::DefaultHasher::new();
    message.header.destination_prefix_length.hash(&mut hasher);
    for attribute in &message.attributes {
        match attribute {
            RouteAttribute::Destination(addr) => {
                route_address_to_ip(addr).hash(&mut hasher);
            }
            RouteAttribute::Gateway(addr) => {
                route_address_to_ip(addr).hash(&mut hasher);
            }
            RouteAttribute::Oif(index) => {
                index.hash(&mut hasher);
            }
            _ => {}
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
    let mut interface_index = None;

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
            RouteAttribute::Oif(index) => {
                interface_index = Some(*index);
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
    if let Some(interface_index) = interface_index {
        route = route.with_interface_index(interface_index);
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
                    if let Some(interface_index) = route.interface_index {
                        builder = builder.output_interface(interface_index);
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
                    if let Some(interface_index) = route.interface_index {
                        builder = builder.output_interface(interface_index);
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
                IpAddr::V4(addr) => {
                    let mut builder = RouteMessageBuilder::<std::net::Ipv4Addr>::new()
                        .destination_prefix(addr, prefix_len);
                    if let Some(interface_index) = route.interface_index {
                        builder = builder.output_interface(interface_index);
                    }
                    builder.build()
                }
                IpAddr::V6(addr) => {
                    let mut builder = RouteMessageBuilder::<std::net::Ipv6Addr>::new()
                        .destination_prefix(addr, prefix_len);
                    if let Some(interface_index) = route.interface_index {
                        builder = builder.output_interface(interface_index);
                    }
                    builder.build()
                }
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

/// Maps Linux `ARPHRD_*` link-layer types (`LinkLayerType`) to the
/// cross-platform [`InterfaceKind`]. Anything not covered falls back to
/// `Other`, carrying the raw type code for diagnostics.
fn link_layer_type_to_kind(link_layer_type: LinkLayerType) -> InterfaceKind {
    match link_layer_type {
        LinkLayerType::Ether => InterfaceKind::Ethernet,
        LinkLayerType::Loopback => InterfaceKind::Loopback,
        LinkLayerType::Ppp => InterfaceKind::PointToPoint,
        LinkLayerType::Ieee80211
        | LinkLayerType::Ieee80211Prism
        | LinkLayerType::Ieee80211Radiotap => InterfaceKind::Wireless,
        other => InterfaceKind::Other(u16::from(other) as u32),
    }
}

fn message_to_interface(message: &LinkMessage) -> Interface {
    let index = message.header.index;
    let mut name = String::new();
    let mut mac = None;
    let mut mtu = None;
    let mut operational_state = OperationalState::Unknown;

    for attribute in &message.attributes {
        match attribute {
            LinkAttribute::IfName(value) => name = value.clone(),
            LinkAttribute::Address(bytes) if bytes.len() == 6 => {
                let mut octets = [0u8; 6];
                octets.copy_from_slice(bytes);
                mac = Some(MacAddress::new(octets));
            }
            LinkAttribute::Mtu(value) => mtu = Some(*value),
            LinkAttribute::OperState(state) => {
                operational_state = match state {
                    State::Up => OperationalState::Up,
                    State::Down | State::LowerLayerDown | State::NotPresent => {
                        OperationalState::Down
                    }
                    State::Dormant => OperationalState::NoCarrier,
                    _ => OperationalState::Unknown,
                };
            }
            _ => {}
        }
    }

    let admin_state = if message
        .header
        .flags
        .contains(rtnetlink::packet_route::link::LinkFlags::Up)
    {
        AdminState::Up
    } else {
        AdminState::Down
    };

    let kind = link_layer_type_to_kind(message.header.link_layer_type);

    let mut interface = Interface::new(Id::new(index as u64), index, name, kind)
        .with_admin_state(admin_state)
        .with_operational_state(operational_state);
    if let Some(mac) = mac {
        interface = interface.with_mac(mac);
    }
    if let Some(mtu) = mtu {
        interface = interface.with_mtu(mtu);
    }
    interface
}

impl InterfaceProvider for LinuxBackend {
    type Interface = Interface;

    fn interfaces(&self) -> Result<Vec<Self::Interface>> {
        self.runtime.block_on(async {
            let mut links = self.handle.link().get().execute();
            let mut interfaces = Vec::new();
            while let Some(message) = links
                .try_next()
                .await
                .map_err(|err| Error::Platform(rtnetlink_error_code(&err)))?
            {
                interfaces.push(message_to_interface(&message));
            }
            Ok(interfaces)
        })
    }
}

/// Placeholder identity scheme, same rationale as `synthesize_route_id`: a
/// neighbor entry has no kernel-assigned numeric ID, so this hashes its
/// interface and address together (the pair the kernel itself keys entries
/// by, via `RTM_GETNEIGH`).
fn synthesize_neighbor_id(interface_index: u32, address: &IpAddress) -> NeighborId {
    let mut hasher = std::collections::hash_map::DefaultHasher::new();
    interface_index.hash(&mut hasher);
    address.hash(&mut hasher);
    NeighborId::new(hasher.finish())
}

/// Maps Linux `NUD_*` neighbor states (`NeighbourState`) to the
/// cross-platform [`NeighborState`].
fn neighbour_state_to_state(state: RtNeighbourState) -> NeighborState {
    match state {
        RtNeighbourState::Incomplete => NeighborState::Incomplete,
        RtNeighbourState::Reachable => NeighborState::Reachable,
        RtNeighbourState::Stale => NeighborState::Stale,
        RtNeighbourState::Delay => NeighborState::Delay,
        RtNeighbourState::Probe => NeighborState::Probe,
        RtNeighbourState::Failed => NeighborState::Failed,
        RtNeighbourState::Permanent => NeighborState::Permanent,
        _ => NeighborState::Unknown,
    }
}

fn message_to_neighbor(message: &NeighbourMessage) -> Option<NeighborEntry> {
    let interface_index = message.header.ifindex;
    let mut address = None;
    let mut mac = None;

    for attribute in &message.attributes {
        match attribute {
            NeighbourAttribute::Destination(NeighbourAddress::Inet(addr)) => {
                address = Some(std_ip_to_ip_address(IpAddr::V4(*addr)));
            }
            NeighbourAttribute::Destination(NeighbourAddress::Inet6(addr)) => {
                address = Some(std_ip_to_ip_address(IpAddr::V6(*addr)));
            }
            NeighbourAttribute::LinkLayerAddress(bytes) if bytes.len() == 6 => {
                let mut octets = [0u8; 6];
                octets.copy_from_slice(bytes);
                mac = Some(MacAddress::new(octets));
            }
            _ => {}
        }
    }

    let address = address?;
    let mut entry = NeighborEntry::new(
        synthesize_neighbor_id(interface_index, &address),
        interface_index,
        address,
    )
    .with_state(neighbour_state_to_state(message.header.state));
    if let Some(mac) = mac {
        entry = entry.with_mac(mac);
    }
    Some(entry)
}

impl NeighborProvider for LinuxBackend {
    type NeighborEntry = NeighborEntry;

    fn neighbors(&self) -> Result<Vec<Self::NeighborEntry>> {
        self.runtime.block_on(async {
            let mut messages = self.handle.neighbours().get().execute();
            let mut neighbors = Vec::new();
            while let Some(message) = messages
                .try_next()
                .await
                .map_err(|err| Error::Platform(rtnetlink_error_code(&err)))?
            {
                neighbors.extend(message_to_neighbor(&message));
            }
            Ok(neighbors)
        })
    }
}

/// Parses the `nameserver`/`search`/`domain` directives out of a
/// `resolv.conf`-format file (`man 5 resolv.conf`) — the same format on
/// Linux and BSD/macOS, so this parser is shared verbatim by
/// `net-lattice-backend-darwin`.
///
/// `domain` is folded into `search_domains` too: it is the legacy
/// single-domain predecessor of `search` and every modern resolver treats a
/// lone `domain` entry as an implicit one-element search list.
fn parse_resolv_conf(contents: &str) -> DnsConfig {
    let mut config = DnsConfig::new();
    for line in contents.lines() {
        let line = line.trim();
        if line.is_empty() || line.starts_with('#') || line.starts_with(';') {
            continue;
        }
        let mut parts = line.split_whitespace();
        match parts.next() {
            Some("nameserver") => {
                if let Some(addr) = parts.next().and_then(|s| s.parse::<IpAddr>().ok()) {
                    config.nameservers.push(std_ip_to_ip_address(addr));
                }
            }
            Some("search") | Some("domain") => {
                config
                    .search_domains
                    .extend(parts.map(|domain| domain.to_string()));
            }
            _ => {}
        }
    }
    config
}

fn resolv_conf_error(err: &std::io::Error) -> Error {
    match err.kind() {
        std::io::ErrorKind::NotFound => Error::NotFound,
        std::io::ErrorKind::PermissionDenied => Error::PermissionDenied,
        _ => Error::Platform(io_error_code(err)),
    }
}

impl CapabilityProvider for LinuxBackend {
    /// `IPV6` unconditionally: every provider this backend implements
    /// (routes, interfaces, neighbors, addresses) already handles both
    /// address families. `MONITORING` unconditionally too, now that
    /// `EventProvider` is implemented below. `VRF`/`NAMESPACES` are left
    /// unset — Linux genuinely supports both at the kernel level, but Net
    /// Lattice doesn't implement either yet, and a `Capability` this
    /// backend can't actually act on would be a lie to the caller.
    fn capabilities(&self) -> Capability {
        Capability::IPV6 | Capability::MONITORING
    }
}

/// Maps one Netlink route/link/neighbour/address notification to an
/// [`Event`]. Reuses the existing `message_to_*` conversion functions
/// (built for the one-shot `routes()`/`interfaces()`/... dumps) purely to
/// derive the same `Id` a consumer would see from those methods — a
/// notification's `DelFoo` variant tells us an object was removed. Netlink
/// uses the corresponding `NewFoo` message for both creation and mutation,
/// so it is reported as `Changed` rather than incorrectly promising an add;
/// only the `Id` needs deriving here.
///
/// Returns `None` for Netlink message kinds this backend doesn't turn into
/// an `Event` (link property changes, neighbour tables, ...) and for any
/// notification whose attributes don't parse into an `Id` (extremely rare
/// for the kernel's own unsolicited notifications, but the parsers are
/// intentionally lenient rather than panicking on unexpected input).
fn route_netlink_message_to_event(message: RouteNetlinkMessage) -> Option<Event> {
    match message {
        RouteNetlinkMessage::NewRoute(msg) => message_to_route(&msg).map(|route| Event::Route {
            id: route.id,
            kind: ChangeKind::Changed,
        }),
        RouteNetlinkMessage::DelRoute(msg) => message_to_route(&msg).map(|route| Event::Route {
            id: route.id,
            kind: ChangeKind::Removed,
        }),
        RouteNetlinkMessage::NewLink(msg) => Some(Event::Interface {
            id: Id::new(msg.header.index as u64),
            kind: ChangeKind::Changed,
        }),
        RouteNetlinkMessage::DelLink(msg) => Some(Event::Interface {
            id: Id::new(msg.header.index as u64),
            kind: ChangeKind::Removed,
        }),
        RouteNetlinkMessage::NewNeighbour(msg) => {
            message_to_neighbor(&msg).map(|entry| Event::Neighbor {
                id: entry.id,
                kind: ChangeKind::Changed,
            })
        }
        RouteNetlinkMessage::DelNeighbour(msg) => {
            message_to_neighbor(&msg).map(|entry| Event::Neighbor {
                id: entry.id,
                kind: ChangeKind::Removed,
            })
        }
        RouteNetlinkMessage::NewAddress(msg) => {
            message_to_interface_address(&msg).map(|addr| Event::Address {
                id: addr.id,
                kind: ChangeKind::Changed,
            })
        }
        RouteNetlinkMessage::DelAddress(msg) => {
            message_to_interface_address(&msg).map(|addr| Event::Address {
                id: addr.id,
                kind: ChangeKind::Removed,
            })
        }
        _ => None,
    }
}

impl EventProvider for LinuxBackend {
    type Event = Event;

    /// Opens a *second*, independent Netlink socket subscribed to the
    /// `RTNLGRP_LINK`/`RTNLGRP_NEIGH`/`RTNLGRP_IPV4_ROUTE`/
    /// `RTNLGRP_IPV6_ROUTE`/`RTNLGRP_IPV4_IFADDR`/`RTNLGRP_IPV6_IFADDR`
    /// multicast groups (`rtnetlink::new_multicast_connection`, the same
    /// mechanism `ip monitor` uses) — separate from `self.handle`'s socket,
    /// which only ever sends request/reply traffic. Two background tasks
    /// are spawned onto `self.runtime`: one drives the new socket's
    /// connection (required for it to make progress at all, mirroring
    /// `LinuxBackend::new`'s `connection` task), the other reads
    /// unsolicited notifications off it and forwards mapped `Event`s into
    /// the channel `EventReceiver` reads from.
    ///
    /// DNS resolver configuration changes (`/etc/resolv.conf`) have no
    /// Netlink signal — see `Event`'s doc comment — so no `Event::Dns` is
    /// ever produced by this backend.
    fn watch(&self) -> Result<EventReceiver<Self::Event>> {
        let groups = [
            MulticastGroup::Link,
            MulticastGroup::Neigh,
            MulticastGroup::Ipv4Route,
            MulticastGroup::Ipv6Route,
            MulticastGroup::Ipv4Ifaddr,
            MulticastGroup::Ipv6Ifaddr,
        ];
        let _guard = self.runtime.enter();
        let (connection, _handle, mut messages) = rtnetlink::new_multicast_connection(&groups)
            .map_err(|err| Error::Platform(io_error_code(&err)))?;
        let connection = self.runtime.spawn(connection);

        let (sender, receiver) = std::sync::mpsc::channel();
        let events = self.runtime.spawn(async move {
            while let Some((message, _addr)) = messages.next().await {
                let (_header, payload) = message.into_parts();
                let rtnetlink::packet_core::NetlinkPayload::InnerMessage(inner) = payload else {
                    continue;
                };
                if let Some(event) = route_netlink_message_to_event(inner)
                    && sender.send(event).is_err()
                {
                    break;
                }
            }
        });

        Ok(EventReceiver::with_subscription(
            receiver,
            LinuxWatch { connection, events },
        ))
    }
}

impl DnsProvider for LinuxBackend {
    type DnsConfig = DnsConfig;

    fn dns_config(&self) -> Result<Self::DnsConfig> {
        let contents =
            std::fs::read_to_string("/etc/resolv.conf").map_err(|err| resolv_conf_error(&err))?;
        Ok(parse_resolv_conf(&contents))
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use net_lattice_ip::{Ipv4Address, Ipv4Network, Ipv4PrefixLength};

    /// Exercises a real round trip through Netlink, no privilege required:
    /// `RTM_GETROUTE` dumps are readable by any user. This is the one test
    /// in this module that runs by default and actually proves the backend
    /// talks to the kernel, rather than only exercising conversion logic.
    #[test]
    fn routes_reads_the_real_kernel_routing_table() {
        let backend = LinuxBackend::new().expect("failed to open a Netlink connection");
        let routes = backend
            .routes()
            .expect("RTM_GETROUTE dump should not require privilege");
        // Not asserting on contents: the routing table of the machine
        // running this test is arbitrary (may even be empty in a minimal
        // container). Reaching here without an error is the assertion.
        let _ = routes;
    }

    /// Exercises a real round trip through Netlink, no privilege required:
    /// `RTM_GETLINK` dumps are readable by any user, and every Linux system
    /// has at least `lo`.
    #[test]
    fn interfaces_includes_the_loopback_interface() {
        let backend = LinuxBackend::new().expect("failed to open a Netlink connection");
        let interfaces = backend
            .interfaces()
            .expect("RTM_GETLINK dump should not require privilege");
        assert!(
            interfaces
                .iter()
                .any(|iface| iface.name == "lo" && iface.kind == InterfaceKind::Loopback),
            "expected a `lo` interface classified as Loopback, got: {interfaces:?}"
        );
    }

    /// Exercises a real round trip through Netlink, no privilege required:
    /// `RTM_GETADDR` dumps are readable by any user, and every Linux system
    /// has at least `lo`'s `127.0.0.1/8`.
    #[test]
    fn addresses_includes_loopbacks_address() {
        let backend = LinuxBackend::new().expect("failed to open a Netlink connection");
        let addresses = backend
            .addresses()
            .expect("RTM_GETADDR dump should not require privilege");
        assert!(
            addresses.iter().any(|addr| matches!(
                addr.address,
                Network::V4(net) if net.address() == Ipv4Address::new(127, 0, 0, 1)
            )),
            "expected `127.0.0.1` among the assigned addresses, got: {addresses:?}"
        );
    }

    /// Exercises a real round trip through Netlink, no privilege required:
    /// `RTM_GETNEIGH` dumps are readable by any user.
    #[test]
    fn neighbors_reads_the_real_kernel_neighbor_table() {
        let backend = LinuxBackend::new().expect("failed to open a Netlink connection");
        let neighbors = backend
            .neighbors()
            .expect("RTM_GETNEIGH dump should not require privilege");
        // Not asserting on contents: the neighbor table of the machine
        // running this test is arbitrary (may even be empty). Reaching here
        // without an error is the assertion.
        let _ = neighbors;
    }

    #[test]
    fn parse_resolv_conf_reads_nameservers_and_search_domains() {
        let contents = "# comment\n\
                         nameserver 1.1.1.1\n\
                         nameserver 2606:4700:4700::1111\n\
                         search example.com corp.example.com\n";
        let config = parse_resolv_conf(contents);
        assert_eq!(
            config.nameservers,
            vec![
                IpAddress::from(Ipv4Address::new(1, 1, 1, 1)),
                std_ip_to_ip_address("2606:4700:4700::1111".parse().unwrap()),
            ]
        );
        assert_eq!(
            config.search_domains,
            vec!["example.com".to_string(), "corp.example.com".to_string()]
        );
    }

    /// Reads the real `/etc/resolv.conf` present on this test environment.
    /// Every Linux system has one (even if empty/symlinked to
    /// systemd-resolved's stub), so this exercises the real filesystem read
    /// without requiring any specific content.
    #[test]
    fn dns_config_reads_the_real_resolv_conf() {
        let backend = LinuxBackend::new().expect("failed to open a Netlink connection");
        let config = backend
            .dns_config()
            .expect("/etc/resolv.conf should be readable");
        let _ = config;
    }

    fn loopback_interface_index(backend: &LinuxBackend) -> u32 {
        backend
            .runtime
            .block_on(async {
                let mut links = backend
                    .handle
                    .link()
                    .get()
                    .match_name("lo".into())
                    .execute();
                links
                    .try_next()
                    .await
                    .ok()
                    .flatten()
                    .map(|link| link.header.index)
            })
            .expect("this test environment has no `lo` interface")
    }

    /// Requires `CAP_NET_ADMIN` (root, or `sudo -E cargo test -- --ignored`
    /// in this crate). Not run by default because most development and CI
    /// environments — including the one this crate was originally written
    /// in — don't grant it, and this test would otherwise fail with
    /// `PermissionDenied` rather than being skipped.
    ///
    /// Uses a documentation-only prefix (RFC 5737 `203.0.113.0/24`,
    /// TEST-NET-3) on `lo` so it can't collide with or disrupt real
    /// routing, and removes what it added regardless of assertion outcome.
    #[test]
    #[ignore = "requires CAP_NET_ADMIN; run with `sudo -E cargo test -p net-lattice-backend-linux -- --ignored`"]
    fn add_then_remove_route_round_trips_through_the_kernel() {
        let backend = LinuxBackend::new().expect("failed to open a Netlink connection");
        let interface_index = loopback_interface_index(&backend);

        let destination = Network::from(Ipv4Network::new(
            Ipv4Address::new(203, 0, 113, 0),
            Ipv4PrefixLength::new(24).unwrap(),
        ));
        let route = Route::new(RouteId::new(0), destination).with_interface_index(interface_index);

        let add_result = backend.add_route(route.clone());
        if matches!(
            add_result,
            Err(Error::PermissionDenied) | Err(Error::Platform(_))
        ) {
            // Best effort even under #[ignore]: if it's run without the
            // capability after all, fail loudly rather than silently
            // passing on a no-op.
            add_result.expect("add_route failed - are you running with CAP_NET_ADMIN?");
        }

        let routes = backend
            .routes()
            .expect("routes() failed after add_route succeeded");
        let found = routes
            .iter()
            .any(|r| r.destination == destination && r.interface_index == Some(interface_index));

        // Clean up before asserting, so a failed assertion doesn't leave
        // the test route behind on the machine that ran this.
        let _ = backend.remove_route(route);

        assert!(found, "added route was not present in routes() afterward");

        let routes_after_removal = backend
            .routes()
            .expect("routes() failed after remove_route");
        assert!(
            !routes_after_removal
                .iter()
                .any(|r| r.destination == destination && r.interface_index == Some(interface_index)),
            "removed route was still present in routes() afterward"
        );
    }
}
