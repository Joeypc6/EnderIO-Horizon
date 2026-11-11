fn enderio_screen_pressed(game: &crate::game::Game<'_>, screen: &crate::game::native_hooks::NativeScreenView<'_>, button: i32, modifiers: u32, canceled: &mut bool) {
    crate::game::enderio_ui::pressed(game, screen, button, modifiers, canceled);
}
