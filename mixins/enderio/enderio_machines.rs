// Ender IO 5.3.72 for Horizon, phase 2 (spec-machines.md 2.5-2.13, 3.2.4-3.2.12): the combustion and zombie
// generator families, the vat, the soul binder with the soul vial, the slice'n'splice, the enchanter, the fluid
// tanks, the wired chargers and the painting machine. The framework (capacitors, energy, tasks, side IO, redstone,
// the item keeping its NBT) is enderio.rs's; this module adds each machine's own rules, their fluids (to the mB, kept
// under Java's field names), the fluid capability other blocks see, their screens, JEI categories and Jade lines.
use super::Game;
use super::enderio::*;
use super::enderio_ui::{ICON_CAPACITOR, button, fill, icon, inside, lang, power_bar, progress, rft, send_button, spr, text, width};
use horizon_client::containers::{BlockEntity, ModBlockEntity};
use horizon_client::crafting::Ingredient;
use horizon_client::hud::{HudBatch, rgba};
use horizon_client::items::ItemStack;
use horizon_client::mod_api::{NativeFluidAction, NativeFluidDrain, NativeFluidStack, NativeFluidTankView};
use horizon_core::geometry::Direction;
use horizon_core::random::RandomSource;
use horizon_world::nbt::{Compound, Tag};
use std::cell::RefCell;
use std::collections::HashSet;
use std::rc::Rc;

enderio_optional::if_mod_items!("jei", {
enderio_optional::kit_use!(window as kit_window);
});

// ---- Fluids (base/fluid/Fluids.java): ids, temperatures, sprites, buckets

/// The fluids a screen names by number (data slots carry the number + 1).
const FLUIDS: [&str; 12] = [
    "minecraft:water", "minecraft:lava", "enderio:nutrient_distillation", "enderio:ender_distillation", "enderio:vapor_of_levity", "enderio:hootch",
    "enderio:rocket_fuel", "enderio:fire_water", "enderio:xp_juice", "enderio:liquid_sunshine", "enderio:cloud_seed", "enderio:cloud_seed_concentrated",
];
const XP: &str = "enderio:xp_juice";
const NUTRIENT: &str = "enderio:nutrient_distillation";
const ENDER: &str = "enderio:ender_distillation";

fn fluid_no(f: Option<&str>) -> i32 {
    f.and_then(|f| FLUIDS.iter().position(|x| *x == f)).map_or(0, |i| i as i32 + 1)
}

fn fluid_of_no(n: i32) -> Option<&'static str> {
    (n >= 1).then(|| FLUIDS.get(n as usize - 1).copied()).flatten()
}

/// Fluid.getTemperature in kelvin: Forge's 300, lava 1300, Ender IO's own.
fn temperature(f: &str) -> f64 {
    match f {
        "minecraft:lava" => 1300.0,
        "enderio:fire_water" => 2000.0,
        ENDER => 175.0,
        "enderio:vapor_of_levity" => 5.0,
        _ => 300.0,
    }
}

/// The still sprite on the GUI atlas (the blocks atlas: content mods' `block/` textures).
fn fluid_sprite(f: &str) -> String {
    match f {
        "minecraft:water" => "minecraft:block/water_still".into(),
        "minecraft:lava" => "minecraft:block/lava_still".into(),
        XP => "enderio:block/fluid_xpjuice_still".into(),
        _ => format!("enderio:block/fluid_{}_still", f.split_once(':').map_or(f, |(_, p)| p)),
    }
}

fn fluid_tint(f: &str) -> u32 {
    if f == "minecraft:water" { 0xFF3F_76E4 } else { 0xFFFF_FFFF }
}

pub(in crate::game) fn fluid_name(f: &str) -> String {
    let (ns, path) = f.split_once(':').unwrap_or(("minecraft", f));
    if ns == "minecraft" {
        return lang(&format!("block.minecraft.{path}"), path);
    }
    lang(&format!("fluid_type.{ns}.{path}"), path)
}

/// Forge's universal bucket of an Ender IO fluid (the content's static buckets) and vanilla's.
fn bucket_of(f: &str) -> Option<String> {
    match f {
        "minecraft:water" => Some("minecraft:water_bucket".into()),
        "minecraft:lava" => Some("minecraft:lava_bucket".into()),
        _ if f.starts_with("enderio:") && FLUIDS.contains(&f) => Some(format!("{f}_bucket")),
        _ => None,
    }
}

fn bucket_fluid(item: &str) -> Option<&'static str> {
    FLUIDS.iter().copied().find(|f| f.starts_with("enderio:") && bucket_of(f).as_deref() == Some(item))
}

/// What one item of `stack` holds: (fluid, amount, capacity); Ender IO's buckets, then Horizon's containers.
pub(in crate::game) fn item_fluid(game: &Game<'_>, stack: &ItemStack) -> Option<(Option<String>, i32, i32)> {
    if let Some(f) = bucket_fluid(&stack.item) {
        return Some((Some(f.to_owned()), 1000, 1000));
    }
    game.native_item_fluid(stack).map(|i| (i.fluid, i.amount, i.capacity))
}

/// IFluidHandlerItem.drain of one item, up to `max` (a bucket all or nothing): (fluid, amount, the item it becomes).
fn item_drain(game: &Game<'_>, stack: &ItemStack, max: i32) -> Option<(String, i32, ItemStack)> {
    if let Some(f) = bucket_fluid(&stack.item) {
        return (max >= 1000).then(|| (f.to_owned(), 1000, ItemStack::new("minecraft:bucket", 1)));
    }
    game.native_item_fluid_drain(stack, max).filter(|(_, a, _)| *a > 0)
}

/// IFluidHandlerItem.fill of one item: (amount taken, the item it becomes).
fn item_fill(game: &Game<'_>, stack: &ItemStack, f: &str, amount: i32) -> Option<(i32, ItemStack)> {
    if &*stack.item == "minecraft:bucket" && f.starts_with("enderio:") {
        let b = bucket_of(f)?;
        return (amount >= 1000 && game.native_max_stack_size(&b).is_some()).then(|| (1000, ItemStack::new(&b, 1)));
    }
    game.native_item_fluid_fill(stack, f, amount).filter(|(n, _)| *n > 0)
}

// ---- Tanks (EC SmartTank), kept as Java's: {FluidName, Amount}

#[derive(Clone, Debug, Default, PartialEq)]
pub(in crate::game) struct Tank {
    pub fluid: Option<String>,
    pub amount: i32,
    pub capacity: i32,
}

impl Tank {
    fn room(&self) -> i32 {
        (self.capacity - self.amount).max(0)
    }
    fn takes(&self, f: &str) -> bool {
        self.fluid.as_deref().is_none_or(|x| x == f)
    }
    /// fillInternal: what fits of `f`.
    fn fill(&mut self, f: &str, n: i32, execute: bool) -> i32 {
        if !self.takes(f) || n <= 0 {
            return 0;
        }
        let k = n.min(self.room());
        if execute && k > 0 {
            self.fluid = Some(f.to_owned());
            self.amount += k;
        }
        k
    }
    /// removeFluidAmount.
    fn remove(&mut self, n: i32) -> i32 {
        let k = n.min(self.amount).max(0);
        self.amount -= k;
        if self.amount <= 0 {
            self.amount = 0;
            self.fluid = None;
        }
        k
    }
}

fn read_tank(n: &Compound, key: &str, capacity: i32) -> Tank {
    let Some(t) = n.get(&js(key)).and_then(|t| if let Tag::Compound(c) = t { Some(c) } else { None }) else {
        return Tank { fluid: None, amount: 0, capacity };
    };
    let amount = gi(t, "Amount", 0).max(0);
    let fluid = gs(t, "FluidName").filter(|_| amount > 0);
    Tank { amount: if fluid.is_some() { amount } else { 0 }, fluid, capacity }
}

fn write_tank(n: &mut Compound, key: &str, t: &Tank) {
    let mut c = Compound::new();
    if let Some(f) = &t.fluid {
        ss(&mut c, "FluidName", f);
        si(&mut c, "Amount", t.amount);
    } else {
        ss(&mut c, "Empty", "");
    }
    si(&mut c, "Capacity", t.capacity);
    n.insert(js(key), Tag::Compound(c));
}

/// A machine's tanks in its SmartTankFluidMachineHandler order: (key, capacity, others may fill it, may drain it).
fn tank_defs(m: &Mach) -> Vec<(&'static str, i32, bool, bool)> {
    match m.kind {
        // combGenTankSize x the efficiency read before Enhanced sets it: 5000 for both.
        Kind::Combustion => vec![("coolantTank", 5000, true, false), ("fuelTank", 5000, true, false)],
        Kind::Zombie => vec![("tank", 2000, true, false)],
        Kind::Vat => vec![("inputTank", 8000, true, false), ("outputTank", 8000, false, true)],
        Kind::Tank => vec![("tank", if m.level > 1 { 32000 } else { 16000 }, true, true)],
        _ => Vec::new(),
    }
}

fn tanks(e: &ModBlockEntity, m: &Mach) -> Vec<Tank> {
    tank_defs(m).iter().map(|(k, c, _, _)| read_tank(&e.nbt, k, *c)).collect()
}

fn set_tank(e: &mut ModBlockEntity, m: &Mach, i: usize, t: &Tank) {
    if let Some((k, ..)) = tank_defs(m).get(i) {
        write_tank(&mut e.nbt, k, t);
    }
}

// ---- XP (base/xp/XpUtil, ExperienceContainer): 20 mB a point

const XP_RATIO: i64 = 20;

/// XpUtil.getExperienceForLevel: vanilla's total for a level.
pub(in crate::game) fn exp_for_level(n: i32) -> i64 {
    let n = i64::from(n.max(0));
    if n <= 16 {
        n * n + 6 * n
    } else if n <= 31 {
        ((2.5 * (n * n) as f64) - 40.5 * n as f64 + 360.0) as i64
    } else {
        ((4.5 * (n * n) as f64) - 162.5 * n as f64 + 2220.0) as i64
    }
}

fn level_for_xp(xp: i64) -> i32 {
    let mut l = 0;
    while exp_for_level(l + 1) <= xp && l < 21862 {
        l += 1;
    }
    l
}

fn xp_of(e: &ModBlockEntity) -> i64 {
    e.nbt.get(&js("xpCont")).and_then(|t| if let Tag::Compound(c) = t { Some(gl(c, "xp").max(i64::from(gi(c, "experienceTotal", 0)))) } else { None }).unwrap_or(0).max(0)
}

fn set_xp(e: &mut ModBlockEntity, xp: i64) {
    let mut c = Compound::new();
    c.insert(js("xp"), Tag::Long(xp.max(0)));
    si(&mut c, "experienceTotal", xp.clamp(0, i64::from(i32::MAX)) as i32);
    e.nbt.insert(js("xpCont"), Tag::Compound(c));
}

/// XpUtil.addPlayerXP on the acting player: their total, level and bar from the new total.
fn add_player_xp(game: &mut Game<'_>, delta: i64) {
    let old = game.experience.level;
    let total = (player_total_xp(game) + delta).max(0);
    let level = level_for_xp(total);
    let cap = horizon_client::entities::Experience::needed(level) as f32;
    game.experience.level = level;
    game.experience.total = total.min(i64::from(i32::MAX)) as i32;
    game.experience.progress = ((total - exp_for_level(level)) as f32 / cap.max(1.0)).clamp(0.0, 1.0);
    game.native_player_level_changed(old);
}

/// XpUtil.getPlayerXP: the points of the level reached and of the bar.
fn player_total_xp(game: &Game<'_>) -> i64 {
    let l = game.experience.level;
    exp_for_level(l) + (game.experience.progress * horizon_client::entities::Experience::needed(l) as f32).round() as i64
}

// ---- The recipe data of the phase 2 machines, cached until the recipe tables change

pub(in crate::game) struct Fuel {
    pub fluid: String,
    pub per_tick: i32,
    pub ticks: i32,
}

pub(in crate::game) struct Coolant {
    pub fluid: String,
    pub degrees: f64,
}

pub(in crate::game) struct VatRecipe {
    pub uid: String,
    pub energy: i32,
    pub groups: Vec<Vec<(Ingredient, f32)>>,
    pub fluid_in: String,
    pub fluid_mult: f32,
    pub fluid_out: String,
}

pub(in crate::game) struct SoulRecipe {
    pub uid: String,
    pub energy: i32,
    pub levels: i32,
    pub logic: String,
    pub mode: String,
    pub souls: Vec<String>,
    pub input: Ingredient,
    pub output: ItemStack,
}

pub(in crate::game) struct SliceRecipe {
    pub uid: String,
    pub energy: i32,
    pub inputs: Vec<Option<Ingredient>>,
    pub output: ItemStack,
}

pub(in crate::game) struct EnchRecipe {
    pub input: Ingredient,
    pub per_level: u32,
    pub enchantment: String,
    pub cost_mult: f64,
}

pub(in crate::game) struct TankRecipe {
    pub fill: bool,
    pub logic: String,
    pub input: Ingredient,
    pub fluid: String,
    pub amount: i32,
    pub output: Option<ItemStack>,
}

pub(in crate::game) struct C2 {
    stamp: Vec<(usize, usize)>,
    pub fuels: Vec<Fuel>,
    pub coolants: Vec<Coolant>,
    pub vat: Vec<VatRecipe>,
    pub soul: Vec<SoulRecipe>,
    pub slice: Vec<SliceRecipe>,
    pub ench: Vec<EnchRecipe>,
    pub tank: Vec<TankRecipe>,
    axes: Option<Ingredient>,
    shears: Option<Ingredient>,
}

impl C2 {
    fn fuel(&self, f: &str) -> Option<&Fuel> {
        self.fuels.iter().find(|x| x.fluid == f)
    }
    fn coolant(&self, f: &str) -> Option<&Coolant> {
        self.coolants.iter().find(|x| x.fluid == f)
    }
}

const TYPES: [&str; 7] = ["enderio:combustion_fuel", "enderio:combustion_coolant", "enderio:fermenting", "enderio:soul_binding", "enderio:slicing", "enderio:enchanting", "enderio:tanking"];

thread_local! {
    static C2_CACHE: RefCell<Option<Rc<C2>>> = const { RefCell::new(None) };
}

pub(in crate::game) fn c2(game: &Game<'_>) -> Rc<C2> {
    let now: Vec<(usize, usize)> = TYPES.iter().map(|t| {
        let s = game.native_recipes_of_type(t);
        (s.as_ptr() as usize, s.len())
    }).collect();
    if let Some(c) = C2_CACHE.with_borrow(|c| c.clone()).filter(|c| c.stamp == now) {
        return c;
    }
    let c = Rc::new(build(game, now));
    C2_CACHE.with_borrow_mut(|s| *s = Some(c.clone()));
    c
}

fn build(game: &Game<'_>, stamp: Vec<(usize, usize)>) -> C2 {
    let int = |v: &serde_json::Value, k: &str| v.get(k).and_then(|x| x.as_i64()).unwrap_or(0) as i32;
    let flt = |v: &serde_json::Value, k: &str| v.get(k).and_then(|x| x.as_f64()).unwrap_or(1.0);
    let st = |v: &serde_json::Value, k: &str| v.get(k).and_then(|x| x.as_str()).unwrap_or("").to_owned();
    let mut c = C2 { stamp, fuels: Vec::new(), coolants: Vec::new(), vat: Vec::new(), soul: Vec::new(), slice: Vec::new(), ench: Vec::new(), tank: Vec::new(), axes: None, shears: None };
    for r in game.native_recipes_of_type("enderio:combustion_fuel") {
        c.fuels.push(Fuel { fluid: st(&r.json, "fluid"), per_tick: int(&r.json, "power_per_tick"), ticks: int(&r.json, "ticks") });
    }
    for r in game.native_recipes_of_type("enderio:combustion_coolant") {
        c.coolants.push(Coolant { fluid: st(&r.json, "fluid"), degrees: flt(&r.json, "degrees_per_mb") });
    }
    for r in game.native_recipes_of_type("enderio:fermenting") {
        let j = &r.json;
        let mut groups = Vec::new();
        for g in j.get("input_groups").and_then(|g| g.as_array()).cloned().unwrap_or_default() {
            let list: Vec<(Ingredient, f32)> = g.as_array().cloned().unwrap_or_default().iter().filter_map(|i| Some((ingredient(game, i.get("ingredient")?)?, flt(i, "multiplier") as f32))).collect();
            if !list.is_empty() {
                groups.push(list);
            }
        }
        let fin = j.get("input_fluid").cloned().unwrap_or_default();
        if groups.is_empty() || groups.len() > 2 {
            continue;
        }
        c.vat.push(VatRecipe { uid: r.id.clone(), energy: int(j, "energy"), groups, fluid_in: st(&fin, "fluid"), fluid_mult: flt(&fin, "multiplier") as f32, fluid_out: st(j, "output_fluid") });
    }
    for r in game.native_recipes_of_type("enderio:soul_binding") {
        let j = &r.json;
        let Some(input) = j.get("input").and_then(|i| i.get("ingredient")).and_then(|v| ingredient(game, v)) else { continue };
        let Some(output) = j.get("output").and_then(|o| game.native_recipe_result(o)) else { continue };
        let souls = j.get("souls").and_then(|s| s.as_array()).map(|a| a.iter().filter_map(|x| x.as_str().map(str::to_owned)).collect()).unwrap_or_default();
        c.soul.push(SoulRecipe { uid: r.id.clone(), energy: int(j, "energy"), levels: int(j, "levels"), logic: st(j, "logic"), mode: st(j, "souls_mode"), souls, input, output });
    }
    for r in game.native_recipes_of_type("enderio:slicing") {
        let j = &r.json;
        let Some(output) = j.get("output").and_then(|o| game.native_recipe_result(o)) else { continue };
        let raw = j.get("inputs").and_then(|i| i.as_array()).cloned().unwrap_or_default();
        if raw.len() != 6 {
            continue;
        }
        let inputs: Vec<Option<Ingredient>> = raw.iter().map(|v| if v.is_null() { None } else { ingredient(game, v) }).collect();
        if inputs.iter().zip(raw.iter()).any(|(i, v)| i.is_none() && !v.is_null()) {
            continue;
        }
        c.slice.push(SliceRecipe { uid: r.id.clone(), energy: int(j, "energy"), inputs, output });
    }
    for r in game.native_recipes_of_type("enderio:enchanting") {
        let j = &r.json;
        let Some(input) = j.get("input").and_then(|v| ingredient(game, v)) else { continue };
        let ench = st(j, "enchantment");
        if game.enchantments().get(&ench).is_none() {
            continue;
        }
        c.ench.push(EnchRecipe { input, per_level: int(j, "count").max(1) as u32, enchantment: ench, cost_mult: flt(j, "cost_multiplier") });
    }
    for r in game.native_recipes_of_type("enderio:tanking") {
        let j = &r.json;
        let logic = st(j, "logic");
        // The dark steel upgrades' DSU logic needs the upgrades (not ported yet).
        if logic == "dsu" {
            continue;
        }
        let Some(input) = j.get("input").and_then(|v| ingredient(game, v)) else { continue };
        let f = j.get("fluid").cloned().unwrap_or_default();
        let output = j.get("output").and_then(|o| game.native_recipe_result(o));
        c.tank.push(TankRecipe { fill: st(j, "mode") == "fill", logic, input, fluid: st(&f, "fluid"), amount: f.get("amount").and_then(|a| a.as_i64()).unwrap_or(1000) as i32, output });
    }
    c.axes = ingredient(game, &serde_json::Value::String("#minecraft:axes".into()));
    c.shears = ingredient(game, &serde_json::json!(["minecraft:shears", "#c:tools/shears"]));
    c
}

