use net_lattice_core::Id;

use crate::IpAddress;
use crate::mac::MacAddress;

/// Identifies a [`NeighborEntry`].
pub type NeighborId = Id<NeighborEntry>;

/// The reachability state of a neighbor entry, per the Neighbor Unreachability
/// Detection state machine shared (with minor naming differences) by Linux's
/// `NUD_*` states, BSD/macOS's route-socket `RTF_*` flags on an ARP/NDP
/// entry, and Windows's `NL_NEIGHBOR_STATE`.
///
/// `#[non_exhaustive]`: not every platform exposes every state (BSD/macOS's
/// route-socket view collapses several of these into a single flags word),
/// and `Unknown` covers whatever a platform can't map cleanly.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
#[non_exhaustive]
pub enum NeighborState {
    /// Address resolution is in progress; no link-layer address yet.
    Incomplete,
    /// Confirmed reachable within the last reachable-time window.
    Reachable,
    /// Reachability is unconfirmed but the entry has not yet been probed.
    Stale,
    /// About to send a reachability probe after a delay.
    Delay,
    /// Actively probing to reconfirm reachability.
    Probe,
    /// Address resolution failed.
    Failed,
    /// Manually configured; never expires or is probed.
    Permanent,
    /// The platform does not expose a separate reachability state.
    Unknown,
}

/// A single entry in the system's neighbor table (ARP for IPv4, NDP for
/// IPv6): the mapping from an on-link IP address to a link-layer (MAC)
/// address, as observed or configured on a given interface.
///
/// `#[non_exhaustive]`: platforms carry different neighbor fields (Linux
/// exposes NUD state and a routing-protocol-style entry `kind`; BSD/macOS
/// expose only a flags word; Windows exposes `IsRouter`/reachability
/// timestamps). Marking this non-exhaustive now means adding
/// platform-specific fields later is not a breaking change for consumers who
/// construct a `NeighborEntry` via [`NeighborEntry::new`] rather than a
/// struct literal — see ARCHITECTURE.md's note on model extensibility.
#[derive(Debug, Clone, PartialEq, Eq, Hash)]
#[non_exhaustive]
pub struct NeighborEntry {
    pub id: NeighborId,
    /// The OS-level interface index this entry was observed on, the same raw
    /// value `Interface::index`/`Route::interface_index` carry.
    pub interface_index: u32,
    pub address: IpAddress,
    /// Absent for an entry still being resolved (`NeighborState::Incomplete`)
    /// or one the platform reports without a link-layer address at all.
    pub mac: Option<MacAddress>,
    pub state: NeighborState,
}

impl NeighborEntry {
    pub fn new(id: NeighborId, interface_index: u32, address: IpAddress) -> Self {
        Self {
            id,
            interface_index,
            address,
            mac: None,
            state: NeighborState::Unknown,
        }
    }

    pub fn with_mac(mut self, mac: MacAddress) -> Self {
        self.mac = Some(mac);
        self
    }

    pub fn with_state(mut self, state: NeighborState) -> Self {
        self.state = state;
        self
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use net_lattice_ip::Ipv4Address;

    #[test]
    fn new_entry_has_no_mac_and_unknown_state() {
        let entry = NeighborEntry::new(
            NeighborId::new(1),
            1,
            IpAddress::from(Ipv4Address::new(192, 168, 1, 1)),
        );
        assert!(entry.mac.is_none());
        assert_eq!(entry.state, NeighborState::Unknown);
    }

    #[test]
    fn builder_methods_set_optional_fields() {
        let mac = MacAddress::new([0x02, 0x00, 0x00, 0x00, 0x00, 0x01]);
        let entry = NeighborEntry::new(
            NeighborId::new(1),
            1,
            IpAddress::from(Ipv4Address::new(192, 168, 1, 1)),
        )
        .with_mac(mac)
        .with_state(NeighborState::Reachable);
        assert_eq!(entry.mac, Some(mac));
        assert_eq!(entry.state, NeighborState::Reachable);
    }
}
