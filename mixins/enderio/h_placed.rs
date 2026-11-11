fn enderio_placed(game: &mut crate::game::Game<'_>, stack: &ItemStack, pos: [i32; 3]) {
    crate::game::enderio::placed(game, stack, pos);
}