// ---- Slot rules for the menus (Slot.isItemValid / getSlotStackLimit)

/// getInventoryStackLimit per slot: the binder's inputs, the slice'n'splice but its output, the charger's input, the
/// enchanter's book and output hold one.
pub(in crate::game) fn slot_limit(m: &Mach, slot: usize) -> Option<u32> {
    match m.kind {
        Kind::Soul if slot < 2 => Some(1),
        Kind::Slice if slot < 8 => Some(1),
        Kind::Charger if slot == 0 => Some(1),
        Kind::Enchanter if slot == 0 || slot == 3 => Some(1),
        _ => None,
    }
}

pub(in crate::game) fn slot_rules(game: &Game<'_>, m: &Mach, slots: &mut [(Option<HashSet<String>>, u32)]) {
    let c = c2(game);
    let items = |i: &Ingredient| i.order.iter().cloned().collect::<HashSet<String>>();
    for s in 0..slots.len() {
        if let Some(l) = slot_limit(m, s) {
            slots[s].1 = l;
        }
    }
    match m.kind {
        Kind::Vat => {
            for g in 0..2 {
                slots[g].0 = Some(c.vat.iter().filter_map(|r| r.groups.get(g)).flat_map(|l| l.iter().flat_map(|(i, _)| items(i))).collect());
            }
        }
        Kind::Soul => {
            slots[0].0 = Some(HashSet::from(["enderio:filled_soul_vial".to_owned()]));
            slots[1].0 = Some(c.soul.iter().flat_map(|r| items(&r.input)).collect());
        }
        Kind::Slice => {
            let grid: HashSet<String> = c.slice.iter().flat_map(|r| r.inputs.iter().flatten().flat_map(items)).collect();
            for s in 0..6 {
                slots[s].0 = Some(grid.clone());
            }
            slots[6].0 = c.axes.as_ref().map(items);
            slots[7].0 = c.shears.as_ref().map(items);
        }
        Kind::Enchanter => {
            slots[0].0 = Some(HashSet::from(["minecraft:book".to_owned()]));
            slots[1].0 = Some(c.ench.iter().flat_map(|r| items(&r.input)).collect());
            slots[2].0 = ingredient(game, &serde_json::Value::String("#c:gems/lapis".into())).map(|i| items(&i));
        }
        Kind::Painter => {
            let targets: HashSet<String> = TEMPLATES.iter().flat_map(|(r, t, _)| t.iter().map(|s| (*s).to_owned()).chain(std::iter::once((*r).to_owned()))).collect();
            slots[0].0 = Some(targets);
        }
        _ => {}
    }
}

// ---- isMachineItemValidForSlot

pub(in crate::game) fn valid_for_slot(game: &Game<'_>, e: &ModBlockEntity, m: &Mach, slot: usize, stack: &ItemStack) -> bool {
    let c = c2(game);
    let at = |i: usize| e.items.get(i).cloned().flatten();
    match m.kind {
        Kind::Combustion | Kind::Zombie => false,
        Kind::Vat => {
            // TileVat.isMachineRecipeInputValid: a recipe still takes this item with the other inputs there.
            if slot > 1 {
                return false;
            }
            let other = at(1 - slot);
            let tank = read_tank(&e.nbt, "inputTank", 8000);
            c.vat.iter().any(|r| {
                r.groups.get(slot).is_some_and(|g| g.iter().any(|(i, _)| i.test(stack)))
                    && other.as_ref().is_none_or(|o| r.groups.get(1 - slot).is_some_and(|g| g.iter().any(|(i, _)| i.test(o))))
                    && tank.fluid.as_deref().is_none_or(|f| f == r.fluid_in)
            })
        }
        Kind::Soul => {
            // TileSoulBinder.isMachineItemValidForSlot: alone, a recipe for it; with the other, a recipe for both.
            if slot > 1 {
                return false;
            }
            let mut ins = [at(0), at(1)];
            ins[slot] = Some(stack.clone());
            let other = ins[1 - slot].is_some();
            c.soul.iter().any(|r| soul_fits(r, ins[0].as_ref(), ins[1].as_ref(), other))
        }
        Kind::Slice => match slot {
            0..=5 => {
                if let Some(have) = at(slot) {
                    return have.item == stack.item;
                }
                let mut grid: Vec<Option<ItemStack>> = (0..6).map(at).collect();
                grid[slot] = Some(stack.clone());
                // isValidRecipeComponents with the inputs locked to their places.
                c.slice.iter().any(|r| grid.iter().enumerate().all(|(i, s)| s.as_ref().is_none_or(|s| r.inputs[i].as_ref().is_some_and(|ing| ing.test(s)))))
            }
            6 => c.axes.as_ref().is_some_and(|i| i.test(stack)),
            7 => c.shears.as_ref().is_some_and(|i| i.test(stack)),
            _ => false,
        },
        Kind::Enchanter => match slot {
            0 => &*stack.item == "minecraft:book",
            1 => c.ench.iter().any(|r| r.input.test(stack)),
            2 => ingredient(game, &serde_json::Value::String("#c:gems/lapis".into())).is_some_and(|i| i.test(stack)),
            _ => false,
        },
        Kind::Tank => tank_valid(game, &c, e, m, slot, stack),
        Kind::Charger => slot == 0 && game.native_item_energy(stack).is_some_and(|b| b.amount < b.capacity && b.max_insert > 0),
        Kind::Painter => match slot {
            0 => painter_partial(game, Some(stack), at(1).as_ref()),
            1 => painter_partial(game, at(0).as_ref(), Some(stack)),
            _ => false,
        },
        _ => false,
    }
}

// ---- The tick of each phase 2 machine

pub(in crate::game) fn tick(game: &mut Game<'_>, c: &Caches, pos: Pos, e: &mut ModBlockEntity, m: &'static Mach, passed: bool) {
    match m.kind {
        Kind::Combustion => combustion_tick(game, c, pos, e, m, passed),
        Kind::Zombie => zombie_tick(game, c, pos, e, m, passed),
        Kind::Enchanter => enchanter_tick(game, pos, e),
        Kind::Tank => tank_tick(game, pos, e, m, passed),
        Kind::Charger => charger_tick(game, c, e, m, passed),
        _ => process_tasks(game, c, pos, e, m, passed),
    }
}

/// CombustionMath: (energy per tick, ticks per coolant mB, ticks per fuel mB) for the current fuel and coolant.
fn combustion_math(c: &Caches, k: &C2, e: &ModBlockEntity, m: &Mach) -> (i32, i32, i32) {
    let fuel = gs(&e.nbt, "hz_cur_fuel").or_else(|| read_tank(&e.nbt, "fuelTank", 5000).fluid);
    let coolant = gs(&e.nbt, "hz_cur_coolant").or_else(|| read_tank(&e.nbt, "coolantTank", 5000).fluid);
    let (Some(fuel), Some(coolant)) = (fuel.and_then(|f| k.fuel(&f)), coolant.and_then(|f| k.coolant(&f))) else { return (0, 0, 0) };
    let data = cap_data(e, m);
    let cap_q = c.float(&m.key("gen"), &data);
    let mach_q = m.efficiency.map_or(1.0, |key| c.float(key, &data));
    if cap_q == 0.0 || mach_q == 0.0 {
        return (0, 0, 0);
    }
    let ept = (fuel.per_tick as f32 * cap_q * mach_q).round() as i32;
    let cooling = (373.25 - temperature(&coolant.fluid)) * coolant.degrees;
    let to_cool = f64::from(1.15e-4f32) * f64::from(ept) * f64::from(mach_q);
    let tpc = ((cooling / to_cool).round() as i32).max(1);
    let tpf = ((fuel.ticks as f32 / cap_q / 1000.0) as i32).max(1);
    (ept, tpc, tpf)
}

/// TileCombustionGenerator.processTasks / generateEnergy.
fn combustion_tick(game: &mut Game<'_>, c: &Caches, pos: Pos, e: &mut ModBlockEntity, m: &Mach, passed: bool) {
    let k = c2(game);
    if !passed {
        si(&mut e.nbt, "active", 0);
        return;
    }
    let max = max_energy(c, e, m);
    let mut coolant = read_tank(&e.nbt, "coolantTank", 5000);
    let mut fuel = read_tank(&e.nbt, "fuelTank", 5000);
    let mut trc = gi(&e.nbt, "ticksRemaingCoolant", 0);
    let mut trf = gi(&e.nbt, "ticksRemaingFuel", 0);
    let mut pause = gi(&e.nbt, "hz_pause", 0) != 0;
    let mut generated = 0;
    let stored = energy(e);
    let mut active = false;
    if (trc > 0 || coolant.amount > 0) && (trf > 0 || fuel.amount > 0) && stored < max {
        let (mut ept, mut tpc, mut tpf) = combustion_math(c, &k, e, m);
        let paused = pause && stored >= max - ept * 200 && stored > max / 8;
        if !paused {
            pause = false;
            if !(trc > 0 && trf > 0 && ept > 0) {
                // The spent fuel or coolant is forgotten; the tanks' are read again.
                if trf <= 0 {
                    e.nbt.remove(&js("hz_cur_fuel"));
                }
                if trc <= 0 {
                    e.nbt.remove(&js("hz_cur_coolant"));
                }
                (ept, tpc, tpf) = combustion_math(c, &k, e, m);
                if ept > 0 {
                    if trc <= 0 {
                        if let Some(f) = coolant.fluid.clone() {
                            ss(&mut e.nbt, "hz_cur_coolant", &f);
                        }
                        trc += tpc * coolant.remove(100);
                    }
                    if trf <= 0 {
                        if let Some(f) = fuel.fluid.clone() {
                            ss(&mut e.nbt, "hz_cur_fuel", &f);
                        }
                        trf += tpf * fuel.remove(100);
                    }
                }
            }
            if ept > 0 && trc > 0 && trf > 0 {
                trc -= 1;
                trf -= 1;
                generated = ept;
                set_energy(e, stored + ept, max);
                active = true;
            }
        }
        si(&mut e.nbt, "hz_tpc", tpc);
        si(&mut e.nbt, "hz_tpf", tpf);
    }
    write_tank(&mut e.nbt, "coolantTank", &coolant);
    write_tank(&mut e.nbt, "fuelTank", &fuel);
    si(&mut e.nbt, "ticksRemaingCoolant", trc);
    si(&mut e.nbt, "ticksRemaingFuel", trf);
    si(&mut e.nbt, "active", i32::from(active));
    si(&mut e.nbt, "generated", generated);
    if energy(e) >= max {
        pause = true;
    }
    si(&mut e.nbt, "hz_pause", i32::from(pause));
    // transmitEnergy with the send key: 1280 / 3840 / 6400 µI/t (the enhanced one a level up).
    let stored = energy(e);
    if stored > 0 {
        let send = stored.min(c.int(&m.key("send"), &cap_data(e, m)));
        let sent = distribute(game, pos, e, send);
        set_energy(e, stored - sent, max);
    }
}

/// TileZombieGenerator.processTasks / generateEnergy.
fn zombie_tick(game: &mut Game<'_>, c: &Caches, pos: Pos, e: &mut ModBlockEntity, m: &Mach, passed: bool) {
    if !passed {
        si(&mut e.nbt, "active", 0);
        return;
    }
    let max = max_energy(c, e, m);
    let rate = power_use(c, e, m);
    let per_bucket = if m.id == "enderio:ender_generator" { 96000.0 } else { 12000.0 };
    let mut tank = read_tank(&e.nbt, "tank", 2000);
    let mut left = gf(&e.nbt, "ticksRemaingFuel", 0.0);
    let mut pause = gi(&e.nbt, "hz_pause", 0) != 0;
    let stored = energy(e);
    // minimumTankLevel 0.7 of 2000 mB.
    let activation = (2000.0 * 0.7f32) as i32;
    let active = if raw_cap(e, m).none {
        false
    } else if left >= 1.0 {
        left -= 1.0;
        set_energy(e, stored + rate, max);
        true
    } else if tank.amount < activation || (pause && stored >= max - rate * 200 && stored > max / 8) {
        false
    } else {
        pause = false;
        while left < 1.0 && tank.amount > 0 {
            tank.remove(1);
            left += per_bucket / 1000.0;
        }
        if left >= 1.0 {
            left -= 1.0;
            set_energy(e, stored + rate, max);
            true
        } else {
            false
        }
    };
    write_tank(&mut e.nbt, "tank", &tank);
    sf(&mut e.nbt, "ticksRemaingFuel", left);
    si(&mut e.nbt, "active", i32::from(active));
    if energy(e) >= max {
        pause = true;
    }
    si(&mut e.nbt, "hz_pause", i32::from(pause));
    // getMaxEnergySent: no send key, twice the generation.
    let stored = energy(e);
    if stored > 0 {
        let sent = distribute(game, pos, e, stored.min(2 * rate));
        set_energy(e, stored - sent, max);
    }
}

/// TileSoulBinder.getPitch: 0.75 + pitch key x 0.05 + a random 0.08 - 0.04, within 0.7 and 1.05.
pub(in crate::game) fn soul_pitch(game: &mut Game<'_>, c: &Caches, e: &ModBlockEntity, m: &Mach) -> f32 {
    let key = c.int(&m.key("pitch"), &cap_data(e, m)) as f32;
    (0.75 + key * 0.05 + next_float(game) * 0.08 - 0.04).clamp(0.7, 1.05)
}

// ---- The task machines (vat, soul binder, slice'n'splice, painter)

/// VatRecipe.matchRecipe: each item an input of its own group, the tank holding enough: (in mB, out mB).
fn vat_match(r: &VatRecipe, items: &[Option<ItemStack>], tank: &Tank) -> Option<(i32, i32)> {
    let present = items.iter().flatten().count() + usize::from(tank.amount > 0);
    if present < r.groups.len() + 1 {
        return None;
    }
    let find = |g: usize| -> Option<f32> {
        let s = items.get(g)?.as_ref()?;
        r.groups.get(g)?.iter().find(|(i, _)| i.test(s)).map(|(_, mult)| *mult)
    };
    let m0 = find(0)?;
    let im = if r.groups.len() == 2 { m0 * find(1)? } else { m0 };
    let in_mb = (1000.0 * im).round() as i32;
    let out_mb = (im * r.fluid_mult * 1000.0).round() as i32;
    (tank.fluid.as_deref() == Some(r.fluid_in.as_str()) && tank.amount >= in_mb).then_some((in_mb, out_mb))
}

/// The captured mob of a soul vial (CapturedMob.create(stack)): its entity id.
fn soul_of(stack: &ItemStack) -> Option<String> {
    let d = custom_data(stack)?;
    gs(d, "entityId").or_else(|| d.get(&js("entity")).and_then(|t| if let Tag::Compound(c) = t { gs(c, "id") } else { None }))
}

