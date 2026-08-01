//! Cross-platform inspection, mutation, and monitoring of operating-system
//! networking through a strongly typed Rust API.
//!
//! Start with [`Lattice::connect`] to inspect interfaces, addresses, routes,
//! DNS configuration, and neighbor tables; perform supported mutations; or
//! subscribe to network change events.
//!
//! # Example
//!
//! ```no_run
//! use net_lattice::{Lattice, Result};
//!
//! fn main() -> Result<()> {
//!     let lattice = Lattice::connect()?;
//!     for interface in lattice.interfaces()? {
//!         println!("{interface:?}");
//!     }
//!     Ok(())
//! }
//! ```
//!
//! # Facade design
//!
//! Re-exports the types consumers need from `net-lattice-model` and
//! `net-lattice-ip`, selects a default backend based on `cfg(target_os =
//! "...")`, and enforces model convergence: `net-lattice-platform`'s generic
//! provider traits are constrained here to Net Lattice's own model types,
//! without `net-lattice-platform` ever depending on `net-lattice-model`. See
//! ARCHITECTURE.md for the full rationale.

/// Async event adapters, enabled by the `async` feature.
#[cfg(feature = "async")]
pub use net_lattice_async::EventStream;
mod executor;
pub use executor::{Cancellation, Compensation, ExecutionOptions, Snapshot};
pub use net_lattice_core::{Error, Id, PlatformErrorCode, Result};
pub use net_lattice_ip::{
    Ipv4Address, Ipv4Network, Ipv4PrefixLength, Ipv6Address, Ipv6Network, Ipv6PrefixLength,
};
pub use net_lattice_model::dns::{DnsConfig, NewDnsConfig};
pub use net_lattice_model::event::{ChangeKind, Event, EventDomain, EventFilter};
pub use net_lattice_model::ifaddr::{InterfaceAddress, InterfaceAddressId, NewInterfaceAddress};
pub use net_lattice_model::interface::{
    AdminState, Interface, InterfaceId, InterfaceKind, OperationalState,
};
pub use net_lattice_model::mac::MacAddress;
pub use net_lattice_model::mutation::{
    Mutation, MutationConfirmation, MutationExecutionPhase, MutationIdempotency, MutationKind,
    MutationOperationReport, MutationOutcome, MutationPlan, MutationPlanReport,
    MutationPrecondition, MutationPreflight, MutationPrivilege, MutationReversibility,
    MutationSemantics, MutationSnapshot, MutationStopReason, RollbackStatus,
};
pub use net_lattice_model::neighbor::{NeighborEntry, NeighborId, NeighborState};
pub use net_lattice_model::route::{Route, RouteId};
pub use net_lattice_model::{IpAddress, Network};
#[cfg(feature = "async")]
pub use net_lattice_platform::TokioEventProvider;
pub use net_lattice_platform::{
    AddressMutator, AddressProvider, Capability, CapabilityProvider, DnsMutator, DnsProvider,
    EventProvider, EventReceiver, InterfaceProvider, NeighborProvider, RouteProvider,
};

#[cfg(test)]
use std::sync::atomic::{AtomicBool, Ordering};
use std::time::Instant;

/// Contracts for implementing a third-party Net Lattice backend.
///
/// These traits are a supported extension API. A backend must preserve the
/// documented read, mutation, event-delivery, and cancellation semantics of
/// each trait it implements. The root re-exports remain available for
/// compatibility; new backend code may import from this module.
pub mod backend {
    pub use crate::LatticeBackend;
    pub use net_lattice_platform::{
        AddressMutator, AddressProvider, CapabilityProvider, DnsMutator, DnsProvider,
        EventProvider, EventReceiver, EventSender, InterfaceProvider, NeighborProvider,
        RouteProvider,
    };
    #[cfg(feature = "async")]
    pub use net_lattice_platform::{TokioEventProvider, TokioEventReceiver, TokioEventSender};
}

/// Bound satisfied by any backend usable with [`Lattice`].
///
/// This is where model convergence is enforced: a backend whose
/// `RouteProvider::Route` (or `InterfaceProvider::Interface`,
/// `DnsProvider::DnsConfig`, `NeighborProvider::NeighborEntry`,
/// `AddressProvider::InterfaceAddress`, `AddressMutator`'s input/output,
/// `EventProvider::Event`) is not
/// literally `net_lattice_model`'s corresponding type fails to satisfy this
/// trait and cannot be used with [`Lattice`] — a compile error at the point
/// the backend is wired in, not a runtime surprise. See ARCHITECTURE.md's
/// `net-lattice` section. `CapabilityProvider` has no associated type to
/// converge — it reports plain runtime facts about the connected system,
/// not domain objects — so it's required as-is.
pub trait LatticeBackend:
    RouteProvider<Route = Route>
    + InterfaceProvider<Interface = Interface>
    + DnsMutator<NewDnsConfig = NewDnsConfig, DnsConfig = DnsConfig>
    + NeighborProvider<NeighborEntry = NeighborEntry>
    + AddressProvider<InterfaceAddress = InterfaceAddress>
    + AddressMutator<NewInterfaceAddress = NewInterfaceAddress, InterfaceAddress = InterfaceAddress>
    + EventProvider<Event = Event, EventFilter = EventFilter>
    + CapabilityProvider
{
}

