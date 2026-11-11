fn enderio_signal(level: &crate::game::GameLevel<'_>, dimension: crate::game::LevelDimension, pos: horizon_core::position::BlockPos, state: horizon_world::registry::BlockStateId, face: horizon_core::geometry::Direction, direct: bool, signal: &mut i32) {
    crate::game::enderio_conduits::block_signal_at(&level.facts.get(state).name, || format!("{dimension:?}"), [pos.x, pos.y, pos.z], face, direct, signal);
}