// Vanilla's IMob and IAnimals classes for the binder's MONSTERS and ANIMALS (DynamicSoulBinderRecipe).
const MONSTERS: [&str; 34] = ["blaze", "bogged", "breeze", "cave_spider", "creaking", "creeper", "drowned", "elder_guardian", "ender_dragon", "endermite", "evoker", "ghast", "giant", "guardian", "hoglin", "husk", "illusioner", "magma_cube", "phantom", "piglin_brute", "pillager", "ravager", "shulker", "silverfish", "skeleton", "slime", "spider", "stray", "vex", "vindicator", "warden", "witch", "wither", "wither_skeleton"];
const MONSTERS2: [&str; 5] = ["zoglin", "zombie", "zombie_villager", "zombified_piglin", "enderman"];
const ANIMALS: [&str; 38] = ["allay", "armadillo", "axolotl", "bat", "bee", "camel", "cat", "chicken", "cod", "cow", "dolphin", "donkey", "fox", "frog", "glow_squid", "goat", "horse", "llama", "mooshroom", "mule", "ocelot", "panda", "parrot", "pig", "polar_bear", "pufferfish", "rabbit", "salmon", "sheep", "sniffer", "squid", "strider", "tadpole", "trader_llama", "tropical_fish", "turtle", "villager", "wolf"];

fn monster(id: &str) -> bool {
    id.strip_prefix("minecraft:").is_some_and(|p| MONSTERS.contains(&p) || MONSTERS2.contains(&p))
}

fn animal(id: &str) -> bool {
    id.strip_prefix("minecraft:").is_some_and(|p| ANIMALS.contains(&p) || p == "wandering_trader" || p == "iron_golem" || p == "snow_golem")
}

fn soul_supported(r: &SoulRecipe, soul: &str) -> bool {
    match r.mode.as_str() {
        "listed" => r.souls.iter().any(|s| s == soul),
        "monsters" => monster(soul),
        "animals" => animal(soul) && !monster(soul),
        _ => true,
    }
}

/// AbstractSoulBinderRecipe.isValidInput / isRecipe: slot 0 a vial with a soul the recipe takes, slot 1 its input
/// (the item, not its NBT); `both` asks for both.
fn soul_fits(r: &SoulRecipe, vial: Option<&ItemStack>, item: Option<&ItemStack>, both: bool) -> bool {
    let v = vial.map(|v| &*v.item == "enderio:filled_soul_vial" && soul_of(v).is_some_and(|s| soul_supported(r, &s)));
    let i = item.map(|s| r.input.order.iter().any(|id| *id == *s.item));
    if both { v == Some(true) && i == Some(true) } else { v != Some(false) && i != Some(false) && (v.is_some() || i.is_some()) }
}

/// The binder's next recipe for its inputs (getNextRecipe).
fn soul_recipe<'a>(k: &'a C2, e: &ModBlockEntity) -> Option<&'a SoulRecipe> {
    let a = e.items.first().cloned().flatten();
    let b = e.items.get(1).cloned().flatten();
    k.soul.iter().find(|r| soul_fits(r, a.as_ref(), b.as_ref(), true))
}

/// getXPRequired: what the next recipe needs over what the binder holds; 0 during a task.
fn xp_required(k: &C2, e: &ModBlockEntity) -> i64 {
    if task(e).is_some() {
        return 0;
    }
    soul_recipe(k, e).map_or(0, |r| exp_for_level(r.levels) - xp_of(e).min(i64::from(i32::MAX)))
}

/// getExcessXP.
fn xp_excess(k: &C2, e: &ModBlockEntity) -> i64 {
    let have = xp_of(e).min(i64::from(i32::MAX));
    if task(e).is_none() {
        if let Some(r) = soul_recipe(k, e) {
            return (have - exp_for_level(r.levels)).max(0);
        }
    }
    have
}

/// The binder's result (OutputFilter): APPLY puts the soul on the output, FILTER adds it to the filter's list.
fn soul_output(r: &SoulRecipe, vial: &ItemStack) -> ItemStack {
    let mut out = r.output.clone();
    let soul = custom_data(vial).cloned().unwrap_or_default();
    match r.logic.as_str() {
        "apply" => {
            let mut d = custom_data(&out).cloned().unwrap_or_default();
            for k in ["entityId", "entity", "customName"] {
                if let Some(v) = soul.get(&js(k)) {
                    d.insert(js(k), v.clone());
                }
            }
            set_custom_data(&mut out, d);
        }
        "filter" => {
            let mut d = custom_data(&out).cloned().unwrap_or_default();
            let mut list = match d.get(&js("souls")) {
                Some(Tag::List(l)) => l.clone(),
                _ => Default::default(),
            };
            let mut one = Compound::new();
            if let Some(id) = soul_of(vial) {
                ss(&mut one, "entityId", &id);
            }
            list.push(Tag::Compound(one));
            d.insert(js("souls"), Tag::List(list));
            set_custom_data(&mut out, d);
        }
        _ => {}
    }
    out
}

/// Recipe.isInputForRecipe for the slice'n'splice: shapeless, each grid item to a different input, all used.
fn slice_match(r: &SliceRecipe, grid: &[Option<ItemStack>]) -> bool {
    let mut open: Vec<usize> = (0..6).filter(|i| r.inputs[*i].is_some()).collect();
    for s in grid.iter().flatten() {
        let Some(at) = open.iter().position(|i| r.inputs[*i].as_ref().is_some_and(|ing| ing.test(s))) else { return false };
        open.remove(at);
    }
    open.is_empty() && grid.iter().any(Option::is_some)
}

pub(in crate::game) fn next_task(game: &Game<'_>, e: &ModBlockEntity, m: &Mach, seed: i64) -> Option<(Task, Vec<(usize, u32)>)> {
    let k = c2(game);
    let at = |i: usize| e.items.get(i).cloned().flatten();
    let blank = |kind: i32, uid: &str, energy: i32| Task { kind, recipe: uid.to_owned(), used: 0.0, required: energy as f32, seed, out_mult: 1.0, chance_mult: 1.0, batch: 1, item: String::new(), results: Vec::new(), fluid_in: String::new(), fluid_out: String::new(), out_mb: 0 };
    match m.kind {
        Kind::Vat => {
            let items = [at(0), at(1)];
            let tank = read_tank(&e.nbt, "inputTank", 8000);
            for r in &k.vat {
                let Some((in_mb, out_mb)) = vat_match(r, &items, &tank) else { continue };
                let mut t = blank(3, &r.uid, r.energy);
                t.fluid_in = r.fluid_in.clone();
                t.fluid_out = r.fluid_out.clone();
                t.out_mb = out_mb;
                t.batch = in_mb.max(0) as u32;
                let consumed = (0..r.groups.len()).map(|g| (g, 1)).collect();
                return Some((t, consumed));
            }
            None
        }
        Kind::Soul => {
            let r = soul_recipe(&k, e)?;
            let vial = at(0)?;
            let mut t = blank(4, &r.uid, r.energy);
            t.results = vec![ItemStack::new("enderio:soul_vial", 1), soul_output(r, &vial)];
            t.batch = r.levels.max(0) as u32;
            Some((t, vec![(0, 1), (1, 1)]))
        }
        Kind::Slice => {
            let grid: Vec<Option<ItemStack>> = (0..6).map(at).collect();
            let r = k.slice.iter().find(|r| slice_match(r, &grid))?;
            let mut t = blank(5, &r.uid, r.energy);
            t.results = vec![r.output.clone()];
            Some((t, (0..6).filter(|i| grid[*i].is_some()).map(|i| (i, 1)).collect()))
        }
        Kind::Painter => {
            let target = at(0)?;
            let paint = at(1)?;
            let out = paint_result(game, &target, &paint)?;
            // RecipeConfig.energyPerTask: 2000 µI a painting; one target, no paint.
            let mut t = blank(6, "painter", 2000);
            t.results = vec![out];
            Some((t, vec![(0, 1)]))
        }
        _ => None,
    }
}

/// canInsertResult's own parts: the vat's output tank takes the whole result; the binder has the XP.
pub(in crate::game) fn can_insert(e: &ModBlockEntity, m: &Mach, t: &Task) -> bool {
    match m.kind {
        Kind::Vat => {
            let mut out = read_tank(&e.nbt, "outputTank", 8000);
            out.fill(&t.fluid_out, t.out_mb, false) >= t.out_mb
        }
        Kind::Soul => xp_of(e) >= exp_for_level(t.batch as i32),
        _ => true,
    }
}

/// startNextTask's own parts: the vat drains its input mB; the binder its XP.
pub(in crate::game) fn task_started(game: &mut Game<'_>, e: &mut ModBlockEntity, m: &Mach, t: &Task) {
    match m.kind {
        Kind::Vat => {
            let mut tank = read_tank(&e.nbt, "inputTank", 8000);
            tank.remove(t.batch as i32);
            write_tank(&mut e.nbt, "inputTank", &tank);
        }
        Kind::Soul => {
            let need = exp_for_level(t.batch as i32);
            let have = xp_of(e);
            set_xp(e, have - need);
        }
        _ => {}
    }
    let _ = game;
}

/// taskComplete's own parts: the vat's fluid result; each slice'n'splice tool takes a damage.
pub(in crate::game) fn task_done(game: &mut Game<'_>, e: &mut ModBlockEntity, m: &Mach, t: &Task) {
    match m.kind {
        Kind::Vat => {
            let mut out = read_tank(&e.nbt, "outputTank", 8000);
            out.fill(&t.fluid_out, t.out_mb, true);
            write_tank(&mut e.nbt, "outputTank", &out);
        }
        Kind::Slice => {
            damage_tool(game, e, 6);
            damage_tool(game, e, 7);
        }
        _ => {}
    }
}

/// TileSliceAndSplice: no task starts or goes on without both tools.
pub(in crate::game) fn can_work(game: &Game<'_>, e: &ModBlockEntity, m: &Mach) -> bool {
    if m.kind != Kind::Slice {
        return true;
    }
    let k = c2(game);
    let at = |i: usize| e.items.get(i).cloned().flatten();
    at(6).is_some_and(|s| k.axes.as_ref().is_some_and(|i| i.test(&s))) && at(7).is_some_and(|s| k.shears.as_ref().is_some_and(|i| i.test(&s)))
}

/// usePower(want > 0) on the slice'n'splice: each tool takes a damage at toolDamageChance 0.01.
pub(in crate::game) fn power_used(game: &mut Game<'_>, e: &mut ModBlockEntity, m: &Mach, want: i32) {
    if m.kind != Kind::Slice || want <= 0 {
        return;
    }
    for slot in [6, 7] {
        if next_float(game) < 0.01 {
            damage_tool(game, e, slot);
        }
    }
}

/// ItemStack.damageItem(1, fakePlayer): Unbreaking may spare it; a tool worn out goes.
fn damage_tool(game: &mut Game<'_>, e: &mut ModBlockEntity, slot: usize) {
    let Some(mut st) = e.items.get(slot).cloned().flatten() else { return };
    let max = game.res.items().get(&st.item).map_or(0, |i| i.max_damage);
    if max == 0 {
        return;
    }
    let unbreaking = st.enchantment_level("minecraft:unbreaking");
    if unbreaking > 0 {
        let dim = game.native_dimension();
        let spared = game.native_level_random(dim).and_then(|r| r.next_int_bound(unbreaking as i32 + 1).ok()).unwrap_or(0) > 0;
        if spared {
            return;
        }
    }
    st.damage += 1;
    e.items[slot] = if st.damage >= max { None } else { Some(st) };
}

// ---- The painter (base/recipe/painter, base/block/painted/*): its templates

#[derive(Clone, Copy, PartialEq)]
enum PaintKind {
    Any,
    Solid,
    NonSolid,
}

const WOOD: [&str; 9] = ["oak", "spruce", "birch", "jungle", "acacia", "dark_oak", "mangrove", "cherry", "pale_oak"];

// (painted block, the blocks it paints, which paints it takes). The two kinds of glowstone, packed ice, redstone and
// reinforced obsidian take opaque full cubes (ISolidBlockPaintableBlock) and the rest (INonSolidBlockPaintableBlock).
static TEMPLATES: std::sync::LazyLock<Vec<(&'static str, Vec<&'static str>, PaintKind)>> = std::sync::LazyLock::new(|| {
    let leak = |v: Vec<String>| -> Vec<&'static str> { v.into_iter().map(|s| &*Box::leak(s.into_boxed_str())).collect() };
    let wood = |suffix: &str| leak(WOOD.iter().map(|w| format!("minecraft:{w}_{suffix}")).collect());
    let colours = ["white", "orange", "magenta", "light_blue", "yellow", "lime", "pink", "gray", "light_gray", "cyan", "purple", "blue", "brown", "green", "red", "black"];
    vec![
        ("enderio:painted_carpet", leak(colours.iter().map(|c| format!("minecraft:{c}_carpet")).collect()), PaintKind::Any),
        ("enderio:painted_fence", wood("fence"), PaintKind::Any),
        ("enderio:painted_stone_fence", vec!["minecraft:nether_brick_fence"], PaintKind::Any),
        ("enderio:painted_fence_gate", wood("fence_gate"), PaintKind::Any),
        ("enderio:painted_glowstone_solid", vec!["minecraft:glowstone"], PaintKind::Solid),
        ("enderio:painted_glowstone", vec!["minecraft:glowstone"], PaintKind::NonSolid),
        ("enderio:painted_packed_ice_solid", vec!["minecraft:packed_ice"], PaintKind::Solid),
        ("enderio:painted_packed_ice", vec!["minecraft:packed_ice"], PaintKind::NonSolid),
        ("enderio:painted_redstone_block_solid", vec!["minecraft:redstone_block"], PaintKind::Solid),
        ("enderio:painted_redstone_block", vec!["minecraft:redstone_block"], PaintKind::NonSolid),
        ("enderio:painted_reinforced_obsidian", vec!["enderio:reinforced_obsidian"], PaintKind::Solid),
        ("enderio:painted_reinforced_obsidian_nonsolid", vec!["enderio:reinforced_obsidian"], PaintKind::NonSolid),
        ("enderio:painted_sand", vec!["minecraft:sand"], PaintKind::Any),
        ("enderio:painted_cobblestone", vec!["minecraft:cobblestone"], PaintKind::Any),
        ("enderio:painted_crafting_table", vec!["minecraft:crafting_table"], PaintKind::Any),
        ("enderio:painted_slab", wood("slab"), PaintKind::Any),
        ("enderio:painted_stone_slab", vec!["minecraft:stone_slab", "minecraft:sandstone_slab", "minecraft:cobblestone_slab", "minecraft:brick_slab", "minecraft:stone_brick_slab", "minecraft:nether_brick_slab", "minecraft:quartz_slab"], PaintKind::Any),
        ("enderio:painted_stairs", wood("stairs"), PaintKind::Any),
        ("enderio:painted_stone_stairs", vec!["minecraft:cobblestone_stairs", "minecraft:brick_stairs", "minecraft:stone_brick_stairs", "minecraft:nether_brick_stairs", "minecraft:sandstone_stairs", "minecraft:quartz_stairs", "minecraft:red_sandstone_stairs", "minecraft:purpur_stairs"], PaintKind::Any),
        ("enderio:painted_wooden_trapdoor", wood("trapdoor"), PaintKind::Any),
        ("enderio:painted_iron_trapdoor", vec!["minecraft:iron_trapdoor"], PaintKind::Any),
        ("enderio:painted_dark_steel_trapdoor", vec!["enderio:dark_steel_trapdoor"], PaintKind::Any),
        ("enderio:painted_wall", vec!["minecraft:cobblestone_wall", "minecraft:mossy_cobblestone_wall"], PaintKind::Any),
        ("enderio:painted_wooden_door", wood("door"), PaintKind::Any),
        ("enderio:painted_iron_door", vec!["minecraft:iron_door"], PaintKind::Any),
        ("enderio:painted_dark_steel_door", vec!["enderio:dark_steel_door"], PaintKind::Any),
        ("enderio:painted_wooden_pressure_plate", vec!["minecraft:oak_pressure_plate"], PaintKind::Any),
        ("enderio:painted_stone_pressure_plate", vec!["minecraft:stone_pressure_plate"], PaintKind::Any),
        ("enderio:painted_heavy_weighted_pressure_plate", vec!["minecraft:heavy_weighted_pressure_plate"], PaintKind::Any),
        ("enderio:painted_light_weighted_pressure_plate", vec!["minecraft:light_weighted_pressure_plate"], PaintKind::Any),
        ("enderio:painted_dark_steel_pressure_plate", vec!["enderio:dark_steel_pressure_plate"], PaintKind::Any),
        ("enderio:painted_soularium_pressure_plate", vec!["enderio:soularium_pressure_plate"], PaintKind::Any),
        ("enderio:painted_fused_quartz", vec!["enderio:fused_quartz"], PaintKind::Any),
        ("enderio:painted_fused_glass", vec!["enderio:fused_glass"], PaintKind::Any),
        ("enderio:painted_enlightened_fused_quartz", vec!["enderio:enlightened_fused_quartz"], PaintKind::Any),
        ("enderio:painted_enlightened_fused_glass", vec!["enderio:enlightened_fused_glass"], PaintKind::Any),
        ("enderio:painted_dark_fused_quartz", vec!["enderio:dark_fused_quartz"], PaintKind::Any),
        ("enderio:painted_dark_fused_glass", vec!["enderio:dark_fused_glass"], PaintKind::Any),
    ]
});

