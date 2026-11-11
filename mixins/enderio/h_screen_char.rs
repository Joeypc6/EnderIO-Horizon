fn enderio_screen_char(game: &crate::game::Game<'_>, screen: &crate::game::native_hooks::NativeScreenView<'_>, c: char, canceled: &mut bool) {
    crate::game::enderio_ui::char_typed(game, screen, c, canceled);
}
