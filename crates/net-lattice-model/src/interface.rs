use std::fmt;

use net_lattice_core::Id;

use crate::mac::MacAddress;

/// Identifies an [`Interface`].
pub type InterfaceId = Id<Interface>;

/// The kind of a network interface.
///
/// Platforms expose different classifications (Linux `ARPHRD_*`, Windows
/// `IfType`, BSD/macOS `IFT_*`); this enum carries the subset that maps
/// cleanly across all of them. It is `#[non_exhaustive]` so adding
/// platform-specific kinds later is not a breaking change — see
/// ARCHITECTURE.md's note on model extensibility.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
#[non_exhaustive]
pub enum InterfaceKind {
    /// Ethernet and other IEEE 802 MAC-layer interfaces.
    Ethernet,
    /// Wi-Fi / 802.11 wireless.
    Wireless,
    /// Loopback (`lo` / `Loopback Pseudo-Interface`).
    Loopback,
    /// Point-to-point tunnels and virtual links without a broadcast domain.
    PointToPoint,
    /// A bridge or other software switch aggregating member interfaces.
    Bridge,
    /// Anything not yet classified — the raw OS-supplied type code, when
    /// available, is carried alongside for diagnostics.
    Other(u32),
}

impl fmt::Display for InterfaceKind {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            InterfaceKind::Ethernet => write!(f, "ethernet"),
            InterfaceKind::Wireless => write!(f, "wireless"),
            InterfaceKind::Loopback => write!(f, "loopback"),
            InterfaceKind::PointToPoint => write!(f, "point-to-point"),
            InterfaceKind::Bridge => write!(f, "bridge"),
            InterfaceKind::Other(code) => write!(f, "other({code})"),
        }
    }
}

/// The administrative state of an interface, as configured by the operator.
///
/// Distinct from [`OperationalState`]: an interface can be administratively
/// up but operationally down (cable unplugged), or administratively down
/// (and therefore always operationally down).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
#[non_exhaustive]
pub enum AdminState {
    Up,
    Down,
    /// The platform does not expose a separate administrative state (e.g.
    /// BSD/macOS route-socket interfaces report a single combined state).
    Unknown,
}

/// The operational state of an interface, as observed from the link layer.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
#[non_exhaustive]
pub enum OperationalState {
    Up,
    Down,
    /// The interface is up but has no carrier / lower-layer connectivity.
    NoCarrier,
    /// The platform does not expose a separate operational state.
    Unknown,
}

/// A network interface.
///
/// `#[non_exhaustive]`: platforms carry different interface fields (Linux
/// exposes `carrier`, `txqueuelen`, `type`; Windows exposes `MediaType`,
/// `OperationalStatus`; BSD/macOS expose a combined flags word). Marking
/// this non-exhaustive now means adding platform-specific fields later is
/// not a breaking change for consumers who construct an `Interface` via
/// [`Interface::new`] rather than a struct literal — see ARCHITECTURE.md's
/// note on model extensibility.
#[derive(Debug, Clone, PartialEq, Eq, Hash)]
#[non_exhaustive]
pub struct Interface {
    pub id: InterfaceId,
    /// The OS-level interface index (`ifindex` on Linux, `InterfaceIndex` on
    /// Windows, `if_index` on BSD/macOS). This is the same raw value
    /// `Route::interface_index` carries — kept here as the canonical home
    /// now that the `interface` domain exists (Stage 0.4).
    pub index: u32,
    pub name: String,
    pub kind: InterfaceKind,
    pub mac: Option<MacAddress>,
    pub mtu: Option<u32>,
    pub admin_state: AdminState,
    pub operational_state: OperationalState,
}

impl Interface {
    pub fn new(id: InterfaceId, index: u32, name: impl Into<String>, kind: InterfaceKind) -> Self {
        Self {
            id,
            index,
            name: name.into(),
            kind,
            mac: None,
            mtu: None,
            admin_state: AdminState::Unknown,
            operational_state: OperationalState::Unknown,
        }
    }

    pub fn with_mac(mut self, mac: MacAddress) -> Self {
        self.mac = Some(mac);
        self
    }

    pub fn with_mtu(mut self, mtu: u32) -> Self {
        self.mtu = Some(mtu);
        self
    }

    pub fn with_admin_state(mut self, state: AdminState) -> Self {
        self.admin_state = state;
        self
    }

    pub fn with_operational_state(mut self, state: OperationalState) -> Self {
        self.operational_state = state;
        self
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn new_interface_has_no_optional_fields() {
        let iface = Interface::new(InterfaceId::new(1), 1, "eth0", InterfaceKind::Ethernet);
        assert!(iface.mac.is_none());
        assert!(iface.mtu.is_none());
        assert_eq!(iface.admin_state, AdminState::Unknown);
        assert_eq!(iface.operational_state, OperationalState::Unknown);
    }

    #[test]
    fn builder_methods_set_optional_fields() {
        let mac = MacAddress::new([0x02, 0x00, 0x00, 0x00, 0x00, 0x01]);
        let iface = Interface::new(InterfaceId::new(1), 1, "eth0", InterfaceKind::Ethernet)
            .with_mac(mac)
            .with_mtu(1500)
            .with_admin_state(AdminState::Up)
            .with_operational_state(OperationalState::Up);
        assert_eq!(iface.mac, Some(mac));
        assert_eq!(iface.mtu, Some(1500));
        assert_eq!(iface.admin_state, AdminState::Up);
        assert_eq!(iface.operational_state, OperationalState::Up);
    }

    #[test]
    fn kind_displays() {
        assert_eq!(InterfaceKind::Ethernet.to_string(), "ethernet");
        assert_eq!(InterfaceKind::Wireless.to_string(), "wireless");
        assert_eq!(InterfaceKind::Loopback.to_string(), "loopback");
        assert_eq!(InterfaceKind::PointToPoint.to_string(), "point-to-point");
        assert_eq!(InterfaceKind::Bridge.to_string(), "bridge");
        assert_eq!(InterfaceKind::Other(42).to_string(), "other(42)");
    }
}