/// PaintUtil.isValid: a block item, not painted itself, of the right kind for the target.
fn paint_valid(game: &Game<'_>, paint: &ItemStack, kind: PaintKind) -> bool {
    if custom_data(paint).is_some_and(|d| d.contains_key(&js("paintSource"))) || paint.item.starts_with("enderio:painted_") {
        return false;
    }
    let Some(state) = game.native_default_state(&paint.item) else { return false };
    let f = game.res.facts().get(state);
    let solid = f.solid_render && f.full_collision;
    match kind {
        PaintKind::Any => true,
        PaintKind::Solid => solid,
        PaintKind::NonSolid => !solid,
    }
}

/// isPartialRecipe: some template takes the pair (either may be missing).
fn painter_partial(game: &Game<'_>, target: Option<&ItemStack>, paint: Option<&ItemStack>) -> bool {
    // FacadePainterRecipe, the conduit facades' template.
    if super::enderio_facades::facade_partial(game, target, paint) {
        return true;
    }
    TEMPLATES.iter().any(|(result, targets, kind)| {
        target.is_none_or(|t| targets.contains(&&*t.item) || *t.item == **result) && paint.is_none_or(|p| paint_valid(game, p, *kind) || *p.item == **result)
    })
}

/// BasicPainterTemplate.getCompletedResult: the painted block with the paint on it (the target's own data kept when it
/// was painted already); painting with the painted block itself gives the plain block back.
fn paint_result(game: &Game<'_>, target: &ItemStack, paint: &ItemStack) -> Option<ItemStack> {
    if let Some(r) = super::enderio_facades::paint_facade(game, target, paint) {
        return r;
    }
    for (result, targets, kind) in TEMPLATES.iter() {
        let is_target = targets.contains(&&*target.item) || *target.item == **result;
        if !is_target {
            continue;
        }
        if *paint.item == **result || (*target.item == **result && targets.contains(&&*paint.item)) {
            // Unpainting: the target's original block.
            let original = custom_data(target).and_then(|d| gs(d, "originalItem")).unwrap_or_else(|| targets[0].to_owned());
            return Some(ItemStack::new(&original, 1));
        }
        if !paint_valid(game, paint, *kind) {
            continue;
        }
        let mut out = ItemStack::new(result, 1);
        let mut d = if *target.item == **result { custom_data(target).cloned().unwrap_or_default() } else { Compound::new() };
        ss(&mut d, "paintSource", &paint.item);
        if *target.item != **result {
            ss(&mut d, "originalItem", &target.item);
        }
        set_custom_data(&mut out, d);
        return Some(out);
    }
    None
}

// ---- The enchanter (TileEnchanter, EnchanterRecipe, ContainerEnchanter)

/// The output for slots 0-2: the book, its XP cost in levels, and what taking it consumes from each slot.
fn enchant_preview(game: &Game<'_>, k: &C2, items: &[Option<ItemStack>]) -> Option<(ItemStack, i32, [u32; 3])> {
    let book = items.first()?.as_ref().filter(|s| &*s.item == "minecraft:book")?;
    let input = items.get(1)?.as_ref()?;
    let lapis = items.get(2)?.as_ref()?;
    let lapis_ok = ingredient(game, &serde_json::Value::String("#c:gems/lapis".into())).is_some_and(|i| i.test(lapis));
    if !lapis_ok {
        return None;
    }
    let _ = book;
    for r in &k.ench {
        if !r.input.test(input) {
            continue;
        }
        let e = game.enchantments().get(&r.enchantment)?;
        let max = e.max_level.max(1) as i32;
        let level = ((input.count / r.per_level) as i32).min(max);
        if level < 1 {
            continue;
        }
        // getLapizForLevel: max(1, round((maxLevel == 1 ? 5 : level) x lapisCostFactor 3.0)).
        let lapis_need = ((if max == 1 { 5 } else { level }) as f64 * 3.0).round().max(1.0) as u32;
        if lapis.count < lapis_need {
            continue;
        }
        let raw = |l: i32| -> i32 { ((f64::from(e.min_cost.calculate(l as u32).max(1)) * r.cost_mult * 0.75).round() as i32) + 2 };
        fn cost(l: i32, max: i32, raw: &dyn Fn(i32) -> i32) -> i32 {
            let l = l.min(max);
            let mut c = raw(l);
            if l < max {
                c = c.max(cost(l + 1, max, raw) / 2);
            }
            c.max(1)
        }
        let xp = cost(level, max, &raw);
        let mut out = ItemStack::new("minecraft:enchanted_book", 1);
        out.extra.get_or_insert_with(Default::default).stored_enchantments = vec![(r.enchantment.as_str().into(), level as u32)];
        return Some((out, xp, [1, r.per_level * level as u32, lapis_need]));
    }
    None
}

/// The levels and creative mode of a player.
fn player_levels(game: &mut Game<'_>, uuid: u128) -> Option<(i32, bool)> {
    if uuid == game.native_acting_id() {
        return Some((game.experience.level, game.creative));
    }
    let Some(Tag::Compound(c)) = game.native_player_compound(uuid) else { return None };
    Some((gi(&c, "XpLevel", 0), gi(&c, "playerGameType", 0) == 1))
}

/// The output slot shows the book while the player at the screen can pay (canTakeStack); a take consumes the inputs
/// and the levels (onTake), with the anvil's sound and bonemeal's particles.
fn enchanter_tick(game: &mut Game<'_>, pos: Pos, e: &mut ModBlockEntity) {
    let k = c2(game);
    let shown = gi(&e.nbt, "hz_shown_book", 0) != 0;
    let taken = shown && e.items.get(3).cloned().flatten().is_none();
    let takers = takers(pos);
    if taken {
        let items: Vec<Option<ItemStack>> = (0..3).map(|i| e.items.get(i).cloned().flatten()).collect();
        if let Some((_, cost, consumed)) = enchant_preview(game, &k, &items) {
            for (i, n) in consumed.iter().enumerate() {
                if let Some(s) = e.items[i].as_mut() {
                    s.count = s.count.saturating_sub(*n);
                    if s.count == 0 {
                        e.items[i] = None;
                    }
                }
            }
            if let Some(u) = takers.first().copied().or_else(|| opener(pos)) {
                if player_levels(game, u).is_some_and(|(_, creative)| !creative) {
                    game.native_give_levels(u, -cost);
                }
            }
            game.level_event_at(1030, pos, 0);
            game.level_event_at(2005, [pos[0], pos[1] + 1, pos[2]], 0);
        }
    }
    let items: Vec<Option<ItemStack>> = (0..3).map(|i| e.items.get(i).cloned().flatten()).collect();
    let preview = enchant_preview(game, &k, &items);
    let cost = preview.as_ref().map_or(0, |p| p.1);
    let affordable = opener(pos).and_then(|u| player_levels(game, u)).is_some_and(|(l, creative)| creative || l >= cost);
    si(&mut e.nbt, "hz_cost", cost);
    si(&mut e.nbt, "hz_afford", i32::from(affordable));
    let book = preview.filter(|_| affordable).map(|p| p.0);
    si(&mut e.nbt, "hz_shown_book", i32::from(book.is_some()));
    e.items[3] = book;
}

// ---- The fluid tank (TileTank)

/// VoidMode: 0 ALWAYS, 1 IF_NOT_CONTAINER, 2 NEVER (the default).
fn void_mode(e: &ModBlockEntity) -> i32 {
    gi(&e.nbt, "voidMode", 2).clamp(0, 2)
}

/// canVoidItems: a fluid hotter than 973 K (allowVoiding is on by default).
fn can_void(e: &ModBlockEntity, m: &Mach) -> bool {
    tanks(e, m)[0].fluid.as_deref().is_some_and(|f| temperature(f) > 973.0)
}

fn tank_recipe<'a>(k: &'a C2, fill: bool, stack: &ItemStack, t: &Tank) -> Option<&'a TankRecipe> {
    k.tank.iter().find(|r| r.fill == fill && r.input.test(stack) && if fill { t.fluid.as_deref() == Some(r.fluid.as_str()) && t.amount > 0 } else { t.takes(&r.fluid) })
}

fn tank_valid(game: &Game<'_>, k: &C2, e: &ModBlockEntity, m: &Mach, slot: usize, stack: &ItemStack) -> bool {
    let t = &tanks(e, m)[0];
    let voiding = can_void(e, m);
    let container = item_fluid(game, stack).is_some();
    match slot {
        _ if voiding && void_mode(e) == 0 && slot < 2 => false,
        0 => item_fluid(game, stack).is_some_and(|(f, a, _)| f.is_some() && a > 0) || tank_recipe(k, false, stack, t).is_some(),
        1 => item_fluid(game, stack).is_some_and(|(_, a, c)| a < c) || mendable(stack) || tank_recipe(k, true, stack, t).is_some(),
        2 => voiding && match void_mode(e) {
            0 => true,
            1 => !container,
            _ => false,
        },
        _ => false,
    }
}

fn mendable(stack: &ItemStack) -> bool {
    stack.damage > 0 && stack.enchantment_level("minecraft:mending") > 0
}

/// TankMachineRecipe.Logic.convertFluidResult: the mB a recipe moves (a fresh roll for bottles).
fn tank_amount(game: &mut Game<'_>, r: &TankRecipe) -> i32 {
    match r.logic.as_str() {
        "xpbottle" => {
            let dim = game.native_dimension();
            let (a, b) = game.native_level_random(dim).map_or((2, 2), |rand| (rand.next_int_bound(5).unwrap_or(0), rand.next_int_bound(5).unwrap_or(0)));
            r.amount * 20 * (3 + a + b)
        }
        "xp" => r.amount * 20,
        "levels" => (XP_RATIO * exp_for_level(r.amount)) as i32,
        _ => r.amount,
    }
}

/// An output slot takes `stack` whole (areStackMergable, within the stack size).
fn fits(game: &Game<'_>, out: &Option<ItemStack>, stack: &ItemStack) -> bool {
    out.as_ref().is_none_or(|o| o.same_item_same_components(stack) && o.count + stack.count <= game.native_max_stack_size(&o.item).unwrap_or(64))
}

fn put_out(e: &mut ModBlockEntity, slot: usize, stack: ItemStack) {
    match e.items[slot].as_mut() {
        Some(o) => o.count += stack.count,
        None => e.items[slot] = Some(stack),
    }
}

fn shrink(e: &mut ModBlockEntity, slot: usize, n: u32) {
    if let Some(s) = e.items[slot].as_mut() {
        s.count = s.count.saturating_sub(n);
        if s.count == 0 {
            e.items[slot] = None;
        }
    }
}

/// processItems every 20 ticks (10 for the pressurized tank): void, then drain, fill and mend.
fn tank_tick(game: &mut Game<'_>, pos: Pos, e: &mut ModBlockEntity, m: &Mach, passed: bool) {
    let k = c2(game);
    let mut t = tanks(e, m)[0].clone();
    if passed && work(e, if m.level > 1 { 10 } else { 20 }) {
        // voidItems
        if let Some(st) = e.items[2].clone().filter(|_| can_void(e, m)) {
            if t.amount < t.capacity && t.fluid.as_deref() == Some("minecraft:lava") && game.native_default_state(&st.item).is_some() {
                let g = {
                    let dim = game.native_dimension();
                    game.native_level_random(dim).map_or(0.0, |r| r.next_gaussian())
                };
                let add = (g * 0.75 + 3.5).clamp(1.0, 10.0) as i32;
                t.amount = (t.amount + add).min(t.capacity);
                shrink(e, 2, 1);
            } else {
                shrink(e, 2, 10);
            }
            let pitch = 2.0 + next_float(game) * 0.4;
            game.native_play_sound_in_place("minecraft:entity.generic.burn", [f64::from(pos[0]) + 0.5, f64::from(pos[1]) + 0.5, f64::from(pos[2]) + 0.5], 4, 0.05, pitch);
        }
        // drainFullContainer: slot 0 into the tank, the container to slot 3.
        if let Some(st) = e.items[0].clone().filter(|_| t.amount < t.capacity) {
            let one = ItemStack { count: 1, ..st.clone() };
            let direct = item_drain(game, &one, t.room()).filter(|(f, _, _)| t.takes(f));
            if let Some((f, a, rest)) = direct {
                let rest = (!rest.item.is_empty() && &*rest.item != "minecraft:air").then_some(rest);
                if rest.as_ref().is_none_or(|r| fits(game, &e.items[3], r)) {
                    t.fill(&f, a, true);
                    shrink(e, 0, 1);
                    if let Some(r) = rest {
                        put_out(e, 3, r);
                    }
                }
            } else if let Some(r) = tank_recipe(&k, false, &st, &t) {
                let out = r.output.clone();
                if out.as_ref().is_none_or(|o| fits(game, &e.items[3], o)) {
                    let amount = tank_amount(game, r);
                    // No capacity check on the recipe path (Java's).
                    t.fluid = Some(r.fluid.clone());
                    t.amount += amount;
                    shrink(e, 0, 1);
                    if let Some(o) = out {
                        put_out(e, 3, o);
                    }
                    if r.logic == "xpbottle" {
                        game.level_event_at(2002, pos, 0x0038_5DC6);
                    }
                }
            }
        }
        // fillEmptyContainer: the tank into slot 1, the filled item to slot 4.
        if let (Some(st), Some(f)) = (e.items[1].clone(), t.fluid.clone()) {
            let one = ItemStack { count: 1, ..st.clone() };
            if let Some((n, full)) = item_fill(game, &one, &f, t.amount) {
                if fits(game, &e.items[4], &full) {
                    t.remove(n);
                    shrink(e, 1, 1);
                    put_out(e, 4, full);
                }
            } else if let Some(r) = tank_recipe(&k, true, &st, &t) {
                let amount = tank_amount(game, r);
                if let Some(o) = r.output.clone().filter(|o| fits(game, &e.items[4], o)) {
                    if t.amount >= amount {
                        t.remove(amount);
                        shrink(e, 1, 1);
                        put_out(e, 4, o);
                    }
                }
            }
        }
        // mendItem: XP juice repairs a Mending item, 2 durability a point.
        if let Some(mut st) = e.items[1].clone().filter(|s| mendable(s) && e.items[4].is_none()) {
            if t.fluid.as_deref() == Some(XP) && t.amount > 0 {
                let mend = ((i64::from(t.amount) / XP_RATIO) as i32 * 2).min(st.damage as i32);
                if mend >= 1 {
                    st.damage -= mend as u32;
                    t.remove((XP_RATIO * i64::from(mend / 2)) as i32);
                    if st.damage == 0 {
                        e.items[1] = None;
                        e.items[4] = Some(st);
                    } else {
                        e.items[1] = Some(st);
                    }
                }
            }
        }
    }
    set_tank(e, m, 0, &t);
    // getComparatorOutput: empty 0, else 1 + amount / capacity x 14.
    let cmp = if t.amount == 0 { 0 } else { (1.0 + f64::from(t.amount) / f64::from(t.capacity) * 14.0) as i32 };
    if cmp != gi(&e.nbt, "hz_cmp", 0) {
        si(&mut e.nbt, "hz_cmp", cmp);
        let dim = game.native_dimension().to_owned();
        game.native_update_output_neighbors(&dim, pos);
    }
}

/// The tank's comparator (TileTank.getComparatorOutput).
pub(in crate::game) fn tank_comparator(e: &ModBlockEntity) -> Option<i32> {
    let m = mach(&e.id).filter(|m| m.kind == Kind::Tank)?;
    let t = &tanks(e, m)[0];
    Some(if t.amount == 0 { 0 } else { (1.0 + f64::from(t.amount) / f64::from(t.capacity) * 14.0) as i32 })
}

// ---- The wired charger (TileWiredCharger.processTasks)

fn charger_rate(game: &Game<'_>, c: &Caches, e: &ModBlockEntity, m: &Mach) -> i32 {
    if m.level == 0 {
        return power_use(c, e, m);
    }
    // getPowerUsePerTick: the item's capacity over the charge key (100 / 60 / 20 at levels 1-3).
    let charge = c.int("enderio:block_wired_charger/charge", &cap_data(e, m)).max(1);
    e.items.first().cloned().flatten().and_then(|s| game.native_item_energy(&s)).map_or(0, |b| b.capacity / charge)
}

fn charger_tick(game: &mut Game<'_>, c: &Caches, e: &mut ModBlockEntity, m: &Mach, passed: bool) {
    if !passed || energy(e) <= 0 {
        return;
    }
    let rate = charger_rate(game, c, e, m);
    si(&mut e.nbt, "hz_rate", rate);
    if let Some(st) = e.items[0].clone() {
        let available = rate.min(energy(e));
        if available > 0 {
            if let Some((used, charged)) = game.native_item_energy_insert(&st, available) {
                if used > 0 {
                    let max = max_energy(c, e, m);
                    let now = energy(e);
                    set_energy(e, now - used, max);
                    let p = game.native_item_energy(&charged).map_or(0.0, |b| b.amount as f32 / b.capacity.max(1) as f32);
                    e.items[0] = Some(charged);
                    sf(&mut e.nbt, "hz_progress", p);
                    return;
                }
            }
            if e.items[1].is_none() {
                e.items[1] = Some(st);
                e.items[0] = None;
            }
        }
    }
    sf(&mut e.nbt, "hz_progress", 0.0);
}

