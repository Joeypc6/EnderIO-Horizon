fn enderio_screen_key(game: &crate::game::Game<'_>, screen: &crate::game::native_hooks::NativeScreenView<'_>, key: &str, canceled: &mut bool) {
    crate::game::enderio_ui::key(game, screen, key, canceled);
}
