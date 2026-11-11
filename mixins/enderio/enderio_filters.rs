// Ender IO 5.3.72's filter items (crazypants.enderio.base.filter, CC0 / Unlicense, the Ender IO team) for Horizon:
// what each filter keeps on its item (custom_data `filter`, Java's NBT names), the edits its screen makes (always on
// the server, from a validated button id and the player's cursor stack, which is only read), and the filter screens
// (AbstractFilterGui and its subclasses, 189 x 207, the player's inventory at 14, 119), drawn over the bundle's
// settings screen when its gear button opens one.
//
// Ghost slots hold copies in the filter's NBT only: a click never takes, gives or changes a real item.
use super::Game;
use super::enderio::{custom_data, gc, gi, gs, js, set_custom_data, ss};
use horizon_client::hud::HudBatch;
use horizon_client::items::ItemStack;
use horizon_world::nbt::{Compound, Tag};

#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub(in crate::game) enum Kind {
    // BasicFilterTypes: slots, isAdvanced, isLimited, isBig.
    Items { slots: usize, advanced: bool, limited: bool, big: bool },
    Mod,
    Power,
    Existing,
    Fluid,
    // ItemEnchantmentFilter / ItemSoulFilter: createNormal 5 slots, createBig 10.
    Enchant { big: bool },
    Soul { big: bool },
}

pub(in crate::game) const MOD_FILTER: &str = "enderio:mod_item_filter";
pub(in crate::game) const POWER_FILTER: &str = "enderio:power_item_filter";
pub(in crate::game) const EXISTING_FILTER: &str = "enderio:existing_item_filter";
pub(in crate::game) const FLUID_FILTER: &str = "enderio:fluid_filter";
pub(in crate::game) const ENCHANT_FILTER: &str = "enderio:enchantment_filter";
pub(in crate::game) const BIG_ENCHANT_FILTER: &str = "enderio:big_enchantment_filter";
pub(in crate::game) const SOUL_FILTER: &str = "enderio:soul_filter";
pub(in crate::game) const BIG_SOUL_FILTER: &str = "enderio:big_soul_filter";

// The enchantment and soul filters' slotCount.
pub(in crate::game) fn list_slots(big: bool) -> usize {
    if big { 10 } else { 5 }
}

pub(in crate::game) fn kind(item: &str) -> Option<Kind> {
    Some(match item {
        ENCHANT_FILTER => Kind::Enchant { big: false },
        BIG_ENCHANT_FILTER => Kind::Enchant { big: true },
        SOUL_FILTER => Kind::Soul { big: false },
        BIG_SOUL_FILTER => Kind::Soul { big: true },
        "enderio:basic_item_filter" => Kind::Items { slots: 5, advanced: false, limited: false, big: false },
        "enderio:advanced_item_filter" => Kind::Items { slots: 10, advanced: true, limited: false, big: false },
        "enderio:limited_item_filter" => Kind::Items { slots: 10, advanced: true, limited: true, big: false },
        "enderio:big_item_filter" => Kind::Items { slots: 36, advanced: false, limited: false, big: true },
        "enderio:big_advanced_item_filter" => Kind::Items { slots: 36, advanced: true, limited: false, big: true },
        MOD_FILTER => Kind::Mod,
        POWER_FILTER => Kind::Power,
        EXISTING_FILTER => Kind::Existing,
        FLUID_FILTER => Kind::Fluid,
        _ => return None,
    })
}

// The item filters of the item conduit's slots (IItemFilterUpgrade), and the fluid conduit's (IItemFilterFluidUpgrade).
pub(in crate::game) const ITEM_FILTER_ITEMS: [&str; 12] = [
    "enderio:basic_item_filter",
    "enderio:advanced_item_filter",
    "enderio:limited_item_filter",
    "enderio:big_item_filter",
    "enderio:big_advanced_item_filter",
    MOD_FILTER,
    POWER_FILTER,
    EXISTING_FILTER,
    ENCHANT_FILTER,
    BIG_ENCHANT_FILTER,
    SOUL_FILTER,
    BIG_SOUL_FILTER,
];

// ---- The NBT (NbtValue.FILTER: the item's `filter` compound)

pub(in crate::game) fn filter_tag(stack: &ItemStack) -> Option<&Compound> {
    custom_data(stack).and_then(|c| gc(c, "filter"))
}

fn put_tag(stack: &mut ItemStack, f: Compound) {
    let mut c = custom_data(stack).cloned().unwrap_or_default();
    c.insert(js("filter"), Tag::Compound(f));
    set_custom_data(stack, c);
}

fn gb(c: &Compound, k: &str, d: bool) -> bool {
    gi(c, k, i32::from(d)) != 0
}

fn sb(c: &mut Compound, k: &str, v: bool) {
    c.insert(js(k), Tag::Byte(if v { 1 } else { 0 }));
}

// ItemFilter: the constructor's defaults where a key is missing (a new filter item has no tag).
#[derive(Clone, Debug, Default, PartialEq)]
pub(in crate::game) struct Items {
    pub blacklist: bool,
    pub meta: bool,
    pub nbt: bool,
    pub ore: bool,
    pub sticky: bool,
    // DamageMode's ordinal.
    pub damage: i32,
    pub items: Vec<Option<ItemStack>>,
}

pub(in crate::game) fn read_items(stack: &ItemStack, slots: usize, advanced: bool) -> Items {
    let mut it = Items { meta: true, nbt: advanced, items: vec![None; slots], ..Items::default() };
    if let Some(c) = filter_tag(stack) {
        it.blacklist = gb(c, "isBlacklist", false);
        it.meta = gb(c, "matchMeta", true);
        it.nbt = gb(c, "matchNBT", advanced);
        it.ore = gb(c, "useOreDict", false);
        it.sticky = gb(c, "sticky", false);
        it.damage = (gi(c, "damageMode", 0) & 0xFF).clamp(0, 10);
        if let Some(Tag::List(l)) = c.get(&js("items")) {
            for (i, t) in l.iter().enumerate().take(slots) {
                it.items[i] = horizon_items::item_nbt::item_from_nbt(t);
            }
        }
    }
    it
}

// writeToNBT; an empty slot is an empty compound (Java's reader takes it as an empty stack), so the slots keep their
// places.
fn write_items(stack: &mut ItemStack, k: Kind, it: &Items) {
    let Kind::Items { advanced, limited, big, .. } = k else { return };
    let mut c = filter_tag(stack).cloned().unwrap_or_default();
    sb(&mut c, "isBlacklist", it.blacklist);
    sb(&mut c, "matchMeta", it.meta);
    sb(&mut c, "matchNBT", it.nbt);
    sb(&mut c, "useOreDict", it.ore);
    sb(&mut c, "sticky", it.sticky);
    sb(&mut c, "isAdvanced", advanced);
    sb(&mut c, "isLimited", limited);
    sb(&mut c, "isBig", big);
    super::enderio::si(&mut c, "damageMode", it.damage);
    let list = it.items.iter().map(|s| s.as_ref().map_or_else(|| Tag::Compound(Compound::new()), horizon_items::item_nbt::item_to_nbt)).collect();
    c.insert(js("items"), Tag::List(list));
    put_tag(stack, c);
}

// ModItemFilter: mod0..mod2 ("-" for none), blacklist.
pub(in crate::game) fn read_mods(stack: &ItemStack) -> ([Option<String>; 3], bool) {
    let mut mods: [Option<String>; 3] = Default::default();
    let mut black = false;
    if let Some(c) = filter_tag(stack) {
        for (i, m) in mods.iter_mut().enumerate() {
            *m = gs(c, &format!("mod{i}")).filter(|s| !s.is_empty() && s != "-");
        }
        black = gb(c, "blacklist", false);
    }
    (mods, black)
}

