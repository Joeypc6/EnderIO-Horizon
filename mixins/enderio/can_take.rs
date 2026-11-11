// LegacyMachineWrapper.extractItem: outputs only, through a face whose IO mode may output.
fn enderio_can_take(c: &Containers<'_>, view: &View, slot: usize, stack: &ItemStack, face: Direction) -> core::ops::ControlFlow<bool> {
    if view.kind == Kind::Mod
        && let Some(BlockEntity::Mod(m)) = c.world.block_entities.get(&view.parts[0].0)
        && m.id.starts_with("enderio:")
    {
        let fuel = c.items.get(&stack.item).is_some_and(|i| i.fuel.is_some());
        if let Some(v) = crate::game::enderio::can_take(m, slot, stack, face, fuel) {
            return core::ops::ControlFlow::Break(v);
        }
    }
    core::ops::ControlFlow::Continue(())
}
