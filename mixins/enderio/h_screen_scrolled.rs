fn enderio_screen_scrolled(game: &crate::game::Game<'_>, screen: &crate::game::native_hooks::NativeScreenView<'_>, scroll: [f64; 2], canceled: &mut bool) {
    crate::game::enderio_ui::scrolled(game, screen, scroll, canceled);
}
