fn kit_guide_scrolled_enderio(game: &crate::game::Game<'_>, scroll: [f64; 2], canceled: &mut bool) {
    enderio_optional::if_kit!("enderio", {
    if game.player.is_some() {
        crate::game::hz_kit_guide_enderio::scroll(scroll[1], canceled);
    }
} else { let _ = (game, scroll, canceled); () })
}
