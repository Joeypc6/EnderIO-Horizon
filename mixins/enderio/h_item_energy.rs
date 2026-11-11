fn enderio_item_energy(item: &str, buffer: &mut Option<crate::game::native_item_energy::ItemEnergyBuffer>) {
    crate::game::enderio::item_energy_buffer(item, buffer);
}
