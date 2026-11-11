fn enderio_screen_dragged(game: &crate::game::Game<'_>, delta: [f32; 2], canceled: &mut bool) {
    crate::game::enderio_ui::dragged(game, delta, canceled);
}
