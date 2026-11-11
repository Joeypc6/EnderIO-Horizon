// Ender IO's menus' slot rules (Slot.isItemValid and getSlotStackLimit: the capacitor slot takes one capacitor, the
// outputs nothing, the SAG mill's ball slot its balls), from the server's data: the game module fills them when its
// recipe tables are made (enderio.rs publish_rules). Until then every slot keeps the menu type's own answer.
use std::collections::{HashMap, HashSet};
use std::sync::RwLock;

type Rules = HashMap<String, Vec<(Option<HashSet<String>>, u32)>>;

static RULES: RwLock<Option<Rules>> = RwLock::new(None);

/// Whether slot `slot` of menu `menu` takes `item`, and how many it holds; None for a menu that isn't Ender IO's.
pub(super) fn rule(menu: &str, slot: usize, item: &str) -> Option<(bool, u32)> {
    if !menu.starts_with("enderio:") {
        return None;
    }
    let rules = RULES.read().ok()?;
    let (allowed, most) = rules.as_ref()?.get(menu)?.get(slot)?;
    Some((allowed.as_ref().is_none_or(|a| a.contains(item)), *most))
}

impl super::Menu {
    /// Ender IO: its menus' slot rules.
    pub fn hz_enderio_set_rules(rules: Rules) {
        if let Ok(mut r) = RULES.write() {
            *r = Some(rules);
        }
    }
}