fn write_mods(stack: &mut ItemStack, mods: &[Option<String>; 3], black: bool) {
    let mut c = Compound::new();
    for (i, m) in mods.iter().enumerate() {
        ss(&mut c, &format!("mod{i}"), m.as_deref().filter(|s| !s.trim().is_empty()).unwrap_or("-"));
    }
    sb(&mut c, "blacklist", black);
    put_tag(stack, c);
}

// PowerItemFilter: mode (CmpMode, EQUAL by default), level (0..4, 4 by default), sticky.
pub(in crate::game) fn read_power(stack: &ItemStack) -> (i32, i32, bool) {
    match filter_tag(stack) {
        Some(c) => ((gi(c, "mode", 2) & 0xFF).clamp(0, 4), gi(c, "level", 4).clamp(0, 4), gb(c, "sticky", false)),
        None => (2, 4, false),
    }
}

fn write_power(stack: &mut ItemStack, mode: i32, level: i32, sticky: bool) {
    let mut c = Compound::new();
    c.insert(js("mode"), Tag::Byte(mode as i8));
    c.insert(js("level"), Tag::Short(level as i16));
    sb(&mut c, "sticky", sticky);
    put_tag(stack, c);
}

// ExistingItemFilter: the flags and an optional snapshot.
#[derive(Clone, Debug, Default)]
pub(in crate::game) struct Existing {
    pub blacklist: bool,
    pub meta: bool,
    pub nbt: bool,
    pub ore: bool,
    pub sticky: bool,
    pub snapshot: Option<Vec<ItemStack>>,
}

pub(in crate::game) fn read_existing(stack: &ItemStack) -> Existing {
    let mut e = Existing { meta: true, ..Existing::default() };
    if let Some(c) = filter_tag(stack) {
        e.blacklist = gb(c, "isBlacklist", false);
        e.meta = gb(c, "matchMeta", true);
        e.nbt = gb(c, "matchNBT", false);
        e.ore = gb(c, "useOreDict", false);
        e.sticky = gb(c, "sticky", false);
        if let Some(Tag::List(l)) = c.get(&js("snapshot")) {
            e.snapshot = Some(l.iter().filter_map(horizon_items::item_nbt::item_from_nbt).collect());
        }
    }
    e
}

fn write_existing(stack: &mut ItemStack, e: &Existing) {
    let mut c = Compound::new();
    sb(&mut c, "isBlacklist", e.blacklist);
    sb(&mut c, "matchMeta", e.meta);
    sb(&mut c, "matchNBT", e.nbt);
    sb(&mut c, "useOreDict", e.ore);
    sb(&mut c, "sticky", e.sticky);
    if let Some(s) = &e.snapshot {
        c.insert(js("snapshot"), Tag::List(s.iter().map(horizon_items::item_nbt::item_to_nbt).collect()));
    }
    put_tag(stack, c);
}

// ExistingItemFilter.stackEqual: with useOreDict a shared ore name (`ore`, ore_match), else the same item, the
// components with matchNBT (metadata is gone in 26.x).
pub(in crate::game) fn existing_equal(e: &Existing, a: &ItemStack, b: &ItemStack, ore: &dyn Fn(&str, &str) -> bool) -> bool {
    (e.ore && ore(&b.item, &a.item)) || (a.item == b.item && (!e.nbt || a.same_item_same_components(b)))
}

// FluidFilter: five fluids by place ("" for none) and the blacklist.
pub(in crate::game) fn read_fluids(stack: &ItemStack) -> ([Option<String>; 5], bool) {
    let mut out: [Option<String>; 5] = Default::default();
    let mut black = false;
    if let Some(c) = filter_tag(stack) {
        if let Some(Tag::List(l)) = c.get(&js("fluids")) {
            for (i, t) in l.iter().enumerate().take(5) {
                if let Tag::String(s) = t {
                    out[i] = Some(s.to_string_lossy().to_string()).filter(|s| !s.is_empty());
                }
            }
        }
        black = gb(c, "isBlacklist", false);
    }
    (out, black)
}

fn write_fluids(stack: &mut ItemStack, fluids: &[Option<String>; 5], black: bool) {
    let mut c = Compound::new();
    sb(&mut c, "isBlacklist", black);
    c.insert(js("fluids"), Tag::List(fluids.iter().map(|f| Tag::String(js(f.as_deref().unwrap_or("")))).collect()));
    put_tag(stack, c);
}

// EnchantmentFilter / SoulFilter (@Store enchantments or souls, blacklist, sticky, slotCount): the list in order,
// never longer than slotCount.
#[derive(Clone, Debug, PartialEq)]
pub(in crate::game) struct Picks<T> {
    pub list: Vec<T>,
    pub blacklist: bool,
    pub sticky: bool,
}

impl<T> Default for Picks<T> {
    fn default() -> Self {
        Self { list: Vec::new(), blacklist: false, sticky: false }
    }
}

// A CapturedMob as the soul filter keeps it: the entity id and the vial's custom name (Java also keeps the entity's
// NBT, which only its spawn uses).
#[derive(Clone, Debug, PartialEq, Eq)]
pub(in crate::game) struct Soul {
    pub id: String,
    pub name: Option<String>,
}

// ResourceLocation's default namespace.
fn full_id(id: &str) -> String {
    if id.contains(':') { id.to_owned() } else { format!("minecraft:{id}") }
}

// CapturedMob.containsSoul + create(stack): a non-empty stack whose data has `entity`, `entityId` or `EntityTag`
// (soul vials, broken spawners; 1.12's spawn eggs carried EntityTag, 26.x's name their mob by item, so those count
// too). The id is `entityId`, else the entity NBT's `id`, else the pig.
pub(in crate::game) fn soul_in(stack: &ItemStack) -> Option<Soul> {
    if stack.count == 0 {
        return None;
    }
    if let Some(d) = custom_data(stack).filter(|d| ["entity", "entityId", "EntityTag"].iter().any(|k| d.get(&js(k)).is_some())) {
        let nbt = gc(d, "entity").or_else(|| gc(d, "EntityTag"));
        let id = gs(d, "entityId").or_else(|| nbt.and_then(|n| gs(n, "id"))).filter(|s| !s.is_empty()).map_or_else(|| "minecraft:pig".to_owned(), |s| full_id(&s));
        return Some(Soul { id, name: gs(d, "customName") });
    }
    let egg = stack.item.strip_prefix("minecraft:")?.strip_suffix("_spawn_egg")?;
    Some(Soul { id: format!("minecraft:{egg}"), name: None })
}

pub(in crate::game) fn read_enchants(stack: &ItemStack) -> Picks<String> {
    let mut p = Picks::default();
    if let Some(c) = filter_tag(stack) {
        if let Some(Tag::List(l)) = c.get(&js("enchantments")) {
            p.list = l.iter().filter_map(|t| if let Tag::String(s) = t { Some(s.to_string_lossy().to_string()) } else { None }).filter(|s| !s.is_empty()).collect();
        }
        p.blacklist = gb(c, "blacklist", false);
        p.sticky = gb(c, "sticky", false);
    }
    p
}