impl<B> LatticeBackend for B where
    B: RouteProvider<Route = Route>
        + InterfaceProvider<Interface = Interface>
        + DnsMutator<NewDnsConfig = NewDnsConfig, DnsConfig = DnsConfig>
        + NeighborProvider<NeighborEntry = NeighborEntry>
        + AddressProvider<InterfaceAddress = InterfaceAddress>
        + AddressMutator<
            NewInterfaceAddress = NewInterfaceAddress,
            InterfaceAddress = InterfaceAddress,
        > + EventProvider<Event = Event, EventFilter = EventFilter>
        + CapabilityProvider
{
}

/// The top-level entry point: a connected backend for the current system.
pub struct Lattice<B: LatticeBackend> {
    backend: B,
}

#[cfg(test)]
static FORCE_CONNECT_FAILURE: AtomicBool = AtomicBool::new(false);

impl<B: LatticeBackend> Lattice<B> {
    pub fn routes(&self) -> Result<Vec<Route>> {
        self.backend.routes()
    }

    pub fn add_route(&self, route: Route) -> Result<()> {
        self.backend.add_route(route)
    }

    pub fn remove_route(&self, route: Route) -> Result<()> {
        self.backend.remove_route(route)
    }

    pub fn interfaces(&self) -> Result<Vec<Interface>> {
        self.backend.interfaces()
    }

    pub fn dns_config(&self) -> Result<DnsConfig> {
        self.backend.dns_config()
    }

    /// Replaces resolver configuration and returns the resulting observed
    /// resolver view.
    pub fn set_dns_config(&self, config: NewDnsConfig) -> Result<DnsConfig> {
        self.backend.set_dns_config(config)
    }

    pub fn neighbors(&self) -> Result<Vec<NeighborEntry>> {
        self.backend.neighbors()
    }

    pub fn addresses(&self) -> Result<Vec<InterfaceAddress>> {
        self.backend.addresses()
    }

    /// Assigns an address to an interface and returns the canonical record
    /// observed from the operating system after creation.
    pub fn add_address(&self, address: NewInterfaceAddress) -> Result<InterfaceAddress> {
        self.backend.add_address(address)
    }

    /// Removes the observed interface address.
    pub fn remove_address(&self, address: InterfaceAddress) -> Result<()> {
        self.backend.remove_address(address)
    }

    /// Performs the runtime portion of mutation preflight.
    ///
    /// This check is side-effect free. It validates capabilities exposed by
    /// the connected backend before an executor submits any operation; native
    /// privilege and current-state checks can still fail at execution time.
    pub fn validate_plan(&self, plan: &MutationPlan) -> Result<()> {
        let mut planned_routes = Vec::new();
        let mut removed_routes = Vec::new();
        let mut planned_addresses = Vec::new();
        let mut removed_addresses = Vec::new();
        for operation in plan.operations() {
            if executor::requires_dns_capability(operation)
                && !self.supports(Capability::DNS_MUTATION)
            {
                return Err(Error::Unsupported);
            }

            match operation {
                Mutation::AddRoute(route) => {
                    let exists_in_system = self
                        .routes()?
                        .iter()
                        .any(|candidate| Self::same_route(candidate, route))
                        && !removed_routes
                            .iter()
                            .any(|candidate| Self::same_route(candidate, route));
                    let exists = exists_in_system
                        || planned_routes
                            .iter()
                            .any(|candidate| Self::same_route(candidate, route));
                    if exists {
                        return Err(Error::AlreadyExists);
                    }
                    planned_routes.push(route.clone());
                    removed_routes.retain(|candidate| !Self::same_route(candidate, route));
                }
                Mutation::RemoveRoute(route) => {
                    let exists_in_system = self
                        .routes()?
                        .iter()
                        .any(|candidate| Self::same_route(candidate, route))
                        && !removed_routes
                            .iter()
                            .any(|candidate| Self::same_route(candidate, route));
                    let exists = exists_in_system
                        || planned_routes
                            .iter()
                            .any(|candidate| Self::same_route(candidate, route));
                    if !exists {
                        return Err(Error::NotFound);
                    }
                    planned_routes.retain(|candidate| !Self::same_route(candidate, route));
                    removed_routes.push(route.clone());
                }
                Mutation::AddAddress(address) => {
                    let interface_index = address.interface_id.value() as u32;
                    if !self.interfaces()?.iter().any(|interface| {
                        interface.id == address.interface_id || interface.index == interface_index
                    }) {
                        return Err(Error::NotFound);
                    }
                    let key = (interface_index, address.address);
                    let exists_in_system =
                        self.addresses()?.iter().any(|candidate| {
                            candidate.interface_index == interface_index
                                && candidate.address == address.address
                        }) && !removed_addresses.iter().any(|candidate| candidate == &key);
                    if exists_in_system
                        || planned_addresses.iter().any(|candidate| candidate == &key)
                    {
                        return Err(Error::AlreadyExists);
                    }
                    planned_addresses.push(key);
                    removed_addresses.retain(|candidate| candidate != &key);
                }
                Mutation::RemoveAddress(address) => {
                    let key = (address.interface_index, address.address);
                    let exists_in_system =
                        self.addresses()?.iter().any(|candidate| {
                            candidate.id == address.id
                                || (candidate.interface_index == address.interface_index
                                    && candidate.address == address.address)
                        }) && !removed_addresses.iter().any(|candidate| candidate == &key);
                    if !exists_in_system
                        && !planned_addresses.iter().any(|candidate| candidate == &key)
                    {
                        return Err(Error::NotFound);
                    }
                    planned_addresses.retain(|candidate| candidate != &key);
                    removed_addresses.push(key);
                }
                Mutation::SetDnsConfig(_) => {}
                _ => return Err(Error::Unsupported),
            }
        }
        Ok(())
    }

