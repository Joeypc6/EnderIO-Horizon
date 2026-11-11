// Energy into an Ender IO block: the intake per call and per face per tick, the IO modes, the bank's network.
fn enderio_energy_transfer(game: &mut crate::game::Game<'_>, dimension: &str, pos: [i32; 3], face: Direction, amount: i32, insert: bool, simulate: bool) -> core::ops::ControlFlow<i32> {
    match crate::game::enderio::energy_transfer(game, dimension, pos, face, amount, insert, simulate) {
        None => core::ops::ControlFlow::Continue(()),
        Some(v) => core::ops::ControlFlow::Break(v),
    }
}
