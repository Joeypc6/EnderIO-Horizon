fn enderio_custom_drain(game: &mut crate::game::Game<'_>, pos: [i32; 3], side: Option<horizon_core::geometry::Direction>, request: &horizon_client::mod_api::NativeFluidDrain, action: horizon_client::mod_api::NativeFluidAction, out: &mut Option<horizon_client::mod_api::NativeFluidStack>) {
    crate::game::enderio_machines::custom_fluid_drain(game, pos, side, request, action, out);
}