// ---- Side IO of fluids (doPull / doPush after the items', every 20 ticks)

/// doPull: up to 250 mB (generators), 100 (vat, tank) or what the binder needs (50 at most) from the neighbour.
pub(in crate::game) fn fluid_pull(game: &mut Game<'_>, pos: Pos, e: &mut ModBlockEntity, m: &Mach, f: usize) -> bool {
    let k = c2(game);
    let dim = game.native_dimension().to_owned();
    let there = step(pos, f);
    let face = FACES[opposite(f)];
    let Some(theirs) = game.native_fluid_tanks(&dim, there, face) else { return false };
    let offered: Vec<String> = theirs.iter().filter(|t| t.can_drain && t.amount > 0).filter_map(|t| t.fluid.clone()).collect();
    let Some(fluid) = offered.first().cloned() else { return false };
    if m.kind == Kind::Soul {
        let need = xp_required(&k, e);
        let max = (XP_RATIO * need).min(50) as i32;
        if need <= 0 || fluid != XP {
            return false;
        }
        let Some((got, n)) = game.native_fluid_extract(&dim, there, face, max) else { return false };
        if got != XP {
            game.native_fluid_insert(there, face, &got, n);
            return false;
        }
        let have = xp_of(e);
        set_xp(e, have + i64::from(n) / XP_RATIO);
        return n > 0;
    }
    let rate = match m.kind {
        Kind::Combustion | Kind::Zombie => 250,
        Kind::Vat | Kind::Tank => 100,
        _ => return false,
    };
    let mut ts = tanks(e, m);
    // The combustion generator pulls into its fuel tank, then its coolant; the others into their input tank.
    let order: Vec<usize> = if m.kind == Kind::Combustion { vec![1, 0] } else { vec![0] };
    for i in order {
        if ts[i].room() <= 0 || !tank_accepts(&k, e, m, i, &fluid) || !ts[i].takes(&fluid) {
            continue;
        }
        let want = rate.min(ts[i].room());
        let Some((got, n)) = game.native_fluid_extract(&dim, there, face, want) else { continue };
        let took = if got == fluid { ts[i].fill(&got, n, true) } else { 0 };
        if took < n {
            game.native_fluid_insert(there, face, &got, n - took);
        }
        if took > 0 {
            set_tank(e, m, i, &ts[i]);
            return true;
        }
    }
    false
}

/// doPush: the vat's output and the tank 100 mB, the binder's spare XP 50 mB.
pub(in crate::game) fn fluid_push(game: &mut Game<'_>, pos: Pos, e: &mut ModBlockEntity, m: &Mach, f: usize) -> bool {
    let there = step(pos, f);
    let face = FACES[opposite(f)];
    if m.kind == Kind::Soul {
        let k = c2(game);
        let max = (XP_RATIO * xp_excess(&k, e)).min(50) as i32;
        let max = max - max % XP_RATIO as i32;
        if max <= 0 {
            return false;
        }
        let left = game.native_fluid_insert(there, face, XP, max);
        let moved = (max - left) / XP_RATIO as i32 * XP_RATIO as i32;
        if moved < max - left {
            // Only whole points leave: the part of a point that went is taken back.
            let _ = game.native_fluid_extract(&game.native_dimension().to_owned(), there, face, max - left - moved);
        }
        let have = xp_of(e);
        set_xp(e, have - i64::from(moved) / XP_RATIO);
        return moved > 0;
    }
    let i = match m.kind {
        Kind::Vat => 1,
        Kind::Tank => 0,
        _ => return false,
    };
    let mut t = tanks(e, m)[i].clone();
    let Some(fluid) = t.fluid.clone() else { return false };
    let n = t.amount.min(100);
    let left = game.native_fluid_insert(there, face, &fluid, n);
    let moved = n - left;
    if moved > 0 {
        t.remove(moved);
        set_tank(e, m, i, &t);
        return true;
    }
    false
}

/// The tanks' own validators: fuels and coolants (FuelTank, CoolantTank), the generator's one fluid, the vat's recipe
/// inputs (getInputTank), the binder's XP.
fn tank_accepts(k: &C2, e: &ModBlockEntity, m: &Mach, i: usize, f: &str) -> bool {
    match (m.kind, i) {
        (Kind::Combustion, 0) => k.coolant(f).is_some(),
        (Kind::Combustion, _) => k.fuel(f).is_some(),
        (Kind::Zombie, _) => f == if m.id == "enderio:ender_generator" { ENDER } else { NUTRIENT },
        (Kind::Vat, 0) => {
            let a = e.items.first().cloned().flatten();
            let b = e.items.get(1).cloned().flatten();
            k.vat.iter().any(|r| r.fluid_in == f && [a.as_ref(), b.as_ref()].iter().enumerate().all(|(g, s)| s.is_none_or(|s| r.groups.get(g).is_some_and(|l| l.iter().any(|(ing, _)| ing.test(s))))))
        }
        (Kind::Tank, _) => true,
        _ => false,
    }
}

/// getInputTank(fluid) of ITankAccess.
fn input_tank(k: &C2, e: &ModBlockEntity, m: &Mach, f: &str) -> Option<usize> {
    match m.kind {
        Kind::Combustion => {
            if k.coolant(f).is_some() {
                Some(0)
            } else if k.fuel(f).is_some() {
                Some(1)
            } else {
                None
            }
        }
        _ => (0..tank_defs(m).len()).find(|i| tank_defs(m)[*i].2 && tank_accepts(k, e, m, *i, f)),
    }
}

// ---- The fluid capability others see (SmartTankFluidMachineHandler): Horizon's custom fluid hooks

pub(in crate::game) fn custom_block_fluids(block: &str, custom: &mut bool) {
    super::enderio_conduits::custom_block_fluids(block, custom);
    if mach(block).is_some_and(|m| matches!(m.kind, Kind::Combustion | Kind::Zombie | Kind::Vat | Kind::Soul | Kind::Tank)) {
        *custom = true;
    }
}

fn side_mode(e: &ModBlockEntity, side: Option<Direction>) -> Option<i32> {
    side.map(|d| face_modes(&e.nbt)[d as usize])
}

fn our_entity<'a>(game: &'a Game<'_>, pos: Pos) -> Option<(&'a ModBlockEntity, &'static Mach)> {
    match game.world.block_entities.get(&(pos[0], pos[1], pos[2])) {
        Some(BlockEntity::Mod(e)) => mach(&e.id).filter(|m| matches!(m.kind, Kind::Combustion | Kind::Zombie | Kind::Vat | Kind::Soul | Kind::Tank)).map(|m| (&**e, m)),
        _ => None,
    }
}

fn stack_of(f: &str, n: i32) -> NativeFluidStack {
    NativeFluidStack { fluid: f.to_owned(), amount: n, components: Compound::new() }
}

pub(in crate::game) fn custom_fluid_tanks(game: &Game<'_>, pos: Pos, side: Option<Direction>, out: &mut Option<Vec<NativeFluidTankView>>) {
    super::enderio_conduits::custom_fluid_tanks(game, pos, side, out);
    if out.is_some() || !game.native_on(NS) {
        return;
    }
    let Some((e, m)) = our_entity(game, pos) else { return };
    let mode = side_mode(e, side);
    // A DISABLED face reaches nothing; no side (the information handler) reads all and moves none.
    if mode == Some(IO_DISABLED) {
        *out = Some(Vec::new());
        return;
    }
    let fill = mode.is_some_and(|m| matches!(m, IO_NONE | IO_PULL | IO_PUSH_PULL));
    let drain = mode.is_some_and(|m| matches!(m, IO_NONE | IO_PUSH | IO_PUSH_PULL));
    if m.kind == Kind::Soul {
        let n = (XP_RATIO * xp_of(e)).min(i64::from(i32::MAX)) as i32;
        *out = Some(vec![NativeFluidTankView { tank: 0, content: (n > 0).then(|| stack_of(XP, n)), capacity: i32::MAX, can_fill: fill, can_drain: drain }]);
        return;
    }
    let defs = tank_defs(m);
    *out = Some(tanks(e, m).into_iter().enumerate().map(|(i, t)| NativeFluidTankView { tank: i, content: t.fluid.as_ref().map(|f| stack_of(f, t.amount)), capacity: t.capacity, can_fill: fill && defs[i].2, can_drain: drain && defs[i].3 }).collect());
}

pub(in crate::game) fn custom_fluid_fill(game: &mut Game<'_>, pos: Pos, side: Option<Direction>, resource: &NativeFluidStack, action: NativeFluidAction, out: &mut Option<i32>) {
    super::enderio_conduits::custom_fluid_fill(game, pos, side, resource, action, out);
    if out.is_some() || !game.native_on(NS) {
        return;
    }
    let Some((e, m)) = our_entity(game, pos) else { return };
    let mode = side_mode(e, side);
    if !mode.is_some_and(|m| matches!(m, IO_NONE | IO_PULL | IO_PUSH_PULL)) || !resource.components.is_empty() || resource.amount <= 0 {
        *out = Some(0);
        return;
    }
    let execute = action == NativeFluidAction::Execute;
    let k = c2(game);
    if m.kind == Kind::Soul {
        // The binder's container takes no more than the next recipe still needs, in whole points.
        let max = (XP_RATIO * xp_required(&k, e)).max(0);
        if resource.fluid != XP || max <= 0 {
            *out = Some(0);
            return;
        }
        let points = i64::from(resource.amount).min(max) / XP_RATIO;
        if execute && points > 0 {
            game.native_with_mod_block_entity(pos, |e| {
                let have = xp_of(e);
                set_xp(e, have + points);
            });
        }
        *out = Some((points * XP_RATIO) as i32);
        return;
    }
    // SideHandler.fill: the first tank that can take this fluid gets it.
    let ts = tanks(e, m);
    let defs = tank_defs(m);
    let Some(i) = (0..ts.len()).find(|i| defs[*i].2 && tank_accepts(&k, e, m, *i, &resource.fluid) && ts[*i].takes(&resource.fluid)) else {
        *out = Some(0);
        return;
    };
    let mut t = ts[i].clone();
    let n = t.fill(&resource.fluid, resource.amount, true);
    if execute && n > 0 {
        game.native_with_mod_block_entity(pos, |e| set_tank(e, m, i, &t));
    }
    *out = Some(n);
}

pub(in crate::game) fn custom_fluid_drain(game: &mut Game<'_>, pos: Pos, side: Option<Direction>, request: &NativeFluidDrain, action: NativeFluidAction, out: &mut Option<NativeFluidStack>) {
    super::enderio_conduits::custom_fluid_drain(game, pos, side, request, action, out);
    if out.is_some() || !game.native_on(NS) {
        return;
    }
    let Some((e, m)) = our_entity(game, pos) else { return };
    let mode = side_mode(e, side);
    if !mode.is_some_and(|m| matches!(m, IO_NONE | IO_PUSH | IO_PUSH_PULL)) {
        return;
    }
    let execute = action == NativeFluidAction::Execute;
    let (want_fluid, max) = match request {
        NativeFluidDrain::Any(n) => (None, *n),
        NativeFluidDrain::Exact(s) if s.components.is_empty() => (Some(s.fluid.clone()), s.amount),
        _ => return,
    };
    if m.kind == Kind::Soul {
        let k = c2(game);
        if want_fluid.as_deref().is_some_and(|f| f != XP) {
            return;
        }
        let points = (i64::from(max) / XP_RATIO).min(xp_excess(&k, e));
        if points <= 0 {
            return;
        }
        if execute {
            game.native_with_mod_block_entity(pos, |e| {
                let have = xp_of(e);
                set_xp(e, have - points);
            });
        }
        *out = Some(stack_of(XP, (points * XP_RATIO) as i32));
        return;
    }
    let defs = tank_defs(m);
    let ts = tanks(e, m);
    // The first drainable tank (the vat's output, the tank); the generators' tanks never drain.
    let Some(i) = (0..ts.len()).find(|i| defs[*i].3) else { return };
    let mut t = ts[i].clone();
    let Some(f) = t.fluid.clone() else { return };
    if want_fluid.as_deref().is_some_and(|w| w != f) {
        return;
    }
    let n = t.remove(max);
    if n <= 0 {
        return;
    }
    if execute {
        game.native_with_mod_block_entity(pos, |e| set_tank(e, m, i, &t));
    }
    *out = Some(stack_of(&f, n));
}

/// The fluid tank's item holds its fluid (TankItemFluidCapability): Horizon's item container of the tank's size.
pub(in crate::game) fn item_fluid_tank(item: &str, capacity: &mut Option<i32>) {
    match item {
        "enderio:fluid_tank" => *capacity = Some(16000),
        "enderio:pressurized_fluid_tank" => *capacity = Some(32000),
        _ => {}
    }
}

/// The dropped item: the tank's fluid as Horizon's item container keeps it too; the enchanter's preview stays behind.
pub(in crate::game) fn item_data(e: &ModBlockEntity, m: &Mach, root: &mut Compound) {
    if m.kind == Kind::Tank {
        let t = &tanks(e, m)[0];
        if let Some(f) = &t.fluid {
            let mut c = Compound::new();
            ss(&mut c, "Fluid", f);
            si(&mut c, "Amount", t.amount);
            root.insert(js("horizon_fluid"), Tag::Compound(c));
        }
    }
    if m.kind == Kind::Enchanter {
        let mut items = e.items.clone();
        if items.len() > 3 {
            items[3] = None;
        }
        if let Some(Tag::Compound(d)) = root.get_mut(&js("data")) {
            d.insert(js("inventory"), horizon_items::item_nbt::encode_items(&items));
        }
    }
}

/// readCustomNBT's tank: a tank item filled through Horizon's item container (its `horizon_fluid`) wins.
pub(in crate::game) fn placed_data(root: &Compound, e: &mut ModBlockEntity, m: &Mach) {
    if m.kind != Kind::Tank {
        return;
    }
    if let Some(Tag::Compound(c)) = root.get(&js("horizon_fluid")) {
        let cap = tank_defs(m)[0].1;
        let amount = gi(c, "Amount", 0).clamp(0, cap);
        let t = Tank { fluid: gs(c, "Fluid").filter(|_| amount > 0), amount, capacity: cap };
        set_tank(e, m, 0, &if t.fluid.is_some() { t } else { Tank { capacity: cap, ..Tank::default() } });
    }
}

// ---- Clicks: buckets on the machines (EnderCore FluidUtil), the soul vial (ItemSoulVial)

fn set_held(game: &mut Game<'_>, stack: Option<ItemStack>) {
    let i = game.selected;
    if let Some(slot) = game.inventory.get_mut(i) {
        *slot = stack;
    }
}

/// The hand's one item turned into `result`: replaced when it was the last, else given (or dropped) beside it.
fn swap_held(game: &mut Game<'_>, uuid: u128, held: &ItemStack, result: ItemStack, pos: Pos) {
    if held.count <= 1 {
        set_held(game, Some(result));
        return;
    }
    set_held(game, Some(ItemStack { count: held.count - 1, ..held.clone() }));
    if let Some(rest) = game.native_give_stack(uuid, result) {
        game.spawn_item(rest, [f64::from(pos[0]) + 0.5, f64::from(pos[1]) + 1.0, f64::from(pos[2]) + 0.5], [0.0; 3], 10);
    }
}

/// fillInternalTankFromPlayerHandItem, then fillPlayerHandItemFromInternalTank. True when one moved fluid.
pub(in crate::game) fn bucket_click(game: &mut Game<'_>, pos: Pos) -> bool {
    let Some(player) = game.native_acting_player() else { return false };
    let Some(held) = game.native_held_stack(player.uuid) else { return false };
    let Some(BlockEntity::Mod(e)) = game.world.block_entities.get(&(pos[0], pos[1], pos[2])) else { return false };
    let e = ModBlockEntity::clone(e);
    let Some(m) = mach(&e.id).filter(|m| matches!(m.kind, Kind::Combustion | Kind::Zombie | Kind::Vat | Kind::Soul | Kind::Tank)) else { return false };
    let Some((content, _, _)) = item_fluid(game, &held) else { return false };
    let k = c2(game);
    let creative = game.creative;
    let one = ItemStack { count: 1, ..held.clone() };
    // Into the machine.
    if let Some(f) = content.clone() {
        if m.kind == Kind::Soul {
            if f == XP {
                if let Some((_, n, rest)) = item_drain(game, &one, i32::MAX) {
                    game.native_with_mod_block_entity(pos, |e| {
                        let have = xp_of(e);
                        set_xp(e, have + i64::from(n) / XP_RATIO);
                    });
                    if !creative {
                        swap_held(game, player.uuid, &held, rest, pos);
                    }
                    return true;
                }
            }
        } else if let Some(i) = input_tank(&k, &e, m, &f) {
            let t = tanks(&e, m)[i].clone();
            if t.takes(&f) {
                if let Some((g, n, rest)) = item_drain(game, &one, t.room()) {
                    let mut t = t;
                    t.fluid = Some(g);
                    t.amount += n;
                    game.native_with_mod_block_entity(pos, |e| set_tank(e, m, i, &t));
                    if !creative {
                        swap_held(game, player.uuid, &held, rest, pos);
                    }
                    return true;
                }
            }
        }
    }
    // Out of the machine, into the container (the output tanks).
    let outs: Vec<usize> = match m.kind {
        Kind::Vat => vec![1],
        Kind::Tank => vec![0],
        Kind::Soul => vec![0],
        _ => Vec::new(),
    };
    for i in outs {
        let (f, amount) = if m.kind == Kind::Soul {
            (XP.to_owned(), (XP_RATIO * xp_of(&e)).min(i64::from(i32::MAX)) as i32)
        } else {
            let t = tanks(&e, m)[i].clone();
            match t.fluid {
                Some(f) => (f, t.amount),
                None => continue,
            }
        };
        if amount <= 0 {
            continue;
        }
        let Some((n, full)) = item_fill(game, &one, &f, amount) else { continue };
        if m.kind == Kind::Soul {
            game.native_with_mod_block_entity(pos, |e| {
                let have = xp_of(e);
                set_xp(e, have - i64::from(n) / XP_RATIO);
            });
        } else {
            game.native_with_mod_block_entity(pos, |e| {
                let mut t = tanks(e, m)[i].clone();
                t.remove(n);
                set_tank(e, m, i, &t);
            });
        }
        // A creative player's empty bucket stays as it was (doFill).
        if !creative || &*held.item != "minecraft:bucket" {
            swap_held(game, player.uuid, &held, full, pos);
        }
        return true;
    }
    false
}

