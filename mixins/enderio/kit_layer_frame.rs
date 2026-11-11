// `frame`: the layer's work while JEI is on; its window shut when JEI is turned off.
fn kit_layer_frame_enderio(game: &mut crate::game::Game<'_>) {
    enderio_optional::if_kit_jei!("enderio", {
    if game.native_on(crate::game::jei::ID) {
        crate::game::hz_kit_layer_enderio::frame(game);
    } else if crate::game::hz_kit_window_enderio::is_open() {
        // JEI turned off with its window open: nothing would close it, and it would keep taking the screen's input.
        crate::game::hz_kit_window_enderio::close();
        let _ = crate::game::hz_kit_window_enderio::take_acts();
    }
} else { let _ = game; () })
}
