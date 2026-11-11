// Ender IO 5.3.72 for Horizon (CC0 / Unlicense, the Ender IO team), rewritten for Horizon's hooks. Its blocks, items,
// recipes and the W guide's pages are the content mod in this bundle's data/ (tools/make_content.py); this module is
// the mod's behaviour, decided on the server from what the server holds.
//
// Phase 1 (spec-machines.md 1, 2.1-2.4, 2.14, 5): the legacy machine framework (capacitors and their scalers from the
// recipe data, energy in µI, tasks, side IO, redstone, the active state, the item keeping its NBT), the alloy smelter
// family, the SAG mill family with grinding balls, the simple powered furnace, the Stirling generators and the
// capacitor bank with its joined networks. The screens, JEI and Jade are in enderio_ui.rs.
use super::Game;
use horizon_client::containers::{BlockEntity, ModBlockEntity, ModContainer, ModEnergyBuffer};
use horizon_client::items::ItemStack;
use horizon_core::geometry::Direction;
use horizon_core::random::{LegacyRandom, RandomSource};
use horizon_world::nbt::{Compound, JavaString, Tag};
use std::cell::RefCell;
use std::collections::{HashMap, HashSet, VecDeque};
use std::rc::Rc;

pub(in crate::game) const NS: &str = "enderio";
pub(in crate::game) type Pos = [i32; 3];

enderio_optional::kit_use!(guide as kit_guide);

// ---- NBT helpers

pub(in crate::game) fn js(s: &str) -> JavaString {
    JavaString::from(s)
}

pub(in crate::game) fn gi(n: &Compound, k: &str, d: i32) -> i32 {
    match n.get(&js(k)) {
        Some(Tag::Int(v)) => *v,
        Some(Tag::Short(v)) => i32::from(*v),
        Some(Tag::Byte(v)) => i32::from(*v),
        _ => d,
    }
}

pub(in crate::game) fn si(n: &mut Compound, k: &str, v: i32) {
    n.insert(js(k), Tag::Int(v));
}

pub(in crate::game) fn gf(n: &Compound, k: &str, d: f32) -> f32 {
    match n.get(&js(k)) {
        Some(Tag::Float(v)) => f32::from_bits(*v),
        Some(Tag::Double(v)) => f64::from_bits(*v) as f32,
        Some(Tag::Int(v)) => *v as f32,
        _ => d,
    }
}

pub(in crate::game) fn sf(n: &mut Compound, k: &str, v: f32) {
    n.insert(js(k), Tag::Float(v.to_bits()));
}

pub(in crate::game) fn gl(n: &Compound, k: &str) -> i64 {
    match n.get(&js(k)) {
        Some(Tag::Long(v)) => *v,
        Some(Tag::Int(v)) => i64::from(*v),
        _ => 0,
    }
}

pub(in crate::game) fn gs(n: &Compound, k: &str) -> Option<String> {
    match n.get(&js(k)) {
        Some(Tag::String(s)) => Some(s.to_string_lossy().to_string()),
        _ => None,
    }
}

pub(in crate::game) fn ss(n: &mut Compound, k: &str, v: &str) {
    n.insert(js(k), Tag::String(js(v)));
}

pub(in crate::game) fn gc<'a>(n: &'a Compound, k: &str) -> Option<&'a Compound> {
    match n.get(&js(k)) {
        Some(Tag::Compound(c)) => Some(c),
        _ => None,
    }
}

pub(in crate::game) fn custom_data(stack: &ItemStack) -> Option<&Compound> {
    match stack.extra.as_ref()?.other.iter().find(|(k, _)| k == "minecraft:custom_data").map(|(_, t)| t) {
        Some(Tag::Compound(c)) => Some(c),
        _ => None,
    }
}

pub(in crate::game) fn set_custom_data(stack: &mut ItemStack, c: Compound) {
    let extra = stack.extra.get_or_insert_with(Default::default);
    extra.other.retain(|(k, _)| k != "minecraft:custom_data");
    if !c.is_empty() {
        extra.other.push(("minecraft:custom_data".into(), Tag::Compound(c)));
    }
}

// ---- Faces (Direction order: down, up, north, south, west, east)

pub(in crate::game) const FACES: [Direction; 6] = [Direction::Down, Direction::Up, Direction::North, Direction::South, Direction::West, Direction::East];
pub(in crate::game) const FACE_NAMES: [&str; 6] = ["down", "up", "north", "south", "west", "east"];

pub(in crate::game) fn step(p: Pos, i: usize) -> Pos {
    let d = [[0, -1, 0], [0, 1, 0], [0, 0, -1], [0, 0, 1], [-1, 0, 0], [1, 0, 0]][i];
    [p[0] + d[0], p[1] + d[1], p[2] + d[2]]
}

pub(in crate::game) fn opposite(i: usize) -> usize {
    [1, 0, 3, 2, 5, 4][i]
}

fn face_of_normal(n: [i32; 3]) -> Option<usize> {
    (0..6).find(|&i| step([0, 0, 0], i) == n)
}

// ---- IoMode (base/machine/modes/IoMode.java): NONE, PULL, PUSH, PUSH_PULL, DISABLED

pub(in crate::game) const IO_NONE: i32 = 0;
pub(in crate::game) const IO_PULL: i32 = 1;
pub(in crate::game) const IO_PUSH: i32 = 2;
pub(in crate::game) const IO_PUSH_PULL: i32 = 3;
pub(in crate::game) const IO_DISABLED: i32 = 4;

pub(in crate::game) fn pulls(m: i32) -> bool {
    m == IO_PULL || m == IO_PUSH_PULL
}
pub(in crate::game) fn pushes(m: i32) -> bool {
    m == IO_PUSH || m == IO_PUSH_PULL
}
// canRecieveInput / canOutput
pub(in crate::game) fn can_receive(m: i32) -> bool {
    matches!(m, IO_NONE | IO_PULL | IO_PUSH_PULL)
}
pub(in crate::game) fn can_output(m: i32) -> bool {
    matches!(m, IO_NONE | IO_PUSH | IO_PUSH_PULL)
}

pub(in crate::game) fn face_modes(n: &Compound) -> [i32; 6] {
    match n.get(&js("faceModes")) {
        Some(Tag::IntArray(a)) if a.len() == 6 => std::array::from_fn(|i| a[i].clamp(0, 4)),
        _ => [IO_NONE; 6],
    }
}

pub(in crate::game) fn set_face_modes(n: &mut Compound, m: [i32; 6]) {
    if m == [IO_NONE; 6] {
        n.remove(&js("faceModes"));
    } else {
        n.insert(js("faceModes"), Tag::IntArray(m.to_vec()));
    }
}

// ---- RedstoneControlMode: IGNORE, ON, OFF, NEVER

pub(in crate::game) fn condition_met(mode: i32, signal: i32) -> bool {
    match mode {
        1 => signal > 0,
        2 => signal == 0,
        3 => false,
        _ => true,
    }
}

// ---- Capacitors (base/capacitor/*, the keys from data/enderio/recipe/capacitor/)

#[derive(Clone, Debug)]
pub(in crate::game) enum Scaler {
    Invalid,
    Identity,
    Linear08,
    Quadratic,
    Quadratic18,
    Cubic,
    Fixed,
    Chemical,
    Cent,
    // IndexedScaler(scale, data), times a factor (BURNTIME is /100).
    Indexed(f32, Vec<f32>, f32),
}

fn indexed(step: f32, data: &[f32]) -> Scaler {
    Scaler::Indexed(step, data.to_vec(), 1.0)
}

impl Scaler {
    fn parse(v: &serde_json::Value) -> Scaler {
        if let Some(o) = v.get("indexed") {
            let step = o.get("step").and_then(|s| s.as_f64()).unwrap_or(1.0) as f32;
            let data: Vec<f32> = o.get("data").and_then(|d| d.as_array()).map(|a| a.iter().filter_map(|x| x.as_f64()).map(|x| x as f32).collect()).unwrap_or_default();
            return if data.is_empty() { Scaler::Invalid } else { indexed(step, &data) };
        }
        let Some(s) = v.as_str() else { return Scaler::Invalid };
        // ScalerFactory.fromString: "idx(step)v0:v1:..."
        if let Some(rest) = s.strip_prefix("idx(") {
            if let Some((step, values)) = rest.split_once(')') {
                let step = step.trim().parse::<f32>().unwrap_or(1.0);
                let data: Vec<f32> = values.split(':').filter_map(|x| x.trim().parse().ok()).collect();
                return if data.is_empty() { Scaler::Invalid } else { indexed(step, &data) };
            }
        }
        match s {
            "IDENTITY" => Scaler::Identity,
            "LINEAR_0_8" => Scaler::Linear08,
            "QUADRATIC" => Scaler::Quadratic,
            "QUADRATIC_1_8" => Scaler::Quadratic18,
            "CUBIC" => Scaler::Cubic,
            "OCTADIC_1_8" => indexed(0.5, &[0.0, 0.5, 1.0, 3.0, 2.0, 4.0, 8.0, 10.0, 16.0]),
            "POWER" => indexed(1.0, &[0.0, 1.0, 3.0, 5.0, 8.0, 13.0, 18.0]),
            "CHARGE" => indexed(1.0, &[1000.0, 100.0, 60.0, 20.0, 10.0, 1.0]),
            "SPEED" => indexed(1.0, &[100.0, 20.0, 10.0, 2.0, 1.0]),
            "POWER10" => indexed(1.0, &[0.0, 1.0, 2.0, 10.0, 20.0, 40.0]),
            "RANGE" => indexed(1.0, &[0.0, 4.0, 6.0, 10.0, 17.0, 24.0]),
            "FIXED" => Scaler::Fixed,
            "SPAWNER" => indexed(1.0, &[0.0, 1.0, 5.0, 10.0, 20.0, 40.0]),
            "BURNTIME" => Scaler::Indexed(1.0, vec![0.8, 1.0, 1.25, 1.5, 1.5, 2.0, 2.5], 0.01),
            "CHEMICAL" => Scaler::Chemical,
            "DROPOFF" => indexed(1.0, &[1.0, 1.0, 1.333_333_4, 2.0, 2.5, 3.0, 3.25]),
            "CENT" => Scaler::Cent,
            _ => Scaler::Invalid,
        }
    }

    fn scale(&self, l: f32) -> f32 {
        match self {
            Scaler::Invalid => 0.0,
            Scaler::Identity => l.max(0.0),
            Scaler::Linear08 => l.clamp(0.0, 8.0),
            Scaler::Quadratic => 2f32.powf(l - 1.0),
            Scaler::Quadratic18 => 2f32.powf(l - 1.0).clamp(1.0, 8.0),
            Scaler::Cubic => 3f32.powf(l - 1.0),
            Scaler::Fixed => 1.0,
            Scaler::Chemical => 1.0 + (l - 1.0) * 0.25,
            Scaler::Cent => 0.01,
            // IndexedScaler.scaleValue: linear interpolation, clamped at both ends.
            Scaler::Indexed(step, v, factor) => {
                let x = l / *step;
                let i = x as i32;
                let f = x - i as f32;
                let out = if i < 0 {
                    v[0]
                } else if i as usize >= v.len() - 1 {
                    v[v.len() - 1]
                } else {
                    (1.0 - f) * v[i as usize] + f * v[i as usize + 1]
                };
                out * factor
            }
        }
    }
}

/// A capacitor's data: NONE (no capacitor), the regular ones' level, or a loot capacitor's `eiocap`.
#[derive(Clone, Debug, PartialEq)]
pub(in crate::game) struct Cap {
    pub none: bool,
    pub level: f32,
    /// eiocap's own values: key registry name or key type name -> unscaled value.
    pub keys: Vec<(String, f32)>,
}

impl Cap {
    pub(in crate::game) fn none() -> Cap {
        Cap { none: true, level: 0.0, keys: Vec::new() }
    }
    pub(in crate::game) fn basic() -> Cap {
        Cap { none: false, level: 1.0, keys: Vec::new() }
    }
    /// getUnscaledValue: the key's own value, then its type's, then the level (NBTCapacitorData's order).
    pub(in crate::game) fn unscaled(&self, key: &str) -> f32 {
        if self.none {
            return 0.0;
        }
        let ty = key_type(key);
        self.keys.iter().find(|(k, _)| k == key).or_else(|| self.keys.iter().find(|(k, _)| k == ty)).map_or(self.level, |(_, v)| *v)
    }
    /// CapacitorHelper.increaseCapacitorLevel: every non-zero unscaled value plus `by`.
    fn increased(&self, by: f32) -> Cap {
        if self.none {
            return self.clone();
        }
        Cap {
            none: false,
            level: if self.level != 0.0 { self.level + by } else { 0.0 },
            keys: self.keys.iter().map(|(k, v)| (k.clone(), if *v != 0.0 { v + by } else { 0.0 })).collect(),
        }
    }
}

/// CapacitorKeyType of a key, by its short name.
fn key_type(key: &str) -> &'static str {
    match key.rsplit('/').next().unwrap_or("") {
        "intake" | "legacy_intake" => "energy_intake",
        "buffer" | "legacy_buffer" => "energy_buffer",
        "use" | "legacy_use" => "energy_use",
        "loss" => "energy_loss",
        "gen" => "energy_gen",
        "efficiency" | "legacy_efficiency" => "energy_efficiency",
        "speed" => "speed",
        "area" => "area",
        _ => "amount",
    }
}

/// CapacitorHelper.getCapacitorDataFromItemStack: `eiocap` first, else the regular capacitors by id.
pub(in crate::game) fn cap_of(stack: &ItemStack) -> Option<Cap> {
    if let Some(eio) = custom_data(stack).and_then(|c| gc(c, "eiocap")) {
        let level = gf(eio, "level", -1.0);
        if (0.0..10.0).contains(&level) {
            let keys = eio
                .iter()
                .filter(|(k, _)| k.to_string_lossy() != "level")
                .filter_map(|(k, v)| {
                    let v = match v {
                        Tag::Float(b) => f32::from_bits(*b),
                        Tag::Double(b) => f64::from_bits(*b) as f32,
                        Tag::Int(i) => *i as f32,
                        _ => return None,
                    };
                    Some((k.to_string_lossy().to_string(), v))
                })
                .collect();
            return Some(Cap { none: false, level, keys });
        }
    }
    let level = match &*stack.item {
        "enderio:basic_capacitor" => 1.0,
        "enderio:double_layer_capacitor" => 2.0,
        "enderio:octadic_capacitor" => 3.0,
        // DefaultCapacitorData SPECIAL and SPECIAL2 (the loot capacitors without their NBT): level 1.
        "enderio:loot_capacitor" | "enderio:loot_capacitor_alt" => 1.0,
        _ => return None,
    };
    Some(Cap { none: false, level, keys: Vec::new() })
}

pub(in crate::game) const CAPACITORS: [&str; 5] = ["enderio:basic_capacitor", "enderio:double_layer_capacitor", "enderio:octadic_capacitor", "enderio:loot_capacitor", "enderio:loot_capacitor_alt"];

// ---- The recipe data, cached until the recipe tables change (a reload gives new slices)

pub(in crate::game) struct AlloyRecipe {
    pub uid: String,
    pub energy: i32,
    pub exp: f32,
    pub inputs: Vec<(horizon_client::crafting::Ingredient, u32)>,
    pub output: ItemStack,
    pub dedupe: bool,
    #[allow(dead_code)] // Read by the JEI category (it leaves the synthetic recipes out).
    pub synthetic: bool,
}

#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub(in crate::game) enum Bonus {
    None,
    Multiply,
    ChanceOnly,
}

impl Bonus {
    fn chances(self) -> bool {
        self != Bonus::None
    }
    fn multiply(self) -> bool {
        self == Bonus::Multiply
    }
}

pub(in crate::game) struct SagRecipe {
    pub uid: String,
    pub energy: i32,
    pub bonus: Bonus,
    pub input: horizon_client::crafting::Ingredient,
    pub count: u32,
    pub outputs: Vec<(ItemStack, f32)>,
}

#[derive(Clone, Debug)]
pub(in crate::game) struct Ball {
    pub item: String,
    pub grinding: f32,
    pub chance: f32,
    pub power: f32,
    pub durability: i32,
}

pub(in crate::game) struct Caches {
    stamp: Vec<(usize, usize)>,
    pub keys: HashMap<String, (f32, Scaler)>,
    pub alloy: Vec<AlloyRecipe>,
    pub sag: Vec<SagRecipe>,
    pub balls: Vec<Ball>,
}

impl Caches {
    /// The key's value at `cap` (`getFloat`): base times the scaler at the unscaled value.
    pub(in crate::game) fn float(&self, key: &str, cap: &Cap) -> f32 {
        match self.keys.get(key) {
            Some((base, scaler)) => {
                if key.ends_with("/loss") && cap.none {
                    // A loss key at NONE is still read (getFloat), with the level 0 scaler value.
                    return base * scaler.scale(0.0);
                }
                base * scaler.scale(cap.unscaled(key))
            }
            None => 0.0,
        }
    }
    /// `get`: the float value truncated.
    pub(in crate::game) fn int(&self, key: &str, cap: &Cap) -> i32 {
        self.float(key, cap) as i32
    }
    /// getDefaultFloat: the value at level 1.
    pub(in crate::game) fn default_float(&self, key: &str) -> f32 {
        self.float(key, &Cap::basic())
    }
    pub(in crate::game) fn ball(&self, item: &str) -> Option<&Ball> {
        self.balls.iter().find(|b| b.item == item)
    }
    pub(in crate::game) fn alloy_by_uid(&self, uid: &str) -> Option<&AlloyRecipe> {
        self.alloy.iter().find(|r| r.uid == uid)
    }
    pub(in crate::game) fn sag_by_uid(&self, uid: &str) -> Option<&SagRecipe> {
        self.sag.iter().find(|r| r.uid == uid)
    }
}

thread_local! {
    static CACHE: RefCell<Option<Rc<Caches>>> = const { RefCell::new(None) };
}

const RECIPE_TYPES: [&str; 4] = ["enderio:capacitor", "enderio:alloy_smelting", "enderio:sag_milling", "enderio:grinding_ball"];

fn stamp(game: &Game<'_>) -> Vec<(usize, usize)> {
    RECIPE_TYPES.iter().map(|t| {
        let s = game.native_recipes_of_type(t);
        (s.as_ptr() as usize, s.len())
    }).collect()
}

pub(in crate::game) fn caches(game: &Game<'_>) -> Rc<Caches> {
    let now = stamp(game);
    if let Some(c) = CACHE.with_borrow(|c| c.clone()).filter(|c| c.stamp == now) {
        return c;
    }
    let c = Rc::new(build_caches(game, now));
    publish_rules(game, &c);
    CACHE.with_borrow_mut(|slot| *slot = Some(c.clone()));
    c
}