/// ItemSoulVial.onItemUse: a filled vial lets its mob out on the face clicked; an empty one comes back.
pub(in crate::game) fn right_click_any(game: &mut Game<'_>, pos: Pos, face: [i32; 3], canceled: &mut bool) -> bool {
    let Some(player) = game.native_acting_player() else { return false };
    let Some(held) = game.native_held_stack(player.uuid).filter(|h| &*h.item == "enderio:filled_soul_vial") else { return false };
    let Some(data) = custom_data(&held).cloned() else { return false };
    let Some(kind) = soul_of(&held) else { return false };
    *canceled = true;
    let mut nbt = match data.get(&js("entity")) {
        Some(Tag::Compound(c)) => c.clone(),
        _ => Compound::new(),
    };
    for k in ["UUID", "Pos", "Motion", "Rotation", "id"] {
        nbt.remove(&js(k));
    }
    if let Some(name) = gs(&data, "customName") {
        nbt.insert(js("CustomName"), Tag::String(js(&name)));
    }
    let at = [f64::from(pos[0] + face[0]) + 0.5, f64::from(pos[1] + face[1]), f64::from(pos[2] + face[2]) + 0.5];
    let dim = game.native_dimension().to_owned();
    if game.native_summon(&dim, &kind, at, nbt).is_some() && !game.creative {
        swap_held(game, player.uuid, &held, ItemStack::new("enderio:soul_vial", 1), pos);
    }
    true
}

/// ItemSoulVial.itemInteractionForEntity: an empty vial (or any vial in creative) takes a living mob, never a player;
/// in creative the mob stays and a filled vial is given.
pub(in crate::game) fn interact_entity(game: &mut Game<'_>, entity: i32, kind: &str, stack: Option<&ItemStack>, claimed: &mut bool) {
    if *claimed || !game.native_on(NS) {
        return;
    }
    let Some(held) = stack.filter(|s| &*s.item == "enderio:soul_vial" || &*s.item == "enderio:filled_soul_vial") else { return };
    let creative = game.creative;
    if &*held.item == "enderio:filled_soul_vial" && !creative {
        return;
    }
    let Some(player) = game.native_acting_player() else { return };
    *claimed = true;
    if kind == "minecraft:player" {
        game.native_tell(player.uuid, horizon_client::text::Text::legacy(&lang("enderio.chat.soulvial.denied_player", "Cannot capture players")));
        return;
    }
    let dim = game.native_dimension().to_owned();
    let Some(r) = game.native_entity(entity as u32) else { return };
    let Some(mut nbt) = game.native_entity_nbt(r) else { return };
    if game.native_mob(&dim, entity as u32).is_none_or(|m| m.health <= 0.0) {
        return;
    }
    nbt.insert(js("id"), Tag::String(js(kind)));
    let mut data = Compound::new();
    ss(&mut data, "entityId", kind);
    data.insert(js("entity"), Tag::Compound(nbt));
    let mut vial = ItemStack::new("enderio:filled_soul_vial", 1);
    set_custom_data(&mut vial, data);
    if creative {
        if let Some(rest) = game.native_give_stack(player.uuid, vial) {
            game.spawn_item(rest, player.position, [0.0; 3], 10);
        }
        return;
    }
    if !game.native_remove_mob(&dim, entity as u32) {
        return;
    }
    let at = [player.position[0] as i32, player.position[1] as i32, player.position[2] as i32];
    swap_held(game, player.uuid, held, vial, at);
}

/// The tooltips: a soul vial's mob, a tank item's fluid.
pub(in crate::game) fn tooltip(stack: &ItemStack, lines: &mut Vec<(horizon_client::text::Text, u32)>) {
    if &*stack.item == "enderio:filled_soul_vial" || &*stack.item == "enderio:broken_spawner" {
        if let Some(id) = soul_of(stack) {
            let (ns, path) = id.split_once(':').unwrap_or(("minecraft", &id));
            lines.push((horizon_client::text::Text::translate_with_fallback(&format!("entity.{ns}.{path}"), path, &[]), u32::MAX));
        }
    }
    if matches!(&*stack.item, "enderio:fluid_tank" | "enderio:pressurized_fluid_tank") {
        if let Some(Tag::Compound(c)) = custom_data(stack).and_then(|d| d.get(&js("horizon_fluid"))) {
            if let Some(f) = gs(c, "Fluid") {
                let line = format!("{} {} mB", fluid_name(&f), super::enderio_ui::commas(i64::from(gi(c, "Amount", 0))));
                lines.push((horizon_client::text::Text::legacy(&line), u32::MAX));
            }
        }
    }
}

// ---- Menu buttons (server side)

pub(in crate::game) const B_DUMP_INPUT: i32 = 400;
pub(in crate::game) const B_VOID_OUTPUT: i32 = 401;
pub(in crate::game) const B_USE_XP: i32 = 410;
pub(in crate::game) const B_VOID_MODE: i32 = 420; // + mode

pub(in crate::game) fn menu_button(game: &mut Game<'_>, pos: Pos, e: &mut ModBlockEntity, m: &Mach, id: i32) -> bool {
    match (m.kind, id) {
        // PacketDumpTank: 1 moves the input into the output (not while active), 2 voids the output.
        (Kind::Vat, B_DUMP_INPUT) => {
            let mut input = read_tank(&e.nbt, "inputTank", 8000);
            let mut output = read_tank(&e.nbt, "outputTank", 8000);
            let active = is_active(e, m);
            let msg = if active {
                Some(lang("enderio.gui.vat.dump.active", "Cannot dump tank while the vat is active"))
            } else if let Some(f) = input.fluid.clone() {
                if output.takes(&f) && output.room() > 0 {
                    let n = output.fill(&f, input.amount, true);
                    input.remove(n);
                    None
                } else {
                    Some(lang("enderio.gui.vat.dump.fail", "Output tank is not empty"))
                }
            } else {
                None
            };
            write_tank(&mut e.nbt, "inputTank", &input);
            write_tank(&mut e.nbt, "outputTank", &output);
            if let (Some(msg), Some(p)) = (msg, game.native_acting_player()) {
                game.native_tell(p.uuid, horizon_client::text::Text::legacy(&msg));
            }
            let _ = pos;
            true
        }
        (Kind::Vat, B_VOID_OUTPUT) => {
            write_tank(&mut e.nbt, "outputTank", &Tank { capacity: 8000, ..Tank::default() });
            true
        }
        // ContainerSoulBinder.doDrainXP: the player's XP up to the level the next recipe needs (creative: free).
        (Kind::Soul, B_USE_XP) => {
            let k = c2(game);
            let Some(r) = soul_recipe(&k, e).filter(|_| task(e).is_none()) else { return true };
            let target = exp_for_level(r.levels);
            let have = xp_of(e);
            if game.creative {
                set_xp(e, have + target);
                return true;
            }
            let need = target - have;
            if need <= 0 {
                return true;
            }
            let drain = need.min(player_total_xp(game));
            if drain > 0 {
                set_xp(e, have + drain);
                add_player_xp(game, -drain);
                let at = [f64::from(pos[0]) + 0.5, f64::from(pos[1]) + 0.5, f64::from(pos[2]) + 0.5];
                game.native_play_sound_in_place("minecraft:entity.experience_orb.pickup", at, 4, 0.1, 0.5);
            }
            true
        }
        // PacketTankVoidMode.
        (Kind::Tank, B_VOID_MODE..=422) => {
            si(&mut e.nbt, "voidMode", id - B_VOID_MODE);
            true
        }
        _ => false,
    }
}

// ---- Data slots for the screens

fn put_tank(d: &mut [i32], at: usize, t: &Tank) {
    d[at] = fluid_no(t.fluid.as_deref());
    put2(d, at + 1, t.amount);
    put2(d, at + 3, t.capacity);
}

fn tank_data(d: &[i32], at: usize) -> Tank {
    Tank { fluid: fluid_of_no(d[at]).map(str::to_owned), amount: get2(d, at + 1), capacity: get2(d, at + 3) }
}

pub(in crate::game) fn fill_data(c: &Caches, e: &ModBlockEntity, m: &Mach, d: &mut [i32]) {
    for (i, t) in tanks(e, m).iter().enumerate().take(2) {
        put_tank(d, if i == 0 { D_TANK0 } else { D_TANK1 }, t);
    }
    match m.kind {
        Kind::Combustion => {
            // The power bar's "Max:" is the energy per tick; under the tanks, ticks per mB.
            let rate = gi(&e.nbt, "generated", 0);
            put2(d, D_USE, rate);
            d[D_X] = gi(&e.nbt, "hz_tpc", 0).clamp(0, 65535);
            d[D_X + 1] = gi(&e.nbt, "hz_tpf", 0).clamp(0, 65535);
        }
        Kind::Zombie => {
            d[D_X] = if m.id == "enderio:ender_generator" { 96 } else { 12 };
        }
        Kind::Vat => {
            d[D_X] = gi(&e.nbt, "hz_vm0", 0).clamp(0, 65535);
            d[D_X + 1] = gi(&e.nbt, "hz_vm1", 0).clamp(0, 65535);
            if let Some(t) = task(e) {
                d[D_X + 2] = fluid_no(Some(&t.fluid_in));
                d[D_X + 3] = fluid_no(Some(&t.fluid_out));
            }
        }
        Kind::Soul => {
            put2(d, D_X, xp_of(e).min(i64::from(i32::MAX)) as i32);
            // The next recipe's level + 1 (0: none), worked out on the server (screen_flags).
            d[D_X + 2] = gi(&e.nbt, "hz_soul_lv", 0).clamp(0, 65535);
        }
        Kind::Enchanter => {
            d[D_X] = gi(&e.nbt, "hz_cost", 0).clamp(0, 65535);
            d[D_X + 1] = gi(&e.nbt, "hz_afford", 0);
        }
        Kind::Tank => {
            d[D_X] = void_mode(e);
            d[D_X + 1] = i32::from(can_void(e, m));
        }
        Kind::Charger => {
            put2(d, D_USE, gi(&e.nbt, "hz_rate", 0));
            let p = gf(&e.nbt, "hz_progress", 0.0);
            d[D_PROGRESS] = if p > 0.0 { (p * 10000.0) as i32 + 1 } else { 0 };
        }
        _ => {}
    }
    let _ = c;
}

/// What the server works out for a screen each tick (the client's copy of the block entity has no item slots): the
/// soul binder's next recipe level.
pub(in crate::game) fn screen_flags(game: &Game<'_>, e: &mut ModBlockEntity, m: &Mach) {
    if m.kind == Kind::Soul {
        let k = c2(game);
        let lv = soul_recipe(&k, e).map_or(0, |r| r.levels.max(0) + 1);
        si(&mut e.nbt, "hz_soul_lv", lv);
    }
    if m.kind == Kind::Vat {
        // The multiplier labels: each input slot's multiplier x 1000 + 1 (0: no label).
        let k = c2(game);
        for slot in 0..2usize {
            let mult = e.items.get(slot).cloned().flatten().and_then(|s| {
                k.vat.iter().filter_map(|r| r.groups.get(slot)).flat_map(|g| g.iter()).find(|(i, _)| i.test(&s)).map(|(_, m)| *m)
            });
            let v = mult.filter(|m| *m > 0.0).map_or(0, |m| (m * 1000.0).round() as i32 + 1);
            si(&mut e.nbt, if slot == 0 { "hz_vm0" } else { "hz_vm1" }, v);
        }
    }
}

/// The binder's needs for its screen, from the menu's data (the server's answer): the required level, and whether
/// the binder holds less XP than it.
fn soul_needs(_game: &Game<'_>, data: &[i32]) -> Option<(i32, bool)> {
    let lv = data[D_X + 2] - 1;
    if lv < 0 || data[D_PROGRESS] > 0 {
        return None;
    }
    Some((lv, exp_for_level(lv) > i64::from(get2(data, D_X))))
}

// ---- Screens (3.2.4-3.2.12)

pub(in crate::game) fn slots(m: &Mach) -> Vec<Option<[f32; 2]>> {
    let all: Vec<[f32; 2]> = match m.kind {
        Kind::Combustion | Kind::Zombie => vec![[12.0, 60.0]],
        Kind::Vat => vec![[56.0, 12.0], [105.0, 12.0], [8.0, 58.0]],
        Kind::Soul => vec![[38.0, 34.0], [59.0, 34.0], [112.0, 34.0], [134.0, 34.0], [12.0, 60.0]],
        Kind::Slice => vec![[44.0, 40.0], [62.0, 40.0], [80.0, 40.0], [44.0, 58.0], [62.0, 58.0], [80.0, 58.0], [54.0, 16.0], [72.0, 16.0], [134.0, 49.0], [12.0, 60.0]],
        Kind::Enchanter => vec![[16.0, 35.0], [65.0, 35.0], [85.0, 35.0], [144.0, 35.0]],
        Kind::Tank => vec![[44.0, 21.0], [116.0, 21.0], [15.0, 36.0], [44.0, 52.0], [116.0, 52.0]],
        Kind::Charger => vec![[75.0, 28.0], [126.0, 28.0], [33.0, 60.0]],
        Kind::Painter => vec![[67.0, 34.0], [38.0, 34.0], [121.0, 34.0], [12.0, 60.0]],
        _ => Vec::new(),
    };
    (0..m.slots).map(|i| all.get(i).copied()).collect()
}

pub(in crate::game) fn texture(m: &Mach) -> &'static str {
    match m.kind {
        Kind::Combustion => "enderio:gui/40/combustion_gen",
        Kind::Zombie => "enderio:gui/40/zombie_generator",
        Kind::Vat => "enderio:gui/40/vat",
        Kind::Soul => "enderio:gui/40/soul_fuser",
        Kind::Slice => "enderio:gui/40/slice_and_splice",
        Kind::Enchanter => "enderio:gui/40/enchanter",
        Kind::Tank => "enderio:gui/40/tank",
        Kind::Charger if m.level == 0 => "enderio:gui/40/wired_charger_simple",
        Kind::Charger => "enderio:gui/40/wired_charger",
        _ => "enderio:gui/40/painter",
    }
}

fn power_rect(m: &Mach) -> Option<[f32; 4]> {
    match m.kind {
        Kind::Enchanter | Kind::Tank => None,
        Kind::Vat => Some([12.0, 12.0, 9.0, 42.0]),
        Kind::Charger => Some([37.0, 14.0, 9.0, 42.0]),
        _ => Some([16.0, 14.0, 9.0, 42.0]),
    }
}

/// EC RenderUtil.renderGuiTank: the still sprite tiled in 16-pixel cells, from the bottom, at least 1 pixel.
pub(in crate::game) fn gui_tank(game: &Game<'_>, batch: &mut HudBatch, o: [f32; 2], fluid: &str, capacity: i32, amount: i32, r: [f32; 4], alpha: f32) {
    if capacity <= 0 || amount <= 0 {
        return;
    }
    let h = ((amount as f32 * r[3] / capacity as f32).min(r[3])).max(1.0).floor();
    let sprite = fluid_sprite(fluid);
    let found = game.gui.atlas.sprites.get(&sprite).or_else(|| game.gui.atlas.sprites.get(sprite.split_once(':').map_or(sprite.as_str(), |(_, p)| p))).copied();
    let tint = fluid_tint(fluid);
    let [a, red, green, blue] = tint.to_be_bytes();
    let color = rgba(red, green, blue, (f32::from(a) * alpha) as u8);
    let rect = [o[0] + r[0], o[1] + r[1] + r[3] - h, r[2], h];
    let Some(s) = found else {
        game.native_fill(batch, rect, tint);
        return;
    };
    let (du, dv) = ((s.u1 - s.u0) / s.w as f32, (s.v1 - s.v0) / s.h as f32);
    let size = (s.w as f32).min(16.0);
    let mut y = 0.0;
    while y < rect[3] {
        let hh = (rect[3] - y).min(16.0);
        let mut x = 0.0;
        while x < rect[2] {
            let ww = (rect[2] - x).min(16.0);
            let mut t = s;
            t.u1 = t.u0 + ww * du * size / 16.0;
            t.v1 = t.v0 + hh * dv * size / 16.0;
            batch.rect_uv(rect[0] + x, rect[1] + rect[3] - y - hh, ww, hh, t, color);
            x += 16.0;
        }
        y += 16.0;
    }
}

