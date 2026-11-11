fn kit_guide_released_enderio(game: &crate::game::Game<'_>, canceled: &mut bool) {
    enderio_optional::if_kit!("enderio", {
    crate::game::hz_kit_guide_enderio::release(game, canceled);
} else { let _ = (game, canceled); () })
}
