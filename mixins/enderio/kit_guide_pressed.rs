fn kit_guide_pressed_enderio(game: &crate::game::Game<'_>, screen: &crate::game::native_hooks::NativeScreenView<'_>, button: i32, canceled: &mut bool) {
    enderio_optional::if_kit!("enderio", {
    if game.player.is_none() {
        return;
    }
    let _ = button;
    crate::game::hz_kit_guide_enderio::click(game, screen.mouse, canceled);
} else { let _ = (game, screen, button, canceled); () })
}
