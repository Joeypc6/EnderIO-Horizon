fn enderio_signal_source(level: &crate::game::GameLevel<'_>, state: horizon_world::registry::BlockStateId, source: &mut bool) {
    crate::game::enderio_conduits::is_signal_source(&level.facts.get(state).name, source);
}
