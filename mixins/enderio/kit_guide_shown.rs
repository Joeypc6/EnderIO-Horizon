fn kit_guide_shown_enderio(stack: &ItemStack) {
    enderio_optional::if_kit!("enderio", {
    crate::game::hz_kit_guide_enderio::hovered(&stack.item);
} else { let _ = stack; () })
}