pub(in crate::game) fn ingredient(game: &Game<'_>, v: &serde_json::Value) -> Option<horizon_client::crafting::Ingredient> {
    game.native_recipe_ingredient(v)
}

fn build_caches(game: &Game<'_>, stamp: Vec<(usize, usize)>) -> Caches {
    let mut keys = HashMap::new();
    for r in game.native_recipes_of_type("enderio:capacitor") {
        let Some(key) = r.json.get("key").and_then(|k| k.as_str()) else { continue };
        let base = r.json.get("base").and_then(|b| b.as_f64()).unwrap_or(0.0) as f32;
        keys.insert(key.to_owned(), (base, Scaler::parse(r.json.get("scaler").unwrap_or(&serde_json::Value::Null))));
    }
    // AlloyRecipeManager.addRecipe: deduped, synthetic or plain, in the XML's order.
    let mut alloy = Vec::new();
    for r in game.native_recipes_of_type("enderio:alloy_smelting") {
        let j = &r.json;
        let Some(output) = j.get("output").and_then(|o| game.native_recipe_result(o)) else { continue };
        let energy = j.get("energy").and_then(|e| e.as_i64()).unwrap_or(0) as i32;
        let exp = j.get("experience").and_then(|e| e.as_f64()).unwrap_or(0.0) as f32;
        let mut inputs = Vec::new();
        let mut names = Vec::new();
        let mut ok = true;
        for i in j.get("inputs").and_then(|i| i.as_array()).cloned().unwrap_or_default() {
            let count = i.get("count").and_then(|c| c.as_u64()).unwrap_or(1) as u32;
            match i.get("ingredient").and_then(|v| ingredient(game, v)) {
                Some(ing) => {
                    names.push(i.get("ingredient").map(|v| v.to_string()).unwrap_or_default());
                    inputs.push((ing, count));
                }
                None => ok = false,
            }
        }
        if !ok || inputs.is_empty() || inputs.len() > 3 {
            continue;
        }
        let dedupe = (0..names.len()).any(|a| (a + 1..names.len()).any(|b| names[a] == names[b]));
        let max_of = |id: &str| game.native_max_stack_size(id).unwrap_or(64);
        let first_max = inputs[0].0.order.first().map_or(64, |i| max_of(i));
        let synth = !dedupe && inputs.len() == 1 && inputs[0].1 <= first_max / 3 && output.count <= max_of(&output.item) / 3;
        if synth {
            // addSyntheticRecipe: 1+1+1, 1+2, 2+1, 1+1, 3, 2, then the recipe itself.
            let (ing, c) = (inputs[0].0.clone(), inputs[0].1);
            for (k, (shape, mult)) in [(vec![c, c, c], 3), (vec![c, 2 * c], 3), (vec![2 * c, c], 3), (vec![c, c], 2), (vec![3 * c], 3), (vec![2 * c], 2)].into_iter().enumerate() {
                let mut out = output.clone();
                out.count *= mult;
                alloy.push(AlloyRecipe { uid: format!("{}#{k}", r.id), energy: energy * mult as i32, exp, inputs: shape.into_iter().map(|n| (ing.clone(), n)).collect(), output: out, dedupe: false, synthetic: true });
            }
        }
        alloy.push(AlloyRecipe { uid: r.id.clone(), energy, exp, inputs, output, dedupe, synthetic: false });
    }
    let mut sag = Vec::new();
    for r in game.native_recipes_of_type("enderio:sag_milling") {
        let j = &r.json;
        let Some(input) = j.get("input").and_then(|v| ingredient(game, v)) else { continue };
        let bonus = match j.get("bonus").and_then(|b| b.as_str()).unwrap_or("multiply_output") {
            "none" => Bonus::None,
            "chance_only" => Bonus::ChanceOnly,
            _ => Bonus::Multiply,
        };
        let mut outputs = Vec::new();
        for o in j.get("outputs").and_then(|o| o.as_array()).cloned().unwrap_or_default() {
            let count = o.get("count").and_then(|c| c.as_u64()).unwrap_or(1) as u32;
            let chance = o.get("chance").and_then(|c| c.as_f64()).unwrap_or(1.0) as f32;
            // A tag output is the tag's first item at load time; an empty tag drops the output.
            let item = if let Some(id) = o.get("item").and_then(|i| i.as_str()) {
                game.native_max_stack_size(id).map(|_| id.to_owned())
            } else if let Some(tag) = o.get("tag").and_then(|t| t.as_str()) {
                ingredient(game, &serde_json::Value::String(format!("#{tag}"))).and_then(|i| i.order.first().cloned())
            } else {
                None
            };
            if let Some(item) = item {
                outputs.push((ItemStack::new(&item, count.max(1)), chance));
            }
        }
        sag.push(SagRecipe {
            uid: r.id.clone(),
            energy: j.get("energy").and_then(|e| e.as_i64()).unwrap_or(0) as i32,
            bonus,
            input,
            count: j.get("count").and_then(|c| c.as_u64()).unwrap_or(1) as u32,
            outputs,
        });
    }
    let mut balls = Vec::new();
    for r in game.native_recipes_of_type("enderio:grinding_ball") {
        let j = &r.json;
        let Some(item) = j.get("item").and_then(|i| i.as_str()) else { continue };
        if game.native_max_stack_size(item).is_none() {
            continue;
        }
        let f = |k: &str| j.get(k).and_then(|v| v.as_f64()).unwrap_or(1.0) as f32;
        balls.push(Ball { item: item.to_owned(), grinding: f("grinding"), chance: f("chance"), power: f("power"), durability: j.get("durability").and_then(|d| d.as_i64()).unwrap_or(1) as i32 });
    }
    Caches { stamp, keys, alloy, sag, balls }
}

/// The menus' slot rules for the client library (it has no game): which items a slot takes and how many.
fn publish_rules(game: &Game<'_>, c: &Caches) {
    let mut rules: HashMap<String, Vec<(Option<HashSet<String>>, u32)>> = HashMap::new();
    let caps: HashSet<String> = CAPACITORS.iter().map(|s| (*s).to_owned()).collect();
    let alloy_items: HashSet<String> = c.alloy.iter().flat_map(|r| r.inputs.iter().flat_map(|(i, _)| i.order.iter().cloned())).collect();
    let sag_items: HashSet<String> = c.sag.iter().flat_map(|r| r.input.order.iter().cloned()).collect();
    let balls: HashSet<String> = c.balls.iter().map(|b| b.item.clone()).collect();
    for m in MACHINES.iter() {
        let mut slots: Vec<(Option<HashSet<String>>, u32)> = vec![(None, 64); m.slots];
        match m.kind {
            Kind::Alloy if m.profile == Profile::SimpleAlloy => {
                for i in 0..3 {
                    slots[i] = (Some(alloy_items.clone()), 64);
                }
            }
            Kind::Sag => {
                slots[0] = (Some(sag_items.clone()), 64);
                slots[1] = (Some(if m.level == 0 { HashSet::new() } else { balls.clone() }), 64);
            }
            Kind::Alloy | Kind::Stirling => {}
            _ => super::enderio_machines::slot_rules(game, m, &mut slots),
        }
        for &o in m.outputs {
            slots[o] = (Some(HashSet::new()), 64);
        }
        if let Some(cs) = m.cap {
            slots[cs] = (Some(caps.clone()), 1);
        }
        rules.insert(m.id.to_owned(), slots);
    }
    for t in BANK_TIERS.iter() {
        rules.insert(t.id.to_owned(), vec![(Some(HashSet::new()), 1)]);
    }
    rules.insert(super::enderio_conduits::BUNDLE.to_owned(), super::enderio_conduits::slot_rules());
    horizon_client::menu::Menu::hz_enderio_set_rules(rules);
}

// ---- The machines

#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub(in crate::game) enum Kind {
    Alloy,
    Sag,
    Stirling,
    // Phase 2 (enderio_machines.rs).
    Combustion,
    Zombie,
    Vat,
    Soul,
    Slice,
    Enchanter,
    Tank,
    Charger,
    Painter,
}

impl Kind {
    // Machines with an energy buffer (the tank and the enchanter have none).
    pub(in crate::game) fn powered(self) -> bool {
        !matches!(self, Kind::Enchanter | Kind::Tank)
    }
    // Generators: energy leaves them only by their own push.
    pub(in crate::game) fn generator(self) -> bool {
        matches!(self, Kind::Stirling | Kind::Combustion | Kind::Zombie)
    }
}

/// OperatingProfile (alloy smelters), and one each for the others' screens.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub(in crate::game) enum Profile {
    SimpleAlloy,
    SimpleFurnace,
    Auto,
    Sag,
    Stirling,
    Other,
}

pub(in crate::game) struct Mach {
    pub id: &'static str,
    pub kind: Kind,
    /// RecipeLevel: 0 SIMPLE, 1 NORMAL, 2 ADVANCED.
    pub level: u8,
    /// The capacitor keys' owner: `<keys>/intake`, `/buffer`, `/use` (or `/gen`).
    pub keys: &'static str,
    pub loss: Option<&'static str>,
    pub efficiency: Option<&'static str>,
    pub double_op: Option<&'static str>,
    /// Enhanced: two blocks high, +1 capacitor level, its top face NONE only.
    pub enhanced: bool,
    pub cap: Option<usize>,
    pub slots: usize,
    pub inputs: &'static [usize],
    pub outputs: &'static [usize],
    pub profile: Profile,
}

const ALLOY_IN: &[usize] = &[0, 1, 2];
const ALLOY_OUT: &[usize] = &[3];
const SAG_IN: &[usize] = &[0, 1];
const SAG_OUT: &[usize] = &[2, 3, 4, 5];

pub(in crate::game) static MACHINES: [Mach; 25] = [
    // TileAlloySmelter.Furnace: the simple alloy smelter's keys, the simple Stirling's loss (line 182, as Java).
    Mach { id: "enderio:simple_powered_furnace", kind: Kind::Alloy, level: 0, keys: "enderio:block_simple_alloy_smelter", loss: Some("enderio:block_simple_stirling_generator/loss"), efficiency: None, double_op: None, enhanced: false, cap: None, slots: 4, inputs: ALLOY_IN, outputs: ALLOY_OUT, profile: Profile::SimpleFurnace },
    Mach { id: "enderio:simple_alloy_smelter", kind: Kind::Alloy, level: 0, keys: "enderio:block_simple_alloy_smelter", loss: Some("enderio:block_simple_alloy_smelter/loss"), efficiency: None, double_op: None, enhanced: false, cap: None, slots: 4, inputs: ALLOY_IN, outputs: ALLOY_OUT, profile: Profile::SimpleAlloy },
    Mach { id: "enderio:alloy_smelter", kind: Kind::Alloy, level: 1, keys: "enderio:block_alloy_smelter", loss: None, efficiency: None, double_op: None, enhanced: false, cap: Some(4), slots: 5, inputs: ALLOY_IN, outputs: ALLOY_OUT, profile: Profile::Auto },
    Mach { id: "enderio:enhanced_alloy_smelter", kind: Kind::Alloy, level: 2, keys: "enderio:block_enhanced_alloy_smelter", loss: None, efficiency: Some("enderio:block_enhanced_alloy_smelter/efficiency"), double_op: Some("enderio:block_enhanced_alloy_smelter/double_op_chance"), enhanced: true, cap: Some(4), slots: 5, inputs: ALLOY_IN, outputs: ALLOY_OUT, profile: Profile::Auto },
    Mach { id: "enderio:simple_sag_mill", kind: Kind::Sag, level: 0, keys: "enderio:block_simple_sag_mill", loss: Some("enderio:block_simple_sag_mill/loss"), efficiency: None, double_op: None, enhanced: false, cap: None, slots: 6, inputs: SAG_IN, outputs: SAG_OUT, profile: Profile::Sag },
    Mach { id: "enderio:sag_mill", kind: Kind::Sag, level: 1, keys: "enderio:block_sag_mill", loss: None, efficiency: None, double_op: None, enhanced: false, cap: Some(6), slots: 7, inputs: SAG_IN, outputs: SAG_OUT, profile: Profile::Sag },
    Mach { id: "enderio:enhanced_sag_mill", kind: Kind::Sag, level: 2, keys: "enderio:block_enhanced_sag_mill", loss: None, efficiency: Some("enderio:block_enhanced_sag_mill/efficiency"), double_op: Some("enderio:block_enhanced_sag_mill/double_op_chance"), enhanced: true, cap: Some(6), slots: 7, inputs: SAG_IN, outputs: SAG_OUT, profile: Profile::Sag },
    Mach { id: "enderio:simple_stirling_generator", kind: Kind::Stirling, level: 0, keys: "enderio:block_simple_stirling_generator", loss: Some("enderio:block_simple_stirling_generator/loss"), efficiency: None, double_op: None, enhanced: false, cap: None, slots: 1, inputs: &[0], outputs: &[], profile: Profile::Stirling },
    Mach { id: "enderio:stirling_generator", kind: Kind::Stirling, level: 1, keys: "enderio:block_stirling_generator", loss: Some("enderio:block_stirling_generator/loss"), efficiency: None, double_op: None, enhanced: false, cap: Some(1), slots: 2, inputs: &[0], outputs: &[], profile: Profile::Stirling },
    // Phase 2. TileCombustionGenerator (.Enhanced: +1 level, efficiency 2.0, the top block).
    Mach { id: "enderio:combustion_generator", kind: Kind::Combustion, level: 1, keys: "enderio:block_combustion_generator", loss: Some("enderio:block_combustion_generator/loss"), efficiency: None, double_op: None, enhanced: false, cap: Some(0), slots: 1, inputs: &[], outputs: &[], profile: Profile::Other },
    Mach { id: "enderio:enhanced_combustion_generator", kind: Kind::Combustion, level: 2, keys: "enderio:block_enhanced_combustion_generator", loss: Some("enderio:block_enhanced_combustion_generator/loss"), efficiency: Some("enderio:block_enhanced_combustion_generator/efficiency"), double_op: None, enhanced: true, cap: Some(0), slots: 1, inputs: &[], outputs: &[], profile: Profile::Other },
    // TileZombieGenerator, TileFrankenZombieGenerator, TileEnderGenerator.
    Mach { id: "enderio:zombie_generator", kind: Kind::Zombie, level: 1, keys: "enderio:block_zombie_generator", loss: Some("enderio:block_zombie_generator/loss"), efficiency: None, double_op: None, enhanced: false, cap: Some(0), slots: 1, inputs: &[], outputs: &[], profile: Profile::Other },
    Mach { id: "enderio:franken_zombie_generator", kind: Kind::Zombie, level: 1, keys: "enderio:block_franken_zombie_generator", loss: Some("enderio:block_franken_zombie_generator/loss"), efficiency: None, double_op: None, enhanced: false, cap: Some(0), slots: 1, inputs: &[], outputs: &[], profile: Profile::Other },
    Mach { id: "enderio:ender_generator", kind: Kind::Zombie, level: 1, keys: "enderio:block_ender_generator", loss: Some("enderio:block_ender_generator/loss"), efficiency: None, double_op: None, enhanced: false, cap: Some(0), slots: 1, inputs: &[], outputs: &[], profile: Profile::Other },
    // TileVat (.Enhanced never sets its efficiency: it stays 1).
    Mach { id: "enderio:vat", kind: Kind::Vat, level: 1, keys: "enderio:block_vat", loss: None, efficiency: None, double_op: None, enhanced: false, cap: Some(2), slots: 3, inputs: &[0, 1], outputs: &[], profile: Profile::Other },
    Mach { id: "enderio:enhanced_vat", kind: Kind::Vat, level: 2, keys: "enderio:block_enhanced_vat", loss: None, efficiency: None, double_op: Some("enderio:block_enhanced_vat/double_op_chance"), enhanced: true, cap: Some(2), slots: 3, inputs: &[0, 1], outputs: &[], profile: Profile::Other },
    Mach { id: "enderio:soul_binder", kind: Kind::Soul, level: 1, keys: "enderio:block_soul_binder", loss: None, efficiency: None, double_op: None, enhanced: false, cap: Some(4), slots: 5, inputs: &[0, 1], outputs: &[2, 3], profile: Profile::Other },
    // TileSliceAndSplice: the grid 0-5, the axe 6, the shears 7, the output 8.
    Mach { id: "enderio:slice_and_splice", kind: Kind::Slice, level: 1, keys: "enderio:block_slice_and_splice", loss: None, efficiency: None, double_op: None, enhanced: false, cap: Some(9), slots: 10, inputs: &[0, 1, 2, 3, 4, 5, 6, 7], outputs: &[8], profile: Profile::Other },
    // TileEnchanter: no energy, no capacitor, no side IO.
    Mach { id: "enderio:enchanter", kind: Kind::Enchanter, level: 1, keys: "", loss: None, efficiency: None, double_op: None, enhanced: false, cap: None, slots: 4, inputs: &[0, 1, 2], outputs: &[3], profile: Profile::Other },
    // TileTank: SlotDefinition(0,2, 3,4, -1,-1); NORMAL and ADVANCED (pressurized).
    Mach { id: "enderio:fluid_tank", kind: Kind::Tank, level: 1, keys: "", loss: None, efficiency: None, double_op: None, enhanced: false, cap: None, slots: 5, inputs: &[0, 1, 2], outputs: &[3, 4], profile: Profile::Other },
    Mach { id: "enderio:pressurized_fluid_tank", kind: Kind::Tank, level: 2, keys: "", loss: None, efficiency: None, double_op: None, enhanced: false, cap: None, slots: 5, inputs: &[0, 1, 2], outputs: &[3, 4], profile: Profile::Other },
    // TileWiredCharger (.Simple: no capacitor, a loss; .Enhanced: the top block, no +1 level).
    Mach { id: "enderio:simple_wired_charger", kind: Kind::Charger, level: 0, keys: "enderio:block_simple_wired_charger", loss: Some("enderio:block_simple_wired_charger/loss"), efficiency: None, double_op: None, enhanced: false, cap: None, slots: 2, inputs: &[0], outputs: &[1], profile: Profile::Other },
    Mach { id: "enderio:wired_charger", kind: Kind::Charger, level: 1, keys: "enderio:block_wired_charger", loss: None, efficiency: None, double_op: None, enhanced: false, cap: Some(2), slots: 3, inputs: &[0], outputs: &[1], profile: Profile::Other },
    Mach { id: "enderio:enhanced_wired_charger", kind: Kind::Charger, level: 2, keys: "enderio:block_enhanced_wired_charger", loss: None, efficiency: None, double_op: None, enhanced: true, cap: Some(2), slots: 3, inputs: &[0], outputs: &[1], profile: Profile::Other },
    // TileEntityPainter: SlotDefinition(2,1), the capacitor at 3.
    Mach { id: "enderio:painting_machine", kind: Kind::Painter, level: 1, keys: "enderio:block_painter", loss: None, efficiency: None, double_op: None, enhanced: false, cap: Some(3), slots: 4, inputs: &[0, 1], outputs: &[2], profile: Profile::Other },
];

