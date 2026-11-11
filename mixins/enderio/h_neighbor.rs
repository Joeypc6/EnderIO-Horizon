fn enderio_neighbor(level: &GameLevel<'_>, pos: BlockPos, state: BlockStateId) {
    crate::game::enderio::neighbor_changed(&level.facts.get(state).name, [pos.x, pos.y, pos.z]);
}
