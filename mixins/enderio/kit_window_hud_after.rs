fn jei_hud_after(game: &mut crate::game::Game<'_>, batch: &mut horizon_client::hud::HudBatch) {
    enderio_optional::if_kit_jei!("enderio", {
    crate::game::hz_kit_window_enderio::hud_after(game, batch);
} else { let _ = (game, batch); () })
}