pub(in crate::game) fn mach(id: &str) -> Option<&'static Mach> {
    MACHINES.iter().find(|m| m.id == id)
}

/// STIRLING_POWER_EFFICIENCY: both Stirlings read the normal one's (getBurnEfficiency).
const STIRLING_EFFICIENCY: &str = "enderio:block_stirling_generator/efficiency";

pub(in crate::game) fn top_of(id: &str) -> Option<&'static str> {
    match id {
        "enderio:enhanced_alloy_smelter" => Some("enderio:enhanced_alloy_smelter_top"),
        "enderio:enhanced_sag_mill" => Some("enderio:enhanced_sag_mill_top"),
        "enderio:enhanced_combustion_generator" => Some("enderio:enhanced_combustion_generator_top"),
        "enderio:enhanced_vat" => Some("enderio:enhanced_vat_top"),
        "enderio:enhanced_wired_charger" => Some("enderio:enhanced_wired_charger_top"),
        _ => None,
    }
}

fn bottom_of(top: &str) -> Option<&'static str> {
    MACHINES.iter().map(|m| m.id).find(|id| top_of(id) == Some(top))
}

impl Mach {
    pub(in crate::game) fn key(&self, short: &str) -> String {
        format!("{}/{short}", self.keys)
    }
    fn use_key(&self) -> String {
        self.key(if matches!(self.kind, Kind::Stirling | Kind::Zombie | Kind::Combustion) { "gen" } else { "use" })
    }
    /// The machine's sound loop: event, volume, and the file's length in ticks (it is played again then).
    pub(in crate::game) fn sound(&self) -> Option<(&'static str, f32, i32)> {
        match self.kind {
            Kind::Sag => Some(("enderio:machine.sagmill", 0.125, 272)),
            Kind::Stirling => Some(("enderio:generator.stirling", 1.0, 199)),
            // TileVat: volume x 0.3 at pitch 0.3 (176 ticks of sound, played 1/0.3 slower).
            Kind::Vat => Some(("enderio:machine.vat", 0.3, 587)),
            Kind::Slice => Some(("enderio:machine.slicensplice", 1.0, 232)),
            // TileSoulBinder: its pitch is drawn per play (enderio_machines::soul_pitch).
            Kind::Soul => Some(("enderio:machine.soulbinder", 1.0, 165)),
            _ => None,
        }
    }
}

/// The raw `capacitorData` field (getMaxUsage reads it): BASIC without a slot, the slot's data, or NONE.
pub(in crate::game) fn raw_cap(e: &ModBlockEntity, m: &Mach) -> Cap {
    match m.cap {
        None => Cap::basic(),
        Some(s) => e.items.get(s).cloned().flatten().and_then(|st| cap_of(&st)).unwrap_or_else(Cap::none),
    }
}

/// getCapacitorData: the raw data, one level up on an enhanced machine.
pub(in crate::game) fn cap_data(e: &ModBlockEntity, m: &Mach) -> Cap {
    let raw = raw_cap(e, m);
    // TileWiredCharger.Enhanced keeps the level (only its top face is NONE).
    if m.enhanced && m.kind != Kind::Charger { raw.increased(1.0) } else { raw }
}

pub(in crate::game) fn energy(e: &ModBlockEntity) -> i32 {
    gi(&e.nbt, "energy", 0).max(0)
}

pub(in crate::game) fn set_energy(e: &mut ModBlockEntity, v: i32, max: i32) {
    si(&mut e.nbt, "energy", v.clamp(0, max.max(0)));
}

pub(in crate::game) fn max_energy(c: &Caches, e: &ModBlockEntity, m: &Mach) -> i32 {
    c.int(&m.key("buffer"), &cap_data(e, m))
}

pub(in crate::game) fn power_use(c: &Caches, e: &ModBlockEntity, m: &Mach) -> i32 {
    c.int(&m.use_key(), &raw_cap(e, m))
}

pub(in crate::game) fn power_loss(c: &Caches, e: &ModBlockEntity, m: &Mach) -> f32 {
    m.loss.map_or(0.0, |k| c.float(k, &cap_data(e, m)))
}

pub(in crate::game) fn intake(c: &Caches, e: &ModBlockEntity, m: &Mach) -> i32 {
    c.int(&m.key("intake"), &cap_data(e, m))
}

pub(in crate::game) fn efficiency(c: &Caches, e: &ModBlockEntity, m: &Mach) -> f32 {
    // LEGACY_ENERGY_EFFICIENCY is 1 FIXED.
    m.efficiency.map_or(1.0, |k| c.float(k, &cap_data(e, m)))
}

/// getBurnEfficiency at a capacitor's data.
pub(in crate::game) fn burn_efficiency(c: &Caches, cap: &Cap) -> f32 {
    c.float(STIRLING_EFFICIENCY, cap)
}

// ---- The task (PoweredTask), kept as `currentTask`

#[derive(Clone, Debug, PartialEq)]
pub(in crate::game) struct Task {
    /// 0 alloying, 1 the furnace (VanillaSmeltingRecipe), 2 SAG milling.
    pub kind: i32,
    pub recipe: String,
    pub used: f32,
    pub required: f32,
    pub seed: i64,
    pub out_mult: f32,
    pub chance_mult: f32,
    /// The furnace's batch (1-3) and the item it smelts.
    pub batch: u32,
    pub item: String,
    /// Phase 2: the results worked out at the start from the inputs it took (the soul binder, the slice'n'splice,
    /// the painter: none of them has chances), and the vat's fluids (in, out, the out mB).
    pub results: Vec<ItemStack>,
    pub fluid_in: String,
    pub fluid_out: String,
    pub out_mb: i32,
}

impl Task {
    pub(in crate::game) fn progress(&self) -> f32 {
        if self.required <= 0.0 { 1.0 } else { (self.used / self.required).clamp(0.0, 1.0) }
    }
    fn complete(&self) -> bool {
        self.used >= self.required
    }
}

pub(in crate::game) fn task(e: &ModBlockEntity) -> Option<Task> {
    let t = gc(&e.nbt, "currentTask")?;
    Some(Task {
        kind: gi(t, "kind", 0),
        recipe: gs(t, "recipeUid").unwrap_or_default(),
        used: gf(t, "usedEnergy", 0.0),
        required: gf(t, "requiredEnergy", 0.0),
        seed: gl(t, "seed"),
        out_mult: gf(t, "chance1", 1.0),
        chance_mult: gf(t, "chance2", 1.0),
        batch: gi(t, "batch", 1).max(1) as u32,
        item: gs(t, "item").unwrap_or_default(),
        results: horizon_items::item_nbt::decode_items(t.get(&js("results")), 4).into_iter().flatten().collect(),
        fluid_in: gs(t, "fluidIn").unwrap_or_default(),
        fluid_out: gs(t, "fluidOut").unwrap_or_default(),
        out_mb: gi(t, "outMb", 0),
    })
}

pub(in crate::game) fn set_task(e: &mut ModBlockEntity, t: Option<&Task>) {
    match t {
        None => {
            e.nbt.remove(&js("currentTask"));
        }
        Some(t) => {
            let mut c = Compound::new();
            si(&mut c, "kind", t.kind);
            ss(&mut c, "recipeUid", &t.recipe);
            sf(&mut c, "usedEnergy", t.used);
            sf(&mut c, "requiredEnergy", t.required);
            c.insert(js("seed"), Tag::Long(t.seed));
            sf(&mut c, "chance1", t.out_mult);
            sf(&mut c, "chance2", t.chance_mult);
            si(&mut c, "batch", t.batch as i32);
            ss(&mut c, "item", &t.item);
            if !t.results.is_empty() {
                let list: Vec<Option<ItemStack>> = t.results.iter().cloned().map(Some).collect();
                c.insert(js("results"), horizon_items::item_nbt::encode_items(&list));
            }
            if !t.fluid_out.is_empty() {
                ss(&mut c, "fluidIn", &t.fluid_in);
                ss(&mut c, "fluidOut", &t.fluid_out);
                si(&mut c, "outMb", t.out_mb);
            }
            e.nbt.insert(js("currentTask"), Tag::Compound(c));
        }
    }
}

// ---- Alloying (base/recipe/Recipe.java, BasicManyToOneRecipe, AlloyRecipeManager)

pub(in crate::game) fn slot_stacks(e: &ModBlockEntity, slots: &[usize]) -> Vec<Option<ItemStack>> {
    slots.iter().map(|s| e.items.get(*s).cloned().flatten()).collect()
}

fn distinct(stacks: &[Option<ItemStack>]) -> bool {
    let ids: Vec<&str> = stacks.iter().flatten().map(|s| &*s.item).collect();
    (0..ids.len()).all(|a| (a + 1..ids.len()).all(|b| ids[a] != ids[b]))
}

/// Recipe.isInputForRecipe: every stack is an input of the recipe, each credited to the first still-open input it
/// fits (its whole count), and every input filled.
fn alloy_matches(r: &AlloyRecipe, stacks: &[Option<ItemStack>]) -> bool {
    if stacks.iter().all(Option::is_none) {
        return false;
    }
    if stacks.iter().flatten().any(|s| !r.inputs.iter().any(|(i, _)| i.test(s))) {
        return false;
    }
    if r.dedupe && !distinct(stacks) {
        return false;
    }
    let mut open: Vec<(usize, i64)> = r.inputs.iter().enumerate().map(|(i, (_, c))| (i, i64::from(*c))).collect();
    for s in stacks.iter().flatten() {
        let Some(at) = open.iter().position(|(i, _)| r.inputs[*i].0.test(s)) else { return false };
        open[at].1 -= i64::from(s.count);
        if open[at].1 <= 0 {
            open.remove(at);
        }
    }
    open.is_empty()
}

/// BasicManyToOneRecipe.isValidRecipeComponents: each stack fits a different input.
fn alloy_components(r: &AlloyRecipe, stacks: &[Option<ItemStack>]) -> bool {
    if r.dedupe && !distinct(stacks) {
        return false;
    }
    let mut left: Vec<usize> = (0..r.inputs.len()).collect();
    for s in stacks.iter().flatten() {
        let Some(at) = left.iter().position(|i| r.inputs[*i].0.test(s)) else { return false };
        left.remove(at);
    }
    true
}

/// getQuantitiesConsumed: each input in order takes what it needs from the slots in order.
fn alloy_consumed(r: &AlloyRecipe, stacks: &[Option<ItemStack>]) -> Vec<(usize, u32)> {
    let mut avail: Vec<u32> = stacks.iter().map(|s| s.as_ref().map_or(0, |s| s.count)).collect();
    let mut out = Vec::new();
    for (ing, count) in &r.inputs {
        let mut need = *count;
        for (i, s) in stacks.iter().enumerate() {
            let Some(s) = s else { continue };
            if avail[i] == 0 || !ing.test(s) {
                continue;
            }
            let take = need.min(avail[i]);
            need -= take;
            avail[i] -= take;
            out.push((i, take));
            if need == 0 {
                break;
            }
        }
    }
    out
}

pub(in crate::game) fn alloy_recipe<'a>(c: &'a Caches, stacks: &[Option<ItemStack>]) -> Option<&'a AlloyRecipe> {
    c.alloy.iter().find(|r| alloy_matches(r, stacks))
}

fn smelt_result(game: &Game<'_>, stack: &ItemStack) -> Option<(ItemStack, f32)> {
    game.recipes().cook("minecraft:smelting", stack).map(|r| (r.result.clone(), r.experience))
}

/// VanillaSmeltingRecipe.isRecipe: every stack smelts, all to the same result.
fn furnace_matches(game: &Game<'_>, stacks: &[Option<ItemStack>]) -> Option<(ItemStack, String)> {
    let mut out: Option<(ItemStack, String)> = None;
    for s in stacks.iter().flatten() {
        let (r, _) = smelt_result(game, s)?;
        match &out {
            None => out = Some((r, s.item.to_string())),
            Some((o, _)) if o.item == r.item && o.damage == r.damage => {}
            Some(_) => return None,
        }
    }
    out
}

/// getNumInputs: up to 3 of the stacks that smelt.
fn furnace_batch(game: &Game<'_>, stacks: &[Option<ItemStack>]) -> u32 {
    stacks.iter().flatten().filter(|s| smelt_result(game, s).is_some()).map(|s| s.count).sum::<u32>().min(3)
}

/// OperatingMode (ALL, ALLOY, FURNACE) of an alloy smelter: its profile's, or `mode` when it may switch.
pub(in crate::game) fn alloy_mode(e: &ModBlockEntity, m: &Mach) -> i32 {
    match m.profile {
        Profile::SimpleAlloy => 1,
        Profile::SimpleFurnace => 2,
        _ => gi(&e.nbt, "mode", 0).clamp(0, 2),
    }
}

/// isMachineItemValidForSlot, for each machine.
pub(in crate::game) fn valid_for_slot(game: &Game<'_>, c: &Caches, e: &ModBlockEntity, m: &Mach, slot: usize, stack: &ItemStack) -> bool {
    if Some(slot) == m.cap {
        return cap_of(stack).is_some();
    }
    match m.kind {
        Kind::Alloy => {
            if !m.inputs.contains(&slot) {
                return false;
            }
            if e.items.get(slot).cloned().flatten().is_some_and(|s| s.item == stack.item && s.damage == stack.damage) {
                return true;
            }
            let mode = alloy_mode(e, m);
            if mode != 2 {
                let mut st = slot_stacks(e, m.inputs);
                st[slot] = Some(stack.clone());
                if c.alloy.iter().any(|r| alloy_components(r, &st)) {
                    return true;
                }
            }
            if mode != 1 {
                for &i in m.inputs {
                    if i != slot && e.items.get(i).cloned().flatten().is_some_and(|s| !(s.item == stack.item && s.damage == stack.damage)) {
                        return false;
                    }
                }
                if smelt_result(game, stack).is_some() {
                    return true;
                }
            }
            false
        }
        Kind::Sag => match slot {
            0 => c.sag.iter().any(|r| r.input.test(stack)),
            1 => m.level > 0 && c.ball(&stack.item).is_some(),
            _ => false,
        },
        Kind::Stirling => {
            slot == 0 && burn_time_of(game, stack) > 0 && (m.level > 0 || remainder(game, &stack.item).is_none())
        }
        _ => super::enderio_machines::valid_for_slot(game, e, m, slot, stack),
    }
}

pub(in crate::game) fn burn_time_of(game: &Game<'_>, stack: &ItemStack) -> i32 {
    game.res.providers().burn_time(game.res.items(), stack, false)
}

pub(in crate::game) fn remainder(game: &Game<'_>, item: &str) -> Option<ItemStack> {
    game.res.items().get(item).and_then(|i| i.crafting_remainder.as_deref()).map(|r| ItemStack::new(r, 1))
}

pub(in crate::game) fn max_stack(game: &Game<'_>, item: &str) -> u32 {
    game.native_max_stack_size(item).unwrap_or(64).max(1)
}

// ---- Results (PoweredTask.getCompletedResult, AbstractMachineRecipe.getCompletedResult)

fn sag_roll(r: &SagRecipe, seed: i64, chance_mult: f32) -> Vec<ItemStack> {
    let mut rand = LegacyRandom::new(seed);
    let mut out = Vec::new();
    for (stack, chance) in &r.outputs {
        let mut n = 0;
        for _ in 0..stack.count {
            if rand.next_float() < chance * chance_mult {
                n += 1;
            }
        }
        if n > 0 {
            out.push(ItemStack { count: n, ..stack.clone() });
        }
    }
    out
}

fn task_results(game: &Game<'_>, c: &Caches, t: &Task) -> Vec<ItemStack> {
    match t.kind {
        0 => c.alloy_by_uid(&t.recipe).map(|r| vec![r.output.clone()]).unwrap_or_default(),
        3..=6 => t.results.clone(),
        1 => smelt_result(game, &ItemStack::new(&t.item, 1)).map(|(r, _)| vec![ItemStack { count: r.count * t.batch, ..r }]).unwrap_or_default(),
        _ => {
            let Some(r) = c.sag_by_uid(&t.recipe) else { return Vec::new() };
            let mut rand = LegacyRandom::new(t.seed);
            let cm = if r.bonus.chances() { t.chance_mult } else { 1.0 };
            let mut out = sag_roll(r, rand.next_long(), cm);
            if r.bonus.multiply() {
                let mut mul = t.out_mult - 1.0;
                while mul > 0.0 {
                    if rand.next_float() < mul {
                        out.extend(sag_roll(r, rand.next_long(), cm));
                    }
                    mul -= 1.0;
                }
            }
            out
        }
    }
}

/// mergeItemResult: top up equal stacks, then the first empty slot; the count merged, 0 when it didn't all fit.
pub(in crate::game) fn merge_item(game: &Game<'_>, item: &ItemStack, outs: &mut [Option<ItemStack>]) -> u32 {
    let mut copy = item.clone();
    let mut free = None;
    for (i, o) in outs.iter_mut().enumerate() {
        match o {
            Some(s) => {
                if s.same_item_same_components(&copy) {
                    let n = max_stack(game, &s.item).saturating_sub(s.count).min(copy.count);
                    s.count += n;
                    copy.count -= n;
                    if copy.count == 0 {
                        return item.count;
                    }
                }
            }
            None => {
                if free.is_none() {
                    free = Some(i);
                }
            }
        }
    }
    match free {
        Some(i) => {
            outs[i] = Some(copy);
            item.count
        }
        None => 0,
    }
}

// ---- Shared state

#[derive(Default)]
struct State {
    /// Blocks whose neighbours changed: redstone, receptors, displays are looked at again.
    dirty: HashSet<Pos>,
    /// Who has which machine's screen open (for the smelting XP), and who took from an output slot.
    open: HashMap<u128, Pos>,
    taken: Vec<(u128, Pos)>,
    /// Blocks to take away on the next level tick (a hook amid a block's change can't set blocks), with a drop.
    removals: Vec<(String, Pos, String, Option<ItemStack>)>,
    /// Blocks to set on the next level tick (the enhanced machines' tops).
    placements: Vec<(String, Pos, String)>,
    /// Our block entities as they were when they left the world this tick (their items taken out first, so
    /// nothing spills): a drop asked for after the removal (a player's break) still keeps them.
    removed: HashMap<Pos, ModBlockEntity>,
}

thread_local! {
    static STATE: RefCell<State> = RefCell::new(State::default());
}

fn take_dirty(pos: Pos) -> bool {
    STATE.with_borrow_mut(|s| s.dirty.remove(&pos))
}

