// Ender IO's conduit bundle screen, laid out again while it is open (in native_hooks.rs, where the open screen's
// fields are visible): a filter screen replaces the settings panel (189 x 207, no container slots, the inventory at
// 14, 119), and each tab shows only the slots its conduit has (GuiExternalConnection's setInOutSlotsVisible).
use super::Game;

// Replaces the open screen's layout when it is `block`'s; false when another screen is open.
pub(crate) fn set_layout(game: &mut Game<'_>, block: &str, screen: super::NativeScreen) -> bool {
    let Some(open) = game.mod_screen.as_mut() else { return false };
    if open.block != block {
        return false;
    }
    open.screen = screen;
    true
}
