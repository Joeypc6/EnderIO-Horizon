fn enderio_accepts(game: &crate::game::Game<'_>, entity: &horizon_client::containers::ModBlockEntity, slot: usize, stack: &horizon_client::items::ItemStack, face: Option<horizon_core::geometry::Direction>, cancel: &mut bool) {
    crate::game::enderio::container_accepts(game, entity, slot, stack, face, cancel);
}
