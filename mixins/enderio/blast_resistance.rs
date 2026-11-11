// A blast that may reach a hardened facade's bundle: the rays with its 10 x resistance (enderio_blast.rs).
fn enderio_blast_resistance(game: &mut crate::game::Game<'_>, center: [f64; 3], radius: f32, resists: Option<([i32; 3], f32)>) -> core::ops::ControlFlow<Vec<[i32; 3]>> {
    match crate::game::explosion::enderio_blast::exploded_positions(game, center, radius, resists) {
        Some(v) => core::ops::ControlFlow::Break(v),
        None => core::ops::ControlFlow::Continue(()),
    }
}