pub(in crate::game) fn next_long(game: &mut Game<'_>) -> i64 {
    let dim = game.native_dimension();
    match game.native_level_random(dim) {
        Some(r) => r.next_long(),
        None => LegacyRandom::new(std::time::SystemTime::now().duration_since(std::time::UNIX_EPOCH).map_or(0, |d| d.as_nanos() as i64)).next_long(),
    }
}

pub(in crate::game) fn next_float(game: &mut Game<'_>) -> f32 {
    let dim = game.native_dimension();
    game.native_level_random(dim).map_or(0.5, |r| r.next_float())
}

pub(in crate::game) fn next_double(game: &mut Game<'_>) -> f64 {
    let dim = game.native_dimension();
    game.native_level_random(dim).map_or(0.5, |r| r.next_double())
}

/// shouldDoWorkThisTick(interval): the machine's own phase.
pub(in crate::game) fn work(e: &ModBlockEntity, interval: i32) -> bool {
    (gi(&e.nbt, "hz_t", 0) + gi(&e.nbt, "hz_off", 0)).rem_euclid(interval) == 0
}

// ---- Block entity, container, energy declarations

pub fn block_entity(block: &str, entity: &mut Option<(String, Compound)>) {
    if entity.is_none() && (mach(block).is_some() || bank(block).is_some()) {
        *entity = Some((block.to_owned(), Compound::new()));
    }
    super::enderio_conduits::block_entity(block, entity);
}

pub fn block_entity_ticks(block: &str, ticks: &mut bool) {
    if mach(block).is_some() || bank(block).is_some() {
        *ticks = true;
    }
    super::enderio_conduits::block_entity_ticks(block, ticks);
}

pub fn block_entity_container(block: &str, out: &mut Option<ModContainer>) {
    super::enderio_conduits::block_entity_container(block, out);
    if let Some(m) = mach(block) {
        // LegacyMachineWrapper: inputs in, outputs out; never the capacitor. Per face, container_accepts and
        // can_take apply the IO modes.
        // The enchanter: ISidedInventory with no slots for any face.
        let io: Vec<usize> = if m.kind == Kind::Enchanter { Vec::new() } else { m.inputs.iter().chain(m.outputs.iter()).copied().filter(|s| !(m.kind == Kind::Sag && m.level == 0 && *s == 1)).collect() };
        let outputs = if m.kind == Kind::Stirling { vec![0] } else { m.outputs.to_vec() };
        *out = Some(ModContainer { slots: m.slots, faces: std::array::from_fn(|_| io.clone()), inputs: m.inputs.to_vec(), outputs, menu: Some(m.id.to_owned()) });
    } else if bank(block).is_some() {
        // ContainerCapBank has no slots; a menu needs one, kept off the screen and closed to items.
        *out = Some(ModContainer { slots: 1, faces: std::array::from_fn(|_| Vec::new()), inputs: Vec::new(), outputs: Vec::new(), menu: Some(block.to_owned()) });
    }
}

pub fn block_entity_energy(block: &str, out: &mut Option<ModEnergyBuffer>) {
    // The buffer's real size, limits and faces are the instance's (capacitor, IO modes, the bank's network): the
    // energy hooks below answer for each block. This declaration only makes the block an energy block.
    if mach(block).is_some_and(|m| m.kind.powered()) || bank(block).is_some() {
        *out = Some(ModEnergyBuffer { capacity: i32::MAX, max_insert: i32::MAX, max_extract: 0, insert_faces: [true; 6], extract_faces: [false; 6] });
    }
}

// ---- The machines' tick

pub(in crate::game) fn block_entity_tick(game: &mut Game<'_>, pos: Pos, e: &mut ModBlockEntity) {
    if !game.native_on(NS) {
        return;
    }
    if let Some(m) = mach(&e.id) {
        machine_tick(game, pos, e, m);
    } else if let Some(t) = bank(&e.id) {
        bank_tick(game, pos, e, t);
    } else if e.id == super::enderio_conduits::BUNDLE {
        super::enderio_conduits::block_entity_tick(game, pos, e);
    }
}

fn machine_tick(game: &mut Game<'_>, pos: Pos, e: &mut ModBlockEntity, m: &'static Mach) {
    let c = caches(game);
    if e.items.len() < m.slots {
        e.items.resize(m.slots, None);
    }
    let t = gi(&e.nbt, "hz_t", 0).wrapping_add(1);
    si(&mut e.nbt, "hz_t", t);
    if !e.nbt.contains_key(&js("hz_off")) {
        // checkOffset = (int)(Math.random() * 20)
        let off = (next_float(game) * 20.0) as i32;
        si(&mut e.nbt, "hz_off", off);
    }
    si(&mut e.nbt, "hz_rx", 0);
    e.nbt.remove(&js("hz_rx_f"));
    let max = max_energy(&c, e, m);
    let now = energy(e);
    if now > max {
        set_energy(e, now, max);
    }
    let dim = game.native_dimension();
    // 1. Redstone, when dirty (a neighbour changed, the mode changed, or never checked).
    if take_dirty(pos) || !e.nbt.contains_key(&js("redstoneCheckPassed")) || gi(&e.nbt, "hz_rs", 0) != 0 {
        let signal = game.native_neighbor_signal(dim, pos);
        let passed = condition_met(gi(&e.nbt, "redstoneControlMode", 0), signal);
        si(&mut e.nbt, "redstoneCheckPassed", i32::from(passed));
        si(&mut e.nbt, "hz_rs", 0);
    }
    let passed = gi(&e.nbt, "redstoneCheckPassed", 1) != 0;
    // XP for what a player took from the output (SlotSmelter.onTake).
    if m.kind == Kind::Alloy {
        award_xp(game, &c, pos, e);
    }
    // 2. Side IO: every 5 ticks, each move again every 20 (the same phase).
    if work(e, 5) && work(e, 20) {
        side_io(game, &c, pos, e, m);
    }
    // 3. Tasks.
    match m.kind {
        Kind::Alloy | Kind::Sag => process_tasks(game, &c, pos, e, m, passed),
        Kind::Stirling => stirling_tick(game, &c, pos, e, m, passed),
        _ => super::enderio_machines::tick(game, &c, pos, e, m, passed),
    }
    // 4. The loss (AbstractPoweredMachineEntity.doUpdate after the rest).
    lose_power(&c, e, m);
    if m.kind == Kind::Alloy {
        let n = e.items.get(3).cloned().flatten().map_or(0, |s| s.count);
        si(&mut e.nbt, "hz_out", n as i32);
    }
    let active = is_active(e, m);
    active_state(game, pos, e, active);
    if let Some((sound, volume, period)) = m.sound() {
        let left = gi(&e.nbt, "hz_snd", 0);
        if active {
            if left <= 0 {
                let at = [f64::from(pos[0]) + 0.5, f64::from(pos[1]) + 0.5, f64::from(pos[2]) + 0.5];
                let pitch = match m.kind {
                    Kind::Soul => super::enderio_machines::soul_pitch(game, &c, e, m),
                    Kind::Vat => 0.3,
                    _ => 1.0,
                };
                game.native_play_sound_in_place(sound, at, 4, volume, pitch);
                let period = if m.kind == Kind::Soul { (period as f32 / pitch).round() as i32 } else { period };
                si(&mut e.nbt, "hz_snd", period);
            } else {
                si(&mut e.nbt, "hz_snd", left - 1);
            }
        } else if left != 0 {
            si(&mut e.nbt, "hz_snd", 0);
        }
    }
    super::enderio_machines::screen_flags(game, e, m);
    fill_machine_data(&c, e, m);
}

/// isActive: a task under way with power and redstone; a Stirling while it burns.
pub(in crate::game) fn is_active(e: &ModBlockEntity, m: &Mach) -> bool {
    match m.kind {
        Kind::Stirling => gi(&e.nbt, "burnTime", 0) > 0,
        Kind::Combustion | Kind::Zombie => gi(&e.nbt, "active", 0) != 0,
        Kind::Enchanter | Kind::Tank | Kind::Charger => false,
        _ => task(e).is_some() && energy(e) > 0 && gi(&e.nbt, "redstoneCheckPassed", 1) != 0,
    }
}

/// The client's smoothing (updateEntityClient): the `active` state turns on after more than 4 ticks and off after
/// more than 20 of the other answer; set from the server here.
pub(in crate::game) fn active_state(game: &mut Game<'_>, pos: Pos, e: &mut ModBlockEntity, active: bool) {
    let shown = gi(&e.nbt, "hz_shown", 0) != 0;
    if active == shown {
        si(&mut e.nbt, "hz_flip", 0);
        return;
    }
    let n = gi(&e.nbt, "hz_flip", 0) + 1;
    if (active && n > 4) || (!active && n > 20) {
        si(&mut e.nbt, "hz_shown", i32::from(active));
        si(&mut e.nbt, "hz_flip", 0);
        game.native_set_block_property_with(pos, "active", if active { "true" } else { "false" }, 3);
    } else {
        si(&mut e.nbt, "hz_flip", n);
    }
}

/// losePower: under 1 µI/t, 1 µI every so often by an accumulator; from 1 up, whole µI.
pub(in crate::game) fn lose_power(c: &Caches, e: &mut ModBlockEntity, m: &Mach) {
    let loss = power_loss(c, e, m);
    if loss <= 0.0 {
        return;
    }
    let max = max_energy(c, e, m);
    let stored = energy(e);
    if loss < 1.0 {
        let mut partial = gf(&e.nbt, "partialEnergyLoss", 0.0);
        if partial < loss {
            if stored <= 0 {
                sf(&mut e.nbt, "partialEnergyLoss", 0.0);
                return;
            }
            set_energy(e, stored - 1, max);
            partial += 1.0;
        }
        partial -= loss;
        sf(&mut e.nbt, "partialEnergyLoss", partial);
    } else {
        let used = (stored as f32).min(loss) as i32;
        if used > 0 {
            set_energy(e, stored - used, max);
        }
    }
}

/// usePower(want): what there is, up to `want`; the SAG mill wears its ball by what was used.
pub(in crate::game) fn use_power(game: &Game<'_>, c: &Caches, e: &mut ModBlockEntity, m: &Mach, want: i32) -> i32 {
    let max = max_energy(c, e, m);
    let stored = energy(e);
    let used = stored.min(want).max(0);
    set_energy(e, stored - used, max);
    if m.kind == Kind::Sag {
        wear_ball(game, c, e, used);
    }
    used
}

/// TileSagMill.usePower: the ball in use wears by the energy used; a worn one is replaced from slot 1 at once.
fn wear_ball(game: &Game<'_>, c: &Caches, e: &mut ModBlockEntity, used: i32) {
    let Some(t) = task(e) else { return };
    if t.kind != 2 || !c.sag_by_uid(&t.recipe).is_some_and(|r| r.bonus != Bonus::None) {
        return;
    }
    if gc(&e.nbt, "grindingBall").is_some() {
        let mut u = gi(&e.nbt, "grindingBallDurabilityUsed", 0) + used;
        let max = gi(&e.nbt, "grindingBallDurabilityMax", 0);
        if u >= max {
            u -= max;
            si(&mut e.nbt, "grindingBallDurabilityMax", 0);
            e.nbt.remove(&js("grindingBall"));
        }
        si(&mut e.nbt, "grindingBallDurabilityUsed", u);
    }
    if gc(&e.nbt, "grindingBall").is_none() {
        let Some(st) = e.items.get(1).cloned().flatten() else { return };
        let Some(b) = c.ball(&st.item).cloned() else { return };
        let mut ball = Compound::new();
        ss(&mut ball, "item", &b.item);
        sf(&mut ball, "grindingMultiplier", b.grinding);
        sf(&mut ball, "chanceMultiplier", b.chance);
        sf(&mut ball, "powerMultiplier", b.power);
        si(&mut ball, "durationMJ", b.durability);
        e.nbt.insert(js("grindingBall"), Tag::Compound(ball));
        si(&mut e.nbt, "grindingBallDurabilityMax", b.durability);
        e.items[1] = if st.count > 1 { Some(ItemStack { count: st.count - 1, ..st }) } else { None };
    }
    let _ = game;
}

fn ball_loaded(e: &ModBlockEntity) -> Option<(f32, f32, f32)> {
    let b = gc(&e.nbt, "grindingBall")?;
    Some((gf(b, "grindingMultiplier", 1.0), gf(b, "chanceMultiplier", 1.0), gf(b, "powerMultiplier", 1.0)))
}

/// AbstractPoweredTaskEntity.processTasks.
pub(in crate::game) fn process_tasks(game: &mut Game<'_>, c: &Caches, pos: Pos, e: &mut ModBlockEntity, m: &Mach, passed: bool) {
    if !passed {
        return;
    }
    // TileSliceAndSplice: without both tools no task starts and none goes on.
    if !super::enderio_machines::can_work(game, e, m) {
        return;
    }
    // checkProgress
    if let Some(mut t) = task(e) {
        if energy(e) > 0 {
            if !t.complete() {
                let want = power_use(c, e, m);
                let eff = efficiency(c, e, m);
                let used = use_power(game, c, e, m, want);
                super::enderio_machines::power_used(game, e, m, want);
                t = task(e).unwrap_or(t);
                t.used += used as f32 * eff;
                set_task(e, Some(&t));
                // shouldDoubleTick (enhanced): the level times this tick's use over the task's energy.
                if let Some(key) = m.double_op {
                    let chance = f64::from(cap_data(e, m).unscaled(key)) * (f64::from(want) / f64::from(t.required.max(1.0)));
                    if next_double(game) < chance {
                        let used = use_power(game, c, e, m, want);
                        super::enderio_machines::power_used(game, e, m, want);
                        t = task(e).unwrap_or(t);
                        t.used += used as f32 * eff;
                        set_task(e, Some(&t));
                    }
                }
            }
            if t.complete() {
                task_complete(game, c, e, m, &t);
            }
        }
    }
    let inputs = slot_stacks(e, m.inputs);
    if task(e).is_some() || energy(e) <= 0 || inputs.iter().all(Option::is_none) {
        return;
    }
    if gi(&e.nbt, "hz_failed", 0) != 0 {
        let n = gi(&e.nbt, "hz_since", 0) + 1;
        si(&mut e.nbt, "hz_since", n);
        // MachineConfig.sleepBetweenFailedTries
        if n < 20 {
            return;
        }
    }
    si(&mut e.nbt, "hz_since", 0);
    // theNextSeed: drawn once, kept until a task starts.
    let seed = if e.nbt.contains_key(&js("hz_seed")) {
        gl(&e.nbt, "hz_seed")
    } else {
        let s = next_long(game);
        e.nbt.insert(js("hz_seed"), Tag::Long(s));
        s
    };
    let started = match next_task(game, c, e, m, seed) {
        Some((t, consumed)) if can_insert(game, c, e, m, &t) => {
            for (slot, n) in consumed {
                let s = m.inputs[slot];
                if let Some(st) = e.items[s].as_mut() {
                    st.count = st.count.saturating_sub(n);
                    if st.count == 0 {
                        e.items[s] = None;
                    }
                }
            }
            super::enderio_machines::task_started(game, e, m, &t);
            set_task(e, Some(&t));
            e.nbt.remove(&js("hz_seed"));
            true
        }
        _ => false,
    };
    si(&mut e.nbt, "hz_failed", i32::from(!started));
    let _ = pos;
}

/// The task the inputs make now (canStartNextTask, createTask) and what starting it consumes, by input index.
fn next_task(game: &Game<'_>, c: &Caches, e: &ModBlockEntity, m: &Mach, seed: i64) -> Option<(Task, Vec<(usize, u32)>)> {
    let inputs = slot_stacks(e, m.inputs);
    match m.kind {
        Kind::Alloy => {
            let mode = alloy_mode(e, m);
            if mode != 2 {
                if let Some(r) = alloy_recipe(c, &inputs) {
                    let t = Task { kind: 0, recipe: r.uid.clone(), used: 0.0, required: r.energy as f32, seed, out_mult: 1.0, chance_mult: 1.0, batch: 1, item: String::new(), results: Vec::new(), fluid_in: String::new(), fluid_out: String::new(), out_mb: 0 };
                    return Some((t, alloy_consumed(r, &inputs)));
                }
            }
            if mode != 1 {
                if let Some((_, item)) = furnace_matches(game, &inputs) {
                    let batch = furnace_batch(game, &inputs);
                    // RF_PER_ITEM: coal's 1600 ticks x 10 / 8.
                    let t = Task { kind: 1, recipe: "VanillaSmeltingRecipe".into(), used: 0.0, required: (batch * 2000) as f32, seed, out_mult: 1.0, chance_mult: 1.0, batch, item, results: Vec::new(), fluid_in: String::new(), fluid_out: String::new(), out_mb: 0 };
                    let mut left = 3u32;
                    let mut consumed = Vec::new();
                    for (i, s) in inputs.iter().enumerate() {
                        let Some(s) = s else { continue };
                        if left == 0 || smelt_result(game, s).is_none() {
                            continue;
                        }
                        let n = left.min(s.count);
                        consumed.push((i, n));
                        left -= n;
                    }
                    return Some((t, consumed));
                }
            }
            None
        }
        Kind::Sag => {
            let s = inputs[0].clone()?;
            let r = c.sag.iter().find(|r| r.input.test(&s) && s.count >= r.count)?;
            let mut t = Task { kind: 2, recipe: r.uid.clone(), used: 0.0, required: r.energy as f32, seed, out_mult: 1.0, chance_mult: 1.0, batch: 1, item: String::new(), results: Vec::new(), fluid_in: String::new(), fluid_out: String::new(), out_mb: 0 };
            // createTask: with a ball already in use (not the one in slot 1), its multipliers and power.
            if let Some((g, ch, p)) = ball_loaded(e).filter(|_| r.bonus.chances()) {
                t.out_mult = g;
                t.chance_mult = ch;
                t.required *= p;
            }
            Some((t, vec![(0, r.count)]))
        }
        Kind::Stirling => None,
        _ => super::enderio_machines::next_task(game, e, m, seed),
    }
}

/// canInsertResult: the task's real results must all fit, and not every output slot may be full.
fn can_insert(game: &Game<'_>, c: &Caches, e: &ModBlockEntity, m: &Mach, t: &Task) -> bool {
    // canInsertResultFluid (the vat's output tank), the soul binder's XP.
    if !super::enderio_machines::can_insert(e, m, t) {
        return false;
    }
    if m.outputs.is_empty() {
        return true;
    }
    let mut outs: Vec<Option<ItemStack>> = slot_stacks(e, m.outputs);
    if outs.iter().all(|o| o.as_ref().is_some_and(|s| s.count >= max_stack(game, &s.item))) {
        return false;
    }
    task_results(game, c, t).iter().all(|r| merge_item(game, r, &mut outs) != 0)
}

