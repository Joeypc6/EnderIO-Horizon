fn ae2_jei_event(
    game: &mut crate::game::Game<'_>,
    mod_id: &str,
    event: &crate::game::native_hooks::NativePanelEvent,
) {
    enderio_optional::if_kit_jei!("enderio", {
    crate::game::hz_kit_layer_enderio::event(game, mod_id, event);
} else { let _ = (game, mod_id, event); () })
}
