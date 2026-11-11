fn enderio_conduit_destroy(game: &mut crate::game::Game<'_>, pos: [i32; 3], canceled: &mut bool) {
    crate::game::enderio_conduits::destroy_block(game, pos, canceled);
}
