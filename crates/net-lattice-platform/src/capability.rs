bitflags::bitflags! {
    /// Runtime-dependent operating system features that cannot be expressed
    /// through Rust trait implementation alone.
    ///
    /// Distinct from provider traits: a backend either implements
    /// `RouteProvider` or it doesn't, and that's fixed at compile time.
    /// Whether the *running* kernel has, say, VRF support enabled is a fact
    /// about the current machine, not the crate — see ARCHITECTURE.md's
    /// `net-lattice-platform` section.
    #[derive(Debug, Clone, Copy, PartialEq, Eq)]
    pub struct Capability: u64 {
        const IPV6 = 1 << 0;
        const VRF = 1 << 1;
        const NAMESPACES = 1 << 2;
        const MONITORING = 1 << 3;
    }
}