/// taskComplete: the last recipe (for XP), the results merged into the outputs.
fn task_complete(game: &mut Game<'_>, c: &Caches, e: &mut ModBlockEntity, m: &Mach, t: &Task) {
    super::enderio_machines::task_done(game, e, m, t);
    let last = if t.kind == 1 { format!("furnace:{}", t.item) } else { t.recipe.clone() };
    ss(&mut e.nbt, "lastCompletedRecipe", &last);
    let mut outs = slot_stacks(e, m.outputs);
    for r in task_results(game, c, t) {
        merge_item(game, &r, &mut outs);
    }
    for (i, s) in m.outputs.iter().enumerate() {
        e.items[*s] = outs[i].take();
    }
    set_task(e, None);
}

/// getExperienceForOutput through lastCompletedRecipe, paid as Util.giveExperience: whole points, and the fraction
/// as a chance.
fn award_xp(game: &mut Game<'_>, c: &Caches, pos: Pos, e: &mut ModBlockEntity) {
    let takers: Vec<u128> = STATE.with_borrow_mut(|s| {
        let mine: Vec<u128> = s.taken.iter().filter(|(_, p)| *p == pos).map(|(u, _)| *u).collect();
        s.taken.retain(|(_, p)| *p != pos);
        mine
    });
    let before = gi(&e.nbt, "hz_out", 0);
    let out = e.items.get(3).cloned().flatten();
    let now = out.as_ref().map_or(0, |s| s.count as i32);
    let Some(uuid) = takers.first().copied() else { return };
    let n = before - now;
    if n <= 0 {
        return;
    }
    let last = gs(&e.nbt, "lastCompletedRecipe").unwrap_or_default();
    let taken_item = out.as_ref().map(|s| s.item.to_string()).or_else(|| c.alloy_by_uid(&last).map(|r| r.output.item.to_string())).unwrap_or_default();
    let xp = if let Some(item) = last.strip_prefix("furnace:") {
        smelt_result(game, &ItemStack::new(item, 1)).map_or(0.0, |(_, x)| x.min(1.0) * n as f32)
    } else if c.alloy_by_uid(&last).is_some() {
        // AlloyRecipeManager.getExperienceForOutput: the first recipe with that output, not times the count.
        c.alloy.iter().find(|r| *r.output.item == *taken_item).map_or(0.0, |r| r.exp)
    } else {
        0.0
    };
    let whole = xp.floor();
    let mut points = whole as u32;
    if next_float(game) < xp - whole {
        points += 1;
    }
    if points > 0 {
        game.native_give_experience(uuid, points);
    }
}

// ---- The Stirling generator (TileStirlingGenerator.processTasks, AbstractGeneratorEntity, PowerDistributor)

fn stirling_tick(game: &mut Game<'_>, c: &Caches, pos: Pos, e: &mut ModBlockEntity, m: &Mach, passed: bool) {
    let max = max_energy(c, e, m);
    let generated = power_use(c, e, m);
    let mut burn = gi(&e.nbt, "burnTime", 0);
    if burn > 0 {
        let stored = energy(e);
        if stored < max {
            set_energy(e, stored + generated, max);
        }
        burn -= 1;
        si(&mut e.nbt, "burnTime", burn);
    }
    // transmitEnergy: up to twice the generation (no send key), round robin over the faces that may output.
    let stored = energy(e);
    if stored > 0 {
        let sent = distribute(game, pos, e, stored.min(2 * generated));
        set_energy(e, stored - sent, max);
    }
    if passed && burn <= 0 && energy(e) < max {
        if let Some(fuel) = e.items[0].clone() {
            let bt = stirling_burn_time(game, c, e, m, &fuel);
            if bt > 0 {
                si(&mut e.nbt, "burnTime", bt);
                si(&mut e.nbt, "totalBurnTime", bt);
                let lava = &*fuel.item == "minecraft:lava_bucket";
                if lava != (gi(&e.nbt, "isLavaFired", 0) != 0) {
                    si(&mut e.nbt, "isLavaFired", i32::from(lava));
                    game.native_block_entity_sync(pos);
                }
                let rest = remainder(game, &fuel.item);
                e.items[0] = if fuel.count > 1 { Some(ItemStack { count: fuel.count - 1, ..fuel.clone() }) } else { None };
                if let Some(r) = rest {
                    if e.items[0].is_none() {
                        e.items[0] = Some(r);
                    } else {
                        game.spawn_item(r, [f64::from(pos[0]) + 0.5, f64::from(pos[1]) + 0.5, f64::from(pos[2]) + 0.5], [0.0; 3], 10);
                    }
                }
            }
        }
    }
}

/// getBurnTime(item, maxUsage, data): vanilla burn time over the generation's step up, times the efficiency, a
/// fifth for fluid containers, over a fifteenth of the base generation; rounded.
pub(in crate::game) fn stirling_burn_time(game: &Game<'_>, c: &Caches, e: &ModBlockEntity, m: &Mach, fuel: &ItemStack) -> i32 {
    let data = cap_data(e, m);
    let gen_key = m.use_key();
    let default = c.default_float(&gen_key);
    if default <= 0.0 {
        return 0;
    }
    let rate = c.int(&gen_key, &data) as f32;
    let mut base = burn_time_of(game, fuel) as f32 / (rate / default) * burn_efficiency(c, &data);
    if game.native_item_fluid(fuel).is_some() || fuel.item.ends_with("_bucket") {
        base /= 5.0;
    }
    (base / (default / 15.0)).round() as i32
}

/// PowerDistributor.transmitEnergy: each receptor offered the whole rest in turn, from where it stopped last tick.
pub(in crate::game) fn distribute(game: &mut Game<'_>, pos: Pos, e: &mut ModBlockEntity, mut available: i32) -> i32 {
    let dim = game.native_dimension();
    let modes = face_modes(&e.nbt);
    let receptors: Vec<usize> = (0..6)
        .filter(|&f| can_output(modes[f]))
        .filter(|&f| game.native_energy(dim, step(pos, f), FACES[opposite(f)]).is_some_and(|n| n.can_insert))
        .collect();
    if receptors.is_empty() {
        return 0;
    }
    let mut at = gi(&e.nbt, "hz_rr", 0).max(0) as usize % receptors.len();
    let mut sent = 0;
    for _ in 0..receptors.len() {
        if available <= 0 {
            break;
        }
        let f = receptors[at];
        at = (at + 1) % receptors.len();
        let used = game.native_energy_insert(step(pos, f), FACES[opposite(f)], available, false).max(0);
        sent += used;
        available -= used;
    }
    si(&mut e.nbt, "hz_rr", at as i32);
    sent
}

// ---- Side IO (AbstractMachineEntity.doSideIo, AbstractInventoryMachineEntity.doPush/doPull, ItemTools.move)

fn side_io(game: &mut Game<'_>, c: &Caches, pos: Pos, e: &mut ModBlockEntity, m: &Mach) {
    let modes = face_modes(&e.nbt);
    for f in 0..6 {
        if pulls(modes[f]) && (pull(game, c, pos, e, m, f) || super::enderio_machines::fluid_pull(game, pos, e, m, f)) {
            return;
        }
        if pushes(modes[f]) && (push(game, pos, e, m, f) || super::enderio_machines::fluid_push(game, pos, e, m, f)) {
            return;
        }
    }
}

fn push_slots(game: &Game<'_>, e: &ModBlockEntity, m: &Mach) -> Vec<usize> {
    if m.kind == Kind::Stirling {
        // hasStuffToPush: a non-fuel item in the fuel slot (the bucket a lava bucket leaves).
        return e.items[0].as_ref().filter(|s| burn_time_of(game, s) <= 0).map(|_| vec![0]).unwrap_or_default();
    }
    m.outputs.to_vec()
}

fn push(game: &mut Game<'_>, pos: Pos, e: &mut ModBlockEntity, m: &Mach, f: usize) -> bool {
    let there = step(pos, f);
    for s in push_slots(game, e, m) {
        let Some(st) = e.items[s].clone() else { continue };
        let n = st.count.min(64);
        let left = game.native_insert_item(there, FACES[opposite(f)], ItemStack { count: n, ..st.clone() }).map_or(0, |l| l.count);
        let moved = n.saturating_sub(left);
        if moved > 0 {
            e.items[s] = if st.count > moved { Some(ItemStack { count: st.count - moved, ..st }) } else { None };
            return true;
        }
    }
    false
}

fn pull(game: &mut Game<'_>, c: &Caches, pos: Pos, e: &mut ModBlockEntity, m: &Mach, f: usize) -> bool {
    let dim = game.native_dimension();
    let inputs: Vec<usize> = m.inputs.iter().copied().filter(|s| !(m.kind == Kind::Sag && m.level == 0 && *s == 1)).collect();
    // hasSpaceToPull
    if !inputs.iter().any(|&s| e.items[s].as_ref().is_none_or(|st| st.count < max_stack(game, &st.item))) {
        return false;
    }
    let there = step(pos, f);
    let Some(slots) = game.native_container_slots(dim, there, FACES[opposite(f)]) else { return false };
    for slot in slots {
        let Some(stack) = slot.stack.filter(|_| slot.can_take) else { continue };
        // What our input slots take of it: merged into equal stacks, then an empty valid slot.
        let mut room = 0u32;
        for &s in &inputs {
            let most = |item: &str| super::enderio_machines::slot_limit(m, s).map_or(max_stack(game, item), |l| l.min(max_stack(game, item)));
            match &e.items[s] {
                Some(have) if have.same_item_same_components(&stack) => room += most(&have.item).saturating_sub(have.count),
                None if valid_for_slot(game, c, e, m, s, &stack) => room += most(&stack.item),
                _ => {}
            }
        }
        let want = stack.count.min(64).min(room);
        if want == 0 {
            continue;
        }
        let Some(got) = game.native_extract_item(dim, there, FACES[opposite(f)], slot.slot, want) else { continue };
        let mut left = got.count;
        for &s in &inputs {
            if left == 0 {
                break;
            }
            let most = super::enderio_machines::slot_limit(m, s).map_or(max_stack(game, &got.item), |l| l.min(max_stack(game, &got.item)));
            if let Some(have) = e.items[s].as_mut().filter(|h| h.same_item_same_components(&got)) {
                let n = most.saturating_sub(have.count).min(left);
                have.count += n;
                left -= n;
            }
        }
        for &s in &inputs {
            if left == 0 {
                break;
            }
            if e.items[s].is_none() && valid_for_slot(game, c, e, m, s, &got) {
                let n = super::enderio_machines::slot_limit(m, s).map_or(max_stack(game, &got.item), |l| l.min(max_stack(game, &got.item))).min(left);
                e.items[s] = Some(ItemStack { count: n, ..got.clone() });
                left -= n;
            }
        }
        if left > 0 {
            // Rejected leftovers go back where they came from, or into the world.
            if let Some(rest) = game.native_insert_item(there, FACES[opposite(f)], ItemStack { count: left, ..got.clone() }) {
                game.spawn_item(rest, [f64::from(pos[0]) + 0.5, f64::from(pos[1]) + 1.0, f64::from(pos[2]) + 0.5], [0.0; 3], 10);
            }
        }
        si(&mut e.nbt, "hz_failed", 0);
        return true;
    }
    false
}

/// container_accepts: a hopper or another mod puts `stack` into `slot` through `face`.
pub(in crate::game) fn container_accepts(game: &Game<'_>, entity: &ModBlockEntity, slot: usize, stack: &ItemStack, face: Option<Direction>, cancel: &mut bool) {
    let Some(m) = mach(&entity.id) else {
        if bank(&entity.id).is_some() {
            *cancel = true;
        }
        return;
    };
    let mode = face.map_or(IO_NONE, |f| face_modes(&entity.nbt)[f as usize]);
    let c = caches(game);
    if !can_receive(mode) || !m.inputs.contains(&slot) || (m.kind == Kind::Sag && m.level == 0 && slot == 1) || !valid_for_slot(game, &c, entity, m, slot, stack) {
        *cancel = true;
    }
    // A slot that holds one (the binder's inputs, the slice'n'splice, the charger) takes no more once filled.
    if let Some(l) = super::enderio_machines::slot_limit(m, slot) {
        if entity.items.get(slot).cloned().flatten().is_some_and(|s| s.count >= l) {
            *cancel = true;
        }
    }
}

/// Containers::can_take: only outputs, through a face that may output (the Stirling's fuel slot: not fuel).
pub(in crate::game) fn can_take(entity: &ModBlockEntity, slot: usize, stack: &ItemStack, face: Direction, fuel: bool) -> Option<bool> {
    let m = mach(&entity.id)?;
    let mode = face_modes(&entity.nbt)[face as usize];
    if !can_output(mode) {
        return Some(false);
    }
    let _ = stack;
    if m.kind == Kind::Enchanter {
        return Some(false);
    }
    Some(if m.kind == Kind::Stirling { slot == 0 && !fuel } else { m.outputs.contains(&slot) })
}

// ---- Energy through the sides (spec 5): the hooks on native_energy and native_energy_transfer

/// native_energy: (amount, capacity, max insert, max extract, can insert, can extract) through `face`; Some(None)
/// where the face reaches nothing (DISABLED); None for a block that isn't Ender IO's.
pub(in crate::game) fn energy_view(game: &Game<'_>, dimension: &str, pos: Pos, face: Direction) -> Option<Option<(i32, i32, i32, i32, bool, bool)>> {
    if dimension != game.native_dimension() {
        return None;
    }
    let Some(BlockEntity::Mod(e)) = game.world.block_entities.get(&(pos[0], pos[1], pos[2])) else { return None };
    if e.id.is_empty() || !e.id.starts_with("enderio:") {
        return None;
    }
    if e.id == super::enderio_conduits::BUNDLE {
        return Some(super::enderio_conduits::energy_view(game, pos, face));
    }
    let mode = face_modes(&e.nbt)[face as usize];
    if let Some(m) = mach(&e.id) {
        if !m.kind.powered() {
            return None;
        }
        if mode == IO_DISABLED {
            return Some(None);
        }
        let c = caches(game);
        let consumer = !m.kind.generator();
        let take = if consumer { intake(&c, e, m) } else { 0 };
        return Some(Some((energy(e), max_energy(&c, e, m), take, 0, consumer && take > 0, false)));
    }
    let t = bank(&e.id)?;
    if mode == IO_DISABLED {
        return Some(None);
    }
    let dim = game.native_dimension().to_owned();
    let (amount, cap, max_in) = BANKS.with_borrow(|b| {
        b.of.get(&(dim, pos)).and_then(|id| b.nets.get(id)).map_or((i64::from(energy(e)), i64::from(t.storage), 0), |n| (n.stored(), n.max, n.max_input()))
    });
    Some(Some((amount.min(i64::from(i32::MAX)) as i32, cap.min(i64::from(i32::MAX)) as i32, max_in, 0, mode != IO_PUSH && max_in > 0, false)))
}

/// native_energy_transfer: what goes in through `face` (nothing comes out: machines and banks only push).
pub(in crate::game) fn energy_transfer(game: &mut Game<'_>, dimension: &str, pos: Pos, face: Direction, amount: i32, insert: bool, simulate: bool) -> Option<i32> {
    if dimension != game.native_dimension() {
        return None;
    }
    let (id, mode) = match game.world.block_entities.get(&(pos[0], pos[1], pos[2])) {
        Some(BlockEntity::Mod(e)) if !e.id.is_empty() && e.id.starts_with("enderio:") => (e.id.clone(), face_modes(&e.nbt)[face as usize]),
        _ => return None,
    };
    if id == super::enderio_conduits::BUNDLE {
        return Some(super::enderio_conduits::energy_transfer(game, pos, face, amount, insert, simulate).unwrap_or(0));
    }
    if mach(&id).is_some_and(|m| !m.kind.powered()) {
        return None;
    }
    if !insert || amount <= 0 {
        return (mach(&id).is_some() || bank(&id).is_some()).then_some(0);
    }
    if let Some(m) = mach(&id) {
        if m.kind.generator() || mode == IO_DISABLED {
            return Some(0);
        }
        let c = caches(game);
        let f = face as usize;
        return game.native_with_mod_block_entity(pos, |e| {
            // AbstractPowerConsumerEntity.receiveEnergy with the SOFT overflow guard: per call the intake, and per
            // face per tick no more than the intake in total.
            let take = intake(&c, e, m);
            let max = max_energy(&c, e, m);
            let stored = energy(e);
            let mut rx: [i32; 6] = match e.nbt.get(&js("hz_rx_f")) {
                Some(Tag::IntArray(a)) if a.len() == 6 => std::array::from_fn(|i| a[i]),
                _ => [0; 6],
            };
            let n = amount.min(take).min(max - stored).min(take - rx[f]).max(0);
            if !simulate && n > 0 {
                set_energy(e, stored + n, max);
                rx[f] += n;
                e.nbt.insert(js("hz_rx_f"), Tag::IntArray(rx.to_vec()));
            }
            n
        });
    }
    bank(&id)?;
    if mode == IO_DISABLED || mode == IO_PUSH {
        return Some(0);
    }
    let dim = game.native_dimension().to_owned();
    Some(BANKS.with_borrow_mut(|b| {
        let Some(id) = b.of.get(&(dim, pos)).copied() else { return 0 };
        b.nets.get_mut(&id).map_or(0, |n| n.receive(amount, simulate))
    }))
}

// ---- The capacitor bank (powertools/machine/capbank)

pub(in crate::game) struct BankTier {
    pub id: &'static str,
    pub max_io: i32,
    pub storage: i32,
    pub creative: bool,
}

pub(in crate::game) static BANK_TIERS: [BankTier; 4] = [
    BankTier { id: "enderio:creative_capacitor_bank", max_io: 500_000, storage: 100_000_000, creative: true },
    BankTier { id: "enderio:basic_capacitor_bank", max_io: 1_000, storage: 1_000_000, creative: false },
    BankTier { id: "enderio:capacitor_bank", max_io: 5_000, storage: 5_000_000, creative: false },
    BankTier { id: "enderio:vibrant_capacitor_bank", max_io: 25_000, storage: 25_000_000, creative: false },
];

pub(in crate::game) fn bank(id: &str) -> Option<&'static BankTier> {
    BANK_TIERS.iter().find(|t| t.id == id)
}

/// PerTickIntAverageCalculator(2).
#[derive(Default, Clone)]
pub(in crate::game) struct Avg {
    last: f32,
    ticks: i32,
    cache: [f32; 2],
    write: usize,
    size: usize,
}

