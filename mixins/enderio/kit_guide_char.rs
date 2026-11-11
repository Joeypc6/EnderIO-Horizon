fn kit_guide_char_enderio(game: &crate::game::Game<'_>, c: char, canceled: &mut bool) {
    enderio_optional::if_kit!("enderio", {
    let _ = c;
    if game.player.is_some() && crate::game::hz_kit_guide_enderio::is_open() {
        *canceled = true;
    }
} else { let _ = (game, c, canceled); () })
}
