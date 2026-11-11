fn enderio_be_energy(block: &str, energy: &mut Option<horizon_client::containers::ModEnergyBuffer>) {
    crate::game::enderio::block_entity_energy(block, energy);
}