pub(in crate::game) fn read_souls(stack: &ItemStack) -> Picks<Soul> {
    let mut p = Picks::default();
    if let Some(c) = filter_tag(stack) {
        if let Some(Tag::List(l)) = c.get(&js("souls")) {
            p.list = l.iter().filter_map(|t| if let Tag::Compound(m) = t { gs(m, "entityId").filter(|s| !s.is_empty()).map(|id| Soul { id: full_id(&id), name: gs(m, "customName") }) } else { None }).collect();
        }
        p.blacklist = gb(c, "blacklist", false);
        p.sticky = gb(c, "sticky", false);
    }
    p
}

fn write_picks<T>(stack: &mut ItemStack, key: &str, p: &Picks<T>, slots: usize, entry: impl Fn(&T) -> Tag) {
    let mut c = Compound::new();
    c.insert(js(key), Tag::List(p.list.iter().map(entry).collect()));
    sb(&mut c, "blacklist", p.blacklist);
    sb(&mut c, "sticky", p.sticky);
    super::enderio::si(&mut c, "slotCount", slots as i32);
    put_tag(stack, c);
}

fn soul_tag(s: &Soul) -> Tag {
    let mut m = Compound::new();
    ss(&mut m, "entityId", &s.id);
    if let Some(n) = &s.name {
        ss(&mut m, "customName", n);
    }
    Tag::Compound(m)
}

// EnchantmentFilter.getInventorySlotContents: an enchanted book with the enchantment at level 1.
pub(in crate::game) fn enchant_book(id: &str) -> ItemStack {
    let mut b = ItemStack::new("minecraft:enchanted_book", 1);
    b.enchant(id, 1);
    b
}

// SoulFilter.getInventorySlotContents: CapturedMob.toGenericStack(soul vial, meta 1): a filled vial with entityId.
pub(in crate::game) fn soul_vial(id: &str) -> ItemStack {
    let mut v = ItemStack::new("enderio:filled_soul_vial", 1);
    let mut d = Compound::new();
    ss(&mut d, "entityId", id);
    set_custom_data(&mut v, d);
    v
}

// EnchantmentFilter.setInventorySlotContents: the stack's enchantments (EnchantmentHelper.getEnchantments: a book's
// stored ones) not yet in the list, the first at `slot`, the rest appended, then cut to slotCount; none clears the slot.
fn put_enchants(list: &mut Vec<String>, slot: usize, input: &[String], slots: usize) {
    if input.is_empty() {
        if slot < list.len() {
            list.remove(slot);
        }
        return;
    }
    let mut slot = slot;
    for e in input {
        if list.contains(e) {
            continue;
        }
        if slot < list.len() {
            list[slot] = e.clone();
        } else {
            list.push(e.clone());
        }
        slot = usize::MAX;
    }
    list.truncate(slots);
}

// SoulFilter.setInventorySlotContents: a soul replaces the slot (and leaves again when another slot has its type) or
// is appended when its type is new; no soul clears the slot.
fn put_soul(list: &mut Vec<Soul>, slot: usize, input: Option<Soul>) {
    match input {
        Some(s) if slot < list.len() => {
            let dup = list.iter().enumerate().any(|(i, x)| i != slot && x.id == s.id);
            list[slot] = s;
            if dup {
                list.remove(slot);
            }
        }
        Some(s) => {
            if !list.iter().any(|x| x.id == s.id) {
                list.push(s);
            }
        }
        None => {
            if slot < list.len() {
                list.remove(slot);
            }
        }
    }
}

// ---- The ore dictionary (useOreDict). 1.12's two stacks match when they share an ore name; here they share a `c:`
// item tag (the content mod writes Ender IO's ore names as c: tags: ingotIron is c:ingots/iron, blockGlass
// c:glass_blocks, sand c:sands). The c: category tags no 1.12 ore name stood for (every ingot, every ore, every dyed
// block of a colour...) are left out, or an ingot would pass for any other. Built once per tag table, then a sorted
// id list per item.
const ORE_SKIP: [&str; 32] = [
    "c:ingots", "c:nuggets", "c:ores", "c:gems", "c:dusts", "c:storage_blocks", "c:raw_materials", "c:raw_blocks",
    "c:tools", "c:melee_weapon_tools", "c:ranged_weapon_tools", "c:mining_tool_tools", "c:armors", "c:foods",
    "c:animal_foods", "c:seeds", "c:crops", "c:buckets", "c:music_discs", "c:rods", "c:gears", "c:plates", "c:bricks",
    "c:player_workstations", "c:villager_job_sites", "c:enchantables", "c:fences", "c:fence_gates", "c:flowers",
    "c:potions", "c:shulker_boxes", "c:sandstone",
];
// Whole subtrees with no ore names: colour across block kinds, ore placement, recipe-viewer hiding.
const ORE_SKIP_TREE: [&str; 4] = ["c:dyed", "c:ores_in_ground", "c:ore_rates", "c:hidden_from_recipe_viewers"];

fn ore_name(t: &str) -> bool {
    t.starts_with("c:") && !ORE_SKIP.contains(&t) && !ORE_SKIP_TREE.iter().any(|p| t == *p || t.strip_prefix(p).is_some_and(|r| r.starts_with('/')))
}

#[derive(Default)]
struct OreIds {
    key: (usize, usize),
    by_item: std::collections::HashMap<String, Box<[u32]>>,
}

thread_local! {
    static ORE_IDS: std::cell::RefCell<OreIds> = std::cell::RefCell::new(OreIds::default());
}

fn ore_build(items: &horizon_client::items::Items) -> OreIds {
    let tags = &items.item_tags;
    let mut by: std::collections::HashMap<String, Vec<u32>> = std::collections::HashMap::new();
    let mut names: Vec<&String> = tags.keys().filter(|t| ore_name(t)).collect();
    names.sort();
    for (i, t) in names.iter().enumerate() {
        for m in &tags[*t] {
            by.entry(m.clone()).or_default().push(i as u32);
        }
    }
    let key = (std::ptr::from_ref(items) as usize, tags.len());
    OreIds { key, by_item: by.into_iter().map(|(k, v)| (k, v.into_boxed_slice())).collect() }
}

// OreDictionary.getOreIDs(a) and (b) share an id.
pub(in crate::game) fn ore_match(items: &horizon_client::items::Items, a: &str, b: &str) -> bool {
    ORE_IDS.with_borrow_mut(|o| {
        if o.key != (std::ptr::from_ref(items) as usize, items.item_tags.len()) {
            *o = ore_build(items);
        }
        let (Some(x), Some(y)) = (o.by_item.get(a), o.by_item.get(b)) else { return false };
        let (mut i, mut j) = (0, 0);
        while i < x.len() && j < y.len() {
            match x[i].cmp(&y[j]) {
                std::cmp::Ordering::Equal => return true,
                std::cmp::Ordering::Less => i += 1,
                std::cmp::Ordering::Greater => j += 1,
            }
        }
        false
    })
}

// ---- The edits (PacketFilterUpdate and PacketExistingItemFilterSnapshot, decided on the server)

