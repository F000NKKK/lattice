use net_lattice_core::Result;

/// Assembles a whole-system snapshot of a backend's observed state.
///
/// Generic over an associated `State` type rather than naming
/// `net_lattice_model::snapshot::CurrentState` directly — `net-lattice-platform`
/// does not depend on `net-lattice-model` (see ARCHITECTURE.md). The facade
/// crate (`net-lattice`) is what constrains `State` to the concrete
/// `CurrentState` model type, via a blanket implementation over any backend
/// that already implements [`RouteProvider`](crate::RouteProvider),
/// [`InterfaceProvider`](crate::InterfaceProvider),
/// [`NeighborProvider`](crate::NeighborProvider),
/// [`AddressProvider`](crate::AddressProvider), and
/// [`DnsProvider`](crate::DnsProvider) with matching associated types.
/// `net-lattice-platform` cannot supply that assembly itself: it would need
/// to construct `CurrentState`, a type it cannot name. No backend crate is
/// expected to implement this trait by hand.
///
/// # Consistency
///
/// A snapshot is not atomic across domains: an implementation is expected to
/// perform its constituent reads sequentially, with no lock or transaction
/// spanning them, the same as every other multi-read path in this crate. Do
/// not assume two fields of the returned state were observed at the same
/// instant.
///
/// # Fail-fast partial-read semantics
///
/// If any constituent read fails, [`snapshot`](SnapshotProvider::snapshot)
/// returns that error and no state at all — never a partially populated
/// state. This matches every other provider read in this crate
/// (`Result<Vec<T>>`, all-or-nothing): there is no per-field `Result`/`Option`
/// escape hatch. A caller that wants best-effort partial data should call the
/// underlying individual provider reads directly instead of `snapshot`.
pub trait SnapshotProvider {
    /// The assembled state type returned by [`snapshot`](Self::snapshot).
    type State;

    /// Assembles and returns the current state, or the first error
    /// encountered while doing so.
    fn snapshot(&self) -> Result<Self::State>;
}

#[cfg(test)]
mod tests {
    use super::SnapshotProvider;
    use net_lattice_core::{Error, Result};

    struct Backend {
        state: Result<u32>,
    }

    impl SnapshotProvider for Backend {
        type State = u32;

        fn snapshot(&self) -> Result<Self::State> {
            match &self.state {
                Ok(state) => Ok(*state),
                Err(Error::NotFound) => Err(Error::NotFound),
                Err(_) => Err(Error::Unsupported),
            }
        }
    }

    #[test]
    fn snapshot_returns_the_assembled_associated_type() {
        let backend = Backend { state: Ok(7) };
        assert_eq!(backend.snapshot().expect("snapshot"), 7);
    }

    #[test]
    fn snapshot_propagates_the_first_error_instead_of_a_partial_state() {
        let backend = Backend {
            state: Err(Error::NotFound),
        };
        assert!(matches!(backend.snapshot(), Err(Error::NotFound)));
    }
}
