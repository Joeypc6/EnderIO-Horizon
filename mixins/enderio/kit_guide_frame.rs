fn kit_guide_frame_enderio(game: &mut crate::game::Game<'_>) {
    enderio_optional::if_kit!("enderio", {
    if game.player.is_some() {
        crate::game::hz_kit_guide_enderio::frame_hook(game);
    }
} else { let _ = game; () })
}