// Button actions, per filter slot: ghost clicks by slot n, then the buttons.
pub(in crate::game) const A_GHOST_LEFT: i32 = 0;
pub(in crate::game) const A_GHOST_RIGHT: i32 = 40;
// The mouse wheel over a ghost slot (GuiContainerBase.mouseWheel: GhostSlotHandler's wheel up / down), by slot n.
pub(in crate::game) const A_WHEEL_UP: i32 = 80;
pub(in crate::game) const A_WHEEL_DOWN: i32 = 120;
pub(in crate::game) const A_WHITELIST: i32 = 160;
pub(in crate::game) const A_META: i32 = 161;
pub(in crate::game) const A_NBT: i32 = 162;
pub(in crate::game) const A_ORE: i32 = 163;
pub(in crate::game) const A_STICKY: i32 = 164;
pub(in crate::game) const A_DAMAGE_NEXT: i32 = 165;
pub(in crate::game) const A_DAMAGE_PREV: i32 = 166;
pub(in crate::game) const A_POWER_MODE: i32 = 167;
pub(in crate::game) const A_POWER_LEVEL: i32 = 168;
pub(in crate::game) const A_SNAP_SET: i32 = 169;
pub(in crate::game) const A_SNAP_CLEAR: i32 = 170;
pub(in crate::game) const A_SNAP_MERGE: i32 = 171;
pub(in crate::game) const A_MOD_DELETE: i32 = 172; // 172..175
// Client only: the existing filter's snapshot overlay; and the close arrow.
const A_SHOW: i32 = 198;
pub(in crate::game) const A_CLOSE: i32 = 199;

pub(in crate::game) struct Ctx<'a> {
    // The player's cursor stack, read only.
    pub carried: Option<&'a ItemStack>,
    // isStickyModeAvailable: the item conduit's output (insert) filter.
    pub sticky_ok: bool,
    // The fluid in the cursor stack (FluidUtil.getFluidTypeFromItem).
    pub carried_fluid: Option<String>,
    // The connected inventory's stacks (getInventoryForSnapshot), for a snapshot.
    pub inventory: Vec<ItemStack>,
    pub max_stack: &'a dyn Fn(&str) -> u32,
    // ore_match on the server's item tags.
    pub ore: &'a dyn Fn(&str, &str) -> bool,
}

// GhostSlotHandler's clicks on an ItemFilterGhostSlot (setInventorySlotContents makes a copy of count 1, then
// putStack sets the count clamped to 1..limit: a count taken down to 0 comes back as 1, as in Java).
fn ghost(items: &mut [Option<ItemStack>], n: usize, right: bool, hand: Option<&ItemStack>, limit: u32, max_stack: &dyn Fn(&str) -> u32) {
    let one = |s: &ItemStack, c: u32| {
        let mut x = s.clone();
        x.count = c.clamp(1, limit);
        Some(x)
    };
    let Some(h) = hand else {
        items[n] = None;
        return;
    };
    match items[n].clone() {
        Some(ex) if ex.same_item_same_components(h) => {
            if right {
                items[n] = one(&ex, ex.count.saturating_sub(1));
            } else if ex.count < max_stack(&ex.item) && ex.count < limit {
                items[n] = one(&ex, ex.count + 1);
            }
        }
        _ => items[n] = if right { one(h, 1) } else { one(h, h.count.min(limit)) },
    }
}

// GhostSlotHandler.ghostSlotClickedMouseWheelUp / Down on an ItemFilterGhostSlot: up adds one while the count is
// under the item's stack size and the slot's limit; down takes one (increaseSlot / descreaseSlot, then putStack clamps
// the count to 1..limit, so 1 stays 1). An empty slot is left alone. A plain filter's limit is 1: the wheel changes
// nothing there.
fn wheel(items: &mut [Option<ItemStack>], n: usize, up: bool, limit: u32, max_stack: &dyn Fn(&str) -> u32) {
    let Some(ex) = items[n].as_mut() else { return };
    if up {
        if ex.count < max_stack(&ex.item) && ex.count < limit {
            ex.count = (ex.count + 1).clamp(1, limit);
        }
    } else {
        ex.count = ex.count.saturating_sub(1).clamp(1, limit);
    }
}

// The new filter stack, or None when the action changes nothing (or isn't one this filter has).
pub(in crate::game) fn edit(stack: &ItemStack, action: i32, cx: &Ctx<'_>) -> Option<ItemStack> {
    let k = kind(&stack.item)?;
    let mut s = stack.clone();
    // A wheel action: (slot, up), the slot one this filter has.
    let wheel_n = |slots: usize| -> Option<(usize, bool)> {
        let (n, up) = if (A_WHEEL_UP..A_WHEEL_DOWN).contains(&action) { (action - A_WHEEL_UP, true) } else if (A_WHEEL_DOWN..A_WHEEL_DOWN + 40).contains(&action) { (action - A_WHEEL_DOWN, false) } else { return None };
        ((n as usize) < slots).then_some((n as usize, up))
    };
    let ghost_n = |slots: usize| -> Option<(usize, bool)> {
        let (n, right) = if (A_GHOST_LEFT..A_GHOST_RIGHT).contains(&action) { (action, false) } else if (A_GHOST_RIGHT..A_GHOST_RIGHT + 40).contains(&action) { (action - A_GHOST_RIGHT, true) } else { return None };
        ((n as usize) < slots).then_some((n as usize, right))
    };
    match k {
        Kind::Items { slots, advanced, limited, .. } => {
            let mut it = read_items(stack, slots, advanced);
            let before = it.clone();
            if let Some((n, right)) = ghost_n(slots) {
                ghost(&mut it.items, n, right, cx.carried, if limited { 192 } else { 1 }, cx.max_stack);
            } else if let Some((n, up)) = wheel_n(slots) {
                wheel(&mut it.items, n, up, if limited { 192 } else { 1 }, cx.max_stack);
            } else {
                match action {
                    A_WHITELIST if !limited => it.blacklist = !it.blacklist,
                    A_META => it.meta = !it.meta,
                    A_NBT if advanced => it.nbt = !it.nbt,
                    A_ORE if advanced => it.ore = !it.ore,
                    A_STICKY if advanced && cx.sticky_ok => it.sticky = !it.sticky,
                    A_DAMAGE_NEXT if advanced => it.damage = (it.damage + 1) % 11,
                    A_DAMAGE_PREV if advanced => it.damage = (it.damage + 10) % 11,
                    _ => return None,
                }
            }
            if it == before {
                return None;
            }
            write_items(&mut s, k, &it);
        }
        Kind::Mod => {
            let (mut mods, mut black) = read_mods(stack);
            if let Some((n, _)) = ghost_n(3) {
                // ModItemFilter.setMod: the item's namespace; an empty cursor clears it.
                mods[n] = cx.carried.map(|h| h.item.split_once(':').map_or("minecraft", |(ns, _)| ns).to_owned());
            } else if (A_MOD_DELETE..A_MOD_DELETE + 3).contains(&action) {
                mods[(action - A_MOD_DELETE) as usize] = None;
            } else if action == A_WHITELIST {
                black = !black;
            } else {
                return None;
            }
            write_mods(&mut s, &mods, black);
        }
        Kind::Power => {
            let (mut mode, mut level, mut sticky) = read_power(stack);
            match action {
                A_STICKY if cx.sticky_ok => sticky = !sticky,
                A_POWER_MODE => mode = (mode + 1) % 5,
                A_POWER_LEVEL => level = (level + 1) % 5,
                _ => return None,
            }
            write_power(&mut s, mode, level, sticky);
        }
        Kind::Existing => {
            let mut e = read_existing(stack);
            match action {
                A_WHITELIST => e.blacklist = !e.blacklist,
                A_META => e.meta = !e.meta,
                A_NBT => e.nbt = !e.nbt,
                A_ORE => e.ore = !e.ore,
                A_STICKY if cx.sticky_ok => e.sticky = !e.sticky,
                A_SNAP_CLEAR => e.snapshot = None,
                A_SNAP_SET | A_SNAP_MERGE => {
                    // setSnapshot / mergeSnapshot: one of each stack the inventory holds, not already in it.
                    let mut snap = if action == A_SNAP_MERGE { e.snapshot.clone().unwrap_or_default() } else { Vec::new() };
                    for st in &cx.inventory {
                        let one = ItemStack { count: 1, ..st.clone() };
                        if !snap.iter().any(|x| existing_equal(&e, &one, x, cx.ore)) {
                            snap.push(one);
                        }
                    }
                    e.snapshot = Some(snap);
                }
                _ => return None,
            }
            write_existing(&mut s, &e);
        }
        Kind::Fluid => {
            let (mut fluids, mut black) = read_fluids(stack);
            if let Some((n, _)) = ghost_n(5) {
                // FluidFilter.setFluid(index, stack): the container's fluid, else none.
                fluids[n] = cx.carried.and(cx.carried_fluid.clone());
            } else if action == A_WHITELIST {
                black = !black;
            } else {
                return None;
            }
            write_fluids(&mut s, &fluids, black);
        }
        // EnchantmentFilterGui / SoulFilterGui: the ghost slots (GhostSlotHandler: an empty cursor clears; a cursor
        // stack equal to the shown book or vial adds nothing on a left click and clears on a right one, the count
        // going to 0; any other stack is put in), whitelist and sticky (these screens always show sticky).
        Kind::Enchant { big } => {
            let slots = list_slots(big);
            let mut p = read_enchants(stack);
            let before = p.clone();
            if let Some((n, right)) = ghost_n(slots) {
                let shown = p.list.get(n).map(|e| enchant_book(e));
                match cx.carried {
                    None => put_enchants(&mut p.list, n, &[], slots),
                    Some(h) if shown.as_ref().is_some_and(|s| s.same_item_same_components(h)) => {
                        if right {
                            put_enchants(&mut p.list, n, &[], slots);
                        }
                    }
                    Some(h) => {
                        let ids: Vec<String> = h.crafting_enchantments().iter().map(|(e, _)| e.to_string()).collect();
                        put_enchants(&mut p.list, n, &ids, slots);
                    }
                }
            } else if let Some((n, up)) = wheel_n(slots) {
                // The wheel on the shown book (count 1, limit 1): up does nothing; down takes it to 0 and puts the
                // empty stack in, which clears the slot.
                if !up {
                    put_enchants(&mut p.list, n, &[], slots);
                }
            } else {
                match action {
                    A_WHITELIST => p.blacklist = !p.blacklist,
                    A_STICKY => p.sticky = !p.sticky,
                    _ => return None,
                }
            }
            if p == before {
                return None;
            }
            write_picks(&mut s, "enchantments", &p, slots, |e| Tag::String(js(e)));
        }
        Kind::Soul { big } => {
            let slots = list_slots(big);
            let mut p = read_souls(stack);
            let before = p.clone();
            if let Some((n, right)) = ghost_n(slots) {
                let shown = p.list.get(n).map(|x| soul_vial(&x.id));
                match cx.carried {
                    None => put_soul(&mut p.list, n, None),
                    Some(h) if shown.as_ref().is_some_and(|s| s.same_item_same_components(h)) => {
                        if right {
                            put_soul(&mut p.list, n, None);
                        }
                    }
                    Some(h) => put_soul(&mut p.list, n, soul_in(h)),
                }
            } else if let Some((n, up)) = wheel_n(slots) {
                // As the enchantment filter: down clears the shown vial's slot, up does nothing.
                if !up {
                    put_soul(&mut p.list, n, None);
                }
            } else {
                match action {
                    A_WHITELIST => p.blacklist = !p.blacklist,
                    A_STICKY => p.sticky = !p.sticky,
                    _ => return None,
                }
            }
            if p == before {
                return None;
            }
            write_picks(&mut s, "souls", &p, slots, soul_tag);
        }
    }
    (s != *stack).then_some(s)
}

