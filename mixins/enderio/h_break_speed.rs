fn enderio_break_speed(game: &crate::game::Game<'_>, pos: [i32; 3], speed: &mut f32) {
    crate::game::enderio_facades::break_speed(game, pos, speed);
}