    fn same_route(left: &Route, right: &Route) -> bool {
        left.destination == right.destination
            && left.gateway == right.gateway
            && left.metric == right.metric
            && left.interface_index == right.interface_index
    }

    /// Captures the currently observed state relevant to one mutation.
    ///
    /// This performs only provider reads and is safe to call before an
    /// operation. The returned value is a point-in-time observation; callers
    /// must still handle concurrent changes before compensation.
    pub fn snapshot_for_mutation(&self, operation: &Mutation) -> Result<MutationSnapshot> {
        match operation {
            Mutation::AddRoute(route) | Mutation::RemoveRoute(route) => {
                let observed = self.routes()?.into_iter().find(|candidate| {
                    candidate.destination == route.destination
                        && candidate.gateway == route.gateway
                        && candidate.metric == route.metric
                        && candidate.interface_index == route.interface_index
                });
                Ok(MutationSnapshot::Route(observed))
            }
            Mutation::AddAddress(address) => {
                let interface_index = address.interface_id.value() as u32;
                let observed = self.addresses()?.into_iter().find(|candidate| {
                    candidate.interface_index == interface_index
                        && candidate.address == address.address
                });
                Ok(MutationSnapshot::InterfaceAddress(observed))
            }
            Mutation::RemoveAddress(address) => {
                Ok(MutationSnapshot::InterfaceAddress(Some(address.clone())))
            }
            Mutation::SetDnsConfig(_) => Ok(MutationSnapshot::Dns(self.dns_config()?)),
            _ => Err(Error::Unsupported),
        }
    }

    /// Executes an ordered mutation plan through this backend.
    ///
    /// The plan remains data-only; this method is the Stage 0.15 execution
    /// boundary. Operations are submitted in order and execution stops after
    /// the first error. The returned report always contains one outcome per
    /// plan operation. Failed operations carry their documented
    /// partial-application risk, and later operations are `NotAttempted`.
    /// Compensation is not inferred: until a caller supplies a prior-state
    /// snapshot, the report records [`RollbackStatus::NotAttempted`].
    pub fn execute_plan(
        &self,
        plan: &MutationPlan,
        options: &mut ExecutionOptions<'_>,
    ) -> MutationPlanReport {
        if let Err(error) = self.validate_plan(plan) {
            return executor::unsupported_plan_report(plan, error);
        }

        let mut outcomes = Vec::with_capacity(plan.len());
        let mut operation_reports = vec![MutationOperationReport::not_attempted(); plan.len()];
        let mut applied = Vec::new();
        let mut stopped = false;
        let mut rollback_boundary = false;

        for (index, operation) in plan.operations().iter().enumerate() {
            if stopped
                || options
                    .cancellation
                    .as_mut()
                    .is_some_and(|callback| callback(index, operation))
            {
                outcomes.push(MutationOutcome::NotAttempted);
                if !stopped {
                    operation_reports[index] = MutationOperationReport {
                        phase: MutationExecutionPhase::Cancellation,
                        duration: std::time::Duration::ZERO,
                        stop_reason: Some(MutationStopReason::Cancelled),
                    };
                }
                rollback_boundary |= !applied.is_empty();
                stopped = true;
                continue;
            }

            let prior = match options.snapshot.as_mut() {
                Some(snapshot) => {
                    let started = Instant::now();
                    match snapshot(index, operation) {
                        Ok(prior) => {
                            operation_reports[index].phase = MutationExecutionPhase::Snapshot;
                            operation_reports[index].duration = started.elapsed();
                            Some(prior)
                        }
                        Err(error) => {
                            outcomes.push(MutationOutcome::Failed {
                                error,
                                may_have_applied: false,
                            });
                            operation_reports[index] = MutationOperationReport {
                                phase: MutationExecutionPhase::Snapshot,
                                duration: started.elapsed(),
                                stop_reason: Some(MutationStopReason::SnapshotFailed),
                            };
                            stopped = true;
                            rollback_boundary = !applied.is_empty();
                            continue;
                        }
                    }
                }
                None => None,
            };

            let started = Instant::now();
            let result = match operation {
                Mutation::AddRoute(route) => self.add_route(route.clone()),
                Mutation::RemoveRoute(route) => self.remove_route(route.clone()),
                Mutation::AddAddress(address) => self.add_address(address.clone()).map(|_| ()),
                Mutation::RemoveAddress(address) => self.remove_address(address.clone()),
                Mutation::SetDnsConfig(config) => self.set_dns_config(config.clone()).map(|_| ()),
                _ => Err(Error::Unsupported),
            };

            match result {
                Ok(()) => {
                    outcomes.push(MutationOutcome::Applied);
                    operation_reports[index].phase = MutationExecutionPhase::Execution;
                    operation_reports[index].duration += started.elapsed();
                    applied.push((index, prior));
                }
                Err(error) => {
                    rollback_boundary =
                        !applied.is_empty() || operation.semantics().may_partially_apply;
                    outcomes.push(MutationOutcome::Failed {
                        error,
                        may_have_applied: operation.semantics().may_partially_apply,
                    });
                    operation_reports[index] = MutationOperationReport {
                        phase: MutationExecutionPhase::Execution,
                        duration: operation_reports[index].duration + started.elapsed(),
                        stop_reason: Some(MutationStopReason::ExecutionFailed),
                    };
                    stopped = true;
                }
            }
        }

        let rollback = if stopped && rollback_boundary {
            if applied.is_empty() {
                RollbackStatus::NotAttempted
            } else {
                let Some(compensate) = options.compensation.as_mut() else {
                    return MutationPlanReport::with_operation_reports(
                        outcomes,
                        RollbackStatus::NotAttempted,
                        operation_reports,
                    );
                };
                let mut status = RollbackStatus::Completed;
                for (index, prior) in applied.into_iter().rev() {
                    if let Err(error) =
                        compensate(index, plan.operation(index).expect("index"), prior.as_ref())
                    {
                        operation_reports[index].phase = MutationExecutionPhase::Compensation;
                        operation_reports[index].stop_reason =
                            Some(MutationStopReason::CompensationFailed);
                        status = RollbackStatus::Failed {
                            operation_index: index,
                            error,
                        };
                        break;
                    }
                }
                status
            }
        } else {
            RollbackStatus::NotNeeded
        };

        MutationPlanReport::with_operation_reports(outcomes, rollback, operation_reports)
    }