// ---- The screens (client)

pub(in crate::game) const W: f32 = 189.0;
pub(in crate::game) const H: f32 = 207.0;
pub(in crate::game) const INV: [f32; 2] = [14.0, 119.0];

#[derive(Clone)]
struct Btn {
    r: [f32; 4],
    action: i32,
    icon: Option<[f32; 2]>,
    label: Option<String>,
    tip: Vec<String>,
    enabled: bool,
}

fn l(key: &str, english: &str) -> String {
    super::enderio_ui::lang(key, english)
}

fn icon_btn(x: f32, y: f32, action: i32, icon: [f32; 2], tip: Vec<String>) -> Btn {
    Btn { r: [x, y, 16.0, 16.0], action, icon: Some(icon), label: None, tip, enabled: true }
}

fn text_btn(x: f32, y: f32, w: f32, action: i32, label: String, tip: Vec<String>, enabled: bool) -> Btn {
    Btn { r: [x, y, w, 20.0], action, icon: None, label: Some(label), tip, enabled }
}

fn whitelist_btn(x: f32, y: f32, black: bool) -> Btn {
    if black {
        icon_btn(x, y, A_WHITELIST, [0.0, 240.0], vec![l("enderio.gui.item_filter.blacklist", "Blacklist")])
    } else {
        icon_btn(x, y, A_WHITELIST, [0.0, 224.0], vec![l("enderio.gui.item_filter.whitelist", "Whitelist")])
    }
}

fn toggle(x: f32, y: f32, action: i32, on: bool, icons: ([f32; 2], [f32; 2]), tips: ([&str; 2], [&str; 2])) -> Btn {
    let [key, english] = if on { tips.0 } else { tips.1 };
    let mut tip = vec![l(key, english)];
    if on && action == A_STICKY {
        tip.push(l("enderio.gui.item_filter.sticky_enabled2", "Selected items will only be sent to this or other sticky outputs"));
    }
    icon_btn(x, y, action, if on { icons.0 } else { icons.1 }, tip)
}

const META: ([f32; 2], [f32; 2]) = ([160.0, 224.0], [208.0, 224.0]);
const NBT: ([f32; 2], [f32; 2]) = ([176.0, 224.0], [224.0, 224.0]);
const ORE: ([f32; 2], [f32; 2]) = ([192.0, 224.0], [240.0, 224.0]);
const STICKY: ([f32; 2], [f32; 2]) = ([16.0, 240.0], [32.0, 240.0]);
const META_TIPS: ([&str; 2], [&str; 2]) = (["enderio.gui.item_filter.match_meta", "Match Metadata"], ["enderio.gui.item_filter.ignore_meta", "Ignore Metadata"]);
const NBT_TIPS: ([&str; 2], [&str; 2]) = (["enderio.gui.item_filter.match_nbt", "Match NBT"], ["enderio.gui.item_filter.ignore_nbt", "Ignore NBT"]);
const ORE_TIPS: ([&str; 2], [&str; 2]) = (["enderio.gui.item_filter.ore_dic_enabled", "Ore Dictionary Enabled"], ["enderio.gui.item_filter.ore_dic_disabled", "Ore Dictionary Disabled"]);
const STICKY_TIPS: ([&str; 2], [&str; 2]) = (["enderio.gui.item_filter.sticky_enabled", "Sticky Mode Enabled"], ["enderio.gui.item_filter.sticky_disabled", "Sticky Mode Disabled"]);
// DamageModeIconHolder, by DamageMode's ordinal.
const DAMAGE_ICONS: [[f32; 2]; 11] = [[160.0, 160.0], [0.0, 160.0], [48.0, 160.0], [16.0, 160.0], [64.0, 160.0], [32.0, 160.0], [80.0, 160.0], [112.0, 160.0], [96.0, 160.0], [128.0, 160.0], [144.0, 160.0]];
const DAMAGE_NAMES: [(&str, &str); 11] = [
    ("disabled", "Ignore Damage"),
    ("damage_00_25", "Up to 25% Damaged"),
    ("damage_25_00", "More than 25% Damaged"),
    ("damage_00_50", "Up to 50% Damaged"),
    ("damage_50_00", "More than 50% Damaged"),
    ("damage_00_75", "Up to 75% Damaged"),
    ("damage_75_00", "More than 75% Damaged"),
    ("damage_00_00", "Not Damaged"),
    ("damage_01_00", "Damaged"),
    ("damage_yes", "Can be Damaged"),
    ("damage_not", "Cannot be Damaged"),
];