fn centred(game: &Game<'_>, batch: &mut HudBatch, o: [f32; 2], s: &str, cx: f32, y: f32, argb: u32, shadow: bool) {
    text(game, batch, s, o[0] + cx - (width(game, s) / 2.0).floor(), o[1] + y, argb, shadow);
}

fn generating(game: &Game<'_>, batch: &mut HudBatch, o: [f32; 2], data: &[i32]) {
    let rate = if data[D_ACTIVE] != 0 { i64::from(get2(data, D_USE)) } else { 0 };
    let line = lang("enderio.gui.combustion_generator.output", "Generating: %s").replacen("%s", &rft(rate), 1);
    centred(game, batch, o, &line, 88.0, 7.0, 0xFFFF_FFFF, true);
}

const ICON_DUMP: [f32; 2] = [128.0, 176.0];
const ICON_VOID: [f32; 2] = [112.0, 176.0];
const ICON_XP: [f32; 2] = [128.0, 224.0];
const VOID_ICONS: [[f32; 2]; 3] = [[0.0, 192.0], [160.0, 208.0], [64.0, 192.0]];
const DUMP1: [f32; 4] = [29.0, 62.0, 16.0, 16.0];
const DUMP2: [f32; 4] = [131.0, 62.0, 16.0, 16.0];
const XP_BUTTON: [f32; 4] = [125.0, 57.0, 16.0, 16.0];
const VOID_BUTTON: [f32; 4] = [155.0, 43.0, 16.0, 16.0];

fn icon_button(game: &Game<'_>, batch: &mut HudBatch, o: [f32; 2], r: [f32; 4], uv: [f32; 2], mouse: [f32; 2]) {
    button(game, batch, o, r, false, mouse);
    icon(game, batch, o, [r[0] + 1.0, r[1] + 1.0], uv, [16.0, 16.0], [14.0, 14.0]);
}

pub(in crate::game) fn draw(game: &Game<'_>, m: &Mach, data: &[i32], batch: &mut HudBatch, o: [f32; 2], mouse: [f32; 2]) {
    let tex = texture(m);
    let w = if m.kind == Kind::Charger { 218.0 } else { 176.0 };
    spr(game, batch, tex, o, [0.0, 0.0], [0.0, 0.0, w, 166.0]);
    let p = progress(data);
    let t0 = tank_data(data, D_TANK0);
    let t1 = tank_data(data, D_TANK1);
    match m.kind {
        Kind::Combustion => {
            // The gauges show what is over 1000 mB; the first 1000 of each are drawn in the middle.
            let (coolant, fuel) = (&t0, &t1);
            if let Some(f) = &fuel.fluid {
                gui_tank(game, batch, o, f, fuel.capacity - 1000, fuel.amount - 1000, [48.0, 21.0, 15.0, 47.0], 1.0);
            }
            if let Some(f) = &coolant.fluid {
                gui_tank(game, batch, o, f, coolant.capacity - 1000, coolant.amount - 1000, [114.0, 21.0, 15.0, 47.0], 1.0);
                gui_tank(game, batch, o, f, 1000, coolant.amount.min(1000), [72.0, 28.0, 33.0, 33.0], 1.0);
            }
            spr(game, batch, tex, o, [72.0, 28.0], [0.0, 223.0, 33.0, 33.0]);
            if let Some(f) = &fuel.fluid {
                gui_tank(game, batch, o, f, 1000, fuel.amount.min(1000), [81.0, 38.0, 14.0, 14.0], 1.0);
            }
            generating(game, batch, o, data);
            if data[D_ACTIVE] != 0 {
                for (x, n) in [(114.0, data[D_X]), (48.0, data[D_X + 1])] {
                    let s = lang("enderio.fluid.millibucket.format.tickper", "%s t/mB").replacen("%s", &n.to_string(), 1);
                    text(game, batch, &s, o[0] + x + 7.0 - (width(game, &s) / 2.0).floor(), o[1] + 21.0 + 4.0 + 47.0, 0xFFFF_FFFF, true);
                }
            }
        }
        Kind::Zombie => {
            if let Some(f) = &t0.fluid {
                gui_tank(game, batch, o, f, t0.capacity, t0.amount, [80.0, 21.0, 16.0, 47.0], 1.0);
            }
            generating(game, batch, o, data);
            if data[D_ACTIVE] != 0 {
                let s = lang("enderio.fluid.millibucket.format.tickper", "%s t/mB").replacen("%s", &data[D_X].to_string(), 1);
                text(game, batch, &s, o[0] + 80.0 + 8.0 - (width(game, &s) / 2.0).floor(), o[1] + 21.0 + 4.0 + 46.0, 0xFFFF_FFFF, true);
            }
        }
        Kind::Vat => {
            if let Some(f) = &t0.fluid {
                gui_tank(game, batch, o, f, t0.capacity, t0.amount, [30.0, 12.0, 15.0, 47.0], 1.0);
            }
            if let Some(f) = &t1.fluid {
                gui_tank(game, batch, o, f, t1.capacity, t1.amount, [132.0, 12.0, 15.0, 47.0], 1.0);
            }
            if let Some(p) = p {
                let s = ((p * 14.0) as i32 + 1) as f32;
                spr(game, batch, tex, o, [81.0, 77.0 - s], [176.0, 14.0 - s, 14.0, s]);
                // The vat window: the input fading out, the output in.
                if let (Some(a), Some(b)) = (fluid_of_no(data[D_X + 2]), fluid_of_no(data[D_X + 3])) {
                    gui_tank(game, batch, o, a, 1, 1, [76.0, 34.0, 26.0, 28.0], 0.75 * (1.0 - p));
                    gui_tank(game, batch, o, b, 1, 1, [76.0, 34.0, 26.0, 28.0], 0.75 * p);
                }
            }
            spr(game, batch, tex, o, [76.0, 34.0], [0.0, 228.0, 26.0, 28.0]);
            vat_labels(game, batch, o, data);
            icon_button(game, batch, o, DUMP1, ICON_DUMP, mouse);
            icon_button(game, batch, o, DUMP2, ICON_VOID, mouse);
        }
        Kind::Soul => {
            if let Some(p) = p {
                spr(game, batch, tex, o, [80.0, 34.0], [176.0, 14.0, ((p * 24.0) as i32 + 1) as f32, 16.0]);
            }
            xp_bar(game, batch, o, data);
            if soul_needs(game, data).is_some_and(|(_, short)| short) {
                icon_button(game, batch, o, XP_BUTTON, ICON_XP, mouse);
            }
        }
        Kind::Slice => {
            if let Some(p) = p {
                spr(game, batch, tex, o, [103.0, 49.0], [176.0, 14.0, ((p * 24.0) as i32 + 1) as f32, 16.0]);
            }
        }
        Kind::Painter => {
            if let Some(p) = p {
                spr(game, batch, tex, o, [88.0, 34.0], [176.0, 14.0, ((p * 24.0) as i32 + 1) as f32, 16.0]);
            }
        }
        Kind::Enchanter => {
            let cost = data[D_X];
            if cost > 0 {
                let ok = data[D_X + 1] != 0;
                let s = horizon_client::lang::get_or("container.repair.cost", "Enchantment Cost: %1$s").replace("%1$s", &cost.to_string()).replacen("%s", &cost.to_string(), 1);
                centred(game, batch, o, &s, 88.0, 57.0, if ok { 0xFF80_FF20 } else { 0xFFFF_6060 }, true);
                if !ok {
                    spr(game, batch, tex, o, [99.0, 33.0], [176.0, 0.0, 28.0, 21.0]);
                }
            }
        }
        Kind::Tank => {
            if data[D_X + 1] != 0 {
                spr(game, batch, tex, o, [14.0, 35.0], [176.0, 0.0, 18.0, 18.0]);
                icon_button(game, batch, o, VOID_BUTTON, VOID_ICONS[data[D_X].clamp(0, 2) as usize], mouse);
            }
            if let Some(f) = &t0.fluid {
                gui_tank(game, batch, o, f, t0.capacity, t0.amount, [80.0, 21.0, 16.0, 47.0], 1.0);
            }
        }
        Kind::Charger => {
            if let Some(p) = p {
                let s = (p * 37.0) as i32 as f32;
                spr(game, batch, tex, o, [103.0, 17.0 + 37.0 - s], [242.0, 37.0 - s, 14.0, 38.0]);
            }
            let name = lang(&format!("block.{}", m.id.replace(':', ".")), m.id);
            centred(game, batch, o, &name, 109.0, 4.0, 0xFFFF_FFFF, true);
        }
        _ => {}
    }
    if m.cap.is_some() && data[D_CAP] == 0 {
        let at = if m.kind == Kind::Vat { [8.0, 58.0] } else if m.kind == Kind::Charger { [33.0, 60.0] } else { [12.0, 60.0] };
        icon(game, batch, o, at, ICON_CAPACITOR, [16.0, 16.0], [16.0, 16.0]);
    }
    if let Some(r) = power_rect(m) {
        power_bar(game, batch, o, r, f64::from(get2(data, D_ENERGY)), f64::from(get2(data, D_MAX)), data[D_CAP] != 0);
    }
}

/// GuiVat's multiplier labels: "x<mult>" of each filled input slot, grey, no shadow.
// The multipliers come from the server through the data (the client's copy of the block entity has no item slots).
fn vat_labels(game: &Game<'_>, batch: &mut HudBatch, o: [f32; 2], data: &[i32]) {
    for (i, cx) in [(0usize, 63.0), (1, 113.0)] {
        let v = data[D_X + i];
        if v > 0 {
            let label = format!("x{}", java_float((v - 1) as f32 / 1000.0));
            text(game, batch, &label, o[0] + cx - (width(game, &label) / 2.0).floor(), o[1] + 32.0, 0xFF40_4040, false);
        }
    }
}

/// Java's Float.toString for the multipliers (1.0, 1.5, 0.25).
fn java_float(v: f32) -> String {
    let s = format!("{v}");
    if s.contains('.') { s } else { format!("{s}.0") }
}

/// ExperienceBarRenderer at (56, 68), 65 long: the level (and the required one), the bar.
fn xp_bar(game: &Game<'_>, batch: &mut HudBatch, o: [f32; 2], data: &[i32]) {
    let xp = i64::from(get2(data, D_X));
    let level = level_for_xp(xp);
    let need = soul_needs(game, data);
    let (x, y, len) = (56.0, 68.0, 65.0);
    let short = need.is_some_and(|(l, _)| level < l);
    let label = match need {
        Some((l, _)) => format!("{level}/{l}"),
        None => level.to_string(),
    };
    let color = if short { 0xFFFF_001A } else { 0xFF80_FF20 };
    centred(game, batch, o, &label, x + len / 2.0, y - 11.0, color, !short);
    icon(game, batch, o, [x, y], [0.0, 91.0], [1.0, 5.0], [1.0, 5.0]);
    icon(game, batch, o, [x + 1.0, y], [1.0, 91.0], [len - 2.0, 5.0], [len - 2.0, 5.0]);
    icon(game, batch, o, [x + len - 1.0, y], [125.0, 91.0], [1.0, 5.0], [1.0, 5.0]);
    let cap = horizon_client::entities::Experience::needed(level) as f32;
    let frac = ((xp - exp_for_level(level)) as f32 / cap.max(1.0)).clamp(0.0, 1.0);
    let filled = (frac * (len - 2.0)).floor();
    if filled > 0.0 {
        fill(game, batch, o, [x + 1.0, y + 1.0, filled, 3.0], 0xFF00_7F0E);
    }
}

/// The highlight rectangles of the hovered face: (rect, blue for pulling or orange for pushing).
pub(in crate::game) fn tint_rects(m: &Mach) -> Vec<([f32; 4], bool)> {
    let sq = |p: [f32; 2]| [p[0], p[1], 16.0, 16.0];
    let s = slots(m);
    let slot = |i: usize| s.get(i).copied().flatten().map(sq);
    let mut out = Vec::new();
    match m.kind {
        Kind::Combustion => {
            out.push(([46.0, 19.0, 19.0, 51.0], true));
            out.push(([112.0, 19.0, 19.0, 51.0], true));
        }
        Kind::Zombie => out.push(([78.0, 19.0, 19.0, 51.0], true)),
        Kind::Vat => {
            out.push(([28.0, 10.0, 19.0, 51.0], true));
            out.push(([130.0, 10.0, 19.0, 51.0], false));
        }
        Kind::Painter => {
            out.extend(slot(0).map(|r| (r, true)));
            out.push(([117.0, 31.0, 24.0, 24.0], false));
        }
        _ => {
            for &i in m.inputs {
                out.extend(slot(i).map(|r| (r, true)));
            }
            for &i in m.outputs {
                out.extend(slot(i).map(|r| (r, false)));
            }
        }
    }
    out
}

fn mb_line(t: &Tank) -> String {
    match &t.fluid {
        Some(f) => format!("{} / {} mB of {}", super::enderio_ui::commas(i64::from(t.amount)), super::enderio_ui::commas(i64::from(t.capacity)), fluid_name(f)),
        None => format!("{} / {} mB", super::enderio_ui::commas(i64::from(t.amount)), super::enderio_ui::commas(i64::from(t.capacity))),
    }
}

pub(in crate::game) fn tooltip_lines(m: &Mach, data: &[i32], at: [f32; 2], lines: &mut Vec<String>) {
    let t0 = tank_data(data, D_TANK0);
    let t1 = tank_data(data, D_TANK1);
    if let Some(r) = power_rect(m).filter(|r| inside(*r, at)) {
        let _ = r;
        if data[D_CAP] == 0 {
            lines.push(lang("enderio.gui.generic.nocap.line1", "§3Capacitor missing"));
            lines.push(lang("enderio.gui.generic.nocap.line2", "Insert any capacitor so"));
            lines.push(lang("enderio.gui.generic.nocap.line3", "this machine can work!"));
        } else {
            lines.push(lang("enderio.gui.generic.max", "Max: %s").replacen("%s", &rft(i64::from(get2(data, D_USE))), 1));
            let loss = get2(data, D_LOSS) as f32 / 10000.0;
            if loss > 0.0 {
                lines.push(lang("enderio.gui.generic.loss", "Leakage: %s").replacen("%s", &format!("{loss:.2} µI/t"), 1));
            }
            lines.push(format!("{} / {} µI", super::enderio_ui::commas(i64::from(get2(data, D_ENERGY))), super::enderio_ui::commas(i64::from(get2(data, D_MAX)))));
        }
        return;
    }
    let tank_tip = |title: String, t: &Tank, lines: &mut Vec<String>| {
        lines.push(title);
        lines.push(mb_line(t));
    };
    let progress_rect: Option<[f32; 4]> = match m.kind {
        Kind::Vat => Some([81.0, 63.0, 14.0, 14.0]),
        Kind::Soul => Some([80.0, 34.0, 24.0, 16.0]),
        Kind::Slice => Some([103.0, 49.0, 24.0, 16.0]),
        Kind::Painter => Some([88.0, 34.0, 24.0, 16.0]),
        _ => None,
    };
    if let (Some(r), Some(p)) = (progress_rect, progress(data)) {
        if inside(r, at) {
            lines.push(lang("enderio.gui.generic.progress", "Progress %s%%").replacen("%s", &((p * 100.0) as i32).to_string(), 1).replace("%%", "%"));
            return;
        }
    }
    match m.kind {
        Kind::Combustion => {
            if inside([48.0, 21.0, 15.0, 47.0], at) {
                let title = t1.fluid.as_deref().map_or(lang("enderio.gui.combustion_generator.fuel", "Fuel"), |f| format!("{}: {}", lang("enderio.gui.combustion_generator.fuel", "Fuel"), fluid_name(f)));
                tank_tip(title, &t1, lines);
            } else if inside([114.0, 21.0, 15.0, 47.0], at) {
                let title = t0.fluid.as_deref().map_or(lang("enderio.gui.combustion_generator.coolant", "Coolant"), |f| format!("{}: {}", lang("enderio.gui.combustion_generator.coolant", "Coolant"), fluid_name(f)));
                tank_tip(title, &t0, lines);
            }
        }
        Kind::Zombie => {
            if inside([80.0, 21.0, 15.0, 47.0], at) {
                tank_tip(lang("enderio.gui.zombie_generator.nutrient_tank", "Nutrient Tank"), &t0, lines);
                if t0.amount < 1400 {
                    lines.push(lang("enderio.gui.zombie_generator.minreq", "%s mB Required for activation").replacen("%s", "1,400", 1));
                }
            }
        }
        Kind::Vat => {
            if inside([30.0, 12.0, 15.0, 47.0], at) {
                tank_tip(lang("enderio.gui.vat.input_tank", "Input Tank"), &t0, lines);
            } else if inside([132.0, 12.0, 15.0, 47.0], at) {
                tank_tip(lang("enderio.gui.vat.output_tank", "Output Tank"), &t1, lines);
            } else if inside(DUMP1, at) {
                lines.push(lang("enderio.gui.vat.dump", "Dump Input Tank Into Output Tank"));
            } else if inside(DUMP2, at) {
                lines.push(lang("enderio.gui.vat.void", "Void Output Tank"));
            }
        }
        Kind::Soul => {
            if inside(XP_BUTTON, at) {
                lines.push(lang("enderio.gui.soul_binder.use_xp", "Use Player XP"));
            }
        }
        Kind::Tank => {
            if inside([80.0, 21.0, 16.0, 47.0], at) {
                tank_tip(lang("enderio.gui.tank.tank", "Tank"), &t0, lines);
            } else if data[D_X + 1] != 0 && inside(VOID_BUTTON, at) {
                let key = ["always", "never_container", "never"][data[D_X].clamp(0, 2) as usize];
                let english = ["Always", "Never containers", "Never"][data[D_X].clamp(0, 2) as usize];
                lines.push(lang("enderio.gui.tank.void.mode", "Void Mode"));
                lines.push(lang(&format!("enderio.gui.tank.void.mode.{key}"), english));
                lines.push(lang(&format!("enderio.gui.tank.void.mode.{key}.desc"), ""));
                lines.retain(|l| !l.is_empty());
            } else if data[D_X + 1] != 0 && inside([14.0, 35.0, 18.0, 18.0], at) {
                lines.push(lang("enderio.gui.tank.void.slot", "Void Slot"));
            }
        }
        _ => {}
    }
}

