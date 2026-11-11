fn enderio_destroy(game: &mut crate::game::Game<'_>, pos: [i32; 3]) {
    crate::game::enderio::destroyed(game, pos);
}
