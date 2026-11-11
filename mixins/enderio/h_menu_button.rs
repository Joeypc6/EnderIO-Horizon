fn enderio_menu_button(game: &mut crate::game::Game<'_>, pos: [i32; 3], entity: &mut horizon_client::containers::ModBlockEntity, id: i32, carried: &mut Option<ItemStack>) {
    crate::game::enderio::menu_button(game, pos, entity, id, carried);
}