/// A click on a phase 2 screen's own button: sent to the server, which checks it (menu_button).
pub(in crate::game) fn pressed(game: &Game<'_>, m: &Mach, data: &[i32], at: [f32; 2], right: bool, shift: bool) -> bool {
    let _ = shift;
    match m.kind {
        Kind::Vat if inside(DUMP1, at) && !right => send_button(game, B_DUMP_INPUT),
        Kind::Vat if inside(DUMP2, at) && !right => send_button(game, B_VOID_OUTPUT),
        Kind::Soul if inside(XP_BUTTON, at) && !right && soul_needs(game, data).is_some_and(|(_, s)| s) => send_button(game, B_USE_XP),
        Kind::Tank if inside(VOID_BUTTON, at) && data[D_X + 1] != 0 => {
            let cur = data[D_X].clamp(0, 2);
            send_button(game, B_VOID_MODE + if right { (cur + 2) % 3 } else { (cur + 1) % 3 });
        }
        _ => return false,
    }
    true
}

// ---- JEI: the categories Ender IO's MachinesPlugin registers

enderio_optional::if_mod_items!("jei", {
use crate::game::jei::Stack;
use super::enderio_ui::{energy_text, picture, stacks};

const CATALYSTS: [(&str, &[&str]); 9] = [
    ("combustion", &["enderio:combustion_generator", "enderio:enhanced_combustion_generator"]),
    ("zombie", &["enderio:zombie_generator", "enderio:franken_zombie_generator"]),
    ("ender", &["enderio:ender_generator"]),
    ("vat", &["enderio:vat", "enderio:enhanced_vat"]),
    ("soul", &["enderio:soul_binder"]),
    ("slice", &["enderio:slice_and_splice"]),
    ("enchanter", &["enderio:enchanter"]),
    ("tank", &["enderio:fluid_tank", "enderio:pressurized_fluid_tank"]),
    ("charger", &["enderio:simple_wired_charger", "enderio:wired_charger", "enderio:enhanced_wired_charger"]),
];

fn cats(name: &str) -> &'static [&'static str] {
    CATALYSTS.iter().find(|(n, _)| *n == name).map_or(&[][..], |(_, c)| *c)
}

pub(in crate::game) fn is_catalyst(item: &str) -> bool {
    CATALYSTS.iter().any(|(_, c)| c.contains(&item))
}

/// A JEI fluid slot (`guiFluidStacks.init`): the fluid's still texture only, as JEI's fluid renderer draws it, with
/// its name and amount under the pointer.
fn pfluid(at: [f32; 2], f: &str, amount: i32, size: [f32; 2], capacity: i32) -> kit_window::PictureFluid {
    let tip = vec![fluid_name(f), format!("§7{amount} mB")];
    kit_window::PictureFluid { at, fluid: f.to_owned(), amount, bucket: None, size: Some(size), capacity, sprite: Some((fluid_sprite(f), fluid_tint(f))), tip }
}

fn tab(icon: &str, title: String, name: &str, cards: Vec<kit_window::Card>) -> Option<kit_window::Tab> {
    (!cards.is_empty()).then(|| kit_window::Tab { icon: icon.to_owned(), title, catalysts: cats(name).iter().map(|s| (*s).to_owned()).collect(), cards })
}

fn with_fluids(mut card: kit_window::Card, f: Vec<kit_window::PictureFluid>) -> kit_window::Card {
    if let kit_window::Card::Picture { fluids, .. } = &mut card {
        *fluids = f;
    }
    card
}

fn one(s: &ItemStack) -> Vec<Stack> {
    vec![Stack { item: s.item.to_string(), count: s.count, components: String::new() }]
}

pub(in crate::game) fn jei_tabs(game: &Game<'_>, item: &Stack, uses: bool) -> Vec<kit_window::Tab> {
    let id = item.item.as_str();
    let k = c2(game);
    let c = caches(game);
    let mut tabs = Vec::new();
    let fluid_of_item = bucket_fluid(id).map(str::to_owned).or_else(|| match id { "minecraft:water_bucket" => Some("minecraft:water".into()), "minecraft:lava_bucket" => Some("minecraft:lava".into()), _ => None });
    let shows_fluid = |f: &str| fluid_of_item.as_deref() == Some(f);
    // CombustionRecipeCategory: combustion_gen (28,7), a fuel and a coolant per card, its rate and ticks.
    let all = cats("combustion").contains(&id);
    let mut cards = Vec::new();
    for fuel in &k.fuels {
        for coolant in &k.coolants {
            if !(all || (uses && (shows_fluid(&fuel.fluid) || shows_fluid(&coolant.fluid)))) {
                continue;
            }
            let ept = (fuel.per_tick as f32).round() as i32;
            let to_cool = f64::from(1.15e-4f32) * f64::from(ept);
            let tpc = ((((373.25 - temperature(&coolant.fluid)) * coolant.degrees) / to_cool).round() as i32).max(1);
            let tpf = ((fuel.ticks as f32 / 1000.0) as i32).max(1);
            let card = picture("enderio:gui/40/combustion_gen", [28.0, 7.0, 120.0, 70.0], vec![([0.0, 223.0, 33.0, 33.0], [44.0, 21.0])], Vec::new(), vec![
                energy_text([9.0, 2.0], ept),
                kit_window::PictureText { at: [86.0, 64.0], text: format!("{tpc} t/mB"), color: 0xFF80_8080, hover: 0xFF80_8080, right: false, tip: Vec::new() },
                kit_window::PictureText { at: [20.0, 64.0], text: format!("{tpf} t/mB"), color: 0xFF80_8080, hover: 0xFF80_8080, right: false, tip: Vec::new() },
            ]);
            cards.push(with_fluids(card, vec![pfluid([86.0, 14.0], &coolant.fluid, 1000, [15.0, 47.0], 1000), pfluid([20.0, 14.0], &fuel.fluid, 1000, [15.0, 47.0], 1000)]));
        }
    }
    tabs.extend(tab("enderio:combustion_generator", lang("block.enderio.combustion_generator", "Combustion Generator"), "combustion", cards));
    // ZombieGeneratorRecipeCategory / EnderGeneratorRecipeCategory: the tank of 2000 and the rate at level 1.
    for (name, block, fluid, t_per_mb) in [("zombie", "enderio:zombie_generator", NUTRIENT, 12), ("ender", "enderio:ender_generator", ENDER, 96)] {
        let all = cats(name).contains(&id);
        if !(all || (uses && shows_fluid(fluid))) {
            continue;
        }
        let rate = mach(block).map_or(0, |m| c.int(&m.key("gen"), &Cap::basic()));
        let card = picture("enderio:gui/40/zombie_generator", [28.0, 7.0, 120.0, 70.0], Vec::new(), Vec::new(), vec![
            energy_text([9.0, 4.0], rate),
            kit_window::PictureText { at: [52.0, 64.0], text: format!("{t_per_mb} t/mB"), color: 0xFF80_8080, hover: 0xFF80_8080, right: false, tip: Vec::new() },
        ]);
        let card = with_fluids(card, vec![pfluid([52.0, 14.0], fluid, 2000, [16.0, 47.0], 2000)]);
        tabs.extend(tab(block, lang(&format!("block.{}", block.replace(':', ".")), block), name, vec![card]));
    }
    // VatRecipeCategory: vat (24,9) 130x70, the inputs at (31,2) and (80,2), tanks (6,3) and (108,3) of 8000.
    let all = cats("vat").contains(&id);
    let mut cards = Vec::new();
    for r in &k.vat {
        let any_item = r.groups.iter().flatten().any(|(i, _)| i.items.contains(id));
        let shows = all || if uses { any_item || shows_fluid(&r.fluid_in) } else { shows_fluid(&r.fluid_out) };
        if !shows {
            continue;
        }
        let g0 = &r.groups[0];
        let g1 = r.groups.get(1);
        let (m0, m1) = (g0[0].1, g1.map_or(1.0, |g| g[0].1));
        let im = m0 * m1;
        let in_mb = (1000.0 * im).round() as i32;
        let out_mb = (im * r.fluid_mult * 1000.0).round() as i32;
        let mut slots = vec![([32.0, 3.0], g0.iter().flat_map(|(i, _)| stacks(i, 1)).collect::<Vec<_>>(), Vec::new())];
        if let Some(g) = g1 {
            slots.push(([81.0, 3.0], g.iter().flat_map(|(i, _)| stacks(i, 1)).collect(), Vec::new()));
        }
        let card = picture("enderio:gui/40/vat", [24.0, 9.0, 130.0, 70.0], Vec::new(), slots, vec![energy_text([76.0, 58.0], r.energy)]);
        cards.push(with_fluids(card, vec![pfluid([6.0, 3.0], &r.fluid_in, in_mb, [15.0, 47.0], 8000), pfluid([108.0, 3.0], &r.fluid_out, out_mb, [15.0, 47.0], 8000)]));
    }
    tabs.extend(tab("enderio:vat", lang("block.enderio.vat", "The Vat"), "vat", cards));
    // SoulBinderRecipeCategory: soul_fuser (34,28) 120x50, vial (3,5), input (24,5), outputs (77,5) (99,5).
    let all = cats("soul").contains(&id);
    let mut cards = Vec::new();
    for r in &k.soul {
        let shows = all || if uses { r.input.items.contains(id) || id == "enderio:soul_vial" || id == "enderio:filled_soul_vial" } else { *r.output.item == *id };
        if !shows {
            continue;
        }
        let tip = vec![format!("§7{} {} ({} levels)", lang("enderio.jei.soulbinder.xp", "XP:"), exp_for_level(r.levels), r.levels)];
        let slots = vec![
            ([4.0, 6.0], vec![Stack::item("enderio:filled_soul_vial")], Vec::new()),
            ([25.0, 6.0], stacks(&r.input, 1), Vec::new()),
            ([78.0, 6.0], vec![Stack::item("enderio:soul_vial")], Vec::new()),
            ([100.0, 6.0], one(&r.output), tip),
        ];
        cards.push(picture("enderio:gui/40/soul_fuser", [34.0, 28.0, 120.0, 50.0], vec![([177.0, 14.0, 22.0, 16.0], [47.0, 7.0])], slots, vec![energy_text([5.0, 35.0], r.energy)]));
    }
    tabs.extend(tab("enderio:soul_binder", lang("block.enderio.soul_binder", "Soul Binder"), "soul", cards));
    // SliceAndSpliceRecipeCategory: slice_and_splice (34,10) 125x70, tools (19,5) (37,5), grid from (9,29), output (99,38).
    let all = cats("slice").contains(&id);
    let mut cards = Vec::new();
    let axes: Vec<Stack> = k.axes.as_ref().map(|i| stacks(i, 1)).unwrap_or_default();
    let shears: Vec<Stack> = k.shears.as_ref().map(|i| stacks(i, 1)).unwrap_or_default();
    for r in &k.slice {
        let shows = all || if uses { r.inputs.iter().flatten().any(|i| i.items.contains(id)) || axes.iter().chain(shears.iter()).any(|s| s.item == id) } else { *r.output.item == *id };
        if !shows {
            continue;
        }
        let mut slots = vec![([20.0, 6.0], axes.clone(), Vec::new()), ([38.0, 6.0], shears.clone(), Vec::new())];
        for (i, ing) in r.inputs.iter().enumerate() {
            if let Some(ing) = ing {
                slots.push(([10.0 + 18.0 * (i % 3) as f32, 30.0 + 18.0 * (i / 3) as f32], stacks(ing, 1), Vec::new()));
            }
        }
        slots.push(([100.0, 39.0], one(&r.output), Vec::new()));
        cards.push(picture("enderio:gui/40/slice_and_splice", [34.0, 10.0, 125.0, 70.0], vec![([177.0, 14.0, 22.0, 16.0], [70.0, 39.0])], slots, vec![energy_text([73.0, 61.0], r.energy)]));
    }
    tabs.extend(tab("enderio:slice_and_splice", lang("block.enderio.slice_and_splice", "Slice'N'Splice"), "slice", cards));
    // EnchanterRecipeCategory: enchanter (16,24), book (1,11), input (50,11), lapis (70,11), output (129,11); one card a
    // level, the XP cost.
    let all = cats("enchanter").contains(&id);
    let mut cards = Vec::new();
    let lapis = ingredient(game, &serde_json::Value::String("#c:gems/lapis".into()));
    for r in &k.ench {
        let shows = all || if uses { r.input.items.contains(id) || id == "minecraft:book" } else { id == "minecraft:enchanted_book" };
        if !shows {
            continue;
        }
        let Some(en) = game.enchantments().get(&r.enchantment) else { continue };
        let max = en.max_level.max(1);
        for level in 1..=max {
            let n = r.per_level * level;
            let items: Vec<Option<ItemStack>> = vec![Some(ItemStack::new("minecraft:book", 1)), r.input.order.first().map(|i| ItemStack::new(i, n)), lapis.as_ref().and_then(|l| l.order.first()).map(|i| ItemStack::new(i, 64))];
            let Some((out, cost, used)) = enchant_preview(game, &k, &items) else { continue };
            let slots = vec![
                ([1.0, 11.0], vec![Stack::item("minecraft:book")], Vec::new()),
                ([50.0, 11.0], stacks(&r.input, n), Vec::new()),
                ([70.0, 11.0], lapis.as_ref().map(|l| stacks(l, used[2])).unwrap_or_default(), Vec::new()),
                ([129.0, 11.0], vec![Stack { item: out.item.to_string(), count: 1, components: String::new() }], Vec::new()),
            ];
            let cost_text = horizon_client::lang::get_or("container.repair.cost", "Enchantment Cost: %1$s").replace("%1$s", &cost.to_string()).replacen("%s", &cost.to_string(), 1);
            cards.push(picture("enderio:gui/40/enchanter", [16.0, 24.0, 146.0, 40.0], Vec::new(), slots, vec![kit_window::PictureText { at: [72.0, 30.0], text: cost_text, color: 0xFF80_FF20, hover: 0xFF80_FF20, right: false, tip: Vec::new() }]));
        }
    }
    tabs.extend(tab("enderio:enchanter", lang("block.enderio.enchanter", "Enchanter"), "enchanter", cards));
    // TankRecipeCategory: tank (15,20) 146x49, the input (29,1) or (101,1), the output (29,32) or (101,32), the gauge.
    let all = cats("tank").contains(&id);
    let mut cards = Vec::new();
    for r in &k.tank {
        let out = r.output.as_ref();
        let shows = all || if uses { r.input.items.contains(id) || shows_fluid(&r.fluid) } else { out.is_some_and(|o| *o.item == *id) };
        if !shows {
            continue;
        }
        let (ix, ox) = if r.fill { (101.0, 101.0) } else { (29.0, 29.0) };
        let mut slots = vec![([ix + 1.0, 2.0], stacks(&r.input, 1), Vec::new())];
        if let Some(o) = out {
            slots.push(([ox + 1.0, 33.0], one(o), Vec::new()));
        }
        let shown = match r.logic.as_str() {
            "xp" => r.amount * 20,
            "levels" => (XP_RATIO * exp_for_level(r.amount)) as i32,
            "xpbottle" => r.amount * 20 * 7,
            _ => r.amount,
        };
        let card = picture("enderio:gui/40/tank", [15.0, 20.0, 146.0, 49.0], Vec::new(), slots, Vec::new());
        cards.push(with_fluids(card, vec![pfluid([65.0, 1.0], &r.fluid, shown, [16.0, 47.0], 16000)]));
    }
    tabs.extend(tab("enderio:fluid_tank", lang("block.enderio.fluid_tank", "Fluid Tank"), "tank", cards));
    tabs
}
});

// ---- Jade (TOPCompatibility): the tanks' contents

enderio_optional::if_mod_items!("jade", {
pub(in crate::game) fn jade_lines(e: &ModBlockEntity, m: &Mach, lines: &mut Vec<String>) {
    for t in tanks(e, m) {
        if t.fluid.is_some() {
            lines.push(mb_line(&t));
        }
    }
    if m.kind == Kind::Soul {
        let xp = xp_of(e);
        if xp > 0 {
            lines.push(format!("{} XP ({} levels)", xp, level_for_xp(xp)));
        }
    }
}
});
