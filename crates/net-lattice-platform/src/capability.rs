bitflags::bitflags! {
    /// Runtime-dependent operating system features that cannot be expressed
    /// through Rust trait implementation alone.
    ///
    /// Distinct from provider traits: a backend either implements
    /// `RouteProvider` or it doesn't, and that's fixed at compile time.
    /// Whether the *running* kernel has, say, VRF support enabled is a fact
    /// about the current machine, not the crate — see ARCHITECTURE.md's
    /// `net-lattice-platform` section.
    ///
    /// Capabilities describe the backend's view when queried; callers should
    /// still handle operation errors because permissions or system
    /// configuration may change later. Use them as a portable feature gate,
    /// for example:
    ///
    /// ```ignore
    /// if lattice.supports(Capability::MONITORING) {
    ///     let watcher = lattice.watch()?;
    /// }
    /// ```
    #[derive(Debug, Clone, Copy, PartialEq, Eq)]
    pub struct Capability: u64 {
        const IPV6 = 1 << 0;
        const VRF = 1 << 1;
        const NAMESPACES = 1 << 2;
        const MONITORING = 1 << 3;
        /// The backend can replace resolver configuration through a supported
        /// operating-system mechanism.
        const DNS_MUTATION = 1 << 4;
        /// The backend can request an interface's administrative up/down
        /// state through a supported operating-system mechanism.
        ///
        /// This is a feature gate, not proof that the current process has the
        /// privilege or policy permission to change the interface.
        const INTERFACE_ADMIN_STATE = 1 << 5;
        /// The backend can request an interface MTU through a supported
        /// operating-system mechanism.
        ///
        /// This is a feature gate, not proof that the current process has the
        /// privilege or policy permission to change the interface.
        const INTERFACE_MTU = 1 << 6;
    }
}

/// Reports which runtime-dependent [`Capability`] flags the connected
/// backend currently has available.
///
/// Every backend implements this (it costs nothing when there's nothing to
/// report — an empty flag set is a valid answer), which is why `capabilities`
/// returns a bare `Capability` rather than `Result<Capability>`: unlike the
/// other provider traits, there is no OS call here that can fail in a way
/// worth surfacing to the caller. `addresses`-style methods that really do
/// call into the OS keep returning `Result`.
pub trait CapabilityProvider {
    fn capabilities(&self) -> Capability;
}

#[cfg(test)]
mod tests {
    use super::Capability;

    #[test]
    fn interface_configuration_capabilities_are_distinct_feature_gates() {
        assert_ne!(
            Capability::INTERFACE_ADMIN_STATE.bits(),
            Capability::INTERFACE_MTU.bits()
        );
        assert!(!Capability::INTERFACE_ADMIN_STATE.contains(Capability::INTERFACE_MTU));
    }
}
