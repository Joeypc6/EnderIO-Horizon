fn enderio_custom_fill(game: &mut crate::game::Game<'_>, pos: [i32; 3], side: Option<horizon_core::geometry::Direction>, resource: &horizon_client::mod_api::NativeFluidStack, action: horizon_client::mod_api::NativeFluidAction, out: &mut Option<i32>) {
    crate::game::enderio_machines::custom_fluid_fill(game, pos, side, resource, action, out);
}
