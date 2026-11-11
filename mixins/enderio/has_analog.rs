fn enderio_has_analog(level: &crate::game::GameLevel<'_>, state: horizon_world::registry::BlockStateId, output: &mut bool) {
    if crate::game::enderio::is_bank(&level.facts.get(state).name) || matches!(&*level.facts.get(state).name, "enderio:fluid_tank" | "enderio:pressurized_fluid_tank") {
        *output = true;
    }
}