impl Avg {
    fn tick(&mut self, v: i64) {
        self.last += v as f32;
        self.ticks += 1;
        if self.ticks == 20 {
            self.cache[self.write] = self.last;
            self.write += 1;
            if self.write > self.size {
                self.size = self.write;
            }
            if self.write == self.cache.len() {
                self.write = 0;
            }
            self.last = 0.0;
            self.ticks = 0;
        }
    }
    pub(in crate::game) fn average(&self) -> f32 {
        let n = self.ticks + self.size as i32 * 20;
        if n == 0 {
            return 0.0;
        }
        let mut total = self.last;
        let (mut idx, mut cnt) = (self.write, self.size);
        while cnt > 0 {
            total += self.cache[idx];
            idx = (idx + 1) % self.cache.len();
            cnt -= 1;
        }
        total / n as f32
    }
}

pub(in crate::game) struct Net {
    dim: String,
    block: String,
    creative: bool,
    members: Vec<Pos>,
    energy: i64,
    pub max: i64,
    pub max_io: i32,
    pub max_input: i32,
    pub max_output: i32,
    pub in_mode: i32,
    pub out_mode: i32,
    powered: HashSet<Pos>,
    receptors: Option<Vec<(Pos, usize)>>,
    rr: usize,
    pub avg_in: Avg,
    pub avg_out: Avg,
    received: i64,
    sent: i64,
    prev: i64,
    shares: HashMap<Pos, i32>,
}

impl Net {
    pub(in crate::game) fn stored(&self) -> i64 {
        if self.creative { self.max / 2 } else { self.energy }
    }
    pub(in crate::game) fn max_input(&self) -> i32 {
        if self.max_input == -1 { self.max_io } else { self.max_input.min(self.max_io) }
    }
    pub(in crate::game) fn max_output(&self) -> i32 {
        if self.max_output == -1 { self.max_io } else { self.max_output.min(self.max_io) }
    }
    fn level(&self) -> i32 {
        if self.powered.is_empty() { 0 } else { 15 }
    }
    fn input_ok(&self) -> bool {
        condition_met(self.in_mode, self.level())
    }
    fn output_ok(&self) -> bool {
        condition_met(self.out_mode, self.level())
    }
    /// addEnergy.
    fn add(&mut self, e: i64) -> i64 {
        if e > 0 {
            self.received += e;
        } else {
            self.sent -= e;
        }
        if self.creative {
            self.energy = self.max / 2;
            return 0;
        }
        self.energy += e;
        if self.energy > self.max {
            let over = self.energy - self.max;
            self.energy = self.max;
            self.received -= over;
            return over;
        }
        if self.energy < 0 {
            self.energy = 0;
        }
        0
    }
    /// receiveEnergy: per call, the room (as an int) and the network's max input; nothing without the redstone.
    fn receive(&mut self, amount: i32, simulate: bool) -> i32 {
        if amount <= 0 || !self.input_ok() {
            return 0;
        }
        let space = (self.max - self.stored()).clamp(0, i64::from(i32::MAX)) as i32;
        let res = amount.min(space).min(self.max_input()).max(0);
        if !simulate {
            self.add(i64::from(res));
        }
        res
    }
}

#[derive(Default)]
struct Banks {
    of: HashMap<(String, Pos), u32>,
    nets: HashMap<u32, Net>,
    next: u32,
    tick: HashMap<String, u64>,
}

thread_local! {
    static BANKS: RefCell<Banks> = RefCell::new(Banks::default());
}

/// The network of the bank at `pos`, read only.
pub(in crate::game) fn with_net<R>(game: &Game<'_>, pos: Pos, f: impl FnOnce(&Net) -> R) -> Option<R> {
    let dim = game.native_dimension().to_owned();
    BANKS.with_borrow(|b| b.of.get(&(dim, pos)).and_then(|id| b.nets.get(id)).map(f))
}

fn bank_setting(n: &Compound, k: &str, d: i32) -> i32 {
    gi(n, k, d)
}

/// NetworkUtil.ensureValidNetwork: the banks of this tier joined to this one (face to face), one network.
fn build_network(game: &Game<'_>, pos: Pos, e: &ModBlockEntity, t: &BankTier) -> u32 {
    let dim = game.native_dimension().to_owned();
    let mut members = vec![pos];
    if !t.creative {
        let mut seen = HashSet::from([pos]);
        let mut queue = VecDeque::from([pos]);
        while let Some(at) = queue.pop_front() {
            for d in 0..6 {
                let n = step(at, d);
                if !seen.contains(&n) && game.native_block(n) == t.id {
                    seen.insert(n);
                    members.push(n);
                    queue.push_back(n);
                }
            }
        }
    }
    let mut total = i64::from(energy(e));
    let mut shares = HashMap::from([(pos, energy(e))]);
    for p in &members[1..] {
        if let Some(BlockEntity::Mod(o)) = game.world.block_entities.get(&(p[0], p[1], p[2])) {
            total += i64::from(energy(o));
            shares.insert(*p, energy(o));
        }
    }
    let n = members.len() as i64;
    let max = i64::from(t.storage) * n;
    let net = Net {
        dim: dim.clone(),
        block: t.id.to_owned(),
        creative: t.creative,
        energy: if t.creative { max / 2 } else { total.min(max) },
        max,
        max_io: (i64::from(t.max_io) * n).min(2_000_000_000) as i32,
        max_input: bank_setting(&e.nbt, "maxInput", -1),
        max_output: bank_setting(&e.nbt, "maxOutput", -1),
        in_mode: bank_setting(&e.nbt, "inputControlMode", 0),
        out_mode: bank_setting(&e.nbt, "outputControlMode", 0),
        powered: HashSet::new(),
        receptors: None,
        rr: 0,
        avg_in: Avg::default(),
        avg_out: Avg::default(),
        received: 0,
        sent: 0,
        prev: total,
        shares,
        members: members.clone(),
    };
    BANKS.with_borrow_mut(|b| {
        b.next = b.next.wrapping_add(1);
        let id = b.next;
        for p in &members {
            b.of.insert((dim.clone(), *p), id);
        }
        b.nets.insert(id, net);
        id
    })
}

/// destroyNetwork: the energy goes back into the banks (but `except`'s, which leaves with its own share).
fn destroy_network(game: &mut Game<'_>, id: u32, except: Option<Pos>) {
    let Some(net) = BANKS.with_borrow_mut(|b| {
        let net = b.nets.remove(&id)?;
        for p in &net.members {
            b.of.remove(&(net.dim.clone(), *p));
        }
        Some(net)
    }) else { return };
    if net.creative {
        return;
    }
    let keep: Vec<Pos> = net.members.iter().copied().filter(|p| Some(*p) != except).collect();
    let total = net.energy - except.and_then(|p| net.shares.get(&p)).map_or(0, |s| i64::from(*s));
    write_shares(game, &keep, total.max(0));
}

/// distributeEnergyToBanks: each bank the same share, the first `rest` one more.
fn write_shares(game: &mut Game<'_>, members: &[Pos], total: i64) -> HashMap<Pos, i32> {
    let mut out = HashMap::new();
    if members.is_empty() {
        return out;
    }
    let per = total / members.len() as i64;
    let mut rest = total % members.len() as i64;
    let dim = game.native_dimension().to_owned();
    for p in members {
        let share = (per + i64::from(rest > 0)) as i32;
        rest -= 1;
        let cmp = game.native_with_mod_block_entity(*p, |e| {
            si(&mut e.nbt, "energy", share);
            let cmp = comparator(e).unwrap_or(0);
            let changed = cmp != gi(&e.nbt, "hz_cmp", 0);
            si(&mut e.nbt, "hz_cmp", cmp);
            changed
        });
        if cmp == Some(true) {
            game.native_update_output_neighbors(&dim, *p);
        }
        out.insert(*p, share);
    }
    out
}

fn bank_tick(game: &mut Game<'_>, pos: Pos, e: &mut ModBlockEntity, t: &'static BankTier) {
    let dim = game.native_dimension().to_owned();
    let known = BANKS.with_borrow(|b| b.of.get(&(dim.clone(), pos)).copied());
    let id = match known {
        Some(id) => id,
        None => build_network(game, pos, e, t),
    };
    let dirty = take_dirty(pos) || !e.nbt.contains_key(&js("hz_rs_seen"));
    let signal = if dirty { Some(game.native_neighbor_signal(&dim, pos)) } else { None };
    if dirty {
        si(&mut e.nbt, "hz_rs_seen", 1);
        validate_displays(game, pos, e, t);
    }
    if t.creative {
        si(&mut e.nbt, "energy", t.storage / 2);
    }
    BANKS.with_borrow_mut(|b| {
        let Some(net) = b.nets.get_mut(&id) else { return };
        if let Some(s) = signal {
            if s > 0 {
                net.powered.insert(pos);
            } else {
                net.powered.remove(&pos);
            }
        }
        if dirty {
            net.receptors = None;
        }
        // The network's settings are every member's (@Store maxInput, maxOutput, the modes).
        si(&mut e.nbt, "maxInput", net.max_input);
        si(&mut e.nbt, "maxOutput", net.max_output);
        si(&mut e.nbt, "inputControlMode", net.in_mode);
        si(&mut e.nbt, "outputControlMode", net.out_mode);
    });
    // getComparatorInputOverride: this block's own energy.
    let stored = energy(e);
    let cmp = if stored == 0 { 0 } else { (1.0 + stored as f64 / f64::from(t.storage) * 14.0) as i32 };
    if cmp != gi(&e.nbt, "hz_cmp", 0) {
        si(&mut e.nbt, "hz_cmp", cmp);
        game.native_update_output_neighbors(&dim, pos);
    }
    fill_bank_data(game, pos, e);
}

/// The receptors of a network: each member's faces set to PUSH or NONE, by a block that takes energy there and isn't
/// a bank of the same tier (getReceptorForFace).
fn receptors(game: &Game<'_>, members: &[Pos], block: &str) -> Vec<(Pos, usize)> {
    let dim = game.native_dimension();
    let mut out = Vec::new();
    for p in members {
        let modes = match game.world.block_entities.get(&(p[0], p[1], p[2])) {
            Some(BlockEntity::Mod(e)) => face_modes(&e.nbt),
            _ => continue,
        };
        for f in 0..6 {
            if !(modes[f] == IO_PUSH || modes[f] == IO_NONE) {
                continue;
            }
            let there = step(*p, f);
            if game.native_block(there) == block {
                continue;
            }
            // updateReceptors: any energy interface on that face, whether it takes energy now or not (a machine
            // without its capacitor yet, a bank whose network is made on its first tick); the transfer decides.
            if game.native_energy(dim, there, FACES[opposite(f)]).is_some() {
                out.push((*p, f));
            }
        }
    }
    out
}

/// level_tick_post: the networks' tickEnd (transmit, distribute, averages), then the queued block changes.
pub(in crate::game) fn level_tick(game: &mut Game<'_>, dimension: &str) {
    if !game.native_on(NS) {
        return;
    }
    let dim = dimension.to_owned();
    // What left the world last tick was dropped (or lost) by now.
    STATE.with_borrow_mut(|s| s.removed.clear());
    // The conduit networks' tickEnd.
    super::enderio_conduits::level_tick(game, dimension);
    let ids: Vec<u32> = BANKS.with_borrow(|b| b.nets.iter().filter(|(_, n)| n.dim == dim).map(|(id, _)| *id).collect());
    for id in ids {
        // transmitEnergy
        let job = BANKS.with_borrow(|b| {
            let n = b.nets.get(&id)?;
            Some((n.members.clone(), n.block.clone(), n.receptors.is_none(), n.output_ok(), n.stored().min(i64::from(n.max_output())) as i32))
        });
        let Some((members, block, stale, out_ok, available)) = job else { continue };
        if stale {
            let r = receptors(game, &members, &block);
            BANKS.with_borrow_mut(|b| {
                if let Some(n) = b.nets.get_mut(&id) {
                    n.receptors = Some(r);
                    n.rr = 0;
                }
            });
        }
        if out_ok && available > 0 {
            let (list, start) = BANKS.with_borrow(|b| b.nets.get(&id).map(|n| (n.receptors.clone().unwrap_or_default(), n.rr)).unwrap_or_default());
            let mut left = available;
            let mut sent = 0i64;
            let mut at = if list.is_empty() { 0 } else { start % list.len() };
            for _ in 0..list.len() {
                if left <= 0 {
                    break;
                }
                let (p, f) = list[at];
                at = (at + 1) % list.len();
                // sendPowerTo: a NONE face leaves an IN_OUT energy conduit to the conduit network.
                let none = matches!(game.world.block_entities.get(&(p[0], p[1], p[2])), Some(BlockEntity::Mod(e)) if face_modes(&e.nbt)[f] == IO_NONE);
                if none && super::enderio_conduits::energy_in_out_toward(game, step(p, f), opposite(f)) {
                    continue;
                }
                let used = game.native_energy_insert(step(p, f), FACES[opposite(f)], left, false).max(0);
                left -= used;
                sent += i64::from(used);
            }
            BANKS.with_borrow_mut(|b| {
                if let Some(n) = b.nets.get_mut(&id) {
                    n.add(-sent);
                    n.rr = at;
                }
            });
        }
        // distributeEnergyToBanks, the averages; a member whose chunk unloaded or whose block is no longer this
        // tier's ends the network (a break ends it at once, `destroyed`). A member's block entity being out of the
        // world for a moment, or not having ticked yet, doesn't.
        let gone: Vec<Pos> = members
            .iter()
            .copied()
            .filter(|p| !game.world.chunks.contains_key(&(p[0].div_euclid(16), p[2].div_euclid(16))) || game.native_block(*p) != block)
            .collect();
        let tick_no = BANKS.with_borrow(|b| b.tick.get(&dim).copied().unwrap_or(0));
        let plan = BANKS.with_borrow_mut(|b| {
            let n = b.nets.get_mut(&id)?;
            n.avg_in.tick(n.received);
            n.avg_out.tick(n.sent);
            n.received = 0;
            n.sent = 0;
            let missing: Vec<Pos> = n.members.iter().copied().filter(|p| gone.contains(p)).collect();
            let changed = n.energy != n.prev;
            n.prev = n.energy;
            let loaded: Vec<Pos> = n.members.iter().copied().filter(|p| !missing.contains(p)).collect();
            let total = n.energy - missing.iter().filter_map(|p| n.shares.get(p)).map(|s| i64::from(*s)).sum::<i64>();
            // The members' NBT follows the network when it changes, and once a second in any case.
            Some(((changed || tick_no % 20 == 0) && !n.creative, loaded, total.max(0), !missing.is_empty()))
        });
        let Some((changed, loaded, total, broken)) = plan else { continue };
        if changed || broken {
            let shares = write_shares(game, &loaded, total);
            BANKS.with_borrow_mut(|b| {
                if let Some(n) = b.nets.get_mut(&id) {
                    n.shares.extend(shares);
                }
            });
        }
        if broken {
            BANKS.with_borrow_mut(|b| {
                if let Some(n) = b.nets.remove(&id) {
                    for p in &n.members {
                        b.of.remove(&(n.dim.clone(), *p));
                    }
                }
            });
        }
    }
    BANKS.with_borrow_mut(|b| {
        *b.tick.entry(dim.clone()).or_insert(0) += 1;
    });
    // The queued block changes of this level.
    let (removals, placements): (Vec<(Pos, String, Option<ItemStack>)>, Vec<(Pos, String)>) = STATE.with_borrow_mut(|s| {
        let (now, later): (Vec<_>, Vec<_>) = s.removals.drain(..).partition(|(d, ..)| *d == dim);
        s.removals = later;
        let (pn, pl): (Vec<_>, Vec<_>) = s.placements.drain(..).partition(|(d, ..)| *d == dim);
        s.placements = pl;
        (now.into_iter().map(|(_, p, b, d)| (p, b, d)).collect(), pn.into_iter().map(|(_, p, b)| (p, b)).collect())
    });
    if let Some(air) = game.native_default_state("minecraft:air") {
        for (p, b, drop) in removals {
            if game.native_block(p) == b {
                game.native_set_block(&dim, p, air, 3);
                if let Some(d) = drop {
                    game.spawn_item(d, [f64::from(p[0]) + 0.5, f64::from(p[1]) + 0.5, f64::from(p[2]) + 0.5], [0.0; 3], 10);
                }
            }
        }
    }
    for (p, b) in placements {
        if let Some(state) = game.native_default_state(&b) {
            if game.native_block(p) == "minecraft:air" {
                game.native_set_block(&dim, p, state, 3);
            }
        }
    }
}

/// The bank's display faces (InfoDisplayType NONE, LEVEL_BAR, IO), horizontal faces only.
pub(in crate::game) fn displays(n: &Compound) -> [i32; 6] {
    match n.get(&js("faceDisplayTypes")) {
        Some(Tag::IntArray(a)) if a.len() == 6 => std::array::from_fn(|i| a[i].clamp(0, 2)),
        _ => [0; 6],
    }
}

fn set_displays(n: &mut Compound, d: [i32; 6]) {
    n.insert(js("faceDisplayTypes"), Tag::IntArray(d.to_vec()));
}

/// validateDisplayTypes: a face against a bank (or a solid block, by its collision) loses its display.
fn validate_displays(game: &Game<'_>, pos: Pos, e: &mut ModBlockEntity, t: &BankTier) {
    let mut d = displays(&e.nbt);
    let mut modes = face_modes(&e.nbt);
    let mut changed = false;
    for f in 2..6 {
        if d[f] == 0 {
            continue;
        }
        let there = step(pos, f);
        let full = game.native_block_collision(there).iter().any(|b| b[0] <= 0.0 && b[1] <= 0.0 && b[2] <= 0.0 && b[3] >= 1.0 && b[4] >= 1.0 && b[5] >= 1.0);
        if bank(&game.native_block(there)).is_some() || full {
            d[f] = 0;
            modes[f] = default_bank_mode(game, pos, f, t);
            changed = true;
        }
    }
    if changed {
        set_displays(&mut e.nbt, d);
        set_face_modes(&mut e.nbt, modes);
    }
}

/// A non-bank-of-this-tier neighbour that has energy on that face (getReceptorForFace).
fn bank_receptor(game: &Game<'_>, pos: Pos, f: usize, t: &BankTier) -> Option<bool> {
    let there = step(pos, f);
    if game.native_block(there) == t.id {
        return None;
    }
    game.native_energy(game.native_dimension(), there, FACES[opposite(f)]).map(|n| n.can_insert)
}

/// Whether the neighbour on that face is a conduit bundle (its receptor's provider is an IConduitBundle).
fn conduit_there(game: &Game<'_>, pos: Pos, f: usize) -> bool {
    game.native_block(step(pos, f)) == super::enderio_conduits::BUNDLE
}

/// setDefaultIoMode: nothing there or a conduit → NONE; something that takes energy → PUSH; else PULL.
fn default_bank_mode(game: &Game<'_>, pos: Pos, f: usize, t: &BankTier) -> i32 {
    if conduit_there(game, pos, f) {
        return IO_NONE;
    }
    match bank_receptor(game, pos, f, t) {
        None => IO_NONE,
        Some(true) => IO_PUSH,
        Some(false) => IO_PULL,
    }
}

