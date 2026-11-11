// The energy of an Ender IO block as a face reaches it.
fn enderio_energy(game: &crate::game::Game<'_>, dimension: &str, pos: [i32; 3], face: Direction) -> core::ops::ControlFlow<Option<NativeEnergy>> {
    match crate::game::enderio::energy_view(game, dimension, pos, face) {
        None => core::ops::ControlFlow::Continue(()),
        Some(v) => core::ops::ControlFlow::Break(v.map(|(amount, capacity, max_insert, max_extract, can_insert, can_extract)| NativeEnergy { amount, capacity, max_insert, max_extract, can_insert, can_extract })),
    }
}