    /// The full set of runtime-dependent [`Capability`] flags the connected
    /// backend currently has available.
    pub fn capabilities(&self) -> Capability {
        self.backend.capabilities()
    }

    /// Whether the connected backend currently has `capability` available.
    /// Shorthand for `self.capabilities().contains(capability)`.
    pub fn supports(&self, capability: Capability) -> bool {
        self.backend.capabilities().contains(capability)
    }

    /// Subscribes to change notifications. See [`EventReceiver`] for how to
    /// consume the returned events. Prefer `recv`/`try_recv`/`recv_timeout`
    /// when errors must be handled explicitly; `Iterator` terminates on any
    /// receiver error.
    ///
    /// ```no_run
    /// use net_lattice::{Lattice, Result};
    ///
    /// fn main() -> Result<()> {
    ///     let lattice = Lattice::connect()?;
    ///     let events = lattice.watch()?;
    ///     loop {
    ///         println!("{:?}", events.recv()?);
    ///     }
    /// }
    /// ```
    pub fn watch(&self) -> Result<EventReceiver<Event>> {
        self.ensure_monitoring()?;
        self.backend.watch()
    }

    /// Subscribes to async change notifications selected by `filter`.
    ///
    /// This is the Stage 0.11 async watcher API. It has the same filter
    /// semantics as [`Self::watch_filtered`].
    ///
    /// ```no_run
    /// use futures::StreamExt;
    /// use net_lattice::{EventFilter, Lattice, Result};
    ///
    /// async fn monitor() -> Result<()> {
    ///     let lattice = Lattice::connect()?;
    ///     let mut events = lattice.watch_async(EventFilter::ALL)?;
    ///     while let Some(event) = events.next().await {
    ///         println!("{:?}", event?);
    ///     }
    ///     Ok(())
    /// }
    /// ```
    #[cfg(feature = "async")]
    pub fn watch_async(&self, filter: EventFilter) -> Result<EventStream<Event>>
    where
        B: TokioEventProvider<Event = Event, EventFilter = EventFilter>,
    {
        self.ensure_monitoring()?;
        Ok(net_lattice_async::from_tokio_receiver(
            self.backend.watch_tokio(filter)?,
        ))
    }

    /// Subscribes to change notifications selected by `filter`.
    pub fn watch_filtered(&self, filter: EventFilter) -> Result<EventReceiver<Event>> {
        self.ensure_monitoring()?;
        self.backend.watch_filtered(filter)
    }

    fn ensure_monitoring(&self) -> Result<()> {
        if self.supports(Capability::MONITORING) {
            Ok(())
        } else {
            Err(Error::Unsupported)
        }
    }
}

#[cfg(target_os = "linux")]
impl Lattice<net_lattice_backend_linux::LinuxBackend> {
    /// Connects using the default backend for the current platform.
    pub fn connect() -> Result<Self> {
        #[cfg(test)]
        if FORCE_CONNECT_FAILURE.swap(false, Ordering::SeqCst) {
            return Err(Error::Unsupported);
        }
        Ok(Self {
            backend: net_lattice_backend_linux::LinuxBackend::new()?,
        })
    }
}

#[cfg(target_os = "windows")]
impl Lattice<net_lattice_backend_windows::WindowsBackend> {
    /// Connects using the default backend for the current platform.
    pub fn connect() -> Result<Self> {
        #[cfg(test)]
        if FORCE_CONNECT_FAILURE.swap(false, Ordering::SeqCst) {
            return Err(Error::Unsupported);
        }
        Ok(Self {
            backend: net_lattice_backend_windows::WindowsBackend::new()?,
        })
    }
}

