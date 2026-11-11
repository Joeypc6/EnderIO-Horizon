fn kit_guide_tooltip_enderio(stack: &horizon_client::items::ItemStack, lines: &mut Vec<(horizon_client::text::Text, u32)>) {
    enderio_optional::if_kit!("enderio", {
    crate::game::hz_kit_guide_enderio::tooltip(stack, lines);
} else { let _ = (stack, lines); () })
}