// The ghost slots: (x, y, index).
fn ghosts(k: Kind) -> Vec<(f32, f32, usize)> {
    match k {
        // ItemFilter.createGhostSlots at (xOffset + 1, yOffset + 1) = (14, 35).
        Kind::Items { advanced, big, .. } => {
            let (rows, spacing, cols) = if big { (4, 0.0, 9) } else if advanced { (2, 2.0, 5) } else { (1, 2.0, 5) };
            let mut out = Vec::new();
            for row in 0..rows {
                for col in 0..cols {
                    out.push((14.0 + col as f32 * 18.0, 35.0 + row as f32 * 18.0 + spacing * row as f32, row * cols + col));
                }
            }
            out
        }
        Kind::Mod => (0..3).map(|i| (20.0, 46.0 + 22.0 * i as f32, i)).collect(),
        Kind::Fluid => (0..5).map(|i| (14.0 + 18.0 * i as f32, 35.0, i)).collect(),
        // createGhostSlots at (xOffset + 1, yOffset + 1 - 9) = (14, 26): GUI_ROWS 5 rows a column, columns 81 apart.
        Kind::Enchant { big } | Kind::Soul { big } => (0..list_slots(big)).map(|i| (14.0 + 81.0 * (i / 5) as f32, 26.0 + 18.0 * (i % 5) as f32, i)).collect(),
        Kind::Power | Kind::Existing => Vec::new(),
    }
}

// The enchantment or soul filter's names by slot (renderCustomOptions): the enchantment's name, or
// CapturedMob.getDisplayName ("custom (base)", the base alone, the custom name alone, else "???").
fn pick_names(stack: &ItemStack, k: Kind) -> Vec<String> {
    match k {
        Kind::Enchant { .. } => read_enchants(stack)
            .list
            .iter()
            .map(|e| {
                let (ns, path) = e.split_once(':').unwrap_or(("minecraft", e));
                l(&format!("enchantment.{ns}.{path}"), e)
            })
            .collect(),
        Kind::Soul { .. } => read_souls(stack)
            .list
            .iter()
            .map(|s| {
                let (ns, path) = s.id.split_once(':').unwrap_or(("minecraft", &s.id));
                let base = l(&format!("entity.{ns}.{path}"), "");
                let custom = s.name.as_deref().filter(|n| !n.trim().is_empty());
                match (base.trim().is_empty(), custom) {
                    (true, Some(c)) => c.to_owned(),
                    (true, None) => "???".to_owned(),
                    (false, Some(c)) => format!("{c} ({base})"),
                    (false, None) => base,
                }
            })
            .collect(),
        _ => Vec::new(),
    }
}

// The shown stacks (getInventorySlotContents).
fn pick_stacks(stack: &ItemStack, k: Kind) -> Vec<ItemStack> {
    match k {
        Kind::Enchant { .. } => read_enchants(stack).list.iter().map(|e| enchant_book(e)).collect(),
        Kind::Soul { .. } => read_souls(stack).list.iter().map(|s| soul_vial(&s.id)).collect(),
        _ => Vec::new(),
    }
}

// The name trimmed as renderCustomOptions does: two characters at a time until it fits with "...".
fn trim_name(game: &Game<'_>, s: &str, limit: f32) -> String {
    if super::enderio_ui::width(game, s) <= limit {
        return s.to_owned();
    }
    let mut c: Vec<char> = s.chars().collect();
    while !c.is_empty() && super::enderio_ui::width(game, &format!("{}...", c.iter().collect::<String>())) > limit {
        let n = c.len().saturating_sub(2);
        c.truncate(n);
    }
    format!("{}...", c.iter().collect::<String>())
}

fn buttons(stack: &ItemStack, sticky_ok: bool) -> Vec<Btn> {
    let Some(k) = kind(&stack.item) else { return Vec::new() };
    let mut out = vec![icon_btn(3.0, 3.0, A_CLOSE, [0.0, 208.0], vec![l("enderio.gui.item_filter.close", "Close Filter"), l("enderio.gui.item_filter.close2", "Go back to previous inventory")])];
    match k {
        Kind::Items { slots, advanced, limited, big } => {
            let it = read_items(stack, slots, advanced);
            // BasicItemFilterGui: butLeft = xOffset + 98; a big filter puts all six in one row at the top.
            let (mut x, mut y) = if big { (if advanced { 58.0 } else { 138.0 }, 13.0) } else { (111.0, 35.0) };
            if !limited {
                out.push(whitelist_btn(x, y, it.blacklist));
            }
            x += 20.0;
            out.push(toggle(x, y, A_META, it.meta, META, META_TIPS));
            x += 20.0;
            if advanced && sticky_ok {
                out.push(toggle(x, y, A_STICKY, it.sticky, STICKY, STICKY_TIPS));
            }
            if big {
                x += 20.0;
            } else {
                x = 111.0;
                y += 20.0;
            }
            if advanced {
                out.push(toggle(x, y, A_ORE, it.ore, ORE, ORE_TIPS));
                out.push(toggle(x + 20.0, y, A_NBT, it.nbt, NBT, NBT_TIPS));
                let (key, english) = DAMAGE_NAMES[it.damage as usize];
                out.push(icon_btn(x + 40.0, y, A_DAMAGE_NEXT, DAMAGE_ICONS[it.damage as usize], vec![l(&format!("enderio.gui.item_filter.damage.{key}"), english)]));
            }
        }
        Kind::Mod => {
            let (_, black) = read_mods(stack);
            out.push(whitelist_btn(39.0, 24.0, black));
            for i in 0..3 {
                out.push(icon_btn(39.0, 46.0 + 22.0 * i as f32, A_MOD_DELETE + i, [16.0, 192.0], vec![l("enderio.gui.mod.item_filter.delete", "Delete")]));
            }
        }
        Kind::Power => {
            let (mode, level, sticky) = read_power(stack);
            if sticky_ok {
                out.push(toggle(33.0, 34.0, A_STICKY, sticky, STICKY, STICKY_TIPS));
            }
            let m = ["<", "<=", "=", ">=", ">"][mode as usize];
            out.push(text_btn(57.0, 34.0, 40.0, A_POWER_MODE, m.to_owned(), vec![l("enderio.gui.power.item_filter.compare", "Comparison Mode")], true));
            out.push(text_btn(101.0, 34.0, 40.0, A_POWER_LEVEL, format!("{}%", level * 100 / 4), vec![l("enderio.gui.power.item_filter.percent", "Power Percentage")], true));
        }
        Kind::Existing => {
            let e = read_existing(stack);
            out.push(toggle(20.0, 36.0, A_META, e.meta, META, META_TIPS));
            out.push(whitelist_btn(40.0, 36.0, e.blacklist));
            out.push(toggle(60.0, 36.0, A_NBT, e.nbt, NBT, NBT_TIPS));
            out.push(toggle(80.0, 36.0, A_ORE, e.ore, ORE, ORE_TIPS));
            if sticky_ok {
                out.push(toggle(100.0, 36.0, A_STICKY, e.sticky, STICKY, STICKY_TIPS));
            }
            let has = e.snapshot.is_some();
            out.push(text_btn(20.0, 60.0, 60.0, A_SNAP_SET, l("enderio.gui.existing.item_filter.snapshot", "Snapshot"), vec![l("enderio.gui.existing.item_filter.snapshot2", "Optional: If no snapshot is set, the inventory content will be used as filter")], true));
            out.push(text_btn(84.0, 60.0, 40.0, A_SNAP_MERGE, l("enderio.gui.existing.item_filter.merge", "Merge"), vec![l("enderio.gui.existing.item_filter.merge2", "Merge the current snapshot with a new one")], has));
            out.push(text_btn(20.0, 84.0, 60.0, A_SNAP_CLEAR, l("enderio.gui.existing.item_filter.clear", "Clear"), vec![l("enderio.gui.existing.item_filter.clear2", "Clear the snapshot")], has));
            out.push(text_btn(84.0, 84.0, 40.0, A_SHOW, l("enderio.gui.existing.item_filter.show", "Show"), vec![l("enderio.gui.existing.item_filter.show2", "Show the current snapshot")], has));
        }
        Kind::Fluid => {
            let (_, black) = read_fluids(stack);
            out.push(whitelist_btn(111.0, 35.0, black));
        }
        // butLeft = xOffset + 98 + 30 = 141, y = yOffset + 1 - 25 = 10; sticky 20 right, always shown.
        Kind::Enchant { .. } | Kind::Soul { .. } => {
            let (black, sticky) = if matches!(k, Kind::Enchant { .. }) {
                let p = read_enchants(stack);
                (p.blacklist, p.sticky)
            } else {
                let p = read_souls(stack);
                (p.blacklist, p.sticky)
            };
            out.push(whitelist_btn(141.0, 10.0, black));
            out.push(toggle(161.0, 10.0, A_STICKY, sticky, STICKY, STICKY_TIPS));
        }
    }
    out
}

