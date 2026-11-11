fn jei_hud_before(game: &mut crate::game::Game<'_>) {
    enderio_optional::if_kit_jei!("enderio", {
    crate::game::hz_kit_window_enderio::hud_before(game);
} else { let _ = game; () })
}
