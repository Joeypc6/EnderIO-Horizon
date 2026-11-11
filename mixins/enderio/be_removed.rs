// A machine's items stay on its dropped item: none spill when its block entity leaves the world.
fn enderio_be_removed(level: &mut crate::game::GameLevel<'_>, pos: horizon_core::position::BlockPos) {
    crate::game::enderio::block_entity_removed(level, [pos.x, pos.y, pos.z]);
}
