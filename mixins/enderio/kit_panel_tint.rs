fn jei_panel_tint(game: &Game<'_>, batch: &mut HudBatch) {
    enderio_optional::if_kit_jei!("enderio", {
    crate::game::hz_kit_window_enderio::tint(game, batch);
} else { let _ = (game, batch); () })
}
