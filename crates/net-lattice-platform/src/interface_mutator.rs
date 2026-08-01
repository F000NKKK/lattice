use net_lattice_core::Result;

use crate::InterfaceProvider;

/// Applies a partial configuration patch to one network interface.
///
/// This is an official third-party backend extension contract. It is separate
/// from [`InterfaceProvider`] because enumerating interfaces is normally
/// unprivileged while changing administrative state or MTU requires elevated
/// operating-system networking privilege. A successful implementation must
/// return a fresh observed [`InterfaceProvider::Interface`] after native
/// submission; a native acknowledgement alone is not sufficient.
///
/// Backends may need distinct native writes for MTU and administrative state.
/// Consequently, a failed combined patch can have partially applied. Callers
/// must re-read observed state after errors and use explicit transaction-layer
/// compensation when they need an attempted restoration.
pub trait InterfaceMutator: InterfaceProvider {
    /// The partial desired interface configuration accepted by this backend.
    type InterfaceConfig;

    /// Applies `config` and returns the resulting observed interface.
    fn set_interface_config(&self, config: Self::InterfaceConfig) -> Result<Self::Interface>;
}

#[cfg(test)]
mod tests {
    use super::InterfaceMutator;
    use crate::InterfaceProvider;
    use net_lattice_core::Result;

    struct Backend;

    impl InterfaceProvider for Backend {
        type Interface = u32;

        fn interfaces(&self) -> Result<Vec<Self::Interface>> {
            Ok(Vec::new())
        }
    }

    impl InterfaceMutator for Backend {
        type InterfaceConfig = u32;

        fn set_interface_config(&self, config: Self::InterfaceConfig) -> Result<Self::Interface> {
            Ok(config)
        }
    }

    #[test]
    fn interface_mutator_returns_the_observed_associated_type() {
        assert_eq!(Backend.set_interface_config(7).expect("configured"), 7);
    }
}
