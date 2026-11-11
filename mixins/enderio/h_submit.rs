fn enderio_submit(game: &crate::game::Game<'_>, pos: [i32; 3], block: &str, entity: &horizon_client::containers::ModBlockEntity, draw: &mut crate::game::native_block_entity_draw::BlockEntityDraw) {
    crate::game::enderio_conduits::submit(game, pos, block, entity, draw);
}
