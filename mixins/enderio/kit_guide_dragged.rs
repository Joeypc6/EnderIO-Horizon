fn kit_guide_dragged_enderio(delta: [f32; 2], canceled: &mut bool) {
    enderio_optional::if_kit!("enderio", {
    crate::game::hz_kit_guide_enderio::drag(delta, canceled);
} else { let _ = (delta, canceled); () })
}
