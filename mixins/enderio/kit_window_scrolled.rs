fn jei_window_scrolled(game: &crate::game::Game<'_>, scroll: [f64; 2], canceled: &mut bool) {
    enderio_optional::if_kit_jei!("enderio", {
    if game.player.is_none() || *canceled {
        return;
    }
    crate::game::hz_kit_window_enderio::scroll(scroll[1], canceled);
} else { let _ = (game, scroll, canceled); () })
}
