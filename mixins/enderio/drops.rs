// BlockEnder.getDrops: the machine's item keeps its NBT, the bank's its energy; an enhanced top drops nothing; an
// explosion drops them with chance 1 / size.
fn enderio_drops(game: &mut crate::game::Game<'_>, pos: [i32; 3], state: BlockStateId, explosion: Option<f32>, drops: &mut Vec<ItemStack>) {
    let block = game.res.facts().get(state).name.to_string();
    if block.starts_with("enderio:") {
        crate::game::enderio::block_drops(game, pos, &block, explosion, drops);
    }
}