fn texture(k: Kind) -> &'static str {
    match k {
        Kind::Items { big: true, .. } => "enderio:gui/40/big_item_filter",
        Kind::Items { advanced: true, .. } => "enderio:gui/40/advanced_item_filter",
        Kind::Items { .. } => "enderio:gui/40/basic_item_filter",
        Kind::Mod => "enderio:gui/40/mod_item_filter",
        Kind::Fluid => "enderio:gui/40/fluid_filter",
        Kind::Power | Kind::Existing => "enderio:gui/40/item_filter",
        // Both screens pass "soul_filter_normal", "soul_filter_big" (the big one when slotCount > 5).
        Kind::Enchant { big: true } | Kind::Soul { big: true } => "enderio:gui/40/soul_filter_big",
        Kind::Enchant { .. } | Kind::Soul { .. } => "enderio:gui/40/soul_filter_normal",
    }
}

fn heading(k: Kind) -> String {
    match k {
        Kind::Items { big: true, advanced: true, .. } => l("enderio.gui.big_advanced_item_filter", "Advanced Big Item Filter"),
        Kind::Items { big: true, .. } => l("enderio.gui.big_item_filter", "Big Item Filter"),
        Kind::Items { limited: true, .. } => l("enderio.gui.limited_item_filter", "Limited Item Filter"),
        Kind::Items { advanced: true, .. } => l("enderio.gui.advanced_item_filter", "Advanced Item Filter"),
        Kind::Items { .. } => l("enderio.gui.basic_item_filter", "Basic Item Filter"),
        Kind::Mod => l("enderio.gui.mod_item_filter", "Mod Item Filter"),
        Kind::Power => l("enderio.gui.power_item_filter", "Power Item Filter"),
        Kind::Existing => l("enderio.gui.existing_item_filter", "Existing Item Filter"),
        Kind::Fluid => l("enderio.gui.fluid_filter", "Basic Fluid Filter"),
        Kind::Enchant { big: true } => l("enderio.gui.enchantment_filter_big", "Big Enchantment Item Filter"),
        Kind::Enchant { .. } => l("enderio.gui.enchantment_filter_normal", "Enchantment Item Filter"),
        Kind::Soul { big: true } => l("enderio.gui.soul_filter_big", "Big Soul Item Filter"),
        Kind::Soul { .. } => l("enderio.gui.soul_filter_normal", "Soul Item Filter"),
    }
}

thread_local! {
    // The existing filter's snapshot overlay is showing.
    static SHOW: std::cell::Cell<bool> = const { std::cell::Cell::new(false) };
}

pub(in crate::game) fn reset_client() {
    SHOW.with(|s| s.set(false));
}

// GuiButton: the vanilla button, its two halves, a grey label when disabled.
fn vanilla_button(game: &Game<'_>, batch: &mut HudBatch, o: [f32; 2], b: &Btn, mouse: [f32; 2]) {
    let hover = b.enabled && super::enderio_ui::inside(b.r, mouse);
    let border = 0xFF00_0000;
    let face = if !b.enabled { 0xFF5A_5A5A } else if hover { 0xFF7E_88BF } else { 0xFF6F_6F6F };
    super::enderio_ui::fill(game, batch, o, b.r, border);
    super::enderio_ui::fill(game, batch, o, [b.r[0] + 1.0, b.r[1] + 1.0, b.r[2] - 2.0, b.r[3] - 2.0], face);
    super::enderio_ui::fill(game, batch, o, [b.r[0] + 1.0, b.r[1] + 1.0, b.r[2] - 2.0, 1.0], if b.enabled { 0xFFAA_AAAA } else { 0xFF6A_6A6A });
    if let Some(s) = &b.label {
        let w = super::enderio_ui::width(game, s);
        let color = if !b.enabled { 0xFFA0_A0A0 } else if hover { 0xFFFF_FFA0 } else { 0xFFE0_E0E0 };
        super::enderio_ui::text(game, batch, s, o[0] + b.r[0] + (b.r[2] - w) / 2.0, o[1] + b.r[1] + (b.r[3] - 8.0) / 2.0, color, true);
    }
}

