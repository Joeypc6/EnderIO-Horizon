// In the client library: the rules the game module published (enderio_rules.rs).
fn enderio_slot_rules(menu: &str, slot: usize, stack: &ItemStack, may_place: &mut bool, max_stack: &mut u32) {
    if let Some((ok, most)) = enderio_rules::rule(menu, slot, &stack.item) {
        *may_place = *may_place && ok;
        *max_stack = (*max_stack).min(most);
    }
}
