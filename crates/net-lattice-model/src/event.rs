use crate::ifaddr::InterfaceAddressId;
use crate::interface::InterfaceId;
use crate::neighbor::NeighborId;
use crate::route::RouteId;

/// What kind of change an [`Event`] reports.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
#[non_exhaustive]
pub enum ChangeKind {
    Added,
    Removed,
    /// Reserved for a future field-mask payload (`Changed { fields: ... }`)
    /// — see ARCHITECTURE.md's note on `Event`. Carries no detail yet: a
    /// consumer that needs to know what changed re-reads the object through
    /// the relevant provider.
    Changed,
}

/// A domain whose state must be re-read after an overflow notification.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
#[non_exhaustive]
pub enum EventDomain {
    Route,
    Interface,
    Neighbor,
    Address,
    All,
}

/// Selects which domains an event watcher delivers.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct EventFilter {
    routes: bool,
    interfaces: bool,
    neighbors: bool,
    addresses: bool,
}

impl EventFilter {
    pub const ALL: Self = Self {
        routes: true,
        interfaces: true,
        neighbors: true,
        addresses: true,
    };
    pub const fn none() -> Self {
        Self {
            routes: false,
            interfaces: false,
            neighbors: false,
            addresses: false,
        }
    }
    pub const fn routes(mut self) -> Self {
        self.routes = true;
        self
    }
    pub const fn interfaces(mut self) -> Self {
        self.interfaces = true;
        self
    }
    pub const fn neighbors(mut self) -> Self {
        self.neighbors = true;
        self
    }
    pub const fn addresses(mut self) -> Self {
        self.addresses = true;
        self
    }
    pub const fn matches(self, event: Event) -> bool {
        match event {
            Event::Route { .. } => self.routes,
            Event::Interface { .. } => self.interfaces,
            Event::Neighbor { .. } => self.neighbors,
            Event::Address { .. } => self.addresses,
            Event::ResyncRequired { domain } => match domain {
                EventDomain::Route => self.routes,
                EventDomain::Interface => self.interfaces,
                EventDomain::Neighbor => self.neighbors,
                EventDomain::Address => self.addresses,
                EventDomain::All => {
                    self.routes || self.interfaces || self.neighbors || self.addresses
                }
            },
        }
    }
}
impl Default for EventFilter {
    fn default() -> Self {
        Self::ALL
    }
}

/// A signal that something changed in the system's networking state —
/// carrying an ID and a [`ChangeKind`], not a clone of the full domain
/// object.
///
/// Deliberately signal-shaped rather than snapshot-shaped (e.g.
/// `Event::Route { id, kind }`, not `Event::RouteAdded(Route)`): native
/// change notifications frequently don't hand over the full object in the
/// first place (an `RTM_NEWROUTE` message or a Windows route-change
/// callback can carry only what changed, not a complete record), and a
/// consumer that only cares *that* something changed before deciding
/// whether to re-query it shouldn't pay for a clone of the full object on
/// every event. A consumer that needs the current value re-reads it
/// through the relevant provider (`Lattice::routes()`, keyed by the ID
/// carried here).
///
/// `#[non_exhaustive]`: DNS resolver configuration changes are a stated
/// long-term goal for this enum (see ARCHITECTURE.md) but are not emitted
/// by any backend yet — no platform in Stage 0.8 exposes a push
/// notification for `/etc/resolv.conf`/per-adapter DNS settings changing
/// without resorting to filesystem polling, which isn't a genuine push
/// signal. Adding a `Dns` variant later is not a breaking change for
/// consumers who already `match` non-exhaustively.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
#[non_exhaustive]
pub enum Event {
    Route {
        id: RouteId,
        kind: ChangeKind,
    },
    Interface {
        id: InterfaceId,
        kind: ChangeKind,
    },
    Neighbor {
        id: NeighborId,
        kind: ChangeKind,
    },
    Address {
        id: InterfaceAddressId,
        kind: ChangeKind,
    },
    /// Notifications were dropped because the bounded queue filled; re-read
    /// the indicated domain before relying on later signals.
    ResyncRequired {
        domain: EventDomain,
    },
}
impl Event {
    pub const fn resync_all() -> Self {
        Self::ResyncRequired {
            domain: EventDomain::All,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn events_carry_id_and_kind() {
        let event = Event::Route {
            id: RouteId::new(1),
            kind: ChangeKind::Added,
        };
        assert_eq!(
            event,
            Event::Route {
                id: RouteId::new(1),
                kind: ChangeKind::Added,
            }
        );
        assert_ne!(
            event,
            Event::Route {
                id: RouteId::new(1),
                kind: ChangeKind::Removed,
            }
        );
    }
}
