fn kit_guide_render_post_enderio(game: &crate::game::Game<'_>, batch: &mut horizon_client::hud::HudBatch, screen: &crate::game::native_hooks::NativeScreenView<'_>) {
    enderio_optional::if_kit!("enderio", {
    if game.player.is_none() {
        return;
    }
    crate::game::hz_kit_guide_enderio::screen_frame(screen.id, screen.menu);
    crate::game::hz_kit_guide_enderio::draw(game, batch, screen.mouse);
} else { let _ = (game, batch, screen); () })
}