#[cfg(target_os = "macos")]
impl Lattice<net_lattice_backend_darwin::DarwinBackend> {
    /// Connects using the default backend for the current platform.
    pub fn connect() -> Result<Self> {
        #[cfg(test)]
        if FORCE_CONNECT_FAILURE.swap(false, Ordering::SeqCst) {
            return Err(Error::Unsupported);
        }
        Ok(Self {
            backend: net_lattice_backend_darwin::DarwinBackend::new()?,
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    struct TestBackend {
        capabilities: Capability,
        fail_events: bool,
        fail_mutations: bool,
    }

    fn network() -> Network {
        Network::from(Ipv4Network::new(
            Ipv4Address::new(192, 0, 2, 0),
            Ipv4PrefixLength::new(24).expect("valid prefix"),
        ))
    }

    fn route() -> Route {
        Route::new(RouteId::new(1), network()).with_interface_index(1)
    }

    fn planned_route() -> Route {
        route().with_metric(7)
    }

    impl RouteProvider for TestBackend {
        type Route = Route;

        fn routes(&self) -> Result<Vec<Self::Route>> {
            Ok(vec![route()])
        }

        fn add_route(&self, _route: Self::Route) -> Result<()> {
            if self.fail_mutations {
                Err(Error::InvalidState)
            } else {
                Ok(())
            }
        }

        fn remove_route(&self, _route: Self::Route) -> Result<()> {
            if self.fail_mutations {
                Err(Error::InvalidState)
            } else {
                Ok(())
            }
        }
    }

    impl InterfaceProvider for TestBackend {
        type Interface = Interface;

        fn interfaces(&self) -> Result<Vec<Self::Interface>> {
            Ok(vec![Interface::new(
                InterfaceId::new(1),
                1,
                "test0",
                InterfaceKind::Ethernet,
            )])
        }
    }

    impl DnsProvider for TestBackend {
        type DnsConfig = DnsConfig;

        fn dns_config(&self) -> Result<Self::DnsConfig> {
            Ok(DnsConfig::new())
        }
    }

    impl DnsMutator for TestBackend {
        type NewDnsConfig = NewDnsConfig;

        fn set_dns_config(&self, _config: Self::NewDnsConfig) -> Result<Self::DnsConfig> {
            if self.fail_mutations {
                Err(Error::InvalidState)
            } else {
                Ok(DnsConfig::new())
            }
        }
    }

    impl NeighborProvider for TestBackend {
        type NeighborEntry = NeighborEntry;

        fn neighbors(&self) -> Result<Vec<Self::NeighborEntry>> {
            Ok(vec![NeighborEntry::new(
                NeighborId::new(1),
                1,
                IpAddress::from(Ipv4Address::new(192, 0, 2, 1)),
            )])
        }
    }

    impl AddressProvider for TestBackend {
        type InterfaceAddress = InterfaceAddress;

        fn addresses(&self) -> Result<Vec<Self::InterfaceAddress>> {
            Ok(vec![InterfaceAddress::new(
                InterfaceAddressId::new(1),
                1,
                network(),
            )])
        }
    }

    impl AddressMutator for TestBackend {
        type NewInterfaceAddress = NewInterfaceAddress;
        type InterfaceAddress = InterfaceAddress;

        fn add_address(
            &self,
            address: Self::NewInterfaceAddress,
        ) -> Result<Self::InterfaceAddress> {
            if self.fail_mutations {
                Err(Error::InvalidState)
            } else {
                Ok(InterfaceAddress::new(
                    InterfaceAddressId::new(1),
                    address.interface_id.value() as u32,
                    address.address,
                ))
            }
        }

        fn remove_address(&self, _address: Self::InterfaceAddress) -> Result<()> {
            if self.fail_mutations {
                Err(Error::InvalidState)
            } else {
                Ok(())
            }
        }
    }

    impl CapabilityProvider for TestBackend {
        fn capabilities(&self) -> Capability {
            self.capabilities
        }
    }

    impl EventProvider for TestBackend {
        type Event = Event;
        type EventFilter = EventFilter;

        fn watch(&self) -> Result<EventReceiver<Self::Event>> {
            self.watch_filtered(EventFilter::ALL)
        }

        fn watch_filtered(&self, filter: Self::EventFilter) -> Result<EventReceiver<Self::Event>> {
            if self.fail_events {
                return Err(Error::InvalidState);
            }
            let (sender, receiver) = EventReceiver::bounded();
            let event = Event::Route {
                id: RouteId::new(1),
                kind: ChangeKind::Added,
            };
            if filter.matches(event) {
                assert!(sender.send(event, Event::resync_all()));
                Ok(receiver)
            } else {
                // Keep an empty filtered watcher connected, matching a real
                // subscription that remains active while it waits for a
                // matching event.
                Ok(receiver.with_subscription(sender))
            }
        }
    }

    #[cfg(feature = "async")]
    impl TokioEventProvider for TestBackend {
        type Event = Event;
        type EventFilter = EventFilter;

        fn watch_tokio(
            &self,
            filter: Self::EventFilter,
        ) -> Result<net_lattice_platform::TokioEventReceiver<Self::Event>> {
            if self.fail_events {
                return Err(Error::InvalidState);
            }
            let (sender, receiver) = net_lattice_platform::TokioEventReceiver::bounded();
            let event = Event::Route {
                id: RouteId::new(1),
                kind: ChangeKind::Added,
            };
            if filter.matches(event) {
                assert!(sender.send(event, Event::resync_all));
                Ok(receiver)
            } else {
                Ok(receiver.with_subscription(sender))
            }
        }
    }

    fn lattice(capabilities: Capability) -> Lattice<TestBackend> {
        Lattice {
            backend: TestBackend {
                capabilities,
                fail_events: false,
                fail_mutations: false,
            },
        }
    }

    #[test]
    fn facade_forwards_all_read_and_mutation_operations() {
        let lattice = lattice(Capability::MONITORING | Capability::DNS_MUTATION);
        let route = route();
        let address = NewInterfaceAddress::new(InterfaceId::new(1), network());

        assert_eq!(lattice.routes().expect("routes").len(), 1);
        lattice.add_route(route.clone()).expect("add route");
        lattice.remove_route(route).expect("remove route");
        assert_eq!(lattice.interfaces().expect("interfaces").len(), 1);
        assert_eq!(lattice.dns_config().expect("dns").nameservers.len(), 0);
        assert_eq!(lattice.neighbors().expect("neighbors").len(), 1);
        assert_eq!(lattice.addresses().expect("addresses").len(), 1);
        let observed = lattice.add_address(address).expect("add address");
        lattice.remove_address(observed).expect("remove address");
        assert_eq!(
            lattice
                .set_dns_config(NewDnsConfig::new())
                .expect("set DNS")
                .search_domains
                .len(),
            0
        );
    }

    #[test]
    fn facade_executes_ordered_plan_and_preserves_report_indices() {
        let lattice = lattice(Capability::DNS_MUTATION);
        let plan = MutationPlan::from_operations([
            Mutation::AddRoute(planned_route()),
            Mutation::SetDnsConfig(NewDnsConfig::new()),
        ]);

        let mut options = ExecutionOptions::default();
        let report = lattice.execute_plan(&plan, &mut options);

        assert_eq!(report.len(), plan.len());
        assert!(report.is_success());
        assert_eq!(report.applied_count(), 2);
        assert!(matches!(report.outcome(0), Some(MutationOutcome::Applied)));
        assert!(matches!(report.outcome(1), Some(MutationOutcome::Applied)));
        assert!(matches!(report.rollback(), RollbackStatus::NotNeeded));
        assert_eq!(report.operation_reports().len(), plan.len());
        assert!(matches!(
            report.operation_reports()[0].phase,
            MutationExecutionPhase::Execution
        ));
    }

    #[test]
    fn facade_cancellation_stops_at_an_operation_boundary() {
        let lattice = lattice(Capability::empty());
        let plan = MutationPlan::from_operations([
            Mutation::AddRoute(planned_route()),
            Mutation::RemoveRoute(planned_route()),
        ]);

        let mut cancelled = |index, _: &Mutation| index == 1;
        let mut options = ExecutionOptions::default().cancellation(&mut cancelled);
        let report = lattice.execute_plan(&plan, &mut options);

        assert_eq!(report.applied_count(), 1);
        assert_eq!(report.not_attempted_count(), 1);
        assert!(matches!(report.outcome(0), Some(MutationOutcome::Applied)));
        assert!(matches!(
            report.outcome(1),
            Some(MutationOutcome::NotAttempted)
        ));
        assert!(matches!(
            report.operation_reports()[1].stop_reason,
            Some(MutationStopReason::Cancelled)
        ));
        assert!(matches!(report.rollback(), RollbackStatus::NotAttempted));
    }

    #[test]
    /// Contract test: a failing resolver write is reported as potentially
    /// partially applied without touching the host resolver configuration.
    fn facade_reports_partial_application_boundary_on_failed_dns_operation() {
        let lattice = Lattice {
            backend: TestBackend {
                capabilities: Capability::DNS_MUTATION,
                fail_events: false,
                fail_mutations: true,
            },
        };
        let plan = MutationPlan::from_operations([Mutation::SetDnsConfig(NewDnsConfig::new())]);

        let mut options = ExecutionOptions::default();
        let report = lattice.execute_plan(&plan, &mut options);

        assert!(matches!(
            report.outcome(0),
            Some(MutationOutcome::Failed {
                may_have_applied: true,
                ..
            })
        ));
        assert!(matches!(report.rollback(), RollbackStatus::NotAttempted));
        assert!(matches!(
            report.operation_reports()[0].stop_reason,
            Some(MutationStopReason::ExecutionFailed)
        ));
    }

    #[test]
    fn facade_rejects_unsupported_capability_before_submitting_a_plan() {
        let lattice = lattice(Capability::empty());
        let plan = MutationPlan::from_operations([
            Mutation::SetDnsConfig(NewDnsConfig::new()),
            Mutation::AddRoute(planned_route()),
        ]);

        assert!(matches!(
            lattice.validate_plan(&plan),
            Err(Error::Unsupported)
        ));
        let mut options = ExecutionOptions::default();
        let report = lattice.execute_plan(&plan, &mut options);
        assert!(matches!(
            report.outcome(0),
            Some(MutationOutcome::Failed {
                error: Error::Unsupported,
                may_have_applied: false,
            })
        ));
        assert!(matches!(
            report.outcome(1),
            Some(MutationOutcome::NotAttempted)
        ));
        assert!(matches!(
            report.operation_reports()[0].stop_reason,
            Some(MutationStopReason::ValidationFailed)
        ));
    }

    #[test]
    fn facade_validates_route_and_address_preconditions() {
        let lattice = lattice(Capability::empty());
        assert!(matches!(
            lattice.validate_plan(&MutationPlan::from_operations([
                Mutation::AddRoute(route())
            ])),
            Err(Error::AlreadyExists)
        ));
        assert!(matches!(
            lattice.validate_plan(&MutationPlan::from_operations([Mutation::RemoveRoute(
                planned_route()
            )])),
            Err(Error::NotFound)
        ));
        assert!(matches!(
            lattice.validate_plan(&MutationPlan::from_operations([Mutation::AddAddress(
                NewInterfaceAddress::new(InterfaceId::new(99), network())
            )])),
            Err(Error::NotFound)
        ));
        assert!(matches!(
            lattice.validate_plan(&MutationPlan::from_operations([Mutation::RemoveAddress(
                InterfaceAddress::new(InterfaceAddressId::new(99), 99, network())
            )])),
            Err(Error::NotFound)
        ));
    }

    #[test]
    fn facade_captures_native_snapshots_for_each_mutation_domain() {
        let lattice = lattice(Capability::DNS_MUTATION);
        let route = route();
        let address = NewInterfaceAddress::new(InterfaceId::new(1), network());
        let observed_address = InterfaceAddress::new(InterfaceAddressId::new(1), 1, network());

        assert!(matches!(
            lattice.snapshot_for_mutation(&Mutation::AddRoute(route.clone())),
            Ok(MutationSnapshot::Route(Some(_)))
        ));
        assert!(matches!(
            lattice.snapshot_for_mutation(&Mutation::AddAddress(address)),
            Ok(MutationSnapshot::InterfaceAddress(Some(_)))
        ));
        assert!(matches!(
            lattice.snapshot_for_mutation(&Mutation::RemoveAddress(observed_address)),
            Ok(MutationSnapshot::InterfaceAddress(Some(_)))
        ));
        assert!(matches!(
            lattice.snapshot_for_mutation(&Mutation::SetDnsConfig(NewDnsConfig::new())),
            Ok(MutationSnapshot::Dns(_))
        ));
    }

    /// Exercises the complete facade transaction path against the native
    /// backend. This is intentionally ignored because route mutation requires
    /// root/CAP_NET_ADMIN/Administrator and changes the host routing table.
    #[cfg(any(target_os = "linux", target_os = "windows", target_os = "macos"))]
    #[test]
    #[ignore = "requires native networking privilege; run with the platform privileged test job"]
    fn native_facade_route_transaction_round_trip() {
        let lattice = Lattice::connect().expect("failed to connect native backend");
        let interface = lattice
            .interfaces()
            .expect("failed to list interfaces")
            .into_iter()
            .find(|interface| matches!(interface.kind, InterfaceKind::Loopback))
            .or_else(|| {
                lattice
                    .interfaces()
                    .ok()
                    .and_then(|mut interfaces| interfaces.pop())
            })
            .expect("native backend reported no interfaces");
        let destination = Network::from(Ipv4Network::new(
            Ipv4Address::new(203, 0, 113, 0),
            Ipv4PrefixLength::new(24).expect("valid prefix"),
        ));
        let route = Route::new(RouteId::new(0), destination).with_interface_index(interface.index);

        let add_plan = MutationPlan::from_operations([Mutation::AddRoute(route.clone())]);
        let mut snapshot = |_, operation: &Mutation| lattice.snapshot_for_mutation(operation);
        let mut options = ExecutionOptions::default().snapshot(&mut snapshot);
        let add_report = lattice.execute_plan(&add_plan, &mut options);

        let observed_route = lattice
            .routes()
            .expect("failed to read route after adding it")
            .into_iter()
            .find(|candidate| {
                candidate.destination == route.destination
                    && candidate.interface_index == route.interface_index
            })
            .expect("added route was not observed");
        let remove_plan = MutationPlan::from_operations([Mutation::RemoveRoute(observed_route)]);
        let mut options = ExecutionOptions::default();
        let remove_report = lattice.execute_plan(&remove_plan, &mut options);
        assert!(add_report.is_success(), "route add report: {add_report:?}");
        assert!(
            remove_report.is_success(),
            "route remove report: {remove_report:?}"
        );
    }

    /// Exercises native first-failure stopping and reverse-order compensation
    /// without leaving the test route behind.
    #[cfg(any(target_os = "linux", target_os = "windows", target_os = "macos"))]
    #[test]
    #[ignore = "requires native networking privilege; run with the platform privileged test job"]
    fn native_facade_compensates_after_second_route_operation_fails() {
        let lattice = Lattice::connect().expect("failed to connect native backend");
        let interface = lattice
            .interfaces()
            .expect("failed to list interfaces")
            .into_iter()
            .find(|interface| matches!(interface.kind, InterfaceKind::Loopback))
            .or_else(|| {
                lattice
                    .interfaces()
                    .ok()
                    .and_then(|mut interfaces| interfaces.pop())
            })
            .expect("native backend reported no interfaces");
        let destination = Network::from(Ipv4Network::new(
            Ipv4Address::new(198, 51, 100, 0),
            Ipv4PrefixLength::new(24).expect("valid prefix"),
        ));
        let route = Route::new(RouteId::new(0), destination).with_interface_index(interface.index);
        let failed_route = Route::new(
            RouteId::new(0),
            Network::from(Ipv4Network::new(
                Ipv4Address::new(198, 51, 101, 0),
                Ipv4PrefixLength::new(24).expect("valid prefix"),
            )),
        )
        .with_interface_index(u32::MAX);
        let plan = MutationPlan::from_operations([
            Mutation::AddRoute(route.clone()),
            Mutation::AddRoute(failed_route),
        ]);

        let mut snapshot = |_, operation: &Mutation| lattice.snapshot_for_mutation(operation);
        let mut compensate = |_, operation: &Mutation, _: Option<&MutationSnapshot>| match operation
        {
            Mutation::AddRoute(route) => lattice.remove_route(route.clone()),
            _ => Ok(()),
        };
        let mut options = ExecutionOptions::default()
            .snapshot(&mut snapshot)
            .compensation(&mut compensate);
        let report = lattice.execute_plan(&plan, &mut options);

        assert!(matches!(report.outcome(0), Some(MutationOutcome::Applied)));
        assert!(matches!(
            report.outcome(1),
            Some(MutationOutcome::Failed { .. })
        ));
        assert!(matches!(report.rollback(), RollbackStatus::Completed));
    }

    #[test]
    fn facade_runs_supplied_compensation_in_reverse_order() {
        let lattice = lattice(Capability::empty());
        let plan = MutationPlan::from_operations([
            Mutation::AddRoute(planned_route()),
            Mutation::RemoveRoute(planned_route()),
        ]);
        let mut compensated = Vec::new();

        let mut cancelled = |index, _: &Mutation| index == 1;
        let mut compensate = |index, _: &Mutation, _: Option<&MutationSnapshot>| {
            compensated.push(index);
            Ok(())
        };
        let mut options = ExecutionOptions::default()
            .cancellation(&mut cancelled)
            .compensation(&mut compensate);
        let report = lattice.execute_plan(&plan, &mut options);

        assert_eq!(compensated, vec![0]);
        assert!(matches!(report.rollback(), RollbackStatus::Completed));
    }

    #[test]
    fn facade_captures_prior_state_before_each_applied_operation() {
        let lattice = lattice(Capability::empty());
        let plan = MutationPlan::from_operations([
            Mutation::AddRoute(planned_route()),
            Mutation::RemoveRoute(planned_route()),
        ]);
        let mut captured = Vec::new();
        let mut restored = Vec::new();

        let mut cancelled = |index, _: &Mutation| index == 1;
        let mut snapshot = |index, _: &Mutation| {
            captured.push(index);
            Ok(MutationSnapshot::Dns(DnsConfig::default()))
        };
        let mut compensate = |index, _: &Mutation, state: Option<&MutationSnapshot>| {
            restored.push((index, state.is_some()));
            Ok(())
        };
        let mut options = ExecutionOptions::default()
            .cancellation(&mut cancelled)
            .snapshot(&mut snapshot)
            .compensation(&mut compensate);
        let report = lattice.execute_plan(&plan, &mut options);

        assert_eq!(captured, vec![0]);
        assert_eq!(restored, vec![(0, true)]);
        assert!(matches!(report.rollback(), RollbackStatus::Completed));
    }

    #[test]
    fn facade_reports_compensation_failure() {
        let lattice = lattice(Capability::empty());
        let plan = MutationPlan::from_operations([
            Mutation::AddRoute(planned_route()),
            Mutation::RemoveRoute(planned_route()),
        ]);

        let mut cancelled = |index, _: &Mutation| index == 1;
        let mut compensate =
            |_, _: &Mutation, _: Option<&MutationSnapshot>| Err(Error::InvalidState);
        let mut options = ExecutionOptions::default()
            .cancellation(&mut cancelled)
            .compensation(&mut compensate);
        let report = lattice.execute_plan(&plan, &mut options);

        assert!(matches!(
            report.rollback(),
            RollbackStatus::Failed {
                operation_index: 0,
                error: Error::InvalidState,
            }
        ));
        assert!(matches!(
            report.operation_reports()[0].stop_reason,
            Some(MutationStopReason::CompensationFailed)
        ));
    }

    #[test]
    fn facade_enforces_monitoring_capability_and_forwards_filters() {
        let unsupported = lattice(Capability::empty());
        assert!(unsupported.watch().is_err());
        assert!(unsupported.watch_filtered(EventFilter::ALL).is_err());

        let lattice = lattice(Capability::MONITORING);
        assert!(lattice.supports(Capability::MONITORING));
        assert!(!lattice.supports(Capability::DNS_MUTATION));
        assert!(lattice.capabilities().contains(Capability::MONITORING));
        assert!(lattice.watch().expect("watch").recv().is_ok());
        assert!(
            lattice
                .watch_filtered(EventFilter::none().route(RouteId::new(1)))
                .expect("filtered watch")
                .recv()
                .is_ok()
        );
        assert!(
            lattice
                .watch_filtered(EventFilter::none())
                .expect("empty filtered watch")
                .try_recv()
                .unwrap()
                .is_none()
        );
    }

    #[test]
    fn facade_propagates_backend_watcher_errors() {
        let lattice = Lattice {
            backend: TestBackend {
                capabilities: Capability::MONITORING,
                fail_events: true,
                fail_mutations: false,
            },
        };
        assert!(lattice.watch().is_err());
        assert!(lattice.watch_filtered(EventFilter::ALL).is_err());
    }

    #[cfg(feature = "async")]
    #[test]
    fn async_facade_propagates_native_watcher_errors() {
        let lattice = Lattice {
            backend: TestBackend {
                capabilities: Capability::MONITORING,
                fail_events: true,
                fail_mutations: false,
            },
        };
        assert!(lattice.watch_async(EventFilter::ALL).is_err());
    }

    #[cfg(feature = "async")]
    #[test]
    fn async_facade_enforces_monitoring_capability() {
        let lattice = lattice(Capability::empty());
        assert!(lattice.watch_async(EventFilter::ALL).is_err());
    }

    #[cfg(feature = "async")]
    #[test]
    fn async_facade_uses_the_backend_native_watcher_contract() {
        use futures::{FutureExt, StreamExt};

        futures::executor::block_on(async {
            let lattice = lattice(Capability::MONITORING);
            let mut events = lattice
                .watch_async(EventFilter::none().route(RouteId::new(1)))
                .expect("async watch");
            assert!(events.next().await.is_some());
            assert!(events.next().await.is_none());

            let mut events = lattice
                .watch_async(EventFilter::none())
                .expect("empty async watch");
            assert!(events.next().now_or_never().is_none());
        });
    }

    #[test]
    fn connect_uses_the_current_platform_backend() {
        let _ = Lattice::connect();
    }

    #[test]
    fn connect_propagates_backend_construction_error() {
        FORCE_CONNECT_FAILURE.store(true, Ordering::SeqCst);
        assert!(Lattice::connect().is_err());
    }
}
