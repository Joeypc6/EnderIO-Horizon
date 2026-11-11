fn jei_screen_tip(game: &crate::game::Game<'_>) -> core::ops::ControlFlow<()> {
    enderio_optional::if_kit_jei!("enderio", {
    if crate::game::hz_kit_window_enderio::defer_tooltip(game) {
        return core::ops::ControlFlow::Break(());
    }
} else { let _ = game; () });
    core::ops::ControlFlow::Continue(())
}