/// toggleIoModeForFace of a bank: PULL, PUSH, DISABLED, then PULL again (a receptor that is no conduit there), or
/// NONE.
fn bank_next_mode(game: &Game<'_>, pos: Pos, f: usize, t: &BankTier, cur: i32) -> i32 {
    match cur {
        IO_PULL => IO_PUSH,
        IO_PUSH => IO_DISABLED,
        IO_DISABLED => if bank_receptor(game, pos, f, t).is_some() && !conduit_there(game, pos, f) { IO_PULL } else { IO_NONE },
        _ => IO_PULL,
    }
}

/// A bank as an energy conduit's network sees it (an IPowerStorage): its network, the face towards the conduit.
pub(in crate::game) struct BankView {
    pub net: u32,
    pub stored: i64,
    pub max: i64,
    pub max_in: i32,
    pub max_out: i32,
    pub creative: bool,
    /// isNetworkControlledIo: the face is NONE or PULL.
    pub network_io: bool,
    /// isInputEnabled / isOutputEnabled of that face, with the network's redstone control.
    pub input_ok: bool,
    pub output_ok: bool,
}

pub(in crate::game) fn bank_view(game: &Game<'_>, pos: Pos, face: usize) -> Option<BankView> {
    let Some(BlockEntity::Mod(e)) = game.world.block_entities.get(&(pos[0], pos[1], pos[2])) else { return None };
    bank(&e.id)?;
    let mode = face_modes(&e.nbt)[face];
    let dim = game.native_dimension().to_owned();
    BANKS.with_borrow(|b| {
        let id = *b.of.get(&(dim, pos))?;
        let n = b.nets.get(&id)?;
        Some(BankView {
            net: id,
            stored: n.stored(),
            max: n.max,
            max_in: n.max_input(),
            max_out: n.max_output(),
            creative: n.creative,
            network_io: mode == IO_NONE || mode == IO_PULL,
            input_ok: (mode == IO_PULL || mode == IO_NONE) && n.input_ok(),
            output_ok: (mode == IO_PUSH || mode == IO_NONE) && n.output_ok(),
        })
    })
}

/// IPowerStorage.addEnergy of the bank at `pos` (its network's): what didn't fit.
pub(in crate::game) fn bank_add(game: &Game<'_>, pos: Pos, amount: i64) -> i64 {
    let dim = game.native_dimension().to_owned();
    BANKS.with_borrow_mut(|b| {
        let Some(id) = b.of.get(&(dim, pos)).copied() else { return amount };
        b.nets.get_mut(&id).map_or(amount, |n| n.add(amount))
    })
}

/// A block beside a changed conduit face looks at its neighbours again on its next tick (a bank: its receptors).
pub(in crate::game) fn neighbour_dirty(pos: Pos) {
    STATE.with_borrow_mut(|s| {
        s.dirty.insert(pos);
    });
}

/// toggleIoModeForFace of a machine: next() until supportsMode (an enhanced machine's top: NONE only).
fn machine_next_mode(m: &Mach, f: usize, cur: i32) -> i32 {
    // supportsMode: an enhanced machine's top and the enchanter NONE only; the combustion and zombie generators
    // never PUSH or PUSH_PULL.
    if (m.enhanced && f == 1) || m.kind == Kind::Enchanter {
        return IO_NONE;
    }
    let mut next = (cur + 1) % 5;
    while matches!(m.kind, Kind::Combustion | Kind::Zombie) && (next == IO_PUSH || next == IO_PUSH_PULL) {
        next = (next + 1) % 5;
    }
    next
}

/// Toggles one face, server side, of the block entity `e` at `pos`.
fn toggle_face(game: &Game<'_>, pos: Pos, e: &mut ModBlockEntity, f: usize) {
    let mut modes = face_modes(&e.nbt);
    if let Some(m) = mach(&e.id) {
        modes[f] = machine_next_mode(m, f, modes[f]);
    } else if let Some(t) = bank(&e.id) {
        modes[f] = bank_next_mode(game, pos, f, t, modes[f]);
    }
    set_face_modes(&mut e.nbt, modes);
    // setIoMode notifies the neighbours: a bank beside it looks at its receptors again.
    STATE.with_borrow_mut(|s| {
        s.dirty.insert(pos);
        for d in 0..6 {
            s.dirty.insert(step(pos, d));
        }
    });
}

fn forget_receptors(game: &Game<'_>, pos: Pos) {
    let dim = game.native_dimension().to_owned();
    BANKS.with_borrow_mut(|b| {
        if let Some(id) = b.of.get(&(dim, pos)).copied() {
            if let Some(n) = b.nets.get_mut(&id) {
                n.receptors = None;
            }
        }
    });
}

// ---- Menu data slots (16 bits each; ints go in two, longs in four)

pub(in crate::game) const D_ENERGY: usize = 0; // 2
pub(in crate::game) const D_MAX: usize = 2; // 2
pub(in crate::game) const D_USE: usize = 4; // 2: getMaxUsage
pub(in crate::game) const D_LOSS: usize = 6; // 2: loss per tick x 10000
pub(in crate::game) const D_CAP: usize = 8; // a capacitor (or no slot)
pub(in crate::game) const D_PROGRESS: usize = 9; // progress x 10000 + 1, 0 without a task
pub(in crate::game) const D_FACES: usize = 10; // 6
pub(in crate::game) const D_REDSTONE: usize = 16;
pub(in crate::game) const D_ACTIVE: usize = 17;
pub(in crate::game) const D_MODE: usize = 18;
pub(in crate::game) const D_BALL: usize = 19; // ballDurationScaled(1000) + 1, 0 without a ball
pub(in crate::game) const D_BALL_STATS: usize = 20; // 3: grinding, chance, power x 100
pub(in crate::game) const D_BURN: usize = 23; // 2
pub(in crate::game) const D_BURN_TOTAL: usize = 25; // 2
pub(in crate::game) const D_EFFICIENCY: usize = 27; // burn efficiency x 1000
pub(in crate::game) const D_CAP_EFF: usize = 28; // 2: the activated and octadic capacitors' efficiency x 1000
pub(in crate::game) const D_NET_ENERGY: usize = 30; // 4
pub(in crate::game) const D_NET_MAX: usize = 34; // 4
pub(in crate::game) const D_MAX_IO: usize = 38; // 2
pub(in crate::game) const D_MAX_IN: usize = 40; // 2
pub(in crate::game) const D_MAX_OUT: usize = 42; // 2
pub(in crate::game) const D_IN_MODE: usize = 44;
pub(in crate::game) const D_OUT_MODE: usize = 45;
pub(in crate::game) const D_AVG: usize = 46; // 2: round(avg change), an int
pub(in crate::game) const D_MEMBERS: usize = 48;
pub(in crate::game) const D_SETTINGS_SEEN: usize = 49; // counts the settings changes (the screen refills its fields)
// Phase 2 (enderio_machines.rs): two tanks (fluid number, amount, capacity) and four of the machine's own.
pub(in crate::game) const D_TANK0: usize = 50; // 5: fluid + 1, amount (2), capacity (2)
pub(in crate::game) const D_TANK1: usize = 55; // 5
pub(in crate::game) const D_X: usize = 60; // 4
pub(in crate::game) const DATA_SLOTS: usize = 64;

pub(in crate::game) fn put2(d: &mut [i32], at: usize, v: i32) {
    d[at] = v & 0xFFFF;
    d[at + 1] = (v >> 16) & 0xFFFF;
}

fn put4(d: &mut [i32], at: usize, v: i64) {
    for i in 0..4 {
        d[at + i] = ((v >> (16 * i)) & 0xFFFF) as i32;
    }
}

pub(in crate::game) fn get2(d: &[i32], at: usize) -> i32 {
    (d[at] & 0xFFFF) | ((d[at + 1] & 0xFFFF) << 16)
}

pub(in crate::game) fn get4(d: &[i32], at: usize) -> i64 {
    (0..4).map(|i| i64::from(d[at + i] & 0xFFFF) << (16 * i)).fold(0, |a, b| a | b)
}

pub(in crate::game) fn fill_machine_data(c: &Caches, e: &mut ModBlockEntity, m: &Mach) {
    let mut d = vec![0; DATA_SLOTS];
    put2(&mut d, D_ENERGY, energy(e));
    put2(&mut d, D_MAX, max_energy(c, e, m));
    put2(&mut d, D_USE, power_use(c, e, m));
    put2(&mut d, D_LOSS, (power_loss(c, e, m) * 10000.0).round() as i32);
    d[D_CAP] = i32::from(!raw_cap(e, m).none);
    d[D_PROGRESS] = match m.kind {
        Kind::Stirling => {
            let total = gi(&e.nbt, "totalBurnTime", 0);
            let burn = gi(&e.nbt, "burnTime", 0);
            if burn > 0 && total > 0 { (burn as f32 / total as f32 * 10000.0) as i32 + 1 } else { 0 }
        }
        _ => task(e).map_or(0, |t| (t.progress() * 10000.0) as i32 + 1),
    };
    let modes = face_modes(&e.nbt);
    d[D_FACES..D_FACES + 6].copy_from_slice(&modes);
    d[D_REDSTONE] = gi(&e.nbt, "redstoneControlMode", 0);
    d[D_ACTIVE] = i32::from(is_active(e, m));
    d[D_MODE] = if m.kind == Kind::Alloy { alloy_mode(e, m) } else { 0 };
    if m.kind == Kind::Sag {
        let max = gi(&e.nbt, "grindingBallDurabilityMax", 0);
        if let Some((g, ch, p)) = ball_loaded(e).filter(|_| max > 0) {
            let used = gi(&e.nbt, "grindingBallDurabilityUsed", 0);
            d[D_BALL] = ((1000.0 * (1.0 - used as f32 / max as f32)) as i32).clamp(0, 1000) + 1;
            d[D_BALL_STATS] = (g * 100.0).round() as i32;
            d[D_BALL_STATS + 1] = (ch * 100.0).round() as i32;
            d[D_BALL_STATS + 2] = (p * 100.0).round() as i32;
        }
    }
    if m.kind == Kind::Stirling {
        put2(&mut d, D_BURN, gi(&e.nbt, "burnTime", 0));
        put2(&mut d, D_BURN_TOTAL, gi(&e.nbt, "totalBurnTime", 0));
        d[D_EFFICIENCY] = (burn_efficiency(c, &cap_data(e, m)) * 1000.0).round() as i32;
        d[D_CAP_EFF] = (burn_efficiency(c, &Cap { none: false, level: 2.0, keys: Vec::new() }) * 1000.0).round() as i32;
        d[D_CAP_EFF + 1] = (burn_efficiency(c, &Cap { none: false, level: 3.0, keys: Vec::new() }) * 1000.0).round() as i32;
    }
    super::enderio_machines::fill_data(c, e, m, &mut d);
    if e.data != d {
        e.data = d;
    }
}

fn fill_bank_data(game: &Game<'_>, pos: Pos, e: &mut ModBlockEntity) {
    let mut d = vec![0; DATA_SLOTS];
    let modes = face_modes(&e.nbt);
    d[D_FACES..D_FACES + 6].copy_from_slice(&modes);
    d[D_CAP] = 1;
    d[D_SETTINGS_SEEN] = gi(&e.nbt, "hz_settings", 0) & 0xFFFF;
    with_net(game, pos, |n| {
        put4(&mut d, D_NET_ENERGY, n.stored());
        put4(&mut d, D_NET_MAX, n.max);
        put2(&mut d, D_MAX_IO, n.max_io);
        put2(&mut d, D_MAX_IN, n.max_input());
        put2(&mut d, D_MAX_OUT, n.max_output());
        d[D_IN_MODE] = n.in_mode;
        d[D_OUT_MODE] = n.out_mode;
        put2(&mut d, D_AVG, (n.avg_in.average() - n.avg_out.average()).round() as i32);
        d[D_MEMBERS] = n.members.len().min(65535) as i32;
    });
    if e.data != d {
        e.data = d;
    }
}

// ---- Menu buttons, checked and applied on the server

pub(in crate::game) const B_REDSTONE: i32 = 120; // + mode
pub(in crate::game) const B_FACE: i32 = 200; // + face
pub(in crate::game) const B_RESET_IO: i32 = 210;
pub(in crate::game) const B_MODE_NEXT: i32 = 300;
pub(in crate::game) const B_MODE_PREV: i32 = 301;
pub(in crate::game) const B_BANK_IN_RS: i32 = 100; // + mode
pub(in crate::game) const B_BANK_OUT_RS: i32 = 110; // + mode
pub(in crate::game) const B_LOW: i32 = 0x1_0000; // + the value's low 16 bits
pub(in crate::game) const B_SET_IN: i32 = 0x2_0000; // + the high 16 bits: commit max input
pub(in crate::game) const B_SET_OUT: i32 = 0x3_0000; // + the high 16 bits: commit max output

pub(in crate::game) fn menu_button(game: &mut Game<'_>, pos: Pos, e: &mut ModBlockEntity, id: i32, carried: &mut Option<ItemStack>) {
    if e.id == super::enderio_conduits::BUNDLE {
        super::enderio_conduits::menu_button(game, pos, e, id, carried);
        return;
    }
    if let Some(m) = mach(&e.id) {
        match id {
            // RedstoneModeButton (the simple machines have none: OperatingProfile.hasRedstoneControl).
            B_REDSTONE..=123 if !matches!(m.profile, Profile::SimpleAlloy | Profile::SimpleFurnace) && !(m.level == 0 && m.kind != Kind::Alloy) && m.kind != Kind::Enchanter => {
                si(&mut e.nbt, "redstoneControlMode", id - B_REDSTONE);
                si(&mut e.nbt, "hz_rs", 1);
            }
            200..=205 => toggle_face(game, pos, e, (id - B_FACE) as usize),
            B_RESET_IO => set_face_modes(&mut e.nbt, [IO_NONE; 6]),
            B_MODE_NEXT | B_MODE_PREV if m.profile == Profile::Auto => {
                let cur = gi(&e.nbt, "mode", 0).clamp(0, 2);
                let next = if id == B_MODE_NEXT { (cur + 1) % 3 } else { (cur + 2) % 3 };
                si(&mut e.nbt, "mode", next);
                game.native_block_entity_sync(pos);
            }
            _ if super::enderio_machines::menu_button(game, pos, e, m, id) => {}
            _ => return,
        }
        let c = caches(game);
        fill_machine_data(&c, e, m);
        return;
    }
    let Some(t) = bank(&e.id) else { return };
    let dim = game.native_dimension().to_owned();
    let Some(net_id) = BANKS.with_borrow(|b| b.of.get(&(dim.clone(), pos)).copied()) else { return };
    match id {
        200..=205 => {
            toggle_face(game, pos, e, (id - B_FACE) as usize);
            forget_receptors(game, pos);
        }
        B_RESET_IO => {
            // clearAllIoModes: every member to its default.
            let members = BANKS.with_borrow(|b| b.nets.get(&net_id).map(|n| n.members.clone()).unwrap_or_default());
            for p in members {
                let defaults: [i32; 6] = std::array::from_fn(|f| default_bank_mode(game, p, f, t));
                if p == pos {
                    set_face_modes(&mut e.nbt, defaults);
                } else {
                    game.native_with_mod_block_entity(p, |o| set_face_modes(&mut o.nbt, defaults));
                }
            }
            forget_receptors(game, pos);
        }
        100..=103 | 110..=113 => {
            BANKS.with_borrow_mut(|b| {
                if let Some(n) = b.nets.get_mut(&net_id) {
                    if id < B_BANK_OUT_RS { n.in_mode = id - B_BANK_IN_RS } else { n.out_mode = id - B_BANK_OUT_RS }
                }
            });
        }
        0x1_0000..=0x1_FFFF => si(&mut e.nbt, "hz_low", id & 0xFFFF),
        0x2_0000..=0x3_FFFF => {
            let v = ((id & 0xFFFF) << 16) | (gi(&e.nbt, "hz_low", 0) & 0xFFFF);
            BANKS.with_borrow_mut(|b| {
                if let Some(n) = b.nets.get_mut(&net_id) {
                    // setMaxInput / setMaxOutput: at or over the max I/O, unlimited (-1); under 0, 0.
                    let v = if v >= n.max_io { -1 } else { v.max(0) };
                    if id < B_SET_OUT { n.max_input = v } else { n.max_output = v }
                }
            });
            let k = gi(&e.nbt, "hz_settings", 0) + 1;
            si(&mut e.nbt, "hz_settings", k);
        }
        _ => return,
    }
    fill_bank_data(game, pos, e);
}

// ---- Placing, breaking, clicking

/// getStateForPlacement: the front towards the placer (Util.getFacingFromEntity); an enhanced machine needs the
/// block above free.
pub(in crate::game) fn state_for_placement(level: &super::GameLevel<'_>, state: horizon_world::registry::BlockStateId, context: &horizon_world::level::PlacementContext, answer: &mut Option<Option<horizon_world::registry::BlockStateId>>) {
    let Some(block) = level.native_state_block(state).map(str::to_owned) else { return };
    let Some(m) = mach(&block) else { return };
    if m.enhanced {
        let above = level.world.block(context.pos.x, context.pos.y + 1, context.pos.z);
        let name = level.facts.get(above).name.to_string();
        if !matches!(name.as_str(), "minecraft:air" | "minecraft:cave_air" | "minecraft:void_air") {
            *answer = Some(None);
            return;
        }
    }
    let y = context.yaw.rem_euclid(360.0);
    let facing = if !(45.0..315.0).contains(&y) {
        "north"
    } else if y < 135.0 {
        "east"
    } else if y < 225.0 {
        "south"
    } else {
        "west"
    };
    if let Some(s) = level.native_state_with_property(state, "facing", facing) {
        *answer = Some(Some(s));
    }
}

/// What the dropped machine item keeps (writeCustomNBT): its config, inventory, task and energy, under `data`.
fn machine_item_data(c: &Caches, e: &ModBlockEntity, m: &Mach) -> Compound {
    let mut data = Compound::new();
    for k in ["redstoneControlMode", "faceModes", "currentTask", "lastCompletedRecipe", "mode", "grindingBall", "grindingBallDurabilityUsed", "grindingBallDurabilityMax", "burnTime", "totalBurnTime", "isLavaFired", "partialEnergyLoss", "fuelTank", "coolantTank", "ticksRemaingFuel", "ticksRemaingCoolant", "tank", "inputTank", "outputTank", "xpCont", "voidMode"] {
        if let Some(v) = e.nbt.get(&js(k)) {
            data.insert(js(k), v.clone());
        }
    }
    data.insert(js("inventory"), horizon_items::item_nbt::encode_items(&e.items));
    let mut root = Compound::new();
    root.insert(js("data"), Tag::Compound(data));
    if m.kind.powered() {
        si(&mut root, "Energy", energy(e));
        si(&mut root, "MaxEnergy", max_energy(c, e, m));
    }
    super::enderio_machines::item_data(e, m, &mut root);
    root
}