// The filter screen for `stack` (the filter in the bundle's slot), at origin `o`.
pub(in crate::game) fn draw(game: &Game<'_>, batch: &mut HudBatch, o: [f32; 2], stack: &ItemStack, sticky_ok: bool) {
    let Some(k) = kind(&stack.item) else { return };
    let mouse = super::enderio_ui::local_cursor(game, o);
    super::enderio_ui::spr(game, batch, texture(k), o, [0.0, 0.0], [0.0, 0.0, W, H]);
    super::enderio_ui::text(game, batch, &heading(k), o[0] + 28.0, o[1] + 13.0, 0xFF40_4040, false);
    for b in buttons(stack, sticky_ok) {
        if b.icon.is_some() {
            super::enderio_ui::button(game, batch, o, b.r, false, mouse);
            super::enderio_ui::icon(game, batch, o, [b.r[0], b.r[1]], b.icon.unwrap_or_default(), [16.0, 16.0], [16.0, 16.0]);
        } else {
            vanilla_button(game, batch, o, &b, mouse);
        }
    }
    match k {
        Kind::Items { slots, advanced, limited, .. } => {
            let it = read_items(stack, slots, advanced);
            for (x, y, n) in ghosts(k) {
                if let Some(s) = it.items.get(n).cloned().flatten() {
                    // setDisplayStdOverlay: the count only on a limited filter.
                    let shown = if limited { s } else { ItemStack { count: 1, ..s } };
                    game.native_gui_draw_item(batch, &shown, [o[0] + x, o[1] + y]);
                }
            }
        }
        Kind::Mod => {
            let (mods, _) = read_mods(stack);
            for (i, m) in mods.iter().enumerate() {
                if let Some(m) = m {
                    // fr.trimStringToWidth(mod, tfWidth - 6), white with a shadow.
                    let mut s = m.clone();
                    while !s.is_empty() && super::enderio_ui::width(game, &s) > 90.0 {
                        s.pop();
                    }
                    super::enderio_ui::text(game, batch, &s, o[0] + 20.0 + 41.0, o[1] + 46.0 + 22.0 * i as f32 + 4.0, 0xFFFF_FFFF, true);
                }
            }
        }
        Kind::Fluid => {
            let (fluids, _) = read_fluids(stack);
            for (i, f) in fluids.iter().enumerate() {
                if let Some(f) = f {
                    // RenderUtil.renderGuiTank(f, 1000, 1000, x + 1, y + 1, 16, 16).
                    super::enderio_machines::gui_tank(game, batch, o, f, 1000, 1000, [14.0 + 18.0 * i as f32, 35.0, 16.0, 16.0], 1.0);
                }
            }
        }
        Kind::Enchant { big } | Kind::Soul { big } => {
            // renderCustomOptions: the names white at the slot + (19, 4), cut to (big ? 63 : 144) - 2; the ghost
            // stacks without the count, greyed by the background at 0.5 (GhostSlot's grayOut).
            let names = pick_names(stack, k);
            let stacks = pick_stacks(stack, k);
            let limit = if big { 61.0 } else { 142.0 };
            for (x, y, n) in ghosts(k) {
                if let Some(s) = stacks.get(n) {
                    game.native_gui_draw_item(batch, s, [o[0] + x, o[1] + y]);
                    super::enderio_ui::spr_alpha(game, batch, texture(k), o, [x, y], [x, y, 16.0, 16.0], 0.5);
                }
                if let Some(name) = names.get(n) {
                    super::enderio_ui::text(game, batch, &trim_name(game, name, limit), o[0] + x + 19.0, o[1] + y + 4.0, 0xFFFF_FFFF, false);
                }
            }
        }
        Kind::Power | Kind::Existing => {}
    }
    for (x, y, _) in ghosts(k) {
        if super::enderio_ui::inside([x, y, 16.0, 16.0], mouse) {
            super::enderio_ui::fill(game, batch, o, [x, y, 16.0, 16.0], 0x80FF_FFFF);
        }
    }
    if k == Kind::Existing && SHOW.with(std::cell::Cell::get) {
        // SnapshotOverlay: a black frame, a grey panel, the snapshot's items nine to a row.
        super::enderio_ui::fill(game, batch, o, [8.0, 8.0, W - 11.0 - 8.0, H - 11.0 - 8.0], 0xFF00_0000);
        super::enderio_ui::fill(game, batch, o, [10.0, 10.0, W - 15.0 - 10.0, H - 15.0 - 10.0], 0xFF99_9999);
        for (i, st) in read_existing(stack).snapshot.unwrap_or_default().iter().enumerate() {
            game.native_gui_draw_item(batch, st, [o[0] + 15.0 + 20.0 * (i % 9) as f32, o[1] + 10.0 + 20.0 * (i / 9) as f32]);
        }
    }
}

// A click at `at` (screen-local): the action to send, `Some(A_CLOSE)` for the arrow; None when nothing was hit.
// A click the screen used up (the overlay, a disabled button) answers Some(-1).
pub(in crate::game) fn press(stack: &ItemStack, sticky_ok: bool, at: [f32; 2], right: bool) -> Option<i32> {
    let k = kind(&stack.item)?;
    if k == Kind::Existing && SHOW.with(std::cell::Cell::get) {
        SHOW.with(|s| s.set(false));
        return Some(-1);
    }
    for (x, y, n) in ghosts(k) {
        if super::enderio_ui::inside([x, y, 16.0, 16.0], at) {
            return Some(n as i32 + if right { A_GHOST_RIGHT } else { A_GHOST_LEFT });
        }
    }
    for b in buttons(stack, sticky_ok) {
        if !super::enderio_ui::inside(b.r, at) {
            continue;
        }
        if !b.enabled {
            return Some(-1);
        }
        if b.action == A_SHOW {
            SHOW.with(|s| s.set(true));
            return Some(-1);
        }
        // CycleButton: the right button goes back.
        if b.action == A_DAMAGE_NEXT && right {
            return Some(A_DAMAGE_PREV);
        }
        return if right { Some(-1) } else { Some(b.action) };
    }
    None
}

// The wheel at `at` (screen-local): the action to send when it is over a ghost slot whose filter answers the wheel
// (a limited item filter, the enchantment and soul filters going down); the server decides the rest. Some(-1): over a
// ghost slot that ignores it (the wheel is still used up, as GhostSlotHandler takes it); None elsewhere.
// Not here: the mod filter's slot shows its last item for 50 ticks (ModFilterGhostSlot.lastItem, not drawn by this
// port), and only then does the wheel act on it.
pub(in crate::game) fn wheel_press(stack: &ItemStack, at: [f32; 2], up: bool) -> Option<i32> {
    let k = kind(&stack.item)?;
    let (_, _, n) = ghosts(k).into_iter().find(|(x, y, _)| super::enderio_ui::inside([*x, *y, 16.0, 16.0], at))?;
    let answers = match k {
        Kind::Items { limited, .. } => limited,
        Kind::Enchant { .. } | Kind::Soul { .. } => !up,
        _ => false,
    };
    Some(if answers { n as i32 + if up { A_WHEEL_UP } else { A_WHEEL_DOWN } } else { -1 })
}

pub(in crate::game) fn tooltip(game: &Game<'_>, stack: &ItemStack, sticky_ok: bool, at: [f32; 2], lines: &mut Vec<String>) {
    let Some(k) = kind(&stack.item) else { return };
    // A ghost slot holding something: that item's own tooltip (GhostSlot draws the stack's tooltip on hover), its name
    // first, the rest grey.
    if let Some(n) = ghosts(k).into_iter().find(|(x, y, _)| super::enderio_ui::inside([*x, *y, 16.0, 16.0], at)).map(|g| g.2) {
        let shown = match k {
            Kind::Items { slots, advanced, .. } => read_items(stack, slots, advanced).items.get(n).cloned().flatten(),
            Kind::Enchant { .. } | Kind::Soul { .. } => pick_stacks(stack, k).get(n).cloned(),
            _ => None,
        };
        if let Some(s) = shown {
            for (i, (text, _)) in game.stack_tooltip(&s).iter().enumerate() {
                let t = text.resolved().plain();
                lines.push(if i == 0 { t } else { format!("\u{a7}7{t}") });
            }
            return;
        }
    }
    if k == Kind::Mod {
        for (x, y, _) in ghosts(k) {
            if super::enderio_ui::inside([x, y, 16.0, 16.0], at) {
                lines.push(l("enderio.gui.mod.item_filter.slot", "Input Slot"));
                return;
            }
        }
    }
    if let Some(b) = buttons(stack, sticky_ok).into_iter().find(|b| super::enderio_ui::inside(b.r, at)) {
        lines.extend(b.tip);
    }
}
