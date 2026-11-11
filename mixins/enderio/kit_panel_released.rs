fn jei_window_released(screen: &crate::game::native_hooks::NativeScreenView<'_>) {
    enderio_optional::if_kit_jei!("enderio", {
    crate::game::hz_kit_window_enderio::release(screen.mouse);
} else { let _ = screen; () })
}