pub(in crate::game) fn machine_drop(c: &Caches, e: &ModBlockEntity, m: &Mach) -> ItemStack {
    let mut st = ItemStack::new(m.id, 1);
    set_custom_data(&mut st, machine_item_data(c, e, m));
    st
}

fn bank_drop(e: &ModBlockEntity, t: &BankTier) -> ItemStack {
    let mut st = ItemStack::new(t.id, 1);
    if !t.creative {
        let mut c = Compound::new();
        si(&mut c, "horizon_energy", energy(e));
        set_custom_data(&mut st, c);
    }
    st
}

pub(in crate::game) fn block_drops(game: &mut Game<'_>, pos: Pos, block: &str, explosion: Option<f32>, drops: &mut Vec<ItemStack>) {
    if block == super::enderio_conduits::BUNDLE {
        super::enderio_conduits::block_drops(game, pos, drops);
        return;
    }
    if top_of(block).is_none() && bottom_of(block).is_some() {
        drops.clear();
        return;
    }
    // The block entity still in the world (an explosion asks first), or as it left it (a player's break asks after).
    let e = match game.world.block_entities.get(&(pos[0], pos[1], pos[2])) {
        Some(BlockEntity::Mod(e)) if e.id == block => ModBlockEntity::clone(e),
        _ => match STATE.with_borrow_mut(|s| s.removed.remove(&pos)) {
            Some(e) if e.id == block => e,
            _ => return,
        },
    };
    let made = if let Some(m) = mach(block) {
        let c = caches(game);
        machine_drop(&c, &e, m)
    } else if let Some(t) = bank(block) {
        bank_drop(&e, t)
    } else {
        return;
    };
    // An explosion (Explosion.doExplosionB): the block drops with chance 1 / size, its contents with it, or not at all.
    if let Some(size) = explosion.filter(|s| *s > 0.0) {
        if next_float(game) > 1.0 / size {
            drops.retain(|d| &*d.item != block);
            return;
        }
    }
    let mut replaced = false;
    for d in drops.iter_mut().filter(|d| &*d.item == block) {
        if !replaced {
            *d = made.clone();
            replaced = true;
        }
    }
    if !replaced {
        drops.push(made);
    }
}

/// `remove_block_entity`: a machine or bank leaving the world spills nothing (Ender IO's blocks have no breakBlock
/// that drops contents: the inventory rides on the dropped item, and a /setblock or creative break loses it, as in
/// 1.12). It is kept as it was for the drop asked after the removal.
pub(in crate::game) fn block_entity_removed(level: &mut super::GameLevel<'_>, pos: Pos) {
    super::enderio_conduits::block_entity_removed(level, pos);
    if let Some(BlockEntity::Mod(m)) = level.world.block_entities.get_mut(&(pos[0], pos[1], pos[2])) {
        if mach(&m.id).is_some() || bank(&m.id).is_some() {
            let copy = ModBlockEntity::clone(m);
            for s in m.items.iter_mut() {
                *s = None;
            }
            STATE.with_borrow_mut(|s| s.removed.insert(pos, copy));
        }
    }
}

/// readCustomNBT, the placer's facing, an enhanced machine's top; a bank's energy and its display face.
pub(in crate::game) fn placed(game: &mut Game<'_>, stack: &ItemStack, pos: Pos) {
    let block = game.native_block(pos);
    let dim = game.native_dimension().to_owned();
    if let Some(m) = mach(&block) {
        if let Some(root) = custom_data(stack).filter(|_| &*stack.item == block).cloned() {
            game.native_with_mod_block_entity(pos, |e| {
                if let Some(data) = gc(&root, "data") {
                    for (k, v) in data {
                        if k.to_string_lossy() == "inventory" {
                            e.items = horizon_items::item_nbt::decode_items(Some(v), m.slots);
                        } else {
                            e.nbt.insert(k.clone(), v.clone());
                        }
                    }
                }
                si(&mut e.nbt, "energy", gi(&root, "Energy", 0).max(0));
                super::enderio_machines::placed_data(&root, e, m);
            });
        }
        if let Some(top) = top_of(&block) {
            STATE.with_borrow_mut(|s| s.placements.push((dim, [pos[0], pos[1] + 1, pos[2]], top.to_owned())));
        }
        STATE.with_borrow_mut(|s| {
            for d in 0..6 {
                s.dirty.insert(step(pos, d));
            }
        });
        return;
    }
    if let Some(t) = bank(&block) {
        let stored = custom_data(stack).map_or(0, |c| gi(c, "horizon_energy", 0)).max(0);
        // onBlockPlaced: no bank of the tier beside it → the face towards the player shows the gauge; otherwise the
        // gauge faces of the banks above and below carry on.
        let player_face = game.native_acting_player().map(|p| {
            let heading = ((p.yaw * 4.0 / 360.0 + 0.5).floor() as i32) & 3;
            [2usize, 5, 3, 4][heading as usize]
        });
        let mut d = [0; 6];
        let mut neighbours = false;
        for f in 0..6 {
            if game.native_block(step(pos, f)) == t.id {
                neighbours = true;
            }
        }
        if !neighbours {
            if let Some(f) = player_face {
                d[opposite(f)] = 1;
            }
        } else {
            for f in [0usize, 1] {
                let p = step(pos, f);
                if game.native_block(p) == t.id {
                    if let Some(BlockEntity::Mod(o)) = game.world.block_entities.get(&(p[0], p[1], p[2])) {
                        let od = displays(&o.nbt);
                        for i in 2..6 {
                            if od[i] == 1 {
                                d[i] = 1;
                            }
                        }
                    }
                }
            }
        }
        game.native_with_mod_block_entity(pos, |e| {
            si(&mut e.nbt, "energy", stored.min(t.storage));
            set_displays(&mut e.nbt, d);
            if d.iter().any(|v| *v != 0) {
                let mut modes = face_modes(&e.nbt);
                for f in 0..6 {
                    if d[f] != 0 {
                        modes[f] = IO_DISABLED;
                    }
                }
                set_face_modes(&mut e.nbt, modes);
            }
        });
        // A bank between networks: they end, and the next tick joins them (reuseNetwork / a new network).
        let near: Vec<u32> = BANKS.with_borrow(|b| (0..6).filter_map(|f| b.of.get(&(dim.clone(), step(pos, f))).copied()).collect());
        for id in near {
            destroy_network(game, id, None);
        }
    }
    STATE.with_borrow_mut(|s| {
        for d in 0..6 {
            s.dirty.insert(step(pos, d));
        }
    });
}

/// `destroy_block`: a broken bank ends its network; an enhanced machine's two blocks go together.
pub(in crate::game) fn destroyed(game: &mut Game<'_>, pos: Pos) {
    let block = game.native_block(pos);
    let dim = game.native_dimension().to_owned();
    if bank(&block).is_some() {
        if let Some(id) = BANKS.with_borrow(|b| b.of.get(&(dim.clone(), pos)).copied()) {
            destroy_network(game, id, Some(pos));
        }
    }
    if let Some(top) = top_of(&block) {
        let above = [pos[0], pos[1] + 1, pos[2]];
        STATE.with_borrow_mut(|s| s.removals.push((dim.clone(), above, top.to_owned(), None)));
    }
    if let Some(bottom) = bottom_of(&block) {
        // BlockMachineExtension: breaking the top breaks the machine, with its drop.
        let below = [pos[0], pos[1] - 1, pos[2]];
        if game.native_block(below) == bottom {
            let drop = match game.world.block_entities.get(&(below[0], below[1], below[2])) {
                Some(BlockEntity::Mod(e)) => {
                    let e = ModBlockEntity::clone(e);
                    let c = caches(game);
                    mach(bottom).map(|m| machine_drop(&c, &e, m))
                }
                _ => None,
            };
            STATE.with_borrow_mut(|s| s.removals.push((dim, below, bottom.to_owned(), drop)));
        }
    }
    STATE.with_borrow_mut(|s| {
        for d in 0..6 {
            s.dirty.insert(step(pos, d));
        }
    });
}

/// `neighbor_changed`: redstone, receptors and display faces are looked at again on the block's next tick.
pub(in crate::game) fn neighbor_changed(block: &str, pos: Pos) {
    super::enderio_conduits::neighbor_changed(block, pos);
    if mach(block).is_some() || bank(block).is_some() {
        STATE.with_borrow_mut(|s| {
            s.dirty.insert(pos);
        });
    }
}

const WRENCH: &str = "enderio:yeta_wrench";

/// `right_click_block`: the wrench's face toggle and dismantle (BlockEio), the bank's display faces, the top block's
/// click going to the machine; and who opened which screen.
pub(in crate::game) fn right_click(game: &mut Game<'_>, pos: Pos, face: [i32; 3], canceled: &mut bool) {
    let block = game.native_block(pos);
    let Some(player) = game.native_acting_player() else { return };
    if super::enderio_conduits::right_click(game, pos, face, canceled) {
        return;
    }
    if super::enderio_machines::right_click_any(game, pos, face, canceled) {
        return;
    }
    if let Some(bottom) = bottom_of(&block) {
        let below = [pos[0], pos[1] - 1, pos[2]];
        if game.native_block(below) == bottom && !player.sneaking {
            *canceled = true;
            let host = game.serving.as_ref().is_some_and(|s| s.owner == player.uuid);
            if host {
                STATE.with_borrow_mut(|s| s.open.insert(player.uuid, below));
                game.open_menu(below);
            }
        }
        return;
    }
    let is_machine = mach(&block).is_some();
    let tier = bank(&block);
    if !is_machine && tier.is_none() {
        return;
    }
    let held = game.native_held_stack(player.uuid);
    let wrench = held.as_ref().is_some_and(|h| &*h.item == WRENCH);
    let f = face_of_normal(face);
    let dim = game.native_dimension().to_owned();
    if wrench && player.sneaking {
        // ToolUtil.breakBlockWithTool: dismantled, with its drop.
        *canceled = true;
        let drop = match game.world.block_entities.get(&(pos[0], pos[1], pos[2])) {
            Some(BlockEntity::Mod(e)) => {
                let e = ModBlockEntity::clone(e);
                if let Some(m) = mach(&block) {
                    let c = caches(game);
                    Some(machine_drop(&c, &e, m))
                } else {
                    tier.map(|t| bank_drop(&e, t))
                }
            }
            _ => None,
        };
        destroyed(game, pos);
        STATE.with_borrow_mut(|s| s.removals.push((dim, pos, block.clone(), drop)));
        return;
    }
    if let Some(f) = f {
        if let Some(t) = tier {
            let horizontal = f >= 2;
            let empty = held.is_none();
            if (player.sneaking && empty && horizontal) || (wrench && horizontal && game.native_mod_block_entity(pos).is_some_and(|e| face_modes(&e.nbt)[f] == IO_DISABLED)) {
                // Cycle the display: NONE, LEVEL_BAR, IO; to NONE, the face's default IO (a wrench toggles it
                // instead), otherwise DISABLED.
                *canceled = true;
                let by_wrench = wrench;
                let default = default_bank_mode(game, pos, f, t);
                let next_io = |cur: i32| bank_next_mode(game, pos, f, t, cur);
                let cur = game.native_mod_block_entity(pos).map_or(IO_NONE, |e| face_modes(&e.nbt)[f]);
                let toggled = next_io(cur);
                game.native_with_mod_block_entity(pos, |e| {
                    let mut d = displays(&e.nbt);
                    d[f] = (d[f] + 1) % 3;
                    let mut modes = face_modes(&e.nbt);
                    modes[f] = if d[f] == 0 { if by_wrench { toggled } else { default } } else { IO_DISABLED };
                    set_displays(&mut e.nbt, d);
                    set_face_modes(&mut e.nbt, modes);
                });
                forget_receptors(game, pos);
                game.native_block_entity_sync(pos);
                return;
            }
        }
        if wrench {
            *canceled = true;
            let g: &Game<'_> = game;
            let mut copy = match g.world.block_entities.get(&(pos[0], pos[1], pos[2])) {
                Some(BlockEntity::Mod(e)) => ModBlockEntity::clone(e),
                _ => return,
            };
            toggle_face(g, pos, &mut copy, f);
            let modes = face_modes(&copy.nbt);
            game.native_with_mod_block_entity(pos, |e| set_face_modes(&mut e.nbt, modes));
            forget_receptors(game, pos);
            game.native_block_entity_sync(pos);
            return;
        }
    }
    // BlockEnder.onBlockActivated: a fluid container in the hand fills or empties the tanks first.
    if !player.sneaking && is_machine && super::enderio_machines::bucket_click(game, pos) {
        *canceled = true;
        return;
    }
    STATE.with_borrow_mut(|s| s.open.insert(player.uuid, pos));
}

/// Who opened the screen of the machine at `pos` last (the enchanter charges them).
pub(in crate::game) fn opener(pos: Pos) -> Option<u128> {
    STATE.with_borrow(|s| s.open.iter().find(|(_, p)| **p == pos).map(|(u, _)| *u))
}

/// The players who took from slot 3 of the machine at `pos` since it last looked.
pub(in crate::game) fn takers(pos: Pos) -> Vec<u128> {
    STATE.with_borrow_mut(|s| {
        let mine: Vec<u128> = s.taken.iter().filter(|(_, p)| *p == pos).map(|(u, _)| *u).collect();
        s.taken.retain(|(_, p)| *p != pos);
        mine
    })
}

/// A player clicked a slot of a menu: the alloy smelter's output may have been taken (SlotSmelter.onTake).
pub fn menu_clicked(uuid: u128, slot: i32) {
    STATE.with_borrow_mut(|s| {
        if slot == 3 {
            if let Some(pos) = s.open.get(&uuid).copied() {
                s.taken.push((uuid, pos));
            }
        }
    });
}

/// `native_menu_quick_move` (AbstractMachineContainer.transferStackInSlot): from the player, the capacitor slot
/// first, then the inputs. The rest is the game's own routing.
pub(in crate::game) fn quick_move(game: &mut Game<'_>, e: &mut ModBlockEntity, container_slots: usize, slot: usize, moving: &mut ItemStack, handled: &mut bool) {
    if e.id == super::enderio_conduits::BUNDLE {
        super::enderio_conduits::quick_move(e, container_slots, slot, moving, handled);
        return;
    }
    let Some(m) = mach(&e.id) else { return };
    if slot < container_slots {
        return;
    }
    let c = caches(game);
    let mut targets: Vec<usize> = m.cap.into_iter().collect();
    targets.extend(m.inputs.iter().copied().filter(|s| !(m.kind == Kind::Sag && m.level == 0 && *s == 1)));
    let start = moving.count;
    for t in targets {
        if moving.count == 0 {
            break;
        }
        if !valid_for_slot(game, &c, e, m, t, moving) {
            continue;
        }
        let most = if Some(t) == m.cap { 1 } else { super::enderio_machines::slot_limit(m, t).map_or(max_stack(game, &moving.item), |l| l.min(max_stack(game, &moving.item))) };
        match e.items[t].as_mut() {
            Some(have) if have.same_item_same_components(moving) => {
                let n = most.saturating_sub(have.count).min(moving.count);
                have.count += n;
                moving.count -= n;
            }
            None => {
                let n = most.min(moving.count);
                e.items[t] = Some(ItemStack { count: n, ..moving.clone() });
                moving.count -= n;
            }
            _ => {}
        }
        if Some(t) == m.cap && moving.count < start {
            break;
        }
    }
    if moving.count < start {
        *handled = true;
        si(&mut e.nbt, "hz_failed", 0);
    }
}

// ---- Items: the bank's energy (BlockItemCapBank), comparators, the client's update tag

pub fn item_energy_buffer(item: &str, out: &mut Option<crate::game::native_item_energy::ItemEnergyBuffer>) {
    if let Some(t) = bank(item).filter(|t| !t.creative) {
        *out = Some(crate::game::native_item_energy::ItemEnergyBuffer { capacity: t.storage, max_insert: t.max_io, max_extract: t.max_io });
    }
}

pub(in crate::game) fn is_bank(block: &str) -> bool {
    bank(block).is_some()
}

/// The comparator: stored == 0 ? 0 : 1 + stored / max * 14, this block's own.
pub(in crate::game) fn comparator(e: &ModBlockEntity) -> Option<i32> {
    let t = bank(&e.id)?;
    let stored = gi(&e.nbt, "energy", 0);
    Some(if stored == 0 { 0 } else { (1.0 + f64::from(stored) / f64::from(t.storage) * 14.0) as i32 })
}

/// `block_entity_update_tag`: what the client draws with (the IO faces, the Stirling's lava, the bank's displays).
pub fn update_tag(entity: &ModBlockEntity, tag: &mut Option<Compound>) {
    super::enderio_conduits::update_tag(entity, tag);
    if mach(&entity.id).is_none() && bank(&entity.id).is_none() {
        return;
    }
    let mut out = Compound::new();
    for k in ["faceModes", "isLavaFired", "faceDisplayTypes", "mode"] {
        if let Some(v) = entity.nbt.get(&js(k)) {
            out.insert(js(k), v.clone());
        }
    }
    *tag = Some(out);
}

/// The bank item's tooltip: LangPower.RF(stored, max).
pub(in crate::game) fn tooltip(stack: &ItemStack, lines: &mut Vec<(horizon_client::text::Text, u32)>) {
    super::enderio_machines::tooltip(stack, lines);
    let Some(t) = bank(&stack.item) else { return };
    let (stored, max) = if t.creative {
        (i64::from(t.storage / 2), i64::from(t.storage))
    } else {
        (i64::from(custom_data(stack).map_or(0, |c| gi(c, "horizon_energy", 0)).max(0)), i64::from(t.storage))
    };
    let line = format!("{} / {}", super::enderio_ui::commas(stored), lang_power(max));
    lines.push((horizon_client::text::Text::legacy(&line), u32::MAX));
}

pub(in crate::game) fn lang_power_pub(v: i64) -> String {
    lang_power(v)
}

fn lang_power(v: i64) -> String {
    horizon_client::lang::get_or("enderio.power.format", "%s µI").replacen("%s", &super::enderio_ui::commas(v), 1)
}

pub(in crate::game) fn kit_frame(game: &mut Game<'_>) {
    super::enderio_conduits::client_frame(game);
    if !game.native_on(NS) {
        return;
    }
    kit_guide::register(NS, "Ender IO", "Ender IO guide");
    super::enderio_ui::register(game);
}
