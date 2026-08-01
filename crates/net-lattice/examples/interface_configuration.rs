//! Apply an explicit administrative-state and/or MTU patch to one interface.
//!
//! This example does not change networking unless all required environment
//! variables are supplied and `NET_LATTICE_APPLY_INTERFACE_CONFIG=1` is set.
//! It operates on real host state and does not guess a rollback. Capture the
//! current [`net_lattice::Interface`] and use an explicit `MutationPlan`
//! compensator if an application needs an attempted restoration.

use net_lattice::{Capability, DesiredAdminState, Error, InterfaceConfig, Lattice, Result};

fn requested_admin_state() -> Result<Option<DesiredAdminState>> {
    match std::env::var("NET_LATTICE_INTERFACE_ADMIN_STATE")
        .ok()
        .as_deref()
    {
        None => Ok(None),
        Some("up") => Ok(Some(DesiredAdminState::Up)),
        Some("down") => Ok(Some(DesiredAdminState::Down)),
        Some(_) => Err(Error::InvalidState),
    }
}

fn main() -> Result<()> {
    if std::env::var("NET_LATTICE_APPLY_INTERFACE_CONFIG").as_deref() != Ok("1") {
        eprintln!(
            "set NET_LATTICE_APPLY_INTERFACE_CONFIG=1, NET_LATTICE_INTERFACE_INDEX, and at least one requested setting"
        );
        return Ok(());
    }

    let index = std::env::var("NET_LATTICE_INTERFACE_INDEX")
        .ok()
        .and_then(|value| value.parse::<u32>().ok())
        .ok_or(Error::InvalidState)?;
    let admin_state = requested_admin_state()?;
    let mtu = std::env::var("NET_LATTICE_INTERFACE_MTU")
        .ok()
        .map(|value| value.parse::<u32>().map_err(|_| Error::InvalidState))
        .transpose()?;

    let lattice = Lattice::connect()?;
    let interface = lattice
        .interfaces()?
        .into_iter()
        .find(|interface| interface.index == index)
        .ok_or(Error::NotFound)?;

    if admin_state.is_some() && !lattice.supports(Capability::INTERFACE_ADMIN_STATE) {
        return Err(Error::Unsupported);
    }
    if mtu.is_some() && !lattice.supports(Capability::INTERFACE_MTU) {
        return Err(Error::Unsupported);
    }

    let patch = InterfaceConfig::new(interface.id, admin_state, mtu)?;
    let observed = lattice.set_interface_config(patch)?;
    println!("observed after configuration: {observed:?}");
    Ok(())
}
