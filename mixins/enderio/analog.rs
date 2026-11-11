// TileCapBank.getComparatorInputOverride: this block's own energy; TileTank.getComparatorOutput: its fluid.
fn enderio_analog(level: &crate::game::GameLevel<'_>, pos: horizon_core::position::BlockPos, state: horizon_world::registry::BlockStateId, output: &mut i32) {
    if !level.facts.get(state).name.starts_with("enderio:") {
        return;
    }
    if let Some(horizon_client::containers::BlockEntity::Mod(m)) = level.world.block_entities.get(&(pos.x, pos.y, pos.z))
        && let Some(v) = crate::game::enderio::comparator(m).or_else(|| crate::game::enderio_machines::tank_comparator(m))
    {
        *output = v;
    }
}
