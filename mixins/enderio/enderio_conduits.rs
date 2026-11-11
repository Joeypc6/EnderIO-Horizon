// Ender IO 5.3.72's conduits (CC0 / Unlicense, the Ender IO team) for Horizon: the conduit bundle and what it holds
// (spec-machines.md 4). One block, `enderio:conduit_bundle`, holds up to nine conduits, one of each base type; its
// block entity keeps them in `conduits` (one compound each, Java's field names). Networks are cached per base type
// and rebuilt only when a connection changes (a place, a break, a wrench, a neighbour change), never scanned per tick.
//
// Phase 3, step 1: the bundle (placing into it, breaking one conduit at a time, the wrench), its drawing (cores, arms,
// connectors, the IO rings, the colour bands, wire frames by the Yeta wrench's display mode) and the energy conduits
// with NetworkPowerManager (receptors, capacitor banks through CapBankSupply, the even share over the conduits).
use super::Game;
use super::enderio::{gi, js, opposite, si, step, FACES, NS, Pos};
use horizon_client::containers::{BlockEntity, ModBlockEntity};
use horizon_client::items::ItemStack;
use horizon_core::geometry::Direction;
use horizon_world::nbt::{Compound, Tag};
use std::cell::RefCell;
use std::collections::{HashMap, HashSet, VecDeque};

pub(in crate::game) const BUNDLE: &str = "enderio:conduit_bundle";
pub(in crate::game) const WRENCH: &str = "enderio:yeta_wrench";
pub(in crate::game) const PROBE: &str = "enderio:conduit_probe";
// TileConduitBundle.addConduit
const MAX_CONDUITS: usize = 9;
const DIR_NAMES: [&str; 6] = ["DOWN", "UP", "NORTH", "SOUTH", "WEST", "EAST"];
const PROP_NAMES: [&str; 6] = ["down", "up", "north", "south", "west", "east"];

// ---- The conduit types (ConduitRegistry order: item, fluid, energy, redstone)

#[derive(Clone, Copy, PartialEq, Eq, Hash, Debug)]
pub(in crate::game) enum Base {
    Item,
    Fluid,
    Energy,
    Redstone,
}

impl Base {
    pub(in crate::game) fn name(self) -> &'static str {
        match self {
            Base::Item => "item",
            Base::Fluid => "fluid",
            Base::Energy => "energy",
            Base::Redstone => "redstone",
        }
    }
    // The base interface's simple name (ConduitDisplayMode names, the probe's copy tags).
    pub(in crate::game) fn java(self) -> &'static str {
        match self {
            Base::Item => "IItemConduit",
            Base::Fluid => "ILiquidConduit",
            Base::Energy => "IPowerConduit",
            Base::Redstone => "IRedstoneConduit",
        }
    }
    // Offsets.registerOffsets: (none, X, Y, Z) as Offset vectors.
    fn offsets(self) -> [[i32; 3]; 4] {
        const E: [i32; 3] = [1, 0, 0];
        const W: [i32; 3] = [-1, 0, 0];
        const U: [i32; 3] = [0, 1, 0];
        const D: [i32; 3] = [0, -1, 0];
        const S: [i32; 3] = [0, 0, 1];
        const N: [i32; 3] = [0, 0, -1];
        match self {
            Base::Item => [E, S, E, E],
            Base::Fluid => [W, N, W, W],
            Base::Energy => [D, D, S, D],
            Base::Redstone => [U, U, N, U],
        }
    }
}

#[derive(Debug)]
pub(in crate::game) struct Def {
    pub item: &'static str,
    pub base: Base,
    pub tier: usize,
}

// The conduit items of the content mod; `tier` is the subtype (energy 0/1/2, fluid normal / pressurized / ender).
pub(in crate::game) static DEFS: [Def; 8] = [
    Def { item: "enderio:item_conduit", base: Base::Item, tier: 0 },
    Def { item: "enderio:fluid_conduit", base: Base::Fluid, tier: 0 },
    Def { item: "enderio:pressurized_fluid_conduit", base: Base::Fluid, tier: 1 },
    Def { item: "enderio:ender_fluid_conduit", base: Base::Fluid, tier: 2 },
    Def { item: "enderio:energy_conduit", base: Base::Energy, tier: 0 },
    Def { item: "enderio:enhanced_energy_conduit", base: Base::Energy, tier: 1 },
    Def { item: "enderio:ender_energy_conduit", base: Base::Energy, tier: 2 },
    Def { item: "enderio:redstone_conduit", base: Base::Redstone, tier: 0 },
];

pub(in crate::game) fn def_of(item: &str) -> Option<&'static Def> {
    DEFS.iter().find(|d| d.item == item)
}

// ConduitConfig: tier1/2/3_maxIO of the energy conduits, µI/t, and each conduit's buffer.
pub(in crate::game) const ENERGY_MAX_IO: [i32; 3] = [640, 5_120, 20_480];

// ---- ConnectionMode: IN_OUT, INPUT, OUTPUT, DISABLED, NOT_SET

pub(in crate::game) const IN_OUT: u8 = 0;
pub(in crate::game) const INPUT: u8 = 1;
pub(in crate::game) const OUTPUT: u8 = 2;
pub(in crate::game) const DISABLED: u8 = 3;
pub(in crate::game) const NOT_SET: u8 = 4;

pub(in crate::game) fn accepts_input(m: u8) -> bool {
    m == IN_OUT || m == INPUT
}
pub(in crate::game) fn accepts_output(m: u8) -> bool {
    m == IN_OUT || m == OUTPUT
}

// AbstractConduit.getNextConnectionMode: NOT_SET is skipped.
fn next_mode(m: u8) -> u8 {
    let n = (m + 1) % 5;
    if n == NOT_SET { IN_OUT } else { n }
}

fn default_mode(base: Base) -> u8 {
    match base {
        Base::Item => INPUT,
        Base::Redstone => OUTPUT,
        _ => IN_OUT,
    }
}

// RedstoneControlMode: IGNORE, ON, OFF, NEVER. DyeColor: BLACK, RED, GREEN, ... WHITE.
pub(in crate::game) const RS_IGNORE: i32 = 0;
pub(in crate::game) const RS_NEVER: i32 = 3;
pub(in crate::game) const RED: i32 = 1;

// ItemDye.DYE_COLORS (1.12), by DyeColor ordinal.
pub(in crate::game) const DYE_COLORS: [u32; 16] = [
    1_973_019, 11_743_532, 3_887_386, 5_320_730, 2_437_522, 8_073_150, 2_651_799, 11_250_603, 4_408_131, 14_188_952,
    4_312_372, 14_602_026, 6_719_955, 12_801_229, 15_435_844, 15_790_320,
];

// The 26.3 dyes by DyeColor ordinal (silver is light gray).
const DYE_ITEMS: [&str; 16] = [
    "minecraft:black_dye", "minecraft:red_dye", "minecraft:green_dye", "minecraft:brown_dye", "minecraft:blue_dye",
    "minecraft:purple_dye", "minecraft:cyan_dye", "minecraft:light_gray_dye", "minecraft:gray_dye", "minecraft:pink_dye",
    "minecraft:lime_dye", "minecraft:yellow_dye", "minecraft:light_blue_dye", "minecraft:magenta_dye",
    "minecraft:orange_dye", "minecraft:white_dye",
];

// DyeColor.getColorFromDye.
fn dye_of(stack: &ItemStack) -> Option<i32> {
    DYE_ITEMS.iter().position(|d| *d == &*stack.item).map(|i| i as i32)
}

// ---- One conduit, as its block entity keeps it

#[derive(Clone, Debug)]
pub(in crate::game) struct Conduit {
    pub def: &'static Def,
    pub conns: u8,
    pub ext: u8,
    pub modes: [Option<u8>; 6],
    // The type's own fields (energyStoredRF, pRsMode.<DIR>...), as Java names them.
    pub nbt: Compound,
}

impl Conduit {
    fn new(def: &'static Def) -> Self {
        Conduit { def, conns: 0, ext: 0, modes: [None; 6], nbt: Compound::new() }
    }
    pub(in crate::game) fn mode(&self, d: usize) -> u8 {
        self.modes[d].unwrap_or(default_mode(self.def.base))
    }
    fn set_mode(&mut self, d: usize, m: u8) {
        self.modes[d] = if m == default_mode(self.def.base) { None } else { Some(m) };
    }
    pub(in crate::game) fn has_conn(&self, d: usize) -> bool {
        self.conns & (1 << d) != 0
    }
    pub(in crate::game) fn has_ext(&self, d: usize) -> bool {
        self.ext & (1 << d) != 0
    }
    fn connected(&self, d: usize) -> bool {
        self.has_conn(d) || self.has_ext(d)
    }
    fn has_connections(&self) -> bool {
        self.conns != 0 || self.ext != 0
    }
    // getExtractionRedstoneMode: energy IGNORE, items and fluids NEVER by default.
    pub(in crate::game) fn rs_mode(&self, d: usize) -> i32 {
        let key = if self.def.base == Base::Energy { "pRsMode." } else { "extRM." };
        let default = if self.def.base == Base::Energy { RS_IGNORE } else { RS_NEVER };
        gi(&self.nbt, &format!("{key}{}", DIR_NAMES[d]), default).clamp(0, 3)
    }
    pub(in crate::game) fn rs_color(&self, d: usize) -> i32 {
        let key = if self.def.base == Base::Energy { "pRsCol." } else { "extSC." };
        gi(&self.nbt, &format!("{key}{}", DIR_NAMES[d]), RED).clamp(0, 15)
    }
    fn set_short(&mut self, key: &str, v: i32) {
        self.nbt.insert(js(key), Tag::Short(v as i16));
    }
    fn set_rs_color(&mut self, d: usize, c: i32) {
        let key = if self.def.base == Base::Energy { "pRsCol." } else { "extSC." };
        self.set_short(&format!("{key}{}", DIR_NAMES[d]), c);
    }
    fn energy(&self) -> i32 {
        gi(&self.nbt, "energyStoredRF", 0)
    }
    // The conduit's item and what rides on it (getDrops).
    fn drops(&self) -> Vec<ItemStack> {
        let mut out = vec![ItemStack::new(self.def.item, 1)];
        // ItemConduit.getDrops: the function upgrades and the filters of each face too.
        for key in ["functionUpgrades", "inputFilterUpgrades", "outputFilterUpgrades"] {
            for d in 0..6 {
                out.extend(self.stack(key, d));
            }
        }
        out
    }
}

fn dirs_of(c: &Compound, key: &str) -> u8 {
    match c.get(&js(key)) {
        Some(Tag::IntArray(a)) => a.iter().filter(|d| (0..6).contains(*d)).fold(0, |m, d| m | (1 << d)),
        _ => 0,
    }
}

fn dirs_tag(m: u8) -> Tag {
    Tag::IntArray((0..6).filter(|d| m & (1 << d) != 0).collect())
}

// The bundle's conduits, from its compound (the server's, or what the client was sent).
pub(in crate::game) fn load(nbt: &Compound) -> Vec<Conduit> {
    let Some(Tag::List(list)) = nbt.get(&js("conduits")) else { return Vec::new() };
    let mut out = Vec::new();
    for t in list.iter() {
        let Tag::Compound(c) = t else { continue };
        let Some(Tag::String(item)) = c.get(&js("item")) else { continue };
        let Some(def) = def_of(&item.to_string_lossy()) else { continue };
        let mut modes = [None; 6];
        if let Some(Tag::ByteArray(b)) = c.get(&js("conModes")) {
            if b.len() == 6 {
                for d in 0..6 {
                    let m = (b[d] as u8).min(NOT_SET);
                    modes[d] = (m != default_mode(def.base)).then_some(m);
                }
            }
        }
        let mut rest = c.clone();
        for k in ["item", "connections", "externalConnections", "conModes"] {
            rest.remove(&js(k));
        }
        out.push(Conduit { def, conns: dirs_of(c, "connections"), ext: dirs_of(c, "externalConnections"), modes, nbt: rest });
    }
    out
}

pub(in crate::game) fn save(conduits: &[Conduit], nbt: &mut Compound) {
    let mut list = Vec::new();
    for c in conduits {
        let mut t = c.nbt.clone();
        t.insert(js("item"), Tag::String(js(c.def.item)));
        t.insert(js("connections"), dirs_tag(c.conns));
        t.insert(js("externalConnections"), dirs_tag(c.ext));
        if c.modes.iter().any(Option::is_some) {
            t.insert(js("conModes"), Tag::ByteArray((0..6).map(|d| c.mode(d)).collect()));
        }
        list.push(Tag::Compound(t));
    }
    nbt.insert(js("conduits"), Tag::List(list.into_iter().collect()));
}

fn find(b: &[Conduit], base: Base) -> Option<usize> {
    b.iter().position(|c| c.def.base == base)
}

// The bundle at `pos` as the world holds it (not the one ticking now: it's out of the world).
fn bundle_at(game: &Game<'_>, pos: Pos) -> Option<Vec<Conduit>> {
    match game.world.block_entities.get(&(pos[0], pos[1], pos[2])) {
        Some(BlockEntity::Mod(e)) if e.id == BUNDLE => Some(load(&e.nbt)),
        _ => None,
    }
}

fn is_bundle(game: &Game<'_>, pos: Pos) -> bool {
    game.native_block(pos) == BUNDLE
}

// ---- Geometry (ConduitGeometryUtil, conduitPixels = 3)

const WIDTH: f32 = 0.1875;
const HWIDTH: f32 = WIDTH / 2.0;
type Bb = [f32; 6];

fn core_bb() -> Bb {
    [0.5 - HWIDTH, 0.5 - HWIDTH, 0.5 - HWIDTH, 0.5 + HWIDTH, 0.5 + HWIDTH, 0.5 + HWIDTH]
}

fn dir_vec(d: usize) -> [f32; 3] {
    let s = step([0, 0, 0], d);
    [s[0] as f32, s[1] as f32, s[2] as f32]
}

fn translate(b: Bb, t: [f32; 3]) -> Bb {
    [b[0] + t[0], b[1] + t[1], b[2] + t[2], b[3] + t[0], b[4] + t[1], b[5] + t[2]]
}

// BoundingBox.scale: about the box's own centre.
fn scale(b: Bb, s: [f32; 3]) -> Bb {
    let w = (b[3] - b[0]) * (1.0 - s[0]) / 2.0;
    let h = (b[4] - b[1]) * (1.0 - s[1]) / 2.0;
    let dd = (b[5] - b[2]) * (1.0 - s[2]) / 2.0;
    [b[0] + w, b[1] + h, b[2] + dd, b[3] - w, b[4] - h, b[5] - dd]
}

fn expand(b: Bb, e: [f32; 3]) -> Bb {
    [b[0] - e[0], b[1] - e[1], b[2] - e[2], b[3] + e[0], b[4] + e[1], b[5] + e[2]]
}

fn union(a: Bb, b: Bb) -> Bb {
    [a[0].min(b[0]), a[1].min(b[1]), a[2].min(b[2]), a[3].max(b[3]), a[4].max(b[4]), a[5].max(b[5])]
}

// BoundingBox.intersects: strictly overlapping.
fn intersects(a: Bb, b: Bb) -> bool {
    b[3] > a[0] && b[0] < a[3] && b[4] > a[1] && b[1] < a[4] && b[5] > a[2] && b[2] < a[5]
}

fn contains(a: Bb, b: Bb) -> bool {
    a[0] <= b[0] && a[1] <= b[1] && a[2] <= b[2] && a[3] >= b[3] && a[4] >= b[4] && a[5] >= b[5]
}

fn volume(b: Bb) -> f32 {
    (b[3] - b[0]) * (b[4] - b[1]) * (b[5] - b[2])
}

fn axis_of(d: Option<usize>) -> usize {
    match d {
        None => 0,
        Some(4 | 5) => 1,
        Some(0 | 1) => 2,
        Some(_) => 3,
    }
}

// TileConduitBundle.getOffset: none while fewer than two conduits use that direction (all of them for none).
fn offset(b: &[Conduit], base: Base, d: Option<usize>) -> [f32; 3] {
    let count = match d {
        None => b.len(),
        Some(d) => b.iter().filter(|c| c.connected(d)).count(),
    };
    if count < 2 {
        return [0.0; 3];
    }
    let o = base.offsets()[axis_of(d)];
    [o[0] as f32 * WIDTH, o[1] as f32 * WIDTH, o[2] as f32 * WIDTH]
}

// createConduitBounds: the core stretched to the face in `d` (none: the core), moved by `off`, clamped to the block.
fn arm_bb(d: Option<usize>, off: [f32; 3]) -> Bb {
    let c = core_bb();
    let mut b = c;
    if let Some(d) = d {
        match d {
            4 => { b[0] = 0.0; b[3] = c[0]; }
            5 => { b[0] = c[3]; b[3] = 1.0; }
            0 => { b[1] = 0.0; b[4] = c[1]; }
            1 => { b[1] = c[4]; b[4] = 1.0; }
            2 => { b[2] = 0.0; b[5] = c[2]; }
            _ => { b[2] = c[5]; b[5] = 1.0; }
        }
    }
    let t = translate(b, off);
    t.map(|v| v.clamp(0.0, 1.0))
}

// createBoundsForConnectionController: the core 0.8 across and 0.5 along the arm, towards the face.
fn controller_bb(d: usize, off: [f32; 3]) -> Bb {
    let v = dir_vec(d);
    let s = [0.8 * (1.0 - (v[0] * 0.5).abs()), 0.8 * (1.0 - (v[1] * 0.5).abs()), 0.8 * (1.0 - (v[2] * 0.5).abs())];
    let b = scale(core_bb(), s);
    let size = (b[3] - b[0]).min(b[4] - b[1]).min(b[5] - b[2]).max(0.075);
    let m = 0.5 - size * 1.2;
    translate(translate(b, [v[0] * m, v[1] * m, v[2] * m]), off)
}

// The external connector's two plates (createExternalConnector, width min(0.125 + 3 x WIDTH, 1), depth 0.05).
fn external_bbs(d: usize) -> [Bb; 2] {
    let w = (0.125 + WIDTH * 3.0).min(1.0);
    let depth = 0.05;
    let part = |c_min: f32, c_max: f32, d_min: f32, d_max: f32| -> Bb {
        let v = step([0, 0, 0], d);
        let fix = |x: f32| if x < 0.0 { 1.0 + x } else { x };
        let comp = |i: usize, c: f32, dd: f32| (1 - v[i].abs()) as f32 * c + v[i] as f32 * dd;
        let mn = [fix(comp(0, c_min, d_min)), fix(comp(1, c_min, d_min)), fix(comp(2, c_min, d_min))];
        let mx = [fix(comp(0, c_max, d_max)), fix(comp(1, c_max, d_max)), fix(comp(2, c_max, d_max))];
        [mn[0].min(mx[0]), mn[1].min(mx[1]), mn[2].min(mx[2]), mn[0].max(mx[0]), mn[1].max(mx[1]), mn[2].max(mx[2])]
    };
    [
        part(0.5 - w / 2.0, 0.5 + w / 2.0, 1.0 - depth / 2.0, 1.0),
        part(0.5 - w / 3.0, 0.5 + w / 3.0, 1.0 - depth, 1.0 - depth / 2.0),
    ]
}

#[derive(Clone, Copy, PartialEq, Debug)]
pub(in crate::game) enum Ck {
    Arm,
    Controller,
    Core,
    Internal,
    External,
}

#[derive(Clone, Copy, Debug)]
pub(in crate::game) struct Comp {
    pub base: Option<Base>,
    pub bb: Bb,
    pub dir: Option<usize>,
    pub kind: Ck,
}

// Whether a base type draws the coloured band (energy, items, ender fluid).
fn has_controller(c: &Conduit) -> bool {
    matches!(c.def.base, Base::Energy | Base::Item) || (c.def.base == Base::Fluid && c.def.tier == 2)
}

// AbstractConduit.getCollidableComponents: an arm for each connected, not disabled direction (and its band).
fn conduit_comps(b: &[Conduit], c: &Conduit) -> Vec<Comp> {
    let mut out = Vec::new();
    for d in 0..6 {
        if !c.connected(d) || c.mode(d) == DISABLED {
            continue;
        }
        let off = offset(b, c.def.base, Some(d));
        out.push(Comp { base: Some(c.def.base), bb: arm_bb(Some(d), off), dir: Some(d), kind: Ck::Arm });
        if has_controller(c) {
            out.push(Comp { base: Some(c.def.base), bb: controller_bb(d, off), dir: Some(d), kind: Ck::Controller });
        }
    }
    out
}

// TileConduitBundle.addConduitCores: a core at each offset the conduit's connections use (or the plain one).
fn cores(b: &[Conduit], c: &Conduit) -> Vec<Comp> {
    let mut offs: Vec<[f32; 3]> = Vec::new();
    if c.has_connections() {
        for d in (0..6).filter(|d| c.has_ext(*d)).chain((0..6).filter(|d| c.has_conn(*d))) {
            let o = offset(b, c.def.base, Some(d));
            if !offs.contains(&o) {
                offs.push(o);
            }
        }
    } else {
        offs.push(offset(b, c.def.base, None));
    }
    offs.into_iter().map(|o| Comp { base: Some(c.def.base), bb: arm_bb(None, o), dir: None, kind: Ck::Core }).collect()
}

// TileConduitBundle.addConnectors: the cores, one internal hub where conduits cross or spread, the external plates.
fn connectors(b: &[Conduit]) -> Vec<Comp> {
    let mut temp: Vec<Comp> = Vec::new();
    let mut all: Vec<Comp> = Vec::new();
    for c in b {
        let cs = cores(b, c);
        temp.extend(cs.iter().copied());
        all.extend(conduit_comps(b, c));
        all.extend(cs);
    }
    let mut colliding: Vec<Base> = Vec::new();
    for (i, a) in all.iter().enumerate() {
        for (j, o) in all.iter().enumerate() {
            if i != j && a.kind != Ck::Controller && o.kind != Ck::Controller && intersects(a.bb, o.bb) {
                if let Some(t) = a.base.filter(|t| !colliding.contains(t)) {
                    colliding.push(t);
                }
            }
        }
    }
    let mut hubs: Vec<Bb> = Vec::new();
    if !colliding.is_empty() {
        let mut bb: Option<Bb> = None;
        for t in &colliding {
            if let Some(c) = b.iter().find(|c| c.def.base == *t) {
                for core in cores(b, c) {
                    bb = Some(bb.map_or(core.bb, |x| union(x, core.bb)));
                }
            }
        }
        if let Some(x) = bb {
            hubs.push(scale(x, [1.05; 3]));
        }
    }
    for c in b.iter().filter(|c| c.has_connections()) {
        let cs = cores(b, c);
        if cs.len() > 1 {
            let first = cs[0].bb;
            let bb = cs.iter().fold(first, |acc, x| union(acc, x.bb));
            if volume(bb) > volume(first) * 1.5 {
                hubs.push(scale(bb, [1.05; 3]));
            }
        }
    }
    if let Some(first) = hubs.first().copied() {
        let hub = hubs.iter().fold(first, |a, x| union(a, *x));
        temp.push(Comp { base: None, bb: hub, dir: None, kind: Ck::Internal });
    }
    for d in 0..6 {
        if b.iter().any(|c| c.has_ext(d) && c.mode(d) != DISABLED) {
            temp.push(Comp { base: None, bb: external_bbs(d)[0], dir: Some(d), kind: Ck::External });
        }
    }
    temp
}

// getCollidableComponents: every conduit's, then the connectors.
pub(in crate::game) fn collidables(b: &[Conduit]) -> Vec<Comp> {
    let mut out: Vec<Comp> = b.iter().flat_map(|c| conduit_comps(b, c)).collect();
    out.extend(connectors(b));
    out
}

// The arms the block's shape shows (its state's down..east), by the directions any conduit draws an arm in.
fn arm_mask(b: &[Conduit]) -> u8 {
    b.iter().fold(0, |m, c| (0..6).filter(|d| c.connected(*d) && c.mode(*d) != DISABLED).fold(m, |m, d| m | (1 << d)))
}

// ---- Ray tracing (BlockConduitBundle.doRayTraceAll), on the server from the acting player's eye

#[derive(Clone, Copy, Debug)]
pub(in crate::game) struct Hit {
    pub comp: Comp,
    pub dist: f64,
    pub face: usize,
}

fn eye_and_look(game: &Game<'_>) -> Option<([f64; 3], [f64; 3])> {
    let p = game.native_acting_player()?;
    let eye = [p.position[0], p.position[1] + if game.native_sneaking() { 1.27 } else { 1.62 }, p.position[2]];
    let (y, x) = (f64::from(p.yaw).to_radians(), f64::from(p.pitch).to_radians());
    Some((eye, [-y.sin() * x.cos(), -x.sin(), y.cos() * x.cos()]))
}

// The ray (from `eye` along `dir`, at most `reach`) against a box of the block at `pos`: the distance and the face hit.
fn ray_box(eye: [f64; 3], dir: [f64; 3], reach: f64, pos: Pos, b: Bb) -> Option<(f64, usize)> {
    let (mut t0, mut t1) = (0.0f64, reach);
    let mut face = 0usize;
    for i in 0..3 {
        let (lo, hi) = (f64::from(pos[i]) + f64::from(b[i]), f64::from(pos[i]) + f64::from(b[i + 3]));
        if dir[i].abs() < 1e-12 {
            if eye[i] < lo || eye[i] > hi {
                return None;
            }
            continue;
        }
        let (a, c) = ((lo - eye[i]) / dir[i], (hi - eye[i]) / dir[i]);
        let (near, far) = if a < c { (a, c) } else { (c, a) };
        if near > t0 {
            t0 = near;
            // Entering through the min side faces the negative direction.
            face = match (i, dir[i] > 0.0) {
                (0, true) => 4,
                (0, false) => 5,
                (1, true) => 0,
                (1, false) => 1,
                (2, true) => 2,
                _ => 3,
            };
        }
        t1 = t1.min(far);
        if t0 > t1 {
            return None;
        }
    }
    Some((t0, face))
}

// How much wider the block state's shape is than the components (5 / 16 against 6.5 / 16 from the centre).
const SHAPE_GROW: f32 = 0.093_75;

// Every component the acting player's look meets, nearest first (RaytraceResult.sort); an empty bundle is a cube.
fn ray_all(game: &Game<'_>, pos: Pos, b: &[Conduit], mode: Mode) -> Vec<Hit> {
    let Some((eye, dir)) = eye_and_look(game) else { return Vec::new() };
    let reach = if game.creative { 5.0 } else { 4.5 };
    let mut hits = Vec::new();
    for comp in collidables(b) {
        if mode.is_all() || comp.base.is_none_or(|t| mode.renders(t)) {
            if let Some((dist, face)) = ray_box(eye, dir, reach + 1.0, pos, comp.bb) {
                hits.push(Hit { comp, dist, face });
            }
        }
    }
    // Horizon's block shape is coarser than Java's components (the core and arms 5..11 px across against 6.5..9.5):
    // a click on the shape's skin that meets no component takes the component under that skin (each grown by the
    // difference, 1.5 px), with the face of the grown box, as Java's selection box (the components) would have
    // taken it. Only when nothing was hit, so what Java's ray meets first is never overridden.
    if hits.is_empty() && !b.is_empty() {
        for comp in collidables(b) {
            if mode.is_all() || comp.base.is_none_or(|t| mode.renders(t)) {
                let grown = expand(comp.bb, [SHAPE_GROW; 3]).map(|v| v.clamp(0.0, 1.0));
                if let Some((dist, face)) = ray_box(eye, dir, reach + 1.0, pos, grown) {
                    hits.push(Hit { comp, dist, face });
                }
            }
        }
    }
    if b.is_empty() {
        if let Some((dist, face)) = ray_box(eye, dir, reach + 1.0, pos, [0.0, 0.0, 0.0, 1.0, 1.0, 1.0]) {
            hits.push(Hit { comp: Comp { base: None, bb: [0.0, 0.0, 0.0, 1.0, 1.0, 1.0], dir: Some(face), kind: Ck::Core }, dist, face });
        }
    }
    hits.sort_by(|a, c| a.dist.total_cmp(&c.dist));
    hits
}

// ---- The Yeta wrench's display mode (ConduitDisplayMode: NEUTRAL, NONE, ALL, then one per base type)

#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub(in crate::game) enum Mode {
    Neutral,
    None,
    All,
    Type(Base),
}

pub(in crate::game) const MODES: [Mode; 7] = [Mode::Neutral, Mode::None, Mode::All, Mode::Type(Base::Item), Mode::Type(Base::Fluid), Mode::Type(Base::Energy), Mode::Type(Base::Redstone)];

impl Mode {
    pub(in crate::game) fn name(self) -> &'static str {
        match self {
            Mode::Neutral => "neutral",
            Mode::None => "none",
            Mode::All => "all",
            Mode::Type(b) => b.java(),
        }
    }
    pub(in crate::game) fn is_all(self) -> bool {
        matches!(self, Mode::All | Mode::Neutral)
    }
    // renderConduit.
    pub(in crate::game) fn renders(self, t: Base) -> bool {
        match self {
            Mode::All | Mode::Neutral => true,
            Mode::None => false,
            Mode::Type(b) => b == t,
        }
    }
}

// ConduitDisplayMode.getDisplayMode: the wrench's DISPLAYMODE, ALL for anything else.
pub(in crate::game) fn display_mode(held: Option<&ItemStack>) -> Mode {
    let Some(h) = held.filter(|h| &*h.item == WRENCH) else { return Mode::All };
    let name = super::enderio::custom_data(h).and_then(|c| super::enderio::gs(c, "DISPLAYMODE"));
    name.and_then(|n| MODES.iter().copied().find(|m| m.name() == n)).unwrap_or(Mode::All)
}

// ---- State: dirty bundles, queued block changes, the copies of bundles leaving the world

#[derive(Default)]
struct State {
    // A neighbour changed: look at the external connections again on the next tick (onNeighborBlockChange).
    dirty: HashSet<Pos>,
    // The bundle's arms changed: its block state's shape follows on the level tick (a hook can't set blocks).
    shapes: HashSet<(String, Pos)>,
    // Bundles to take away on the level tick (the last conduit wrenched off).
    removals: Vec<(String, Pos)>,
    // The block's external redstone level (getExternalRedstoneLevel), until a neighbour changes.
    ext_signal: HashMap<Pos, i32>,
    removed: HashMap<Pos, ModBlockEntity>,
}

thread_local! {
    static STATE: RefCell<State> = RefCell::new(State::default());
}

fn key(game: &Game<'_>, pos: Pos) -> (String, Pos) {
    (game.native_dimension().to_owned(), pos)
}

// ---- Networks (AbstractConduitNetwork; PowerConduitNetwork and NetworkPowerManager)

// PowerTracker: what the network sent and received, averaged over 5 seconds (PerTickIntAverageCalculator(5)).
#[derive(Default, Clone)]
struct Tracker {
    sent: Vec<i64>,
    recv: Vec<i64>,
}

impl Tracker {
    fn tick(&mut self, sent: i64, recv: i64) {
        for (v, x) in [(&mut self.sent, sent), (&mut self.recv, recv)] {
            v.push(x);
            if v.len() > 100 {
                v.remove(0);
            }
        }
    }
    fn avg(v: &[i64]) -> f32 {
        if v.is_empty() { 0.0 } else { v.iter().sum::<i64>() as f32 / v.len() as f32 }
    }
}

struct Receptor {
    pos: Pos,
    d: usize,
    mode: u8,
    rs: i32,
    color: i32,
}

#[derive(Default)]
struct EnergyNet {
    max_io: i32,
    // Each conduit's stored energy while the network lives (written into its block entity each tick).
    stored: HashMap<Pos, i32>,
    receptors_dirty: bool,
    receptors: Vec<Receptor>,
    storage: Vec<Receptor>,
    at: usize,
    energy: i64,
    max: i64,
    in_banks: i64,
    max_banks: i64,
    tracker: Tracker,
}

struct Net {
    dim: String,
    base: Base,
    members: Vec<Pos>,
    energy: Option<EnergyNet>,
    item: Option<ItemNet>,
    fluid: Option<FluidNet>,
    redstone: Option<RedstoneNet>,
}

#[derive(Default)]
struct Nets {
    of: HashMap<(String, Pos, Base), u32>,
    nets: HashMap<u32, Net>,
    next: u32,
}

thread_local! {
    static NETS: RefCell<Nets> = RefCell::new(Nets::default());
}

fn net_of(game: &Game<'_>, pos: Pos, base: Base) -> Option<u32> {
    let dim = game.native_dimension().to_owned();
    NETS.with_borrow(|n| n.of.get(&(dim, pos, base)).copied())
}

// destroyNetwork: the members forget it and rebuild on their next tick; each energy conduit keeps its share.
fn destroy_network(game: &mut Game<'_>, id: u32) {
    let Some(net) = NETS.with_borrow_mut(|n| {
        let net = n.nets.remove(&id)?;
        for p in &net.members {
            n.of.remove(&(net.dim.clone(), *p, net.base));
        }
        Some(net)
    }) else { return };
    if net.dim != game.native_dimension() {
        return;
    }
    if let Some(en) = &net.energy {
        for (p, v) in &en.stored {
            let v = *v;
            game.native_with_mod_block_entity(*p, |e| set_conduit_energy(&mut e.nbt, v));
        }
    }
    if let Some(f) = &net.fluid {
        for p in &net.members {
            let (fl, v) = fluid_share(f, &net.members, *p);
            game.native_with_mod_block_entity(*p, |e| {
                let mut b = load(&e.nbt);
                if let Some(i) = find(&b, Base::Fluid) {
                    b[i].set_tank(fl.as_deref(), v);
                    save(&b, &mut e.nbt);
                }
            });
        }
    }
}

fn destroy_at(game: &mut Game<'_>, pos: Pos, base: Base) {
    if let Some(id) = net_of(game, pos, base) {
        destroy_network(game, id);
    }
}

// The energy conduit's `energyStoredRF` in a bundle compound, written in place.
fn set_conduit_energy(nbt: &mut Compound, v: i32) {
    if let Some(Tag::List(list)) = nbt.get_mut(&js("conduits")) {
        for t in list.iter_mut() {
            if let Tag::Compound(c) = t {
                let is_energy = matches!(c.get(&js("item")), Some(Tag::String(s)) if def_of(&s.to_string_lossy()).is_some_and(|d| d.base == Base::Energy));
                if is_energy {
                    si(c, "energyStoredRF", v);
                }
            }
        }
    }
}

// ConduitUtil.ensureValidNetwork: the conduits joined to this one, one network (the neighbours' old ones end).
// None when a joined conduit's chunk isn't loaded (setNetworkBuildFailed).
fn build_network(game: &mut Game<'_>, pos: Pos, own: &[Conduit], base: Base) -> Option<u32> {
    let dim = game.native_dimension().to_owned();
    let mut members = vec![pos];
    let mut energies: HashMap<Pos, i32> = HashMap::new();
    let mut tier = 0usize;
    if let Some(i) = find(own, base) {
        energies.insert(pos, own[i].energy());
        tier = own[i].def.tier;
    }
    let mut seen = HashSet::from([pos]);
    let mut queue = VecDeque::from([(pos, own.to_vec())]);
    let mut old = HashSet::new();
    while let Some((at, b)) = queue.pop_front() {
        let Some(i) = find(&b, base) else { continue };
        for d in (0..6).filter(|d| b[i].has_conn(*d)) {
            let n = step(at, d);
            if seen.contains(&n) {
                continue;
            }
            if !game.world.chunks.contains_key(&(n[0].div_euclid(16), n[2].div_euclid(16))) {
                return None;
            }
            let Some(nb) = bundle_at(game, n) else { continue };
            let Some(j) = find(&nb, base) else { continue };
            seen.insert(n);
            members.push(n);
            energies.insert(n, nb[j].energy());
            if let Some(id) = net_of(game, n, base) {
                old.insert(id);
            }
            queue.push_back((n, nb));
        }
    }
    for id in old {
        destroy_network(game, id);
    }
    // The old networks wrote their shares back: read them again.
    for p in &members[1..] {
        if let Some(nb) = bundle_at(game, *p) {
            if let Some(j) = find(&nb, base) {
                energies.insert(*p, nb[j].energy());
            }
        }
    }
    let fluid = (base == Base::Fluid).then(|| fluid_net_new(game, &members, own, pos));
    let energy = (base == Base::Energy).then(|| {
        let max_io = ENERGY_MAX_IO[tier.min(2)];
        EnergyNet { max_io, stored: energies.into_iter().map(|(p, v)| (p, v.clamp(0, max_io))).collect(), receptors_dirty: true, ..EnergyNet::default() }
    });
    Some(NETS.with_borrow_mut(|n| {
        n.next = n.next.wrapping_add(1);
        let id = n.next;
        for p in &members {
            n.of.insert((dim.clone(), *p, base), id);
        }
        let item = (base == Base::Item).then(|| ItemNet { routes_dirty: true, ..ItemNet::default() });
        let redstone = (base == Base::Redstone).then(|| RedstoneNet { dirty: true, ..RedstoneNet::default() });
        n.nets.insert(id, Net { dim: dim.clone(), base, members, energy, item, fluid, redstone });
        id
    }))
}

// The energy networks' receptors are looked at again (a face's mode, a neighbour, an external connection changed).
// The item networks' routes too (ItemConduitNetwork.routesChanged).
fn receptors_changed(game: &Game<'_>, pos: Pos) {
    if let Some(id) = net_of(game, pos, Base::Energy) {
        NETS.with_borrow_mut(|n| {
            if let Some(e) = n.nets.get_mut(&id).and_then(|x| x.energy.as_mut()) {
                e.receptors_dirty = true;
            }
        });
    }
    if let Some(id) = net_of(game, pos, Base::Item) {
        NETS.with_borrow_mut(|n| {
            if let Some(e) = n.nets.get_mut(&id).and_then(|x| x.item.as_mut()) {
                e.routes_dirty = true;
            }
        });
    }
    if let Some(id) = net_of(game, pos, Base::Fluid) {
        NETS.with_borrow_mut(|n| {
            if let Some(e) = n.nets.get_mut(&id).and_then(|x| x.fluid.as_mut()) {
                e.dirty = true;
            }
        });
    }
}

// ---- Redstone control (ConduitUtil.isRedstoneControlModeMet)


fn external_signal(game: &mut Game<'_>, pos: Pos) -> i32 {
    if let Some(v) = STATE.with_borrow(|s| s.ext_signal.get(&pos).copied()) {
        return v;
    }
    let dim = game.native_dimension().to_owned();
    let v = game.native_neighbor_signal(&dim, pos);
    STATE.with_borrow_mut(|s| s.ext_signal.insert(pos, v));
    v
}

fn rs_met(game: &mut Game<'_>, pos: Pos, mode: i32, color: i32) -> bool {
    match mode {
        RS_IGNORE => true,
        RS_NEVER => false,
        _ => {
            let mut s = internal_signal(game, pos, color);
            if s < 1 && color == RED {
                s = s.max(external_signal(game, pos));
            }
            super::enderio::condition_met(mode, s)
        }
    }
}

// ---- External connections (canConnectToExternal) and the bundle's block entity

// An energy interface on the neighbour's side that is no bundle (PowerConduit.getExternalPowerReceptor).
fn energy_external(game: &Game<'_>, pos: Pos, d: usize) -> bool {
    let n = step(pos, d);
    !is_bundle(game, n) && game.native_energy(game.native_dimension(), n, FACES[opposite(d)]).is_some()
}

fn can_connect_external(game: &Game<'_>, pos: Pos, c: &Conduit, d: usize) -> bool {
    match c.def.base {
        Base::Energy => energy_external(game, pos, d),
        Base::Item => item_external(game, pos, d),
        Base::Fluid => fluid_external(game, pos, d),
        Base::Redstone => redstone_external(game, pos, c, d),
    }
}

// AbstractConduit.updateConnections: drop the external connections that went, add the new ones (not where a
// conduit connection is). True when one changed.
fn update_externals(game: &Game<'_>, pos: Pos, b: &mut [Conduit]) -> bool {
    let mut changed = false;
    for c in b.iter_mut() {
        // A joined conduit whose bundle went without a break (a command, a piston) lets go here (onBlockRemoved).
        for d in 0..6 {
            if !c.has_conn(d) {
                continue;
            }
            let n = step(pos, d);
            let loaded = game.world.chunks.contains_key(&(n[0].div_euclid(16), n[2].div_euclid(16)));
            if loaded && bundle_at(game, n).is_none_or(|nb| find(&nb, c.def.base).is_none_or(|j| !nb[j].has_conn(opposite(d)))) {
                c.conns &= !(1 << d);
                changed = true;
            }
        }
        let mut ext = 0u8;
        for d in 0..6 {
            if !c.has_conn(d) && can_connect_external(game, pos, c, d) {
                ext |= 1 << d;
            }
        }
        if ext != c.ext {
            c.ext = ext;
            changed = true;
        }
    }
    changed
}

pub fn block_entity(block: &str, entity: &mut Option<(String, Compound)>) {
    if entity.is_none() && block == BUNDLE {
        *entity = Some((block.to_owned(), Compound::new()));
    }
}

pub fn block_entity_ticks(block: &str, ticks: &mut bool) {
    if block == BUNDLE {
        *ticks = true;
    }
}

// TileConduitBundle.doUpdate: each conduit's updateNetwork and updateConnections; the energy share is written.
pub(in crate::game) fn block_entity_tick(game: &mut Game<'_>, pos: Pos, e: &mut ModBlockEntity) {
    if !game.native_on(NS) {
        return;
    }
    let k = key(game, pos);
    let mut b = load(&e.nbt);
    let mut changed = false;
    let dirty = STATE.with_borrow_mut(|s| s.dirty.remove(&pos)) || !e.nbt.contains_key(&js("hz_seen"));
    if dirty {
        si(&mut e.nbt, "hz_seen", 1);
        if update_externals(game, pos, &mut b) {
            changed = true;
            STATE.with_borrow_mut(|s| s.shapes.insert(k.clone()));
        }
        receptors_changed(game, pos);
    }
    let t = gi(&e.nbt, "hz_t", 0).wrapping_add(1);
    si(&mut e.nbt, "hz_t", t);
    for i in 0..b.len() {
        let base = b[i].def.base;
        if net_of(game, pos, base).is_none() {
            // setNetworkBuildFailed: try again after 200 + rand * 100 ticks.
            let retry = gi(&e.nbt, &format!("hz_retry_{}", base.name()), 0);
            if retry > 0 && t < retry {
                continue;
            }
            match build_network(game, pos, &b, base) {
                Some(_) => {
                    e.nbt.remove(&js(&format!("hz_retry_{}", base.name())));
                    receptors_changed(game, pos);
                }
                None => {
                    let wait = 200 + (super::enderio::next_float(game) * 100.0) as i32;
                    si(&mut e.nbt, &format!("hz_retry_{}", base.name()), t.wrapping_add(wait));
                }
            }
        }
    }
    // The energy conduit's share of its network.
    if let Some(i) = find(&b, Base::Energy) {
        if let Some(id) = net_of(game, pos, Base::Energy) {
            let v = NETS.with_borrow(|n| n.nets.get(&id).and_then(|x| x.energy.as_ref()).and_then(|en| en.stored.get(&pos).copied()));
            if let Some(v) = v {
                if b[i].energy() != v {
                    si(&mut b[i].nbt, "energyStoredRF", v);
                    changed = true;
                }
            }
        }
    }
    // The fluid conduit's share of its network.
    if let Some(i) = find(&b, Base::Fluid) {
        if let Some(id) = net_of(game, pos, Base::Fluid) {
            let share = NETS.with_borrow(|n| n.nets.get(&id).and_then(|x| x.fluid.as_ref().map(|f| fluid_share(f, &x.members, pos))));
            if let Some((fl, v)) = share {
                let now = b[i].tank();
                let fl = fl.filter(|_| v > 0);
                if now != (fl.clone(), v) {
                    b[i].set_tank(fl.as_deref(), v);
                    changed = true;
                }
            }
        }
    }
    if screen_tick(game, pos, e, &mut b) {
        changed = true;
    }
    if changed {
        save(&b, &mut e.nbt);
    }
    sync_visuals(game, pos, e, &b);
}

// What the client draws, as a key: when it changes, the client is sent the bundle again.
fn visual_key(b: &[Conduit]) -> String {
    let mut s = String::new();
    for c in b {
        s.push_str(&format!("{}:{}:{}:{:?}", c.def.item, c.conns, c.ext, c.modes));
        if c.def.base == Base::Fluid {
            let (f, a) = c.tank();
            let cap = if c.def.tier == 1 { 1000 } else { c.tank_capacity().max(1) };
            s.push_str(&format!("{f:?}{}", a * 16 / cap));
        }
        for d in 0..6 {
            if c.def.base == Base::Energy || c.def.base == Base::Item {
                s.push_str(&format!(",{}{}", c.rs_mode(d), c.rs_color(d)));
            }
        }
        s.push(';');
    }
    s
}

fn sync_visuals(game: &mut Game<'_>, pos: Pos, e: &mut ModBlockEntity, b: &[Conduit]) {
    let k = visual_key(b);
    let old = super::enderio::gs(&e.nbt, "hz_vis").unwrap_or_default();
    if k != old {
        super::enderio::ss(&mut e.nbt, "hz_vis", &k);
        game.native_block_entity_sync(pos);
    }
}

// `neighbor_changed`: the external connections and receptors are looked at again on the bundle's next tick.
pub(in crate::game) fn neighbor_changed(block: &str, pos: Pos) {
    if block == BUNDLE {
        STATE.with_borrow_mut(|s| {
            s.dirty.insert(pos);
            s.ext_signal.remove(&pos);
        });
        RS_DIRTY.with_borrow_mut(|s| {
            s.insert(pos);
        });
    }
}

// The client's update tag: the conduits.
pub fn update_tag(entity: &ModBlockEntity, tag: &mut Option<Compound>) {
    if entity.id != BUNDLE {
        return;
    }
    let mut out = Compound::new();
    if let Some(v) = entity.nbt.get(&js("conduits")) {
        out.insert(js("conduits"), v.clone());
    }
    for k in ["facadeType", "paintSource"] {
        if let Some(v) = entity.nbt.get(&js(k)) {
            out.insert(js(k), v.clone());
        }
    }
    *tag = Some(out);
}

// ---- Adding and removing conduits (TileConduitBundle.addConduit / removeConduit)

// onAddedToBundle: joined to the neighbours' conduits of its base type where both allow it, then the external ones.
fn added(game: &mut Game<'_>, pos: Pos, b: &mut Vec<Conduit>, i: usize) {
    let base = b[i].def.base;
    b[i].conns = 0;
    for d in 0..6 {
        let n = step(pos, d);
        let Some(mut nb) = bundle_at(game, n) else { continue };
        let Some(j) = find(&nb, base) else { continue };
        if can_join(&nb[j], opposite(d), &b[i]) && can_join(&b[i], d, &nb[j]) {
            b[i].conns |= 1 << d;
            nb[j].conns |= 1 << opposite(d);
            nb[j].ext &= !(1 << opposite(d));
            write_bundle(game, n, &nb);
            destroy_at(game, n, base);
        }
    }
    b[i].ext = 0;
    for d in 0..6 {
        if !b[i].has_conn(d) && can_connect_external(game, pos, &b[i], d) {
            b[i].ext |= 1 << d;
        }
    }
}

// canConnectToConduit: neither side DISABLED towards the other, and the type's own rule (energy: the same tier).
fn can_join(c: &Conduit, d: usize, other: &Conduit) -> bool {
    if c.mode(d) == DISABLED || other.mode(opposite(d)) == DISABLED || c.def.base != other.def.base {
        return false;
    }
    match c.def.base {
        // canDifferentTiersConnect = false; the fluid classes don't mix.
        Base::Energy | Base::Fluid => c.def.tier == other.def.tier,
        _ => true,
    }
}

// onAfterRemovedFromBundle: the neighbours let go, the network ends.
fn removed(game: &mut Game<'_>, pos: Pos, c: &Conduit) {
    let base = c.def.base;
    destroy_at(game, pos, base);
    for d in (0..6).filter(|d| c.has_conn(*d)) {
        let n = step(pos, d);
        let Some(mut nb) = bundle_at(game, n) else { continue };
        let Some(j) = find(&nb, base) else { continue };
        nb[j].conns &= !(1 << opposite(d));
        write_bundle(game, n, &nb);
        destroy_at(game, n, base);
        STATE.with_borrow_mut(|s| s.dirty.insert(n));
    }
}

// A bundle's conduits written back (not the ticking one), its shape and its clients to follow.
fn write_bundle(game: &mut Game<'_>, pos: Pos, b: &[Conduit]) {
    game.native_with_mod_block_entity(pos, |e| save(b, &mut e.nbt));
    let k = key(game, pos);
    STATE.with_borrow_mut(|s| s.shapes.insert(k));
    game.native_block_entity_sync(pos);
}

// ConduitUtil.playBreakSound(SoundType.METAL): (volume + 1) / 2, pitch x 0.8.
// Whether the acting player sees facades drawn solid (YetaUtil.isSolidFacadeRendered without the bundle).
pub(in crate::game) fn facade_solid_for(game: &Game<'_>) -> bool {
    let held = game.native_acting_player().and_then(|p| game.native_held_stack(p.uuid));
    !super::enderio_facades::hides_facades(held.as_ref())
}

// FacadeRenderState.WIRE_FRAME: the block's outline in the wire frame texture.
fn facade_wire() -> Vec<Quad> {
    let mut out = Vec::new();
    let e = 1.0 / 64.0;
    for a in 0..3 {
        for (u, v) in [(0.0, 0.0), (0.0, 1.0), (1.0, 0.0), (1.0, 1.0)] {
            let mut lo = [0.0f32; 3];
            let mut hi = [0.0f32; 3];
            let (b, c) = ((a + 1) % 3, (a + 2) % 3);
            lo[a] = 0.0;
            hi[a] = 1.0;
            lo[b] = if u == 0.0 { 0.0 } else { 1.0 - e };
            hi[b] = lo[b] + e;
            lo[c] = if v == 0.0 { 0.0 } else { 1.0 - e };
            hi[c] = lo[c] + e;
            cube(&mut out, [lo[0], lo[1], lo[2], hi[0], hi[1], hi[2]], "enderio:block/wire_frame", FULL, 0xFF_FFFF);
        }
    }
    out
}

fn metal_sound(game: &mut Game<'_>, pos: Pos) {
    let at = [f64::from(pos[0]) + 0.5, f64::from(pos[1]) + 0.5, f64::from(pos[2]) + 0.5];
    game.native_play_sound_in_place("minecraft:block.metal.break", at, 4, 1.0, 1.2);
}

fn bundle_full(game: &mut Game<'_>, uuid: u128) {
    let line = horizon_client::lang::get_or("enderio.gui.conduit_bundle_full", "Bundle Full");
    game.native_action_bar(uuid, horizon_client::text::Text::legacy(&line));
}

// The acting player's held stack, one taken (not in creative).
fn use_one(game: &mut Game<'_>) {
    if game.creative {
        return;
    }
    let i = game.selected;
    if let Some(slot) = game.inventory.get_mut(i) {
        if let Some(s) = slot.as_mut() {
            s.count = s.count.saturating_sub(1);
            if s.count == 0 {
                *slot = None;
            }
        }
    }
}

fn spawn_drops(game: &mut Game<'_>, pos: Pos, drops: Vec<ItemStack>) {
    for d in drops {
        game.spawn_item(d, [f64::from(pos[0]) + 0.5, f64::from(pos[1]) + 0.5, f64::from(pos[2]) + 0.5], [0.0; 3], 10);
    }
}

// Adds a conduit of `def` to the bundle at `pos`: false when it's full (or already has the type).
fn add_conduit(game: &mut Game<'_>, pos: Pos, def: &'static Def) -> bool {
    let Some(mut b) = bundle_at(game, pos) else { return false };
    if b.len() >= MAX_CONDUITS || find(&b, def.base).is_some() {
        return false;
    }
    b.push(Conduit::new(def));
    let i = b.len() - 1;
    added(game, pos, &mut b, i);
    write_bundle(game, pos, &b);
    for d in 0..6 {
        receptors_changed(game, step(pos, d));
    }
    true
}

fn remove_conduit(game: &mut Game<'_>, pos: Pos, b: &mut Vec<Conduit>, i: usize) -> Conduit {
    let c = b.remove(i);
    removed(game, pos, &c);
    c
}

// ---- The right click (AbstractItemConduit.onItemUseFirst / onItemUse, BlockConduitBundle.onBlockActivated)

// True when the click was the conduits'.
pub(in crate::game) fn right_click(game: &mut Game<'_>, pos: Pos, face: [i32; 3], canceled: &mut bool) -> bool {
    let Some(player) = game.native_acting_player() else { return false };
    let held = game.native_held_stack(player.uuid);
    let sneaking = game.native_sneaking();
    let on_bundle = is_bundle(game, pos);
    let Some(f) = (0..6).find(|&i| step([0, 0, 0], i) == face) else { return false };
    // A painted facade: onto the bundle clicked (BlockConduitBundle.onBlockActivated), else onto or as the block
    // beside the face (ItemConduitFacade.onItemUse).
    if let Some(h) = held.as_ref().filter(|h| super::enderio_facades::facade_type(&h.item).is_some() && super::enderio_facades::paint_of(h).is_some()) {
        let at = if on_bundle { pos } else { step(pos, f) };
        let has_be = game.world.block_entities.get(&(pos[0], pos[1], pos[2])).is_some();
        if !on_bundle && !sneaking && has_be {
            return false;
        }
        if let Some(drop) = super::enderio_facades::place(game, at, h, sneaking) {
            *canceled = true;
            if !game.creative {
                use_one(game);
                if let Some(d) = drop {
                    if let Some(rest) = game.native_give_stack(player.uuid, d) {
                        spawn_drops(game, at, vec![rest]);
                    }
                }
            }
            // ConduitUtil.playPlaceSound of the paint (its sound type: Horizon gives no block's sound type here).
            let c = [f64::from(at[0]) + 0.5, f64::from(at[1]) + 0.5, f64::from(at[2]) + 0.5];
            game.native_play_sound_in_place("minecraft:block.stone.place", c, 4, 1.0, 0.8);
            let k = key(game, at);
            STATE.with_borrow_mut(|s| {
                s.shapes.insert(k);
                s.dirty.insert(at);
            });
            game.native_block_entity_sync(at);
            return true;
        }
        if on_bundle {
            *canceled = !sneaking;
            return !sneaking;
        }
    }
    if let Some(def) = held.as_ref().and_then(|h| def_of(&h.item)) {
        // The clicked block's own activation comes first (a chest, a machine opens); sneaking skips it.
        let has_be = game.world.block_entities.get(&(pos[0], pos[1], pos[2])).is_some();
        if !on_bundle && !sneaking && has_be {
            return false;
        }
        *canceled = true;
        if on_bundle && !sneaking {
            let b = bundle_at(game, pos).unwrap_or_default();
            match find(&b, def.base) {
                // onItemUseFirst: another tier of the same type swaps in, the old one's drops back.
                Some(i) if b[i].def.item != def.item => {
                    let mut b = b;
                    let old = remove_conduit(game, pos, &mut b, i);
                    b.push(Conduit::new(def));
                    let j = b.len() - 1;
                    added(game, pos, &mut b, j);
                    write_bundle(game, pos, &b);
                    if !game.creative {
                        use_one(game);
                        for d in old.drops() {
                            if let Some(rest) = game.native_give_stack(player.uuid, d) {
                                spawn_drops(game, pos, vec![rest]);
                            }
                        }
                    }
                    return true;
                }
                Some(_) => {}
                // handleConduitClick: the type goes into this bundle.
                None => {
                    if add_conduit(game, pos, def) {
                        metal_sound(game, pos);
                        use_one(game);
                    } else {
                        bundle_full(game, player.uuid);
                    }
                    return true;
                }
            }
        }
        // onItemUse: a new bundle where the click lands (the clicked block if replaceable), or into the bundle there.
        let clicked_replaceable = game.res.facts().get(game.world.block(pos[0], pos[1], pos[2])).replaceable;
        let at = if clicked_replaceable { pos } else { step(pos, f) };
        let here = game.world.block(at[0], at[1], at[2]);
        if game.res.facts().get(here).replaceable && !is_bundle(game, at) {
            let dim = game.native_dimension().to_owned();
            if let Some(state) = game.native_default_state(BUNDLE) {
                if game.native_set_block(&dim, at, state, 3) {
                    if add_conduit(game, at, def) {
                        metal_sound(game, at);
                    } else {
                        bundle_full(game, player.uuid);
                    }
                    use_one(game);
                }
            }
            return true;
        }
        let place = step(pos, f);
        if is_bundle(game, place) && bundle_at(game, place).is_some_and(|b| find(&b, def.base).is_none()) {
            if add_conduit(game, place, def) {
                metal_sound(game, place);
                use_one(game);
            }
            return true;
        }
        if on_bundle {
            return bundle_click(game, pos, player.uuid, held.as_ref(), sneaking, canceled);
        }
        *canceled = false;
        return false;
    }
    if !on_bundle {
        return false;
    }
    // ItemConduitProbe in probe mode: the bundle's lines to the player's chat.
    if held.as_ref().is_some_and(|h| &*h.item == PROBE && probe_mode(h) == 0) {
        *canceled = true;
        probe(game, pos, player.uuid);
        return true;
    }
    // BlockConduitBundle.handleConduitProbeClick: copy / paste mode on a component other than a core.
    if let Some(h) = held.as_ref().filter(|h| &*h.item == PROBE && probe_mode(h) == 1) {
        if probe_copy_paste(game, pos, player.uuid, h, sneaking) {
            *canceled = true;
            return true;
        }
    }
    bundle_click(game, pos, player.uuid, held.as_ref(), sneaking, canceled)
}

// AbstractConduit.writeConnectionSettingsToNBT: {connectionMode, the type's settings} under the base interface's
// name, for face `d`.
fn write_settings(c: &Conduit, d: usize) -> Compound {
    let mut r = Compound::new();
    let short = |r: &mut Compound, k: &str, v: i32| {
        r.insert(js(k), Tag::Short(v as i16));
    };
    let byte = |r: &mut Compound, k: &str, v: bool| {
        r.insert(js(k), Tag::Byte(if v { 1 } else { 0 }));
    };
    short(&mut r, "connectionMode", i32::from(c.mode(d)));
    match c.def.base {
        Base::Redstone => {
            short(&mut r, "inputSignalColor", c.rs_input_color(d));
            short(&mut r, "outputSignalColor", c.rs_output_color(d));
            byte(&mut r, "signalStrong", c.rs_strong(d));
        }
        _ => {
            short(&mut r, "extractionSignalColor", c.rs_color(d));
            short(&mut r, "extractionRedstoneMode", c.rs_mode(d));
            // ItemConduit and EnderLiquidConduit: the channels, self feed, round robin, priority.
            if c.def.base == Base::Item || (c.def.base == Base::Fluid && c.def.tier == 2) {
                short(&mut r, "inputColor", c.in_color(d));
                short(&mut r, "outputColor", c.out_color(d));
                byte(&mut r, "selfFeed", c.self_feed(d));
                byte(&mut r, "roundRobin", c.round_robin(d));
                r.insert(js("outputPriority"), Tag::Int(c.priority(d)));
            }
        }
    }
    r
}

// readConduitSettingsFromNBT / readTypeSettings.
fn read_settings(c: &mut Conduit, d: usize, r: &Compound) {
    if r.get(&js("connectionMode")).is_some() {
        let m = gi(r, "connectionMode", i32::from(default_mode(c.def.base))).clamp(0, 4) as u8;
        c.set_mode(d, m);
    }
    let dir = DIR_NAMES[d];
    match c.def.base {
        Base::Redstone => {
            c.set_dir_i("inputSignalColor", d, gi(r, "inputSignalColor", RED).clamp(0, 15));
            c.set_dir_i("outputSignalColor", d, gi(r, "outputSignalColor", GREEN).clamp(0, 15));
            c.set_dir_i("signalStrong", d, i32::from(gi(r, "signalStrong", 0) != 0));
        }
        _ => {
            c.set_rs_color(d, gi(r, "extractionSignalColor", RED).clamp(0, 15));
            let key = if c.def.base == Base::Energy { "pRsMode" } else { "extRM" };
            c.set_short(&format!("{key}.{dir}"), gi(r, "extractionRedstoneMode", 0).clamp(0, 3));
            if c.def.base == Base::Item || (c.def.base == Base::Fluid && c.def.tier == 2) {
                c.set_short(&format!("inSC.{dir}"), gi(r, "inputColor", GREEN).clamp(0, 15));
                c.set_short(&format!("outSC.{dir}"), gi(r, "outputColor", GREEN).clamp(0, 15));
                c.set_dir_i("selfFeed", d, i32::from(gi(r, "selfFeed", 0) != 0));
                c.set_dir_i("roundRobin", d, i32::from(gi(r, "roundRobin", 0) != 0));
                c.set_dir_i("priority", d, gi(r, "outputPriority", 0));
            }
        }
    }
}

// ItemConduitProbe.copyPasteSettings: sneaking copies the settings of every conduit with an external connection on
// the face looked at into the probe (replacing what it held), else they are pasted onto the conduits that have or
// may have one there.
fn probe_copy_paste(game: &mut Game<'_>, pos: Pos, uuid: u128, held: &ItemStack, sneaking: bool) -> bool {
    let Some(mut b) = bundle_at(game, pos) else { return false };
    let Some(hit) = ray_all(game, pos, &b, Mode::All).into_iter().next() else { return false };
    if hit.comp.kind == Ck::Core {
        return false;
    }
    let Some(d) = hit.comp.dir else { return false };
    if sneaking {
        let mut data = Compound::new();
        for c in b.iter().filter(|c| c.has_ext(d)) {
            data.insert(js(c.def.base.java()), Tag::Compound(write_settings(c, d)));
        }
        if data.is_empty() {
            return false;
        }
        si(&mut data, "probeMode", 1);
        let mut new = held.clone();
        super::enderio::set_custom_data(&mut new, data);
        sync_probe_model(&mut new);
        let i = game.selected;
        if let Some(slot) = game.inventory.get_mut(i) {
            *slot = Some(new);
        }
        let line = horizon_client::lang::get_or("enderio.gui.probe.copied", "Copied conduit settings");
        game.native_action_bar(uuid, horizon_client::text::Text::legacy(&line));
        return true;
    }
    let Some(data) = super::enderio::custom_data(held).cloned() else { return false };
    let mut done = false;
    for k in 0..b.len() {
        let Some(r) = super::enderio::gc(&data, b[k].def.base.java()).cloned() else { continue };
        if !b[k].has_ext(d) && !can_connect_external(game, pos, &b[k], d) {
            continue;
        }
        read_settings(&mut b[k], d, &r);
        done = true;
    }
    if !done {
        return false;
    }
    write_bundle(game, pos, &b);
    receptors_changed(game, pos);
    rs_dirty(game, pos);
    super::enderio::neighbour_dirty(step(pos, d));
    let k2 = key(game, pos);
    STATE.with_borrow_mut(|s| {
        s.shapes.insert(k2);
        s.dirty.insert(pos);
    });
    let line = horizon_client::lang::get_or("enderio.gui.probe.pasted", "Pasted conduit settings");
    game.native_action_bar(uuid, horizon_client::text::Text::legacy(&line));
    true
}

// BlockConduitBundle.onBlockActivated past the conduit items: the wrench's dismantle, then the components hit.
fn bundle_click(game: &mut Game<'_>, pos: Pos, uuid: u128, held: Option<&ItemStack>, sneaking: bool, canceled: &mut bool) -> bool {
    let wrench = held.is_some_and(|h| &*h.item == WRENCH);
    if wrench && sneaking {
        // handleWrenchClick: removedByPlayer, one conduit with its drops.
        *canceled = true;
        remove_by_player(game, pos, true);
        return true;
    }
    let mode = display_mode(held);
    let b = bundle_at(game, pos).unwrap_or_default();
    for hit in ray_all(game, pos, &b, mode) {
        match hit.comp.kind {
            Ck::Internal => {
                if mode.is_all() {
                    *canceled = true;
                    return true;
                }
            }
            Ck::External => {
                // The per-side settings screen (ExternalConnectionContainer): the menu opens on this face.
                let Some(d) = hit.comp.dir else { continue };
                open_face(game, pos, uuid, d);
                return true;
            }
            _ => {
                let Some(base) = hit.comp.base else { continue };
                if !mode.renders(base) {
                    continue;
                }
                if conduit_activated(game, pos, base, held, hit) {
                    *canceled = true;
                    return true;
                }
            }
        }
    }
    // Nothing of the bundle's took it: no menu (only a plate opens one); sneaking leaves it to the held item.
    if !sneaking {
        *canceled = true;
    }
    false
}

// IConduit.onBlockActivated of the conduit hit: a dye on its band, the wrench's connect / disconnect / mode.
fn conduit_activated(game: &mut Game<'_>, pos: Pos, base: Base, held: Option<&ItemStack>, hit: Hit) -> bool {
    let Some(mut b) = bundle_at(game, pos) else { return false };
    let Some(i) = find(&b, base) else { return false };
    if held.is_some_and(|h| &*h.item == PROBE) {
        return false;
    }
    let wrench = held.is_some_and(|h| &*h.item == WRENCH);
    // PowerConduit: a dye on a shown band sets the extraction signal colour.
    if base == Base::Energy {
        if let (Some(col), Some(d)) = (held.and_then(dye_of), hit.comp.dir) {
            if b[i].mode(d) != DISABLED && b[i].rs_mode(d) != RS_IGNORE {
                b[i].set_rs_color(d, col);
                write_bundle(game, pos, &b);
                receptors_changed(game, pos);
                return true;
            }
        }
    }
    if base == Base::Redstone {
        return redstone_activated(game, pos, &mut b, i, held, hit);
    }
    // AbstractTankConduit (the normal and pressurized fluid conduits): an empty bucket, or a fluid container.
    if base == Base::Fluid && b[i].def.tier < 2 && !wrench {
        if let Some(h) = held {
            if let Some(done) = tank_click(game, pos, h) {
                return done;
            }
        }
    }
    if !wrench || !matches!(base, Base::Energy | Base::Item | Base::Fluid) {
        return false;
    }
    match hit.comp.dir.filter(|_| hit.comp.kind != Ck::Core) {
        None => {
            // On the core, at the face hit: a disabled face comes back, else join the neighbour there.
            let d = hit.face;
            if b[i].mode(d) == DISABLED {
                let m = next_mode(DISABLED);
                b[i].set_mode(d, m);
                if !b[i].has_conn(d) && can_connect_external(game, pos, &b[i], d) {
                    b[i].ext |= 1 << d;
                }
                write_bundle(game, pos, &b);
                destroy_at(game, pos, base);
                STATE.with_borrow_mut(|s| s.dirty.insert(pos));
                return true;
            }
            connect(game, pos, &mut b, i, d)
        }
        Some(d) => {
            if b[i].has_ext(d) {
                let m = next_mode(b[i].mode(d));
                b[i].set_mode(d, m);
                write_bundle(game, pos, &b);
                receptors_changed(game, pos);
                // setConnectionMode notifies the block beside it (a bank looks at its receptors again).
                super::enderio::neighbour_dirty(step(pos, d));
            } else if b[i].has_conn(d) {
                disconnect(game, pos, &mut b, i, d);
            }
            true
        }
    }
}

thread_local! {
    // AbstractTankConduit.lastEmptyTick / numEmptyEvents, per conduit.
    static EMPTY_CLICKS: RefCell<HashMap<(String, Pos), (i64, u32)>> = RefCell::new(HashMap::new());
}

// AbstractTankConduit.onBlockActivated with an empty bucket or a fluid container (None for any other item): the
// bucket unlocks the network's fluid; a second within 20 ticks empties it (setFluidType(null) voids the fluid); a
// container locks it to its fluid while the network is empty, under 500 mB or of that fluid (a new type voids the
// old fluid, as Java's setFluidType does). Every member's tank and fluidLocked change; the network is made again.
fn tank_click(game: &mut Game<'_>, pos: Pos, held: &ItemStack) -> Option<bool> {
    let bucket = &*held.item == "minecraft:bucket";
    let fluid = if bucket { None } else { Some(super::enderio_machines::item_fluid(game, held).and_then(|f| f.0)?) };
    let Some(id) = net_of(game, pos, Base::Fluid) else { return Some(true) };
    let Some((members, net_fluid, total, locked)) = NETS.with_borrow(|n| n.nets.get(&id).and_then(|x| x.fluid.as_ref().map(|f| (x.members.clone(), f.fluid.clone(), f.amount, f.locked)))) else { return Some(true) };
    let uuid = game.native_acting_id();
    let dim = game.native_dimension().to_owned();
    // What each member gets: (new fluid type or None to clear, keep the amount, locked).
    let change: Option<(Option<String>, bool, bool)> = if bucket {
        let now = game.world.game_time;
        let n = EMPTY_CLICKS.with_borrow_mut(|m| {
            let e = m.entry((dim.clone(), pos)).or_insert((i64::MIN / 2, 0));
            e.1 = if now - e.0 < 20 { e.1 + 1 } else { 1 };
            e.0 = now;
            e.1
        });
        if n < 2 {
            if locked {
                EMPTY_CLICKS.with_borrow_mut(|m| m.insert((dim.clone(), pos), (now, 0)));
                let line = horizon_client::lang::get_or("enderio.item_liquid_conduit.unlocked_type", "Fluid type unlocked");
                game.native_action_bar(uuid, horizon_client::text::Text::legacy(&line));
                Some((net_fluid.clone(), true, false))
            } else {
                None
            }
        } else {
            EMPTY_CLICKS.with_borrow_mut(|m| m.insert((dim.clone(), pos), (now, 0)));
            Some((None, false, locked))
        }
    } else {
        let f = fluid?;
        if net_fluid.is_none() || total < 500 || net_fluid.as_deref() == Some(f.as_str()) {
            let name = super::enderio_machines::fluid_name(&f);
            let line = horizon_client::lang::get_or("enderio.item_liquid_conduit.locked_type", "Fluid type locked to");
            game.native_action_bar(uuid, horizon_client::text::Text::legacy(&format!("{line} {name}")));
            let same = net_fluid.as_deref() == Some(f.as_str());
            Some((Some(f), same, true))
        } else {
            None
        }
    };
    let Some((to, keep, lock)) = change else { return Some(true) };
    // The network's state goes back into its members first.
    destroy_network(game, id);
    for p in &members {
        game.native_with_mod_block_entity(*p, |e| {
            let mut b = load(&e.nbt);
            if let Some(i) = find(&b, Base::Fluid) {
                let (have, amount) = b[i].tank();
                let amount = if keep && have == to { amount } else { 0 };
                b[i].set_tank(to.as_deref(), amount);
                if to.is_some() && amount == 0 {
                    // ConduitTank.setLiquid(type with 0 mB): the type stays without fluid.
                    let mut t = Compound::new();
                    super::enderio::ss(&mut t, "FluidName", to.as_deref().unwrap_or_default());
                    si(&mut t, "Amount", 0);
                    b[i].nbt.insert(js("tank"), Tag::Compound(t));
                }
                si(&mut b[i].nbt, "fluidLocked", i32::from(lock));
                save(&b, &mut e.nbt);
            }
        });
        STATE.with_borrow_mut(|s| s.dirty.insert(*p));
    }
    Some(true)
}

// ConduitUtil.connectConduits: joined to the neighbour's conduit there if both allow it; both networks end.
fn connect(game: &mut Game<'_>, pos: Pos, b: &mut [Conduit], i: usize, d: usize) -> bool {
    let n = step(pos, d);
    let base = b[i].def.base;
    let Some(mut nb) = bundle_at(game, n) else { return false };
    let Some(j) = find(&nb, base) else { return false };
    if !(can_join(&b[i], d, &nb[j]) && can_join(&nb[j], opposite(d), &b[i])) {
        return false;
    }
    b[i].conns |= 1 << d;
    b[i].ext &= !(1 << d);
    nb[j].conns |= 1 << opposite(d);
    nb[j].ext &= !(1 << opposite(d));
    write_bundle(game, pos, b);
    write_bundle(game, n, &nb);
    destroy_at(game, pos, base);
    destroy_at(game, n, base);
    true
}

// ConduitUtil.disconnectConduits: both sides let go, both networks end.
fn disconnect(game: &mut Game<'_>, pos: Pos, b: &mut [Conduit], i: usize, d: usize) {
    let n = step(pos, d);
    let base = b[i].def.base;
    b[i].conns &= !(1 << d);
    write_bundle(game, pos, b);
    if let Some(mut nb) = bundle_at(game, n) {
        if let Some(j) = find(&nb, base) {
            nb[j].conns &= !(1 << opposite(d));
            write_bundle(game, n, &nb);
        }
    }
    destroy_at(game, pos, base);
    destroy_at(game, n, base);
    STATE.with_borrow_mut(|s| {
        s.dirty.insert(pos);
        s.dirty.insert(n);
    });
}

// ---- Breaking (BlockConduitBundle.removedByPlayer and breakConduit)

// One conduit (the one looked at) leaves with its drops (not in creative). True when the bundle is now empty.
// `wrenched`: the sneak + wrench dismantle, which takes the empty block away itself.
fn remove_by_player(game: &mut Game<'_>, pos: Pos, wrenched: bool) -> bool {
    let Some(mut b) = bundle_at(game, pos) else { return true };
    let held = game.native_acting_player().and_then(|p| game.native_held_stack(p.uuid));
    // BlockConduitBundle.removedByPlayer: a facade drawn solid goes first, the conduits stay.
    let facade = game.native_with_mod_block_entity(pos, |e| super::enderio_facades::bundle_facade(&e.nbt)).flatten();
    if let Some((t, paint)) = facade.as_ref() {
        if !super::enderio_facades::hides_facades(held.as_ref()) || b.is_empty() {
            game.native_with_mod_block_entity(pos, |e| super::enderio_facades::set_bundle_facade(&mut e.nbt, None));
            if !game.creative {
                spawn_drops(game, pos, vec![super::enderio_facades::facade_stack(*t, paint)]);
            }
            let k = key(game, pos);
            STATE.with_borrow_mut(|s| {
                s.shapes.insert(k);
                s.dirty.insert(pos);
            });
            game.native_block_entity_sync(pos);
            let empty = b.is_empty();
            if empty && wrenched {
                let k = key(game, pos);
                STATE.with_borrow_mut(|s| s.removals.push(k));
            }
            return empty;
        }
    }
    let mode = display_mode(held.as_ref());
    let mut drops = Vec::new();
    let mut hits: Vec<Option<Base>> = ray_all(game, pos, &b, mode).into_iter().map(|h| h.comp.base).collect();
    if hits.is_empty() {
        // The block's shape is wider than the components: a break that meets none acts on the connectors.
        hits.push(None);
    }
    for hit in hits {
        match hit {
            Some(t) => {
                if !mode.renders(t) {
                    continue;
                }
                if let Some(i) = find(&b, t) {
                    let c = remove_conduit(game, pos, &mut b, i);
                    drops.extend(c.drops());
                }
            }
            None => {
                // A connector: the unconnected conduits, or (none) every one shown.
                let loose: Vec<usize> = (0..b.len()).filter(|&i| !b[i].has_connections() && mode.renders(b[i].def.base)).collect();
                let take: Vec<usize> = if loose.is_empty() { (0..b.len()).filter(|&i| mode.renders(b[i].def.base)).collect() } else { loose };
                for i in take.into_iter().rev() {
                    let c = remove_conduit(game, pos, &mut b, i);
                    drops.extend(c.drops());
                }
            }
        }
        break;
    }
    write_bundle(game, pos, &b);
    if !game.creative {
        spawn_drops(game, pos, drops);
    }
    let empty = b.is_empty();
    if empty && wrenched {
        let k = key(game, pos);
        STATE.with_borrow_mut(|s| s.removals.push(k));
    }
    empty
}

// `destroy_block`: a player's break takes one conduit; the block stays while any is left.
pub(in crate::game) fn destroy_block(game: &mut Game<'_>, pos: Pos, canceled: &mut bool) {
    if !is_bundle(game, pos) || *canceled {
        return;
    }
    if !remove_by_player(game, pos, false) {
        *canceled = true;
    }
}

// getDrops (explosions and other breaks than a player's): every conduit's drops.
pub(in crate::game) fn block_drops(game: &mut Game<'_>, pos: Pos, drops: &mut Vec<ItemStack>) {
    let b = match game.world.block_entities.get(&(pos[0], pos[1], pos[2])) {
        Some(BlockEntity::Mod(e)) if e.id == BUNDLE => load(&e.nbt),
        _ => match STATE.with_borrow_mut(|s| s.removed.remove(&pos)) {
            Some(e) if e.id == BUNDLE => load(&e.nbt),
            _ => Vec::new(),
        },
    };
    drops.retain(|d| &*d.item != BUNDLE);
    let facade = match game.world.block_entities.get(&(pos[0], pos[1], pos[2])) {
        Some(BlockEntity::Mod(e)) if e.id == BUNDLE => super::enderio_facades::bundle_facade(&e.nbt),
        _ => STATE.with_borrow(|s| s.removed.get(&pos).and_then(|e| super::enderio_facades::bundle_facade(&e.nbt))),
    };
    if let Some((t, paint)) = facade {
        drops.push(super::enderio_facades::facade_stack(t, &paint));
    }
    for c in &b {
        drops.extend(c.drops());
    }
}

// `remove_block_entity`: the bundle as it was, for the drops asked after; its networks end.
pub(in crate::game) fn block_entity_removed(level: &mut super::GameLevel<'_>, pos: Pos) {
    if let Some(BlockEntity::Mod(m)) = level.world.block_entities.get(&(pos[0], pos[1], pos[2])) {
        if m.id == BUNDLE {
            let copy = ModBlockEntity::clone(m);
            STATE.with_borrow_mut(|s| s.removed.insert(pos, copy));
        }
    }
    // The settings slots only mirror the conduits' upgrades (they drop with the conduit): none spill.
    if let Some(BlockEntity::Mod(m)) = level.world.block_entities.get_mut(&(pos[0], pos[1], pos[2])) {
        if m.id == BUNDLE {
            for s in m.items.iter_mut() {
                *s = None;
            }
            OPEN.with_borrow_mut(|o| o.remove(&pos));
        }
    }
}

// ---- Energy through the bundle's faces (PowerConduit.ConnectionSide)

// native_energy: Some(None) for a face with no energy conduit's external connection.
pub(in crate::game) fn energy_view(game: &Game<'_>, pos: Pos, face: Direction) -> Option<(i32, i32, i32, i32, bool, bool)> {
    let BlockEntity::Mod(e) = game.world.block_entities.get(&(pos[0], pos[1], pos[2]))? else { return None };
    let b = load(&e.nbt);
    let c = &b[find(&b, Base::Energy)?];
    let d = face as usize;
    if !c.has_ext(d) {
        return None;
    }
    let max = ENERGY_MAX_IO[c.def.tier.min(2)];
    let stored = net_of(game, pos, Base::Energy)
        .and_then(|id| NETS.with_borrow(|n| n.nets.get(&id).and_then(|x| x.energy.as_ref()).and_then(|en| en.stored.get(&pos).copied())))
        .unwrap_or(c.energy());
    // isReceive: IN_OUT or INPUT, the redstone condition (read when it was last looked at).
    let mode = c.mode(d);
    let rs = c.rs_mode(d);
    let receive = accepts_input(mode) && rs != RS_NEVER;
    Some((stored, max, if receive { (max - stored).max(0) } else { 0 }, 0, receive, false))
}

// native_energy_transfer: in through an external face into the conduit's buffer (never out).
pub(in crate::game) fn energy_transfer(game: &mut Game<'_>, pos: Pos, face: Direction, amount: i32, insert: bool, simulate: bool) -> Option<i32> {
    let b = bundle_at(game, pos)?;
    let i = find(&b, Base::Energy)?;
    let d = face as usize;
    if !b[i].has_ext(d) || !insert || amount <= 0 {
        return Some(0);
    }
    let c = &b[i];
    let mode = c.mode(d);
    if !accepts_input(mode) || !rs_met(game, pos, c.rs_mode(d), c.rs_color(d)) {
        return Some(0);
    }
    let max = ENERGY_MAX_IO[c.def.tier.min(2)];
    if let Some(id) = net_of(game, pos, Base::Energy) {
        let got = NETS.with_borrow_mut(|n| {
            let en = n.nets.get_mut(&id)?.energy.as_mut()?;
            let have = en.stored.entry(pos).or_insert(0);
            let take = amount.min(max - *have).max(0);
            if !simulate {
                *have += take;
            }
            Some(take)
        });
        if let Some(g) = got {
            return Some(g);
        }
    }
    let have = c.energy();
    let take = amount.min(max - have).max(0);
    if !simulate && take > 0 {
        game.native_with_mod_block_entity(pos, |e| set_conduit_energy(&mut e.nbt, have + take));
    }
    Some(take)
}

// Whether the bundle at `pos` has an energy conduit facing `d` (towards a bank) set IN_OUT: a bank's NONE face then
// leaves it to the conduit network (CapBankNetwork.sendPowerTo).
pub(in crate::game) fn energy_in_out_toward(game: &Game<'_>, pos: Pos, d: usize) -> bool {
    bundle_at(game, pos).and_then(|b| find(&b, Base::Energy).map(|i| b[i].mode(d) == IN_OUT)).unwrap_or(false)
}

// ---- The network tick (PowerConduitNetwork.tickEnd → NetworkPowerManager.doApplyRecievedPower)

struct BankEntry {
    pos: Pos,
    can_extract: i32,
    can_fill: i32,
    to_balance: i32,
    in_out: bool,
    creative: bool,
    stored: i64,
    max: i64,
}

fn rebuild_receptors(game: &mut Game<'_>, id: u32) {
    let Some(members) = NETS.with_borrow(|n| n.nets.get(&id).map(|x| x.members.clone())) else { return };
    let mut rec = Vec::new();
    let mut storage = Vec::new();
    for p in members {
        let Some(b) = bundle_at(game, p) else { continue };
        let Some(i) = find(&b, Base::Energy) else { continue };
        let c = &b[i];
        for d in (0..6).filter(|d| c.has_ext(*d)) {
            let n = step(p, d);
            let r = Receptor { pos: p, d, mode: c.mode(d), rs: c.rs_mode(d), color: c.rs_color(d) };
            if super::enderio::is_bank(&game.native_block(n)) {
                storage.push(r);
            } else if game.native_energy(game.native_dimension(), n, FACES[opposite(d)]).is_some() {
                rec.push(r);
            }
        }
    }
    NETS.with_borrow_mut(|n| {
        if let Some(en) = n.nets.get_mut(&id).and_then(|x| x.energy.as_mut()) {
            en.receptors = rec;
            en.storage = storage;
            en.at = 0;
            en.receptors_dirty = false;
        }
    });
}

// getMaxEnergyExtracted / getMaxEnergyRecieved of a receptor's face.
fn max_extracted(game: &mut Game<'_>, r: &Receptor, max_io: i32) -> i32 {
    if r.mode != INPUT && r.mode != DISABLED && rs_met(game, r.pos, r.rs, r.color) { max_io } else { 0 }
}

fn max_received(game: &mut Game<'_>, r: &Receptor, max_io: i32) -> i32 {
    if r.mode != OUTPUT && r.mode != DISABLED && rs_met(game, r.pos, r.rs, r.color) { max_io } else { 0 }
}

fn energy_tick(game: &mut Game<'_>, id: u32) {
    let dirty = NETS.with_borrow(|n| n.nets.get(&id).and_then(|x| x.energy.as_ref()).is_some_and(|e| e.receptors_dirty));
    if dirty {
        rebuild_receptors(game, id);
    }
    // updateNetworkStorage
    let Some((max_io, members, receptors, storage)) = NETS.with_borrow(|n| {
        let x = n.nets.get(&id)?;
        let en = x.energy.as_ref()?;
        let r: Vec<Receptor> = en.receptors.iter().map(|r| Receptor { pos: r.pos, d: r.d, mode: r.mode, rs: r.rs, color: r.color }).collect();
        let s: Vec<Receptor> = en.storage.iter().map(|r| Receptor { pos: r.pos, d: r.d, mode: r.mode, rs: r.rs, color: r.color }).collect();
        Some((en.max_io, x.members.clone(), r, s))
    }) else { return };
    let max = i64::from(max_io) * members.len() as i64;
    let mut stored: i64 = NETS.with_borrow(|n| n.nets.get(&id).and_then(|x| x.energy.as_ref()).map_or(0, |en| en.stored.values().map(|v| i64::from(*v)).sum()));
    stored = stored.clamp(0, max);
    let tick_start = stored;
    // capSupply.init: each bank face the network controls (NONE, PULL), what it can give and take.
    let mut banks: Vec<BankEntry> = Vec::new();
    let mut seen_nets: HashSet<u32> = HashSet::new();
    let (mut bank_stored, mut bank_max) = (0i64, 0i64);
    let (mut can_extract, mut can_fill) = (0i32, 0i32);
    let (mut to_balance, mut max_to_balance) = (0f64, 0f64);
    for r in &storage {
        let there = step(r.pos, r.d);
        let Some(v) = super::enderio::bank_view(game, there, opposite(r.d)) else { continue };
        if seen_nets.insert(v.net) {
            bank_stored += v.stored;
            bank_max += v.max;
        }
        let in_out = r.mode == IN_OUT;
        if in_out {
            to_balance += v.stored as f64;
            max_to_balance += v.max as f64;
        }
        if v.network_io {
            let mut get = 0i64;
            let mut fill = 0i64;
            if v.output_ok {
                get = v.stored.min(i64::from(v.max_out)).min(i64::from(max_received(game, r, max_io)));
                can_extract += get as i32;
            }
            if v.input_ok {
                fill = (v.max - v.stored).min(i64::from(v.max_in)).min(i64::from(max_extracted(game, r, max_io)));
                can_fill += fill as i32;
            }
            banks.push(BankEntry { pos: there, can_extract: get as i32, can_fill: fill as i32, to_balance: 0, in_out, creative: v.creative, stored: v.stored, max: v.max });
        }
    }
    let filled_ratio = if max_to_balance > 0.0 { to_balance / max_to_balance } else { 0.0 };
    let n = receptors.len();
    let mut available = stored + i64::from(can_extract);
    let was = available;
    let mut sent_total = 0i64;
    let mut recv_from_banks = 0i64;
    if available > 0 && (!receptors.is_empty() || !storage.is_empty()) {
        let mut at = NETS.with_borrow(|x| x.nets.get(&id).and_then(|x| x.energy.as_ref()).map_or(0, |e| e.at));
        for _ in 0..n {
            if at >= n {
                at = 0;
            }
            let r = &receptors[at];
            at += 1;
            let offer = i64::from(max_extracted(game, r, max_io)).min(available) as i32;
            let there = step(r.pos, r.d);
            let used = game.native_energy_insert(there, FACES[opposite(r.d)], offer, false).max(0);
            sent_total += i64::from(used);
            available -= i64::from(used);
            if available <= 0 {
                break;
            }
        }
        NETS.with_borrow_mut(|x| {
            if let Some(en) = x.nets.get_mut(&id).and_then(|x| x.energy.as_mut()) {
                en.at = at;
            }
        });
        let used = was - available;
        stored -= used;
        if !banks.is_empty() {
            let mut change = 0i64;
            if stored < 0 {
                change = stored;
                stored = 0;
            } else if stored > 0 {
                change = stored.min(i64::from(can_fill));
                stored -= change;
            }
            if change < 0 {
                // capSupply.remove: from each bank by its share, rounded up.
                let mut amount = -change;
                if can_extract > 0 {
                    let ratio = amount as f64 / f64::from(can_extract);
                    for b in &banks {
                        let use_ = ((ratio * f64::from(b.can_extract)).ceil() as i64).min(amount).min(i64::from(b.can_extract));
                        super::enderio::bank_add(game, b.pos, -use_);
                        recv_from_banks += use_;
                        amount -= use_;
                        if amount == 0 {
                            break;
                        }
                    }
                }
            } else if change > 0 {
                // capSupply.add: into each bank by its share; what they don't take stays in the conduits.
                let mut amount = change;
                if can_fill > 0 {
                    let ratio = amount as f64 / f64::from(can_fill);
                    for b in &banks {
                        let mut add = ((ratio * f64::from(b.can_fill)).ceil() as i64).min(i64::from(b.can_fill)).min(amount);
                        add -= super::enderio::bank_add(game, b.pos, add);
                        amount -= add;
                        if amount == 0 {
                            break;
                        }
                    }
                }
                stored += amount;
            }
            // capSupply.balance, with two entries or more: from the fuller IN_OUT banks to the emptier.
            if banks.len() >= 2 {
                let mut entries: Vec<BankEntry> = Vec::new();
                for b in &banks {
                    if let Some(v) = super::enderio::bank_view(game, b.pos, 0) {
                        entries.push(BankEntry { stored: v.stored, max: v.max, ..BankEntry { ..*b } });
                    }
                }
                let (mut remove, mut add) = (0i32, 0i32);
                for e in entries.iter_mut().filter(|e| e.in_out) {
                    e.to_balance = if e.creative {
                        0
                    } else if (e.max as f64 * filled_ratio).floor() as i64 - e.stored < 0 {
                        -e.can_extract
                    } else {
                        e.can_fill
                    };
                    if e.to_balance < 0 { remove += -e.to_balance } else { add += e.to_balance }
                }
                let mut total = add.min(remove);
                for i in 0..entries.len() {
                    if total <= 0 {
                        break;
                    }
                    if !entries[i].in_out {
                        continue;
                    }
                    let amount = entries[i].to_balance;
                    let amount = if amount < 0 { amount.max(-total) } else { amount.min(total) };
                    super::enderio::bank_add(game, entries[i].pos, i64::from(amount));
                    total -= amount.abs();
                    let mut moving = amount.abs();
                    for j in i + 1..entries.len() {
                        if moving <= 0 {
                            break;
                        }
                        if (amount as f32).signum() != (entries[j].to_balance as f32).signum() {
                            let to = moving.min(entries[j].to_balance.abs());
                            super::enderio::bank_add(game, entries[j].pos, i64::from(to * entries[j].to_balance.signum()));
                            moving -= to;
                        }
                    }
                }
            }
        }
        // distributeStorageToConduits: each conduit ceil(maxIO x ratio), in network order, until it's gone.
        stored = stored.clamp(0, max);
        let ratio = if max > 0 { stored as f32 / max as f32 } else { 0.0 };
        let mut left = stored;
        let shares: Vec<(Pos, i32)> = members
            .iter()
            .map(|p| {
                if left > 0 {
                    let give = ((max_io as f32 * ratio).ceil() as i64).min(i64::from(max_io)).min(left) as i32;
                    left -= i64::from(give);
                    (*p, give)
                } else {
                    (*p, 0)
                }
            })
            .collect();
        NETS.with_borrow_mut(|x| {
            if let Some(en) = x.nets.get_mut(&id).and_then(|x| x.energy.as_mut()) {
                en.stored = shares.into_iter().collect();
            }
        });
    }
    let _ = recv_from_banks;
    NETS.with_borrow_mut(|x| {
        if let Some(en) = x.nets.get_mut(&id).and_then(|x| x.energy.as_mut()) {
            // networkPowerTracker: sent to receptors; received = the conduits' gain since the last tick end.
            let recv = (tick_start - en.energy).max(0);
            en.tracker.tick(sent_total, recv);
            en.energy = stored;
            en.max = max;
            en.in_banks = bank_stored;
            en.max_banks = bank_max;
        }
    });
}

// `level_tick_post`: each network's tickEnd; members gone (a chunk unloaded, the block changed) end it; the bundles'
// shapes follow their arms.
pub(in crate::game) fn level_tick(game: &mut Game<'_>, dimension: &str) {
    if !game.native_on(NS) {
        return;
    }
    STATE.with_borrow_mut(|s| s.removed.clear());
    let ids: Vec<(u32, Base)> = NETS.with_borrow(|n| n.nets.iter().filter(|(_, x)| x.dim == dimension).map(|(id, x)| (*id, x.base)).collect());
    for (id, base) in ids {
        let members = NETS.with_borrow(|n| n.nets.get(&id).map(|x| x.members.clone())).unwrap_or_default();
        let gone = members.iter().any(|p| !game.world.chunks.contains_key(&(p[0].div_euclid(16), p[2].div_euclid(16))) || !is_bundle(game, *p));
        if gone {
            destroy_network(game, id);
            continue;
        }
        if base == Base::Energy {
            energy_tick(game, id);
        } else if base == Base::Item {
            item_tick(game, id);
        } else if base == Base::Fluid {
            fluid_tick(game, id);
        } else {
            redstone_tick(game, id);
        }
    }
    RS_DIRTY.with_borrow_mut(|s| s.clear());
    let (shapes, removals): (Vec<Pos>, Vec<Pos>) = STATE.with_borrow_mut(|s| {
        let (now, later): (Vec<_>, Vec<_>) = s.shapes.drain().partition(|(d, _)| d == dimension);
        s.shapes = later.into_iter().collect();
        let (rn, rl): (Vec<_>, Vec<_>) = s.removals.drain(..).partition(|(d, _)| d == dimension);
        s.removals = rl;
        (now.into_iter().map(|(_, p)| p).collect(), rn.into_iter().map(|(_, p)| p).collect())
    });
    for p in shapes {
        let Some(b) = bundle_at(game, p) else { continue };
        let m = arm_mask(&b);
        for d in 0..6 {
            game.native_set_block_property(p, PROP_NAMES[d], if m & (1 << d) != 0 { "true" } else { "false" });
        }
        // A facade makes the bundle a full block (its collision and selection).
        let faced = game.native_with_mod_block_entity(p, |e| super::enderio_facades::bundle_facade(&e.nbt).is_some()).unwrap_or(false);
        game.native_set_block_property(p, "opaque", if faced { "true" } else { "false" });
    }
    if let Some(air) = game.native_default_state("minecraft:air") {
        for p in removals {
            let faced = game.native_with_mod_block_entity(p, |e| super::enderio_facades::bundle_facade(&e.nbt).is_some()).unwrap_or(false);
            if !faced && bundle_at(game, p).is_some_and(|b| b.is_empty()) {
                game.native_set_block(dimension, p, air, 3);
            }
        }
    }
}

// ---- Drawing (ConduitBundleRenderer.addConduitQuads, DefaultConduitRenderer, ConduitInOutRenderer)

#[derive(Clone, Copy)]
struct Vtx {
    p: [f32; 3],
    uv: [f32; 2],
}

#[derive(Clone)]
struct Quad {
    sprite: &'static str,
    corners: [[f32; 3]; 4],
    uv: [f32; 4],
    tint: u32,
}

// A quad of four corners with their own UVs (0..1 of the sprite), drawn in the order Horizon's quads take (the two
// first corners share their u); the winding is kept.
fn push_quad(out: &mut Vec<Quad>, sprite: &'static str, v: [Vtx; 4], tint: u32) {
    let same_u = |a: Vtx, b: Vtx| (a.uv[0] - b.uv[0]).abs() < 1e-6;
    let c = if same_u(v[0], v[1]) { v } else { [v[1], v[2], v[3], v[0]] };
    out.push(Quad { sprite, corners: [c[0].p, c[1].p, c[2].p, c[3].p], uv: [c[0].uv[0] * 16.0, c[0].uv[1] * 16.0, c[2].uv[0] * 16.0, c[1].uv[1] * 16.0], tint });
}

// BoundingBox.getCornersWithUvForFace.
fn face_corners(b: Bb, face: usize, min_u: f32, max_u: f32, min_v: f32, max_v: f32) -> [Vtx; 4] {
    let (x0, y0, z0, x1, y1, z1) = (b[0], b[1], b[2], b[3], b[4], b[5]);
    let v = |p: [f32; 3], u: f32, vv: f32| Vtx { p, uv: [u, vv] };
    match face {
        2 => [v([x1, y0, z0], min_u, min_v), v([x0, y0, z0], max_u, min_v), v([x0, y1, z0], max_u, max_v), v([x1, y1, z0], min_u, max_v)],
        3 => [v([x0, y0, z1], max_u, min_v), v([x1, y0, z1], min_u, min_v), v([x1, y1, z1], min_u, max_v), v([x0, y1, z1], max_u, max_v)],
        5 => [v([x1, y1, z0], max_u, max_v), v([x1, y1, z1], min_u, max_v), v([x1, y0, z1], min_u, min_v), v([x1, y0, z0], max_u, min_v)],
        4 => [v([x0, y0, z0], max_u, min_v), v([x0, y0, z1], min_u, min_v), v([x0, y1, z1], min_u, max_v), v([x0, y1, z0], max_u, max_v)],
        1 => [v([x1, y1, z1], min_u, min_v), v([x1, y1, z0], min_u, max_v), v([x0, y1, z0], max_u, max_v), v([x0, y1, z1], max_u, min_v)],
        _ => [v([x0, y0, z0], max_u, max_v), v([x1, y0, z0], min_u, max_v), v([x1, y0, z1], min_u, min_v), v([x0, y0, z1], max_u, min_v)],
    }
}

// RenderUtil.addBakedQuadForFace with uvs (x, y, z, w = minU, minV, maxU, maxV), rotateUV and flipU.
fn face_quad(out: &mut Vec<Quad>, b: Bb, sprite: &'static str, face: usize, uvs: [f32; 4], rotate: bool, flip: bool, tint: u32) {
    let mut c = face_corners(b, face, uvs[0], uvs[2], uvs[1], uvs[3]);
    if rotate {
        let first = c[0].uv;
        c[0].uv = c[1].uv;
        c[1].uv = c[2].uv;
        c[2].uv = c[3].uv;
        c[3].uv = first;
    }
    if flip {
        for v in c.iter_mut() {
            v.uv[0] = uvs[2] - v.uv[0];
        }
    }
    push_quad(out, sprite, c, tint);
}

fn cube(out: &mut Vec<Quad>, b: Bb, sprite: &'static str, uvs: [f32; 4], tint: u32) {
    for f in 0..6 {
        face_quad(out, b, sprite, f, uvs, false, false, tint);
    }
}

const FULL: [f32; 4] = [0.0, 0.0, 1.0, 1.0];

// ConduitTexture.arm(i) and core(i).
fn arm_uv(i: usize) -> [f32; 4] {
    [0.0, 4.0 * i as f32 / 16.0, 0.8125, 4.0 * (i + 1) as f32 / 16.0]
}

fn core_uv(i: Option<usize>) -> [f32; 4] {
    match i {
        Some(0) => [0.0, 0.5, 0.5, 0.0],
        Some(1) => [0.5, 0.5, 1.0, 0.0],
        Some(2) => [0.0, 1.0, 0.5, 0.5],
        Some(3) => [0.5, 1.0, 1.0, 0.5],
        _ => [0.125, 0.875, 0.875, 0.125],
    }
}

// getTextureForState: the core, the arm and the band of each type.
fn core_texture(c: &Conduit) -> (&'static str, [f32; 4]) {
    match c.def.base {
        Base::Energy => ("enderio:block/conduit_core_0", core_uv(Some(c.def.tier))),
        Base::Item => ("enderio:block/item_conduit_core", core_uv(None)),
        Base::Fluid => ("enderio:block/conduit_core_1", core_uv(Some(c.def.tier))),
        Base::Redstone => ("enderio:block/conduit_core_1", core_uv(Some(3))),
    }
}

fn arm_texture_dir(c: &Conduit, d: usize) -> (&'static str, [f32; 4]) {
    if c.def.base == Base::Fluid {
        let has = c.tank().0.is_some();
        match c.def.tier {
            0 => {
                return match c.mode(d) {
                    INPUT => (if has { "enderio:block/liquid_conduit_extract" } else { "enderio:block/empty_liquid_conduit_extract" }, FULL),
                    OUTPUT => (if has { "enderio:block/liquid_conduit_insert" } else { "enderio:block/empty_liquid_conduit_insert" }, FULL),
                    _ if c.fluid_locked() => ("enderio:block/liquid_conduit_locked", FULL),
                    _ => ("enderio:block/liquid_conduit", arm_uv(0)),
                };
            }
            1 if c.fluid_locked() => return ("enderio:block/liquid_conduit", arm_uv(2)),
            _ => {}
        }
    }
    arm_texture(c)
}

fn arm_texture(c: &Conduit) -> (&'static str, [f32; 4]) {
    match c.def.base {
        Base::Energy => ("enderio:block/power_conduit", arm_uv(c.def.tier)),
        Base::Item => ("enderio:block/conduit", arm_uv(0)),
        Base::Fluid => ("enderio:block/liquid_conduit", arm_uv([0, 1, 3][c.def.tier.min(2)])),
        Base::Redstone => ("enderio:block/conduit", arm_uv(1)),
    }
}

// The transmission inside the arm: the item conduit's ender, the redstone conduit's signal line.
fn transmission_texture(c: &Conduit) -> Option<(&'static str, [f32; 4])> {
    match c.def.base {
        Base::Item => Some(("enderio:block/ender_still", [0.09375, 0.375, 0.90625, 0.625])),
        Base::Redstone => Some(("enderio:block/conduit", arm_uv(3))),
        _ => None,
    }
}

// DefaultConduitRenderer.addQuadsForSection: the four sides of an arm, turned as Java turns them.
fn section(out: &mut Vec<Quad>, b: Bb, sprite: &'static str, uvs: [f32; 4], dir: usize, tint: u32) {
    let rotate_sides = dir == 1 || dir == 0;
    let rotate_tb = dir == 0 || dir == 5 || dir == 3;
    for face in 0..6 {
        if face == dir || face == opposite(dir) {
            continue;
        }
        let mut rs = rotate_sides;
        let mut rt = rotate_tb;
        if face == 1 || face == 0 {
            rs = dir == 3 || dir == 2;
        }
        // dir.getAxis().isHorizontal() with the north or east face.
        if dir >= 2 && (face == 2 || face == 5) {
            rt = !rotate_tb;
        }
        if (dir == 2 || dir == 3) && face == 0 {
            rt = !rt;
        }
        face_quad(out, b, sprite, face, uvs, rs, rt, tint);
    }
}

fn shrunk(comp: &Comp) -> Bb {
    let v = dir_vec(comp.dir.unwrap_or(0));
    let s = 0.031_25;
    expand(comp.bb, [if v[0] != 0.0 { 0.0 } else { -s }, if v[1] != 0.0 { 0.0 } else { -s }, if v[2] != 0.0 { 0.0 } else { -s }])
}

// Quaternion rotation as EnderCore's Quat4d.makeRotate / rotate.
fn rotate(p: [f32; 3], angle: f64, axis: [f64; 3]) -> [f32; 3] {
    let (s, c) = ((0.5 * angle).sin(), (0.5 * angle).cos());
    let (qx, qy, qz, qw) = (axis[0] * s, axis[1] * s, axis[2] * s, c);
    let v = [f64::from(p[0]) - 0.5, f64::from(p[1]) - 0.5, f64::from(p[2]) - 0.5];
    let d = -qx * v[0] - qy * v[1] - qz * v[2];
    let d1 = qw * v[0] + qy * v[2] - qz * v[1];
    let d2 = qw * v[1] - qx * v[2] + qz * v[0];
    let d3 = qw * v[2] + qx * v[1] - qy * v[0];
    let x = d1 * qw - d * qx - d2 * qz + d3 * qy;
    let y = d2 * qw - d * qy + d1 * qz - d3 * qx;
    let z = d3 * qw - d * qz - d1 * qy + d2 * qx;
    [(x + 0.5) as f32, (y + 0.5) as f32, (z + 0.5) as f32]
}

// ConduitGeometryUtil.IO_RING_VERTS for `dir`, moved by `off` (addModeConnectorQuads); `grow` lifts an icon over its
// background (both are one box in Java, drawn in order).
fn io_ring(out: &mut Vec<Quad>, dir: usize, off: [f32; 3], sprite: &'static str, tint: u32, grow: f32) {
    let r = scale(scale(core_bb(), [0.9; 3]), [0.9, 1.0, 1.0]);
    let r = scale(r, [grow; 3]);
    let (angle, axis): (f64, [f64; 3]) = match dir {
        3 => (std::f64::consts::FRAC_PI_2, [0.0, 1.0, 0.0]),
        2 => (std::f64::consts::PI * 3.0 / 2.0, [0.0, 1.0, 0.0]),
        5 => (std::f64::consts::PI, [0.0, 1.0, 0.0]),
        4 => (0.0, [0.0, 1.0, 0.0]),
        1 => (-std::f64::consts::FRAC_PI_2, [0.0, 0.0, 1.0]),
        _ => (std::f64::consts::FRAC_PI_2, [0.0, 0.0, 1.0]),
    };
    let o = f64::from(HWIDTH) * 0.9 * 0.9 + 0.05;
    let v = dir_vec(dir);
    let m = (0.5 - o) as f32;
    let t = [v[0] * m + off[0], v[1] * m + off[1], v[2] * m + off[2]];
    for face in 0..6 {
        let mut c = face_corners(r, face, 0.0, 1.0, 0.0, 1.0);
        for x in c.iter_mut() {
            let p = rotate(x.p, angle, axis);
            x.p = [p[0] + t[0], p[1] + t[1], p[2] + t[2]];
        }
        push_quad(out, sprite, c, tint);
    }
}

// ConduitInOutRenderer.renderIO (energy and item conduits): the band when extraction waits on redstone, the ring by
// the connection's mode.
fn render_io(out: &mut Vec<Quad>, b: &[Conduit], c: &Conduit, d: usize, in_col: i32, out_col: i32) {
    if !c.has_ext(d) {
        return;
    }
    let mode = c.mode(d);
    if !(accepts_input(mode) || accepts_output(mode)) {
        return;
    }
    let off = offset(b, c.def.base, Some(d));
    // addColorBand: the extraction signal colour on the band, 0.12 back from the face.
    let rs = c.rs_mode(d);
    if has_controller(c) && rs != RS_IGNORE && rs != RS_NEVER && accepts_input(mode) {
        let v = dir_vec(d);
        let bb = translate(controller_bb(d, off), [v[0] * -0.12, v[1] * -0.12, v[2] * -0.12]);
        for f in (0..6).filter(|f| *f != d) {
            face_quad(out, bb, "enderio:block/white", f, FULL, false, false, DYE_COLORS[c.rs_color(d) as usize]);
        }
    }
    let mut m = mode;
    if c.def.base == Base::Redstone {
        m = match m {
            INPUT => OUTPUT,
            OUTPUT => INPUT,
            x => x,
        };
    }
    io_ring(out, d, off, "enderio:block/item_conduit_io_connector", 0xFF_FFFF, 1.0);
    if accepts_input(m) {
        if accepts_output(m) {
            io_ring(out, d, off, "enderio:block/item_conduit_in_out_in", DYE_COLORS[in_col as usize], 1.002);
            io_ring(out, d, off, "enderio:block/item_conduit_in_out_out", DYE_COLORS[out_col as usize], 1.004);
        } else {
            io_ring(out, d, off, "enderio:block/item_conduit_input", DYE_COLORS[in_col as usize], 1.002);
        }
    } else if accepts_output(m) {
        io_ring(out, d, off, "enderio:block/item_conduit_output", DYE_COLORS[out_col as usize], 1.002);
    }
}

// The wire frame of a hidden component (addWireBounds).
fn wire(comp: &Comp) -> Bb {
    match comp.dir.filter(|_| comp.kind != Ck::Core) {
        Some(d) => {
            let v = dir_vec(d);
            scale(comp.bb, [if v[0] != 0.0 { 1.0 } else { 0.7 }, if v[1] != 0.0 { 1.0 } else { 0.7 }, if v[2] != 0.0 { 1.0 } else { 0.7 }])
        }
        None => comp.bb,
    }
}

fn bundle_quads(b: &[Conduit], mode: Mode) -> Vec<Quad> {
    let mut out = Vec::new();
    let mut externals = [false; 6];
    let mut wires: Vec<Bb> = Vec::new();
    for c in b {
        let comps = conduit_comps(b, c);
        if mode.renders(c.def.base) {
            for comp in comps.iter().filter(|x| x.kind == Ck::Arm) {
                let d = comp.dir.unwrap_or(0);
                let (arm_sprite, arm) = arm_texture_dir(c, d);
                let bb = shrunk(comp);
                // The fluid inside a normal or pressurized conduit, its width by how full it is.
                if let (Base::Fluid, 0 | 1) = (c.def.base, c.def.tier) {
                    let (f, a) = c.tank();
                    if let Some(f) = f.filter(|_| a > 0) {
                        let cap = if c.def.tier == 1 { 1000 } else { c.tank_capacity().max(1) };
                        let r = (a as f32 / cap as f32).clamp(0.05, 1.0) * 0.6;
                        let v = dir_vec(d);
                        let inner = scale(bb, [if v[0] != 0.0 { 0.998 } else { r }, if v[1] != 0.0 { 0.998 } else { r }, if v[2] != 0.0 { 0.998 } else { r }]);
                        let (sprite, tint) = fluid_sprite(&f);
                        section(&mut out, inner, sprite, FULL, d, tint);
                    }
                }
                if let Some((t, uv)) = transmission_texture(c) {
                    // Drawn first, a hair inside the arm (Java draws both on one box).
                    section(&mut out, scale(bb, [0.998; 3]), t, uv, d, 0xFF_FFFF);
                }
                section(&mut out, bb, arm_sprite, arm, d, 0xFF_FFFF);
                if c.def.base == Base::Redstone {
                    render_io(&mut out, b, c, d, c.rs_input_color(d), c.rs_output_color(d));
                }
                if matches!(c.def.base, Base::Energy | Base::Item) {
                    let (i, o) = if c.def.base == Base::Item { (gi(&c.nbt, &format!("inSC.{}", DIR_NAMES[d]), 2), gi(&c.nbt, &format!("outSC.{}", DIR_NAMES[d]), 2)) } else { (RED, RED) };
                    render_io(&mut out, b, c, d, i.clamp(0, 15), o.clamp(0, 15));
                }
            }
            for d in 0..6 {
                if c.has_ext(d) && c.mode(d) != DISABLED && c.mode(d) != NOT_SET {
                    externals[d] = true;
                }
            }
        } else {
            wires.extend(comps.iter().map(wire));
        }
    }
    let conns = connectors(b);
    let hub = if mode.is_all() { conns.iter().find(|x| x.kind == Ck::Internal).copied() } else { None };
    for comp in &conns {
        if let Some(h) = hub {
            if comp.kind != Ck::Internal && contains(h.bb, comp.bb) {
                continue;
            }
        }
        match comp.base {
            Some(t) => {
                let Some(c) = b.iter().find(|c| c.def.base == t) else { continue };
                if mode.renders(t) {
                    let (s, uv) = core_texture(c);
                    cube(&mut out, comp.bb, s, uv, 0xFF_FFFF);
                } else {
                    wires.push(wire(comp));
                }
            }
            None => {
                if mode.is_all() {
                    cube(&mut out, comp.bb, "enderio:block/conduit_connector", FULL, 0xFF_FFFF);
                }
            }
        }
    }
    for w in wires {
        cube(&mut out, w, "enderio:block/wire_frame", FULL, 0xFF_FFFF);
    }
    for d in (0..6).filter(|d| externals[*d]) {
        for bb in external_bbs(d) {
            cube(&mut out, bb, "enderio:block/conduit_connector", FULL, 0xFF_FFFF);
        }
    }
    if b.is_empty() {
        let unit = [0.0, 0.0, 0.0, 1.0, 1.0, 1.0];
        for s in [0.1, 0.15, 0.2, 0.25] {
            cube(&mut out, scale(unit, [s; 3]), "enderio:block/wire_frame", FULL, 0xFF_FFFF);
        }
    }
    out
}

thread_local! {
    // What each bundle drew, by its block entity's revision and the display mode it was drawn in.
    static DRAWN: RefCell<HashMap<Pos, (horizon_client::containers::ModRevision, Mode, Vec<Quad>)>> = RefCell::new(HashMap::new());
}

// `block_entity_submit`.
pub(in crate::game) fn submit(game: &Game<'_>, pos: Pos, block: &str, entity: &ModBlockEntity, draw: &mut crate::game::native_block_entity_draw::BlockEntityDraw) {
    if block != BUNDLE {
        return;
    }
    let mode = display_mode(game.held());
    // ConduitRenderMapper / the facade renderer: the paint's own model; an opaque paint or a transparent facade
    // hides the conduits. A held tool that hides facades (YetaUtil) shows a wire frame instead.
    if let Some((t, paint)) = super::enderio_facades::bundle_facade(&entity.nbt) {
        if super::enderio_facades::hides_facades(game.held()) {
            for q in facade_wire() {
                game.native_draw_quad(draw, q.sprite, q.corners, q.uv, q.tint);
            }
        } else {
            game.native_draw_block(draw, &paint, &[]);
            if super::enderio_facades::is_transparent(t) || super::enderio_facades::paint_opaque(game, &paint) {
                return;
            }
        }
    }
    let rev = entity.revision();
    DRAWN.with_borrow_mut(|cache| {
        let fresh = cache.get(&pos).is_some_and(|(r, m, _)| *r == rev && *m == mode);
        if !fresh {
            if cache.len() > 4096 {
                cache.clear();
            }
            cache.insert(pos, (rev, mode, bundle_quads(&load(&entity.nbt), mode)));
        }
        if let Some((_, _, quads)) = cache.get(&pos) {
            for q in quads {
                game.native_draw_quad(draw, q.sprite, q.corners, q.uv, q.tint);
            }
        }
    });
}

// ---- Item conduits (ItemConduit, ItemConduitNetwork, NetworkedInventory)

// A container on the neighbour's side that is no bundle (ItemConduit.getExternalInventory).
fn item_external(game: &Game<'_>, pos: Pos, d: usize) -> bool {
    let n = step(pos, d);
    !is_bundle(game, n) && game.native_container_slots(game.native_dimension(), n, FACES[opposite(d)]).is_some()
}

pub(in crate::game) const SPEED_UPGRADE: &str = "enderio:extract_speed_upgrade";
pub(in crate::game) const SPEED_DOWNGRADE: &str = "enderio:extract_speed_downgrade";
const GREEN: i32 = 2;

impl Conduit {
    // functionUpgrades.<DIR>, inputFilterUpgrades.<DIR>, outputFilterUpgrades.<DIR>.
    pub(in crate::game) fn stack(&self, key: &str, d: usize) -> Option<ItemStack> {
        self.nbt.get(&js(&format!("{key}.{}", DIR_NAMES[d]))).and_then(horizon_items::item_nbt::item_from_nbt)
    }
    pub(in crate::game) fn set_stack(&mut self, key: &str, d: usize, s: Option<&ItemStack>) {
        let k = js(&format!("{key}.{}", DIR_NAMES[d]));
        match s {
            Some(s) => {
                self.nbt.insert(k, horizon_items::item_nbt::item_to_nbt(s));
            }
            None => {
                self.nbt.remove(&k);
            }
        }
    }
    fn dir_i(&self, key: &str, d: usize, default: i32) -> i32 {
        gi(&self.nbt, &format!("{key}.{}", DIR_NAMES[d]), default)
    }
    pub(in crate::game) fn set_dir_i(&mut self, key: &str, d: usize, v: i32) {
        si(&mut self.nbt, &format!("{key}.{}", DIR_NAMES[d]), v);
    }
    pub(in crate::game) fn in_color(&self, d: usize) -> i32 {
        self.dir_i("inSC", d, GREEN).clamp(0, 15)
    }
    pub(in crate::game) fn out_color(&self, d: usize) -> i32 {
        self.dir_i("outSC", d, GREEN).clamp(0, 15)
    }
    pub(in crate::game) fn self_feed(&self, d: usize) -> bool {
        self.dir_i("selfFeed", d, 0) != 0
    }
    pub(in crate::game) fn round_robin(&self, d: usize) -> bool {
        self.dir_i("roundRobin", d, 0) != 0
    }
    pub(in crate::game) fn priority(&self, d: usize) -> i32 {
        self.dir_i("priority", d, 0)
    }
    // getMaximumExtracted: 4; speed upgrades 4 + 4 x min(n, 15); downgrades n.
    pub(in crate::game) fn max_extracted(&self, d: usize) -> i32 {
        match self.stack("functionUpgrades", d) {
            Some(s) if &*s.item == SPEED_UPGRADE => 4 + 4 * (s.count as i32).min(15),
            Some(s) if &*s.item == SPEED_DOWNGRADE => s.count as i32,
            _ => 4,
        }
    }
}

// The item filters (ItemFilter, ModItemFilter, PowerItemFilter, ExistingItemFilter), read from the filter item's
// custom_data `filter` (enderio_filters.rs writes it).
#[derive(Clone, Debug, Default)]
pub(in crate::game) struct ItemFilter {
    pub kind: Option<super::enderio_filters::Kind>,
    pub blacklist: bool,
    pub match_nbt: bool,
    pub sticky: bool,
    pub limited: bool,
    pub items: Vec<Option<ItemStack>>,
    // DamageMode's ordinal.
    pub damage: i32,
    pub mods: [Option<String>; 3],
    // PowerItemFilter's CmpMode and level.
    pub power: (i32, i32),
    pub snapshot: Option<Vec<ItemStack>>,
    // ExistingItemFilter without a snapshot: the connected inventory (`with_inventory`).
    pub inventory: Vec<ItemStack>,
    // useOreDict (the advanced item filters, the existing filter).
    pub ore: bool,
    // EnchantmentFilter's enchantments, SoulFilter's entity ids.
    pub enchants: Vec<String>,
    pub souls: Vec<String>,
}

pub(in crate::game) fn item_filter(stack: &ItemStack) -> Option<ItemFilter> {
    use super::enderio_filters as ef;
    let k = ef::kind(&stack.item)?;
    let mut f = ItemFilter { kind: Some(k), ..ItemFilter::default() };
    match k {
        ef::Kind::Items { slots, advanced, limited, .. } => {
            let it = ef::read_items(stack, slots, advanced);
            f.blacklist = it.blacklist;
            f.match_nbt = it.nbt;
            f.ore = it.ore;
            f.sticky = it.sticky;
            f.limited = limited;
            f.damage = it.damage;
            f.items = it.items;
        }
        ef::Kind::Mod => {
            let (mods, black) = ef::read_mods(stack);
            f.mods = mods;
            f.blacklist = black;
        }
        ef::Kind::Power => {
            let (mode, level, sticky) = ef::read_power(stack);
            f.power = (mode, level);
            f.sticky = sticky;
        }
        ef::Kind::Existing => {
            let e = ef::read_existing(stack);
            f.blacklist = e.blacklist;
            f.match_nbt = e.nbt;
            f.ore = e.ore;
            f.sticky = e.sticky;
            f.snapshot = e.snapshot;
        }
        ef::Kind::Enchant { .. } => {
            let p = ef::read_enchants(stack);
            f.enchants = p.list;
            f.blacklist = p.blacklist;
            f.sticky = p.sticky;
        }
        ef::Kind::Soul { .. } => {
            let p = ef::read_souls(stack);
            f.souls = p.list.into_iter().map(|s| s.id).collect();
            f.blacklist = p.blacklist;
            f.sticky = p.sticky;
        }
        ef::Kind::Fluid => return None,
    }
    Some(f)
}

// DamageMode.passesFilter: the damage fraction, -1 for an item that can't be damaged.
fn damage_passes(game: &Game<'_>, mode: i32, item: &ItemStack) -> bool {
    let max = game.res.items().get(&item.item).map_or(0, |i| i.max_damage);
    let p = if max > 0 { item.damage as f32 / max as f32 } else { -1.0 };
    match mode {
        1 => p <= 0.25,
        2 => p > 0.25,
        3 => p <= 0.5,
        4 => p > 0.5,
        5 => p <= 0.75,
        6 => p > 0.75,
        7 => p <= 0.0,
        8 => p > 0.0,
        9 => p >= 0.0,
        10 => p < 0.0,
        _ => true,
    }
}

// ItemFilter.FilterResult.
enum Matched {
    Fail,
    Pass,
    Limit(u32),
}

impl ItemFilter {
    fn items_kind(&self) -> bool {
        matches!(self.kind, Some(super::enderio_filters::Kind::Items { .. }))
    }
    // isValid: an item filter with an item in it or a damage mode, a soul filter with a soul; the others always.
    fn valid(&self) -> bool {
        match self.kind {
            Some(super::enderio_filters::Kind::Soul { .. }) => !self.souls.is_empty(),
            _ => !self.items_kind() || self.damage != 0 || self.items.iter().any(Option::is_some),
        }
    }
    // ItemFilter.itemMatched: the damage mode, then the first filter item of the same item (and components when
    // matchNBT), its count the limit.
    fn matched(&self, game: &Game<'_>, item: &ItemStack) -> Matched {
        if !damage_passes(game, self.damage, item) {
            return Matched::Fail;
        }
        let mut pass = self.damage != 0;
        for f in self.items.iter().flatten() {
            if f.item == item.item && (!self.match_nbt || f.same_item_same_components(item)) {
                return Matched::Limit(f.count);
            }
            // isOreDicMatch: the filter stack's and the item's ore names (cached per item, ore_match).
            if self.ore && super::enderio_filters::ore_match(game.items(), &f.item, &item.item) {
                return Matched::Limit(f.count);
            }
            pass = false;
        }
        if pass { Matched::Pass } else { Matched::Fail }
    }
    // doesItemPassFilter.
    pub(in crate::game) fn passes(&self, game: &Game<'_>, item: &ItemStack) -> bool {
        use super::enderio_filters::Kind;
        match self.kind {
            Some(Kind::Mod) => {
                let ns = item.item.split_once(':').map_or("minecraft", |(n, _)| n);
                self.mods.iter().flatten().any(|m| m == ns) != self.blacklist
            }
            Some(Kind::Power) => {
                // The stack's energy against max x level / 4 by the comparison mode; no energy storage fails.
                let Some(b) = game.native_item_energy(item) else { return false };
                let (max, cur) = (b.capacity as i64, b.amount as i64);
                let r = max * i64::from(self.power.1) / 4;
                match self.power.0 {
                    0 => cur < r,
                    1 => cur <= r,
                    2 => cur == r,
                    3 => cur >= r,
                    _ => cur > r,
                }
            }
            Some(Kind::Existing) => {
                let e = super::enderio_filters::Existing { nbt: self.match_nbt, ore: self.ore, ..Default::default() };
                let list = self.snapshot.as_ref().unwrap_or(&self.inventory);
                let items = game.items();
                let ore = |a: &str, b: &str| super::enderio_filters::ore_match(items, a, b);
                list.iter().any(|s| super::enderio_filters::existing_equal(&e, item, s, &ore)) != self.blacklist
            }
            // EnchantmentFilter.doesItemPassFilter: an item with enchantments (a book's stored ones) passes an empty
            // filter or one sharing an enchantment, unless blacklisting; one without fails, unless blacklisting.
            Some(Kind::Enchant { .. }) => {
                let have = item.crafting_enchantments();
                if !have.is_empty() && (self.enchants.is_empty() || have.iter().any(|(e, _)| self.enchants.iter().any(|x| **x == **e))) {
                    return !self.blacklist;
                }
                self.blacklist
            }
            // SoulFilter.doesItemPassFilter: a stack holding a soul of a listed type.
            Some(Kind::Soul { .. }) => {
                if super::enderio_filters::soul_in(item).is_some_and(|s| self.souls.contains(&s.id)) {
                    return !self.blacklist;
                }
                self.blacklist
            }
            _ => !self.valid() || (!matches!(self.matched(game, item), Matched::Fail) != self.blacklist),
        }
    }
    // getMaxCountThatPassesFilter.
    pub(in crate::game) fn max_count(&self, game: &Game<'_>, item: &ItemStack) -> u32 {
        if !self.items_kind() {
            return if self.passes(game, item) { u32::MAX } else { 0 };
        }
        if !self.valid() {
            return 0;
        }
        let m = self.matched(game, item);
        if self.limited {
            if let Matched::Limit(n) = m {
                return n;
            }
        }
        if !self.limited && (!matches!(m, Matched::Fail) != self.blacklist) {
            return u32::MAX;
        }
        0
    }
    // The existing filter without a snapshot reads the inventory beside face `d` of `pos`.
    pub(in crate::game) fn with_inventory(mut self, game: &Game<'_>, pos: Pos, d: usize) -> Self {
        if self.kind == Some(super::enderio_filters::Kind::Existing) && self.snapshot.is_none() {
            self.inventory = face_inventory(game, pos, d);
        }
        self
    }
}

// The stacks of the inventory beside face `d` of `pos`.
pub(in crate::game) fn face_inventory(game: &Game<'_>, pos: Pos, d: usize) -> Vec<ItemStack> {
    let dim = game.native_dimension();
    game.native_container_slots(dim, step(pos, d), FACES[opposite(d)]).unwrap_or_default().into_iter().filter_map(|s| s.stack).collect()
}

fn valid_filter(c: &Conduit, key: &str, d: usize) -> Option<ItemFilter> {
    c.stack(key, d).and_then(|s| item_filter(&s)).filter(ItemFilter::valid)
}

// One external connection of the network (NetworkedInventory).
#[derive(Clone, Debug)]
struct NetInv {
    pos: Pos,
    d: usize,
    deficit: i32,
    slot: i32,
    // sendPriority, as indices into the network's inventories.
    targets: Vec<usize>,
    // RoundRobinIterator.index.
    rr: i32,
}

#[derive(Default)]
struct ItemNet {
    routes_dirty: bool,
    invs: Vec<NetInv>,
    // The members' item conduits as the routes were made.
    cons: HashMap<Pos, Conduit>,
}

// checkInventoryConnections and updateInsertOrder: every external connection not DISABLED; for each that extracts,
// its targets (insert allowed, the source's input colour = the target's output colour; itself only with self feed),
// sticky first, then priority (high first), then the conduit path length.
fn rebuild_routes(game: &Game<'_>, id: u32) {
    let Some(members) = NETS.with_borrow(|n| n.nets.get(&id).map(|x| x.members.clone())) else { return };
    let mut cons: HashMap<Pos, Conduit> = HashMap::new();
    for p in &members {
        if let Some(b) = bundle_at(game, *p) {
            if let Some(i) = find(&b, Base::Item) {
                cons.insert(*p, b[i].clone());
            }
        }
    }
    let old: Vec<NetInv> = NETS.with_borrow(|n| n.nets.get(&id).and_then(|x| x.item.as_ref()).map(|i| i.invs.clone()).unwrap_or_default());
    let mut invs: Vec<NetInv> = Vec::new();
    for p in &members {
        let Some(c) = cons.get(p) else { continue };
        for d in (0..6).filter(|d| c.has_ext(*d) && c.mode(*d) != DISABLED) {
            let kept = old.iter().find(|o| o.pos == *p && o.d == d);
            invs.push(NetInv { pos: *p, d, deficit: kept.map_or(100, |k| k.deficit), slot: kept.map_or(-1, |k| k.slot), targets: Vec::new(), rr: -1 });
        }
    }
    for i in 0..invs.len() {
        let src = &cons[&invs[i].pos];
        let sd = invs[i].d;
        if !accepts_input(src.mode(sd)) {
            continue;
        }
        // calculateDistances: the conduit steps from the source's bundle.
        let mut dist: HashMap<Pos, i32> = HashMap::from([(invs[i].pos, 0)]);
        let mut queue = VecDeque::from([invs[i].pos]);
        while let Some(at) = queue.pop_front() {
            let here = dist[&at];
            if let Some(c) = cons.get(&at) {
                for d in (0..6).filter(|d| c.has_conn(*d)) {
                    let n = step(at, d);
                    if cons.contains_key(&n) && !dist.contains_key(&n) {
                        dist.insert(n, here + 1);
                        queue.push_back(n);
                    }
                }
            }
        }
        let mut t: Vec<(usize, bool, i32, i32)> = Vec::new();
        for (j, o) in invs.iter().enumerate() {
            let oc = &cons[&o.pos];
            if (src.self_feed(sd) || j != i) && accepts_output(oc.mode(o.d)) && src.in_color(sd) == oc.out_color(o.d) {
                let sticky = valid_filter(oc, "outputFilterUpgrades", o.d).is_some_and(|f| f.sticky);
                t.push((j, sticky, oc.priority(o.d), dist.get(&o.pos).copied().unwrap_or(9_999_999)));
            }
        }
        t.sort_by(|a, b| b.1.cmp(&a.1).then(b.2.cmp(&a.2)).then(a.3.cmp(&b.3)));
        invs[i].targets = t.into_iter().map(|x| x.0).collect();
    }
    NETS.with_borrow_mut(|n| {
        if let Some(x) = n.nets.get_mut(&id).and_then(|x| x.item.as_mut()) {
            x.invs = invs;
            x.cons = cons;
            x.routes_dirty = false;
        }
    });
}

// ItemConduitNetwork.tickEnd: each inventory's shouldTick, then onTick.
fn item_tick(game: &mut Game<'_>, id: u32) {
    if NETS.with_borrow(|n| n.nets.get(&id).and_then(|x| x.item.as_ref()).is_some_and(|i| i.routes_dirty)) {
        rebuild_routes(game, id);
    }
    let count = NETS.with_borrow(|n| n.nets.get(&id).and_then(|x| x.item.as_ref()).map_or(0, |i| i.invs.len()));
    for i in 0..count {
        let Some((inv, src, invs, cons)) = NETS.with_borrow(|n| {
            let x = n.nets.get(&id)?.item.as_ref()?;
            let inv = x.invs.get(i)?.clone();
            let src = x.cons.get(&inv.pos)?.clone();
            Some((inv, src, x.invs.clone(), x.cons.clone()))
        }) else { continue };
        let mut inv = inv;
        if inv.deficit > 0 {
            inv.deficit -= 1;
        } else if accepts_input(src.mode(inv.d)) && rs_met(game, inv.pos, src.rs_mode(inv.d), src.rs_color(inv.d)) && !inv.targets.is_empty() {
            if !transfer_items(game, &mut inv, &src, &invs, &cons) {
                inv.deficit = 50;
            } else if inv.deficit <= 0 {
                inv.deficit = 20;
            }
        } else {
            inv.deficit = 20;
        }
        NETS.with_borrow_mut(|n| {
            if let Some(x) = n.nets.get_mut(&id).and_then(|x| x.item.as_mut()) {
                if let Some(v) = x.invs.get_mut(i) {
                    *v = inv;
                }
            }
        });
    }
}

// transferItems: from the slot after the last, up to maxSlotCheckPerTick (27) slots that hold something; the first
// slot that moves anything ends it.
fn transfer_items(game: &mut Game<'_>, inv: &mut NetInv, src: &Conduit, invs: &[NetInv], cons: &HashMap<Pos, Conduit>) -> bool {
    let dim = game.native_dimension().to_owned();
    let there = step(inv.pos, inv.d);
    let face = FACES[opposite(inv.d)];
    let Some(slots) = game.native_container_slots(&dim, there, face) else { return false };
    let n = slots.len() as i32;
    if n < 1 {
        return false;
    }
    let max = src.max_extracted(inv.d).max(1) as u32;
    let filter = valid_filter(src, "inputFilterUpgrades", inv.d).map(|f| f.with_inventory(game, inv.pos, inv.d));
    let mut checks = 27;
    let mut i = 0;
    while i < n && i < checks {
        i += 1;
        inv.slot += 1;
        if inv.slot >= n || inv.slot < 0 {
            inv.slot = 0;
        }
        let at = inv.slot as usize;
        let Some(stack) = slots[at].stack.clone().filter(|_| slots[at].can_take) else {
            checks += 1;
            continue;
        };
        let mut take = stack.count.min(max);
        if let Some(f) = &filter {
            if f.limited {
                let keep = f.max_count(game, &stack);
                if keep == 0 {
                    continue;
                }
                if keep < u32::MAX {
                    if stack.count <= keep {
                        continue;
                    }
                    if stack.count - take < keep {
                        take = stack.count - keep;
                    }
                }
            } else if !f.passes(game, &stack) {
                continue;
            }
        }
        if take == 0 {
            continue;
        }
        let Some(got) = game.native_extract_item(&dim, there, face, slots[at].slot, take) else { continue };
        let total = got.count;
        let left = insert_into_targets(game, inv, src, invs, cons, got);
        let moved = total - left.as_ref().map_or(0, |l| l.count);
        if let Some(rest) = left {
            // What no target took goes back where it came from.
            if let Some(rest) = game.native_insert_item(there, face, rest) {
                spawn_drops(game, inv.pos, vec![rest]);
            }
        }
        if moved > 0 {
            // onItemExtracted: the next try waits round(moved x 10 / maxExtracted) ticks.
            inv.deficit = (moved as f32 * (10.0 / max as f32)).round() as i32;
            let now_empty = game.native_container_slots(&dim, there, face).and_then(|s| s.get(at).map(|s| s.stack.is_none())).unwrap_or(true);
            inv.slot = if now_empty { at as i32 } else { at as i32 - 1 };
            return true;
        }
    }
    false
}

// insertIntoTargets: by the send order (or round robin), sticky targets first; a sticky target whose filter matched
// keeps the rest from the others. What is left.
fn insert_into_targets(game: &mut Game<'_>, inv: &mut NetInv, src: &Conduit, invs: &[NetInv], cons: &HashMap<Pos, Conduit>, item: ItemStack) -> Option<ItemStack> {
    let mut left = Some(item);
    let order: Vec<usize> = if src.round_robin(inv.d) {
        let len = inv.targets.len() as i32;
        (0..len)
            .map(|_| {
                inv.rr += 1;
                if inv.rr >= len {
                    inv.rr = 0;
                }
                inv.targets[inv.rr as usize]
            })
            .collect()
    } else {
        inv.targets.clone()
    };
    let mut matched_sticky = false;
    for j in order {
        let Some(cur) = left.clone() else { break };
        let t = &invs[j];
        let Some(tc) = cons.get(&t.pos) else { continue };
        let filter = valid_filter(tc, "outputFilterUpgrades", t.d).map(|f| f.with_inventory(game, t.pos, t.d));
        let sticky = filter.as_ref().is_some_and(|f| f.sticky);
        if sticky && !matched_sticky {
            matched_sticky = filter.as_ref().is_some_and(|f| f.passes(game, &cur));
        }
        if !sticky && matched_sticky {
            break;
        }
        left = insert_item(game, t, tc, filter.as_ref(), cur);
    }
    left
}

// NetworkedInventory.insertItem through its output filter (a limited one caps it by what the target holds).
fn insert_item(game: &mut Game<'_>, t: &NetInv, tc: &Conduit, filter: Option<&ItemFilter>, item: ItemStack) -> Option<ItemStack> {
    if !accepts_output(tc.mode(t.d)) {
        return Some(item);
    }
    let there = step(t.pos, t.d);
    let face = FACES[opposite(t.d)];
    let mut give = item.count;
    if let Some(f) = filter {
        if f.limited {
            let count = f.max_count(game, &item);
            if count == 0 {
                return Some(item);
            }
            // ItemTools.getInsertLimit: the limit less what the target holds of it.
            let dim = game.native_dimension().to_owned();
            let held: u32 = game.native_container_slots(&dim, there, face).unwrap_or_default().iter().filter_map(|s| s.stack.as_ref()).filter(|s| s.item == item.item).map(|s| s.count).sum();
            if held >= count {
                return Some(item);
            }
            give = give.min(count - held);
        } else if !f.passes(game, &item) {
            return Some(item);
        }
    }
    let keep = item.count - give;
    let rest = game.native_insert_item(there, face, ItemStack { count: give, ..item.clone() }).map_or(0, |r| r.count);
    let left = keep + rest;
    (left > 0).then(|| ItemStack { count: left, ..item })
}

// ---- The per-side settings screen (ExternalConnectionContainer, GuiExternalConnection and its settings panels)

// The face each bundle's screen shows, and the stacks its three slots held when they were last matched up.
thread_local! {
    static OPEN: RefCell<HashMap<Pos, (usize, [Option<ItemStack>; 3])>> = RefCell::new(HashMap::new());
    // The tab the screen shows (whose conduit the slots mirror) and the filter screen open on slot 0 or 1.
    static VIEW: RefCell<HashMap<Pos, (Base, Option<usize>)>> = RefCell::new(HashMap::new());
}

// The slots: the insert filter (23, 71, the face's outputFilterUpgrades), the extract filter (113, 71, its
// inputFilterUpgrades) and the function upgrade (131, 71).
const SLOT_KEYS: [&str; 3] = ["outputFilterUpgrades", "inputFilterUpgrades", "functionUpgrades"];
const SLOTS: [[f32; 2]; 3] = [[23.0, 71.0], [113.0, 71.0], [131.0, 71.0]];
const GUI_W: f32 = 206.0;
const GUI_H: f32 = 194.0;
const TAB_BASES: [Base; 4] = [Base::Item, Base::Fluid, Base::Energy, Base::Redstone];

pub fn block_entity_container(block: &str, out: &mut Option<horizon_client::containers::ModContainer>) {
    if block == BUNDLE {
        *out = Some(horizon_client::containers::ModContainer { slots: 3, faces: std::array::from_fn(|_| Vec::new()), inputs: Vec::new(), outputs: Vec::new(), menu: Some(BUNDLE.to_owned()) });
    }
}

// The menu's slot rules: a filter each in 0 and 1 (the item filters, the fluid filter, the redstone signal filters:
// the tab's conduit decides which it uses), the speed upgrades (15) and downgrades (3, kept by the tick) in 2.
pub(in crate::game) fn slot_rules() -> Vec<(Option<HashSet<String>>, u32)> {
    let mut filters: HashSet<String> = super::enderio_filters::ITEM_FILTER_ITEMS.iter().map(|f| (*f).to_owned()).collect();
    filters.insert(super::enderio_filters::FLUID_FILTER.to_owned());
    filters.extend(super::enderio_signal_filters::ITEMS.iter().map(|f| (*f).to_owned()));
    let upgrades: HashSet<String> = [SPEED_UPGRADE, SPEED_DOWNGRADE].iter().map(|s| (*s).to_owned()).collect();
    vec![(Some(filters.clone()), 1), (Some(filters), 1), (Some(upgrades), 15)]
}

// Which of the three slots a tab's conduit has (ItemSettings: both filters and the function upgrade; LiquidSettings
// of the ender fluid conduit: the same; RedstoneSettings: the two signal filters; energy and the other fluid
// conduits: none), as a mask.
fn slot_mask(c: Option<&Conduit>) -> u8 {
    match c {
        Some(c) if c.def.base == Base::Item => 0b111,
        Some(c) if c.def.base == Base::Fluid && c.def.tier == 2 => 0b111,
        Some(c) if c.def.base == Base::Redstone => 0b011,
        _ => 0,
    }
}

// GuiExternalConnection adds a conduit's tab when containsExternalConnection(dir) || canConnectToExternal(dir, true);
// the redstone conduit's canConnectToExternal with ignoreConnectionState is always true (a disabled or unconnected
// face keeps its tab, so its mode can be set back).
fn face_shown(c: &Conduit, d: usize) -> bool {
    c.has_ext(d) || c.def.base == Base::Redstone
}

fn tab_of(pos: Pos) -> Base {
    VIEW.with_borrow(|v| v.get(&pos).map_or(Base::Item, |x| x.0))
}

fn face_stacks(b: &[Conduit], base: Base, d: usize) -> [Option<ItemStack>; 3] {
    let Some(i) = find(b, base) else { return [None, None, None] };
    let mask = slot_mask(Some(&b[i]));
    std::array::from_fn(|k| if mask & (1 << k) != 0 { b[i].stack(SLOT_KEYS[k], d) } else { None })
}

// A plate clicked: the menu (opened by the game after the click) shows face `d`, on the first tab
// (getGuiPanelTabOrder: item, fluid, redstone, energy, of the conduits connected there).
fn open_face(game: &mut Game<'_>, pos: Pos, _uuid: u128, d: usize) {
    let drops = game.native_with_mod_block_entity(pos, |e| {
        let mut b = load(&e.nbt);
        let mut drops = Vec::new();
        sync_slots(e, &mut b, pos, &mut drops);
        let base = [Base::Item, Base::Fluid, Base::Redstone, Base::Energy].into_iter().find(|t| find(&b, *t).is_some_and(|i| face_shown(&b[i], d))).unwrap_or(Base::Item);
        VIEW.with_borrow_mut(|v| v.insert(pos, (base, None)));
        let stacks = face_stacks(&b, base, d);
        e.items = stacks.to_vec();
        save(&b, &mut e.nbt);
        OPEN.with_borrow_mut(|o| o.insert(pos, (d, stacks)));
        fill_screen_data_at(e, &b, pos, d);
        drops
    });
    if let Some(drops) = drops.filter(|d| !d.is_empty()) {
        spawn_drops(game, pos, drops);
    }
}

// The three slots and the open face's conduit kept equal: a player's change goes into the tab's conduit (a downgrade
// stack over 3 drops its rest), the conduit's into the slots (it was taken out). A stack put into a slot the tab's
// conduit doesn't have (only a client that shows it could) goes to `drops`. True when the conduit changed.
fn sync_slots(e: &mut ModBlockEntity, b: &mut [Conduit], pos: Pos, drops: &mut Vec<ItemStack>) -> bool {
    let Some((d, seen)) = OPEN.with_borrow(|o| o.get(&pos).cloned()) else { return false };
    if e.items.len() < 3 {
        e.items.resize(3, None);
    }
    let base = tab_of(pos);
    let now: [Option<ItemStack>; 3] = std::array::from_fn(|k| e.items[k].clone());
    let held = face_stacks(b, base, d);
    let mut changed = false;
    if now != seen {
        let i = find(b, base);
        let mask = slot_mask(i.map(|i| &b[i]));
        for k in 0..3 {
            if now[k] == seen[k] {
                continue;
            }
            match i.filter(|_| mask & (1 << k) != 0) {
                Some(i) => {
                    b[i].set_stack(SLOT_KEYS[k], d, now[k].as_ref());
                    changed = true;
                }
                None => {
                    drops.extend(now[k].clone());
                    e.items[k] = None;
                }
            }
        }
    } else if held != seen {
        e.items = held.to_vec();
    }
    let kept: [Option<ItemStack>; 3] = std::array::from_fn(|k| e.items[k].clone());
    OPEN.with_borrow_mut(|o| o.insert(pos, (d, kept)));
    changed
}

// Data slots: 0 the face + 1; per base type (item, fluid, energy, redstone) from 2 + 10 x type: shown, connection
// mode, extraction redstone mode, signal colour, extract channel, insert channel, priority + 32768, round robin,
// self feed, items extracted per operation. 42..: the tab's own (the redstone conduit's signal filters' values);
// 50 the filter screen open (slot + 1), 51 the tab's type, 52 the tab's slot mask.
fn fill_screen_data(e: &mut ModBlockEntity, b: &[Conduit], d: usize) {
    if e.data.len() < 64 {
        e.data.resize(64, 0);
    }
    e.data[0] = d as i32 + 1;
    for (t, base) in TAB_BASES.into_iter().enumerate() {
        let at = 2 + t * 10;
        let Some(i) = find(b, base) else {
            e.data[at] = 0;
            continue;
        };
        let c = &b[i];
        e.data[at] = i32::from(face_shown(c, d));
        e.data[at + 1] = i32::from(c.mode(d));
        e.data[at + 2] = c.rs_mode(d);
        e.data[at + 3] = c.rs_color(d);
        e.data[at + 4] = c.in_color(d);
        e.data[at + 5] = c.out_color(d);
        // Data slots travel as signed 16 bits.
        e.data[at + 6] = c.priority(d).clamp(-32768, 32767);
        e.data[at + 7] = i32::from(c.round_robin(d));
        e.data[at + 8] = i32::from(c.self_feed(d));
        // The function upgrade's tooltip: items per operation, or the ender fluid conduit's speed in percent.
        e.data[at + 9] = if base == Base::Fluid { (100.0 * c.fluid_speed(d)) as i32 } else { c.max_extracted(d) };
    }
    // 53: the fluid conduit's tier (LiquidSettings lays out the ender one's panel apart).
    e.data[53] = find(b, Base::Fluid).map_or(0, |i| b[i].def.tier as i32);
}

// Called from the bundle's tick: the open face's slots and data.
fn screen_tick(game: &mut Game<'_>, pos: Pos, e: &mut ModBlockEntity, b: &mut Vec<Conduit>) -> bool {
    let Some((d, _)) = OPEN.with_borrow(|o| o.get(&pos).cloned()) else { return false };
    let mut drops = Vec::new();
    let mut changed = sync_slots(e, b, pos, &mut drops);
    // ItemFunctionUpgrade: a downgrade stack holds 3 at most.
    let base = tab_of(pos);
    if let Some(i) = find(b, base).filter(|i| slot_mask(Some(&b[*i])) & 0b100 != 0) {
        if let Some(mut s) = b[i].stack("functionUpgrades", d).filter(|s| &*s.item == SPEED_DOWNGRADE && s.count > 3) {
            drops.push(ItemStack { count: s.count - 3, ..s.clone() });
            s.count = 3;
            b[i].set_stack("functionUpgrades", d, Some(&s));
            e.items[2] = Some(s);
            OPEN.with_borrow_mut(|o| {
                if let Some(x) = o.get_mut(&pos) {
                    x.1[2] = e.items[2].clone();
                }
            });
            changed = true;
        }
    }
    if !drops.is_empty() {
        spawn_drops(game, pos, drops);
    }
    if changed {
        receptors_changed(game, pos);
    }
    fill_screen_data_at(e, b, pos, d);
    changed
}

// fill_screen_data for the bundle at `pos` (its view, not another open bundle's on the same face).
fn fill_screen_data_at(e: &mut ModBlockEntity, b: &[Conduit], pos: Pos, d: usize) {
    fill_screen_data(e, b, d);
    let (base, view) = VIEW.with_borrow(|v| v.get(&pos).copied()).unwrap_or((Base::Item, None));
    e.data[50] = view.map_or(0, |k| k as i32 + 1);
    e.data[51] = TAB_BASES.iter().position(|x| *x == base).unwrap_or(0) as i32;
    e.data[52] = i32::from(slot_mask(find(b, base).map(|i| &b[i])));
    if let Some(i) = find(b, Base::Redstone) {
        super::enderio_signal_filters::fill_data(e, &b[i], d);
    }
}

// The bundle's shift-click (ExternalConnectionContainer.transferStackInSlot): from the inventory into the first
// slot the tab's conduit has that takes it (a filter: one; upgrades: up to 15); nothing while a filter screen is open.
pub(in crate::game) fn quick_move(e: &mut ModBlockEntity, container_slots: usize, slot: usize, moving: &mut ItemStack, handled: &mut bool) {
    *handled = true;
    if slot < container_slots || e.data.get(50).copied().unwrap_or(0) != 0 || e.items.len() < 3 {
        return;
    }
    let mask = e.data.get(52).copied().unwrap_or(0);
    let rules = slot_rules();
    for k in 0..3 {
        if mask & (1 << k) == 0 || !rules[k].0.as_ref().is_some_and(|s| s.contains(&*moving.item)) {
            continue;
        }
        let most = rules[k].1;
        match e.items[k].as_mut() {
            Some(have) if have.same_item_same_components(moving) => {
                let n = most.saturating_sub(have.count).min(moving.count);
                have.count += n;
                moving.count -= n;
            }
            None => {
                let n = most.min(moving.count);
                e.items[k] = Some(ItemStack { count: n, ..moving.clone() });
                moving.count -= n;
            }
            _ => {}
        }
        if moving.count == 0 {
            break;
        }
    }
}

// Button ids: 100 + 20 x type + action (the type's index as in the data).
const B_INSERT: i32 = 0;
const B_EXTRACT: i32 = 1;
const B_RS: i32 = 3; // 3..6: the redstone mode chosen
const B_COLOR_NEXT: i32 = 7;
const B_COLOR_PREV: i32 = 8;
const B_EXTRACT_CH_NEXT: i32 = 9;
const B_EXTRACT_CH_PREV: i32 = 10;
const B_INSERT_CH_NEXT: i32 = 11;
const B_INSERT_CH_PREV: i32 = 12;
const B_ROUND_ROBIN: i32 = 13;
const B_SELF_FEED: i32 = 14;
const B_PRIORITY_UP: i32 = 15;
const B_PRIORITY_DOWN: i32 = 16;

// The tabs, the gear buttons and the filter screens: 186 / 187 open the filter screen of slot 0 / 1 (Java's
// doOpenFilterGui), 188 closes it, 190 + type selects a tab, 1000 + 200 x slot + action edits that filter.
const B_GEAR: i32 = 186;
const B_FILTER_CLOSE: i32 = 188;
const B_TAB: i32 = 190;
const B_FILTER: i32 = 1000;

// Whether the tab's conduit edits this filter in a screen (hasFilterGui): its own kind of filter.
fn has_filter_screen(base: Base, s: &ItemStack) -> bool {
    match base {
        Base::Item => super::enderio_filters::kind(&s.item).is_some_and(|k| k != super::enderio_filters::Kind::Fluid),
        Base::Fluid => &*s.item == super::enderio_filters::FLUID_FILTER,
        Base::Redstone => super::enderio_signal_filters::has_screen(&s.item),
        Base::Energy => false,
    }
}

fn set_seen(pos: Pos, k: usize, s: Option<ItemStack>) {
    OPEN.with_borrow_mut(|o| {
        if let Some(x) = o.get_mut(&pos) {
            x.1[k] = s;
        }
    });
}

// True when `id` was one of these (done or refused).
fn view_button(game: &mut Game<'_>, pos: Pos, e: &mut ModBlockEntity, d: usize, id: i32, carried: Option<&ItemStack>) -> bool {
    let panel = (super::enderio_signal_filters::B_PANEL..super::enderio_signal_filters::B_PANEL + 5).contains(&id);
    let is_view = panel || (B_GEAR..=B_FILTER_CLOSE).contains(&id) || (B_TAB..B_TAB + 4).contains(&id) || (B_FILTER..B_FILTER + 400).contains(&id);
    if !is_view {
        return false;
    }
    let mut b = load(&e.nbt);
    let mut drops = Vec::new();
    // What the player did to the slots first.
    let mut changed = sync_slots(e, &mut b, pos, &mut drops);
    let (base, view) = VIEW.with_borrow(|v| v.get(&pos).copied()).unwrap_or((Base::Item, None));
    if panel {
        if let Some(i) = find(&b, Base::Redstone).filter(|i| face_shown(&b[*i], d)) {
            changed |= super::enderio_signal_filters::panel_button(&mut b[i], d, id);
            let k2 = key(game, pos);
            STATE.with_borrow_mut(|s| {
                s.shapes.insert(k2);
                s.dirty.insert(pos);
            });
            game.native_block_entity_sync(pos);
        }
    } else if (B_TAB..B_TAB + 4).contains(&id) {
        let t = TAB_BASES[(id - B_TAB) as usize];
        if find(&b, t).is_some_and(|i| face_shown(&b[i], d)) {
            VIEW.with_borrow_mut(|v| v.insert(pos, (t, None)));
            let stacks = face_stacks(&b, t, d);
            e.items = stacks.to_vec();
            OPEN.with_borrow_mut(|o| o.insert(pos, (d, stacks)));
        }
    } else if id == B_FILTER_CLOSE {
        VIEW.with_borrow_mut(|v| v.insert(pos, (base, None)));
    } else if id < B_FILTER {
        let k = (id - B_GEAR) as usize;
        let ok = find(&b, base).filter(|i| slot_mask(Some(&b[*i])) & (1 << k) != 0).and_then(|i| b[i].stack(SLOT_KEYS[k], d)).is_some_and(|s| has_filter_screen(base, &s));
        if ok && view.is_none() {
            VIEW.with_borrow_mut(|v| v.insert(pos, (base, Some(k))));
        }
    } else {
        let k = ((id - B_FILTER) / 200) as usize;
        let action = (id - B_FILTER) % 200;
        if let (Some(i), Some(open)) = (find(&b, base), view) {
            if open == k {
                if let Some(s) = b[i].stack(SLOT_KEYS[k], d).filter(|s| has_filter_screen(base, s)) {
                    let next = if base == Base::Redstone {
                        super::enderio_signal_filters::edit(&s, action)
                    } else {
                        let snapshot = matches!(action, super::enderio_filters::A_SNAP_SET | super::enderio_filters::A_SNAP_MERGE);
                        let inventory = if snapshot { face_inventory(game, pos, d) } else { Vec::new() };
                        let carried_fluid = carried.and_then(|c| super::enderio_machines::item_fluid(game, c)).and_then(|f| f.0);
                        let g: &Game<'_> = game;
                        let max_stack = |item: &str| super::enderio::max_stack(g, item);
                        let items = g.items();
                        let ore = |a: &str, b: &str| super::enderio_filters::ore_match(items, a, b);
                        let cx = super::enderio_filters::Ctx { carried, sticky_ok: base == Base::Item && k == 0, carried_fluid, inventory, max_stack: &max_stack, ore: &ore };
                        super::enderio_filters::edit(&s, action, &cx)
                    };
                    if let Some(n) = next {
                        b[i].set_stack(SLOT_KEYS[k], d, Some(&n));
                        e.items[k] = Some(n.clone());
                        set_seen(pos, k, Some(n));
                        changed = true;
                    }
                }
            }
        }
    }
    save(&b, &mut e.nbt);
    if !drops.is_empty() {
        spawn_drops(game, pos, drops);
    }
    if changed {
        receptors_changed(game, pos);
        rs_dirty(game, pos);
    }
    fill_screen_data_at(e, &b, pos, d);
    true
}

// `menu_button`: a settings panel's button, decided on the server for the face the screen shows.
pub(in crate::game) fn menu_button(game: &mut Game<'_>, pos: Pos, e: &mut ModBlockEntity, id: i32, carried: &mut Option<ItemStack>) {
    let Some((d, _)) = OPEN.with_borrow(|o| o.get(&pos).cloned()) else { return };
    if view_button(game, pos, e, d, id, carried.as_ref()) {
        return;
    }
    if !(100..180).contains(&id) {
        return;
    }
    let base = [Base::Item, Base::Fluid, Base::Energy, Base::Redstone][((id - 100) / 20) as usize];
    let k = (id - 100) % 20;
    let mut b = load(&e.nbt);
    let Some(i) = find(&b, base) else { return };
    let c = &mut b[i];
    // Only what the face's panel has (a modified client sends anything): the check boxes everywhere, the redstone
    // mode and signal colour on every panel but RedstoneSettings, the channels, round robin, self feed and priority
    // on ItemSettings and the ender conduit's LiquidSettings.
    let ender = base == Base::Item || (base == Base::Fluid && c.def.tier == 2);
    let allowed = match k {
        B_INSERT | B_EXTRACT => true,
        B_COLOR_NEXT | B_COLOR_PREV => base != Base::Redstone,
        x if (B_RS..B_RS + 4).contains(&x) => base != Base::Redstone,
        _ => ender,
    };
    if !allowed || !face_shown(c, d) {
        return;
    }
    let cycle = |v: i32, by: i32| (v + by).rem_euclid(16);
    match k {
        B_INSERT | B_EXTRACT => {
            // updateConnectionMode: both IN_OUT, insert alone OUTPUT, extract alone INPUT, neither DISABLED.
            let m = c.mode(d);
            let (mut ins, mut ext) = (accepts_output(m), accepts_input(m));
            if k == B_INSERT { ins = !ins } else { ext = !ext }
            let next = match (ins, ext) {
                (true, true) => IN_OUT,
                (true, false) => OUTPUT,
                (false, true) => INPUT,
                _ => DISABLED,
            };
            if base == Base::Redstone {
                // PacketConnectionMode: the redstone conduit's mode is forced (forceConnectionMode), and
                // onAddedToBundle takes or drops the external connection by it.
                c.force(d, next);
                if next == DISABLED {
                    c.ext &= !(1 << d);
                } else if !c.has_conn(d) {
                    c.ext |= 1 << d;
                }
            } else {
                c.set_mode(d, next);
            }
            super::enderio::neighbour_dirty(step(pos, d));
        }
        x if (B_RS..B_RS + 4).contains(&x) => {
            let key = if base == Base::Energy { "pRsMode" } else { "extRM" };
            let v = x - B_RS;
            c.set_short(&format!("{key}.{}", DIR_NAMES[d]), v);
        }
        B_COLOR_NEXT | B_COLOR_PREV => {
            let v = cycle(c.rs_color(d), if k == B_COLOR_NEXT { 1 } else { -1 });
            c.set_rs_color(d, v);
        }
        B_EXTRACT_CH_NEXT | B_EXTRACT_CH_PREV => {
            let v = cycle(c.in_color(d), if k == B_EXTRACT_CH_NEXT { 1 } else { -1 });
            c.set_short(&format!("inSC.{}", DIR_NAMES[d]), v);
        }
        B_INSERT_CH_NEXT | B_INSERT_CH_PREV => {
            let v = cycle(c.out_color(d), if k == B_INSERT_CH_NEXT { 1 } else { -1 });
            c.set_short(&format!("outSC.{}", DIR_NAMES[d]), v);
        }
        B_ROUND_ROBIN => {
            let v = !c.round_robin(d);
            c.set_dir_i("roundRobin", d, i32::from(v));
        }
        B_SELF_FEED => {
            let v = !c.self_feed(d);
            c.set_dir_i("selfFeed", d, i32::from(v));
        }
        B_PRIORITY_UP | B_PRIORITY_DOWN => {
            let v = c.priority(d) + if k == B_PRIORITY_UP { 1 } else { -1 };
            c.set_dir_i("priority", d, v);
        }
        _ => return,
    }
    save(&b, &mut e.nbt);
    receptors_changed(game, pos);
    rs_dirty(game, pos);
    let k2 = key(game, pos);
    STATE.with_borrow_mut(|s| {
        s.shapes.insert(k2);
        s.dirty.insert(pos);
    });
    fill_screen_data_at(e, &b, pos, d);
    game.native_block_entity_sync(pos);
}

// ---- The screen on the client

#[derive(Default)]
struct Cui {
    pos: Option<Pos>,
    tab: usize,
    // The redstone picker open (the base type's index).
    picker: Option<usize>,
    // The data slots as last drawn.
    data: Vec<i32>,
    // The layout the open screen has (filter screen x 8 + slot mask), and the one the server's data asks for.
    layout: Option<i32>,
    want: Option<i32>,
    // The filter screen as last drawn (for the tooltip and the clicks), and the gear buttons shown.
    view: Option<(usize, ItemStack, bool)>,
    gears: [bool; 2],
    // The three slots empty as last drawn (the empty slots' tooltips).
    empty: [bool; 3],
}

// The stack in the open bundle menu's slot `k`, as the client has it: the open container's slots, which
// ContainerSetSlot fills. A client of a server makes that block entity fresh for the menu, with no id, so the id is
// not checked (only the bundle's screen calls this).
fn client_slot(game: &Game<'_>, k: usize) -> Option<ItemStack> {
    let key = game.container?;
    match game.world.block_entities.get(&key)? {
        BlockEntity::Mod(m) => m.items.get(k).cloned().flatten(),
        _ => None,
    }
}

// The tab's type from the data (51), and whether slot 0's filter has sticky mode (the item conduit's insert filter).
fn data_base(data: &[i32]) -> Base {
    TAB_BASES[data.get(51).copied().unwrap_or(0).clamp(0, 3) as usize]
}

// Every frame on the client: the open bundle screen takes the layout its data asks for.
pub(in crate::game) fn client_frame(game: &mut Game<'_>) {
    let Some(want) = CUI.with_borrow(|u| u.want.filter(|w| u.layout != Some(*w))) else { return };
    if game.native_screen_pos().is_none() || game.native_screen_pos() != CUI.with_borrow(|u| u.pos) {
        return;
    }
    let screen = if want >= 8 {
        super::native_hooks::NativeScreen { width: super::enderio_filters::W, height: super::enderio_filters::H, slots: vec![None, None, None], inventory_y: super::enderio_filters::INV[1], inventory_x: super::enderio_filters::INV[0], own_labels: true, own_background: true }
    } else {
        super::native_hooks::NativeScreen { width: GUI_W, height: GUI_H, slots: (0..3).map(|k| (want & (1 << k) != 0).then_some(SLOTS[k])).collect(), inventory_y: 113.0, inventory_x: 23.0, own_labels: true, own_background: true }
    };
    if super::native_hooks::enderio_screen::set_layout(game, BUNDLE, screen) {
        CUI.with_borrow_mut(|u| u.layout = Some(want));
        if want < 8 {
            super::enderio_filters::reset_client();
        }
    }
}

// The filter screen open on the client, from the data: (slot, its stack, sticky mode available).
fn filter_view(game: &Game<'_>, data: &[i32]) -> Option<(usize, ItemStack, bool)> {
    let v = data.get(50).copied().unwrap_or(0);
    if !(1..=2).contains(&v) {
        return None;
    }
    let k = (v - 1) as usize;
    let s = client_slot(game, k)?;
    Some((k, s, data_base(data) == Base::Item && k == 0))
}

const GEARS: [[f32; 4]; 2] = [[22.0, 92.0, 16.0, 16.0], [112.0, 92.0, 16.0, 16.0]];
const ICON_GEAR_LIGHT: [f32; 2] = [16.0, 224.0];

// updateFilterButtons: a gear beside each filter slot that holds a filter with a screen.
fn gear_shown(game: &Game<'_>, data: &[i32], k: usize) -> bool {
    data.get(52).copied().unwrap_or(0) & (1 << k) != 0 && client_slot(game, k).is_some_and(|s| has_filter_screen(data_base(data), &s))
}

thread_local! {
    static CUI: RefCell<Cui> = RefCell::new(Cui::default());
}

pub fn screen(block: &str, out: &mut Option<super::native_hooks::NativeScreen>) {
    if block == BUNDLE {
        *out = Some(super::native_hooks::NativeScreen { width: GUI_W, height: GUI_H, slots: SLOTS.iter().map(|s| Some(*s)).collect(), inventory_y: 113.0, inventory_x: 23.0, own_labels: true, own_background: true });
    }
}

// The tabs shown, by getGuiPanelTabOrder (item 0, fluid 1, redstone 2, energy 3): the types' data indices.
fn tabs(data: &[i32]) -> Vec<usize> {
    [0usize, 1, 3, 2].into_iter().filter(|t| data.get(2 + t * 10).copied().unwrap_or(0) != 0).collect()
}

fn active(game: &Game<'_>, data: &[i32]) -> Option<usize> {
    let pos = game.native_screen_pos();
    let t = tabs(data);
    CUI.with_borrow_mut(|u| {
        if u.pos != pos {
            *u = Cui { pos, ..Cui::default() };
        }
        if u.tab >= t.len() {
            u.tab = 0;
        }
        t.get(u.tab).copied()
    })
}

// IconEIO, on widgetsv2.
const TAB_ICONS: [[f32; 2]; 4] = [[16.0, 128.0], [0.0, 128.0], [0.0, 112.0], [16.0, 112.0]];
const ICON_ROUND_ROBIN: [f32; 2] = [64.0, 240.0];
const ICON_ROUND_ROBIN_OFF: [f32; 2] = [80.0, 240.0];
const ICON_LOOP: [f32; 2] = [80.0, 176.0];
const ICON_LOOP_OFF: [f32; 2] = [96.0, 176.0];
// EnderWidget, on EnderCore's widgets.
const BUTTON_CHECKED: [f32; 2] = [112.0, 208.0];

// Where each panel puts its buttons (BaseSettingsPanel: leftColumn 22, rightColumn 112, customTop = top 0 + gap 5 x 5
// + 2 font lines of 9 - 16 = 27; the second row 16 + 6 below it).
const CHECK_INSERT: [f32; 4] = [22.0, 6.0, 16.0, 16.0];
const CHECK_EXTRACT: [f32; 4] = [112.0, 6.0, 16.0, 16.0];
const ITEM_INSERT_CH: [f32; 4] = [22.0, 27.0, 16.0, 16.0];
const ITEM_EXTRACT_CH: [f32; 4] = [112.0, 27.0, 16.0, 16.0];
const ITEM_RR: [f32; 4] = [132.0, 27.0, 16.0, 16.0];
const ITEM_LOOP: [f32; 4] = [152.0, 27.0, 16.0, 16.0];
const ITEM_RS: [f32; 4] = [112.0, 49.0, 16.0, 16.0];
const ITEM_COLOR: [f32; 4] = [132.0, 49.0, 16.0, 16.0];
const ITEM_PRI_UP: [f32; 4] = [87.0, 49.0, 8.0, 8.0];
const ITEM_PRI_DOWN: [f32; 4] = [87.0, 57.0, 8.0, 8.0];
// PowerSettings, and LiquidSettings of the normal and pressurized conduits: the redstone mode at
// (rightColumn, customTop), its colour 20 right of it.
const RS_TOP: [f32; 4] = [112.0, 27.0, 16.0, 16.0];
const COLOR_TOP: [f32; 4] = [132.0, 27.0, 16.0, 16.0];
// renderCustomOptions(top = guiTop + 10 + gap + font height + gap = 29): the priority field (BUTTON_DOWN stretched
// to 32 x 16) at (left + priLeft, top - 5), "Priority" at (left + 12, top + 25), the value right-aligned to
// priLeft + priWidth - gap.
const PRI_FIELD: [f32; 4] = [56.0, 24.0, 32.0, 16.0];
const PRI_TEXT_Y: f32 = 54.0;
// BaseSettingsPanel's tooltips over the empty slots (filterInsertUpgradeTooltip, filterExtractUpgradeTooltip,
// functionUpgradeTooltip at rightColumn + 18, customTop + 43) and the function upgrade's INFO FakeButton.
const TIP_SLOTS: [[f32; 4]; 3] = [[22.0, 70.0, 18.0, 18.0], [112.0, 70.0, 18.0, 18.0], [130.0, 70.0, 18.0, 18.0]];
const INFO: [f32; 4] = [131.0, 92.0, 16.0, 16.0];
const ICON_INFO: [f32; 2] = [64.0, 144.0];

// One tab's panel: which settings class draws it and what that class has.
#[derive(Clone, Copy)]
struct Panel {
    // The redstone mode button and the signal colour beside it (ItemSettings, LiquidSettings, PowerSettings).
    rs: Option<([f32; 4], [f32; 4])>,
    // The channel row, round robin, self feed and the priority (ItemSettings, the ender conduit's LiquidSettings).
    ender: bool,
    // hasFilters / hasUpgrades.
    filters: bool,
    upgrades: bool,
    texture: &'static str,
}

fn panel(t: usize, fluid_tier: i32) -> Panel {
    let full = Panel { rs: Some((ITEM_RS, ITEM_COLOR)), ender: true, filters: true, upgrades: true, texture: "enderio:gui/40/filter_upgrade_settings" };
    match t {
        0 => full,
        // LiquidSettings: the ender conduit as ItemSettings (its own texture), the others only the redstone row.
        1 if fluid_tier == 2 => full,
        1 => Panel { rs: Some((RS_TOP, COLOR_TOP)), ender: false, filters: false, upgrades: false, texture: "enderio:gui/40/in_out_settings" },
        2 => Panel { rs: Some((RS_TOP, COLOR_TOP)), ender: false, filters: false, upgrades: false, texture: "enderio:gui/40/in_out_settings" },
        _ => Panel { rs: None, ender: false, filters: true, upgrades: false, texture: "enderio:gui/40/filter_settings" },
    }
}

// ConduitRedstoneModeControlable.setRedstoneControlMode: the signal colour button shows only for ON and OFF.
fn color_shown(data: &[i32], t: usize) -> bool {
    matches!(data.get(2 + t * 10 + 2).copied(), Some(1 | 2))
}

fn panel_of(data: &[i32], t: usize) -> Panel {
    panel(t, data.get(53).copied().unwrap_or(0))
}

fn tab_rect(i: usize) -> [f32; 4] {
    [GUI_W - 3.0, 4.0 + 24.0 * i as f32, 24.0, 24.0]
}

// createGhostSlots: the items drawn greyed in an empty slot, one a second (GhostBackgroundItemSlot).
fn ghost_items(t: usize, fluid_tier: i32, k: usize) -> &'static [&'static str] {
    use super::enderio_signal_filters::ITEMS as RS;
    const ITEM: [&str; 1] = ["enderio:basic_item_filter"];
    const FLUID: [&str; 1] = [super::enderio_filters::FLUID_FILTER];
    const UPGRADES: [&str; 2] = [SPEED_UPGRADE, SPEED_DOWNGRADE];
    // RedstoneSettings: sensor and timer in, the logic filters out.
    const RS_IN: [&str; 2] = [RS[10], RS[9]];
    const RS_OUT: [&str; 8] = [RS[0], RS[2], RS[3], RS[6], RS[1], RS[7], RS[5], RS[4]];
    match (t, k) {
        (0, 0 | 1) => &ITEM,
        (1, 0 | 1) if fluid_tier == 2 => &FLUID,
        (0, 2) => &UPGRADES,
        (1, 2) if fluid_tier == 2 => &UPGRADES,
        (3, 0) => &RS_IN,
        (3, 1) => &RS_OUT,
        _ => &[],
    }
}

fn color_button(game: &Game<'_>, batch: &mut horizon_client::hud::HudBatch, o: [f32; 2], r: [f32; 4], col: i32, mouse: [f32; 2]) {
    super::enderio_ui::button(game, batch, o, r, false, mouse);
    let c = DYE_COLORS[col.clamp(0, 15) as usize];
    super::enderio_ui::fill(game, batch, o, [r[0] + 2.0, r[1] + 2.0, r[2] - 4.0, r[3] - 4.0], 0xFF00_0000 | c);
}

fn check(game: &Game<'_>, batch: &mut horizon_client::hud::HudBatch, o: [f32; 2], r: [f32; 4], on: bool, mouse: [f32; 2]) {
    super::enderio_ui::button(game, batch, o, r, false, mouse);
    if on {
        super::enderio_ui::ec(game, batch, o, [r[0], r[1]], [BUTTON_CHECKED[0], BUTTON_CHECKED[1], 16.0, 16.0]);
    }
}

pub(in crate::game) fn screen_draw(game: &Game<'_>, data: &[i32], batch: &mut horizon_client::hud::HudBatch, o: [f32; 2]) {
    if data.len() < 64 {
        return;
    }
    let mouse = super::enderio_ui::local_cursor(game, o);
    let Some(t) = active(game, data) else { return };
    let view = filter_view(game, data);
    let want = if view.is_some() { 8 * data[50] } else { data[52] & 7 };
    CUI.with_borrow_mut(|u| {
        u.data = data.to_vec();
        u.want = Some(want);
        u.view = view.clone();
    });
    if let Some((_, stack, sticky_ok)) = view {
        if data_base(data) == Base::Redstone {
            super::enderio_signal_filters::draw(game, batch, o, &stack, data);
        } else {
            super::enderio_filters::draw(game, batch, o, &stack, sticky_ok);
        }
        return;
    }
    let at = 2 + t * 10;
    let p = panel_of(data, t);
    super::enderio_ui::spr(game, batch, p.texture, o, [0.0, 0.0], [0.0, 0.0, GUI_W, GUI_H]);
    // The ghost backgrounds of the empty slots the tab shows, greyed by the background at 0.75.
    let second = std::time::SystemTime::now().duration_since(std::time::UNIX_EPOCH).map_or(0, |d| d.as_secs()) as usize;
    for k in 0..3 {
        let items = ghost_items(t, data[53], k);
        if items.is_empty() || data[52] & (1 << k) == 0 || client_slot(game, k).is_some() {
            continue;
        }
        let s = SLOTS[k];
        game.native_gui_draw_item(batch, &ItemStack::new(items[second % items.len()], 1), [o[0] + s[0], o[1] + s[1]]);
        super::enderio_ui::spr_alpha(game, batch, p.texture, o, s, [s[0], s[1], 16.0, 16.0], 0.75);
    }
    let gears = [gear_shown(game, data, 0), gear_shown(game, data, 1)];
    let empty: [bool; 3] = std::array::from_fn(|k| client_slot(game, k).is_none());
    CUI.with_borrow_mut(|u| {
        u.gears = gears;
        u.empty = empty;
    });
    for k in 0..2 {
        if gears[k] {
            super::enderio_ui::button(game, batch, o, GEARS[k], false, mouse);
            super::enderio_ui::icon(game, batch, o, [GEARS[k][0], GEARS[k][1]], ICON_GEAR_LIGHT, [16.0, 16.0], [16.0, 16.0]);
        }
    }
    if t == 3 {
        super::enderio_signal_filters::draw_panel(game, batch, o, data, mouse);
    }
    // renderStdTab: each tab at (xSize - 3, 4 + 24 x n), its frame from TAB_BG / TAB_FRAME_LEFT / TAB_FRAME_RIGHT.
    let cur = CUI.with_borrow(|u| u.tab);
    for (i, tt) in tabs(data).into_iter().enumerate() {
        let r = tab_rect(i);
        let on = i == cur;
        let bx = if on { 0.0 } else { 3.0 };
        let w = 24.0 - 3.0 - bx;
        super::enderio_ui::icon(game, batch, o, [r[0] + bx, r[1]], [208.0 + bx, 48.0], [w, 24.0], [w, 24.0]);
        super::enderio_ui::icon(game, batch, o, [r[0] + bx, r[1]], [208.0 + bx, 24.0], [w, 24.0], [w, 24.0]);
        super::enderio_ui::icon(game, batch, o, [r[0] + 3.0, r[1]], [208.0 + 48.0 - 21.0, 0.0], [21.0, 24.0], [21.0, 24.0]);
        super::enderio_ui::icon(game, batch, o, [r[0] + 4.0, r[1] + 4.0], TAB_ICONS[tt], [16.0, 16.0], [16.0, 16.0]);
    }
    let gray = 0xFF40_4040;
    let m = data[at + 1].clamp(0, 4) as u8;
    // The headings: RedstoneSettings has its own (getInputHeading / getOutputHeading).
    let (insert, extract) = if t == 3 {
        (super::enderio_ui::lang("enderio.gui.redstone_conduit_input_mode", "Input"), super::enderio_ui::lang("enderio.gui.redstone_conduit_output_mode", "Output"))
    } else {
        (super::enderio_ui::lang("enderio.gui.conduit_insert_mode", "Insert"), super::enderio_ui::lang("enderio.gui.conduit_extract_mode", "Extract"))
    };
    super::enderio_ui::text(game, batch, &insert, o[0] + 42.0, o[1] + 10.0, gray, false);
    super::enderio_ui::text(game, batch, &extract, o[0] + 134.0, o[1] + 10.0, gray, false);
    check(game, batch, o, CHECK_INSERT, accepts_output(m), mouse);
    check(game, batch, o, CHECK_EXTRACT, accepts_input(m), mouse);
    if p.upgrades {
        // The function upgrade's FakeButton (IconEIO.INFO, no frame).
        super::enderio_ui::icon(game, batch, o, [INFO[0], INFO[1]], ICON_INFO, [16.0, 16.0], [16.0, 16.0]);
    }
    if let Some((rs, col)) = p.rs {
        super::enderio_ui::button(game, batch, o, rs, false, mouse);
        super::enderio_ui::icon(game, batch, o, [rs[0], rs[1]], super::enderio_ui::ICON_RS[data[at + 2].clamp(0, 3) as usize], [16.0, 16.0], [16.0, 16.0]);
        if color_shown(data, t) {
            color_button(game, batch, o, col, data[at + 3], mouse);
        }
    }
    if p.ender {
        color_button(game, batch, o, ITEM_INSERT_CH, data[at + 5], mouse);
        color_button(game, batch, o, ITEM_EXTRACT_CH, data[at + 4], mouse);
        for (r, on, a, b) in [(ITEM_RR, data[at + 7] != 0, ICON_ROUND_ROBIN, ICON_ROUND_ROBIN_OFF), (ITEM_LOOP, data[at + 8] != 0, ICON_LOOP, ICON_LOOP_OFF)] {
            super::enderio_ui::button(game, batch, o, r, false, mouse);
            super::enderio_ui::icon(game, batch, o, [r[0], r[1]], if on { a } else { b }, [16.0, 16.0], [16.0, 16.0]);
        }
        // The priority (renderCustomOptions): the field, its + and - buttons, the label and the value.
        super::enderio_ui::ec_stretch(game, batch, o, [48.0, 208.0, 16.0, 16.0], PRI_FIELD);
        for (r, uv) in [(ITEM_PRI_UP, [208.0, 32.0]), (ITEM_PRI_DOWN, [208.0, 40.0])] {
            let hover = super::enderio_ui::inside(r, mouse);
            super::enderio_ui::ec(game, batch, o, [r[0], r[1]], [uv[0], uv[1] + if hover { 16.0 } else { 0.0 }, 8.0, 8.0]);
        }
        let pri = i32::from(data[at + 6] as i16).to_string();
        let label = super::enderio_ui::lang("enderio.gui.priority", "Priority");
        super::enderio_ui::text(game, batch, &label, o[0] + 22.0, o[1] + PRI_TEXT_Y, 0xFF00_0000, false);
        let w = super::enderio_ui::width(game, &pri);
        super::enderio_ui::text(game, batch, &pri, o[0] + PRI_FIELD[0] + PRI_FIELD[2] - w - 5.0, o[1] + PRI_TEXT_Y, 0xFF00_0000, false);
    }
    if let Some(pk) = CUI.with_borrow(|u| u.picker) {
        if let (true, Some((rs, _))) = (pk == t, p.rs) {
            super::enderio_ui::draw_picker(game, batch, o, rs, data[at + 2]);
        }
    }
}

pub(in crate::game) fn screen_tooltip(game: &Game<'_>, data: &[i32], at: [f32; 2], lines: &mut Vec<String>) {
    if data.len() < 64 || CUI.with_borrow(|u| u.picker.is_some()) {
        return;
    }
    if let Some((_, stack, sticky_ok)) = CUI.with_borrow(|u| u.view.clone()).filter(|_| data[50] != 0) {
        if data_base(data) == Base::Redstone {
            super::enderio_signal_filters::tooltip(&stack, data, at, lines);
        } else {
            super::enderio_filters::tooltip(game, &stack, sticky_ok, at, lines);
        }
        return;
    }
    let gears = CUI.with_borrow(|u| u.gears);
    if GEARS.iter().enumerate().any(|(k, r)| gears[k] && super::enderio_ui::inside(*r, at)) {
        lines.push(super::enderio_ui::lang("enderio.gui.edit_item_filter", "Edit Filter Settings"));
        return;
    }
    if data_base(data) == Base::Redstone {
        super::enderio_signal_filters::panel_tooltip(data, at, lines);
    }
    let Some(t) = CUI.with_borrow(|u| tabs(data).get(u.tab).copied()) else { return };
    let i = 2 + t * 10;
    let l = super::enderio_ui::lang;
    let color_name = |c: i32| {
        let names = ["black", "red", "green", "brown", "blue", "purple", "cyan", "silver", "gray", "pink", "lime", "yellow", "lightBlue", "magenta", "orange", "white"];
        l(&format!("enderio.color.{}", names[c.clamp(0, 15) as usize]), names[c.clamp(0, 15) as usize])
    };
    let p = panel_of(data, t);
    let rs = p.rs.filter(|(r, _)| super::enderio_ui::inside(*r, at)).is_some();
    let col = color_shown(data, t) && p.rs.filter(|(_, c)| super::enderio_ui::inside(*c, at)).is_some();
    // The empty slots' tooltips (GuiToolTip.shouldDraw: no stack in the slot); a filled slot shows its stack's.
    let empty_tip = |k: usize| super::enderio_ui::inside(TIP_SLOTS[k], at) && data[52] & (1 << k) != 0 && CUI.with_borrow(|u| u.empty[k]);
    if rs {
        let key = ["ignore", "on", "off", "never"][data[i + 2].clamp(0, 3) as usize];
        let english = ["Always active", "Active with signal", "Active without signal", "Never active"][data[i + 2].clamp(0, 3) as usize];
        lines.push(l("enderio.gui.tooltip.redstoneControlMode", "Redstone Mode"));
        lines.push(l(&format!("enderio.gui.tooltip.redstoneControlMode.{key}"), english));
    } else if col {
        lines.push(l("enderio.gui.signal_color", "Signal Color"));
        lines.push(color_name(data[i + 3]));
    } else if p.ender && super::enderio_ui::inside(ITEM_INSERT_CH, at) {
        lines.push(l("enderio.gui.conduit_channel", "Channel"));
        lines.push(color_name(data[i + 5]));
    } else if p.ender && super::enderio_ui::inside(ITEM_EXTRACT_CH, at) {
        lines.push(l("enderio.gui.conduit_channel", "Channel"));
        lines.push(color_name(data[i + 4]));
    } else if p.ender && super::enderio_ui::inside(ITEM_RR, at) {
        lines.push(if data[i + 7] != 0 { l("enderio.gui.round_robin_enabled", "Round Robin Enabled") } else { l("enderio.gui.round_robin_disabled", "Round Robin Disabled") });
    } else if p.ender && super::enderio_ui::inside(ITEM_LOOP, at) {
        lines.push(if data[i + 8] != 0 { l("enderio.gui.self_feed_enabled", "Self Feed Enabled") } else { l("enderio.gui.self_feed_disabled", "Self Feed Disabled") });
    } else if p.upgrades && super::enderio_ui::inside(INFO, at) {
        // getFunctionUpgradeToolTipText: ItemConduit's items per operation, EnderLiquidConduit's speed and mB.
        if t == 0 {
            lines.push(l("enderio.gui.item_function_upgrade.details", "Effective speed:"));
            lines.push(l("enderio.gui.item_function_upgrade.details2", "%s item(s) per operation").replacen("%s", &data[i + 9].to_string(), 1));
        } else {
            let pct = data[i + 9];
            let mb = l("enderio.fluid.millibucket.format", "%s mB").replacen("%s", &(2 * pct).to_string(), 1);
            lines.push(l("enderio.gui.liquid_function_upgrade.details", "Effective speed:"));
            lines.push(l("enderio.gui.liquid_function_upgrade.details2", "%s %% / %s per operation").replacen("%s", &pct.to_string(), 1).replacen("%s", &mb, 1).replace("%%", "%"));
        }
    } else if p.filters && (empty_tip(0) || empty_tip(1)) {
        lines.push(l("enderio.gui.item_filter_upgrade", "Filter Upgrade"));
    } else if p.upgrades && empty_tip(2) {
        lines.push(l("enderio.gui.item_function_upgrade", "Function Upgrade"));
        lines.push(l("enderio.gui.item_function_upgrade2", "Max Speed Upgrades is 15"));
        lines.push(l("enderio.gui.item_function_upgrade3", "Max Speed Downgrades is 3"));
    }
}

// The wheel on the settings screen: over an open filter screen's ghost slot (GuiContainerBase.mouseWheel), a request
// the server checks like a click (`edit`). True when the wheel was used.
pub(in crate::game) fn scrolled(game: &Game<'_>, at: [f32; 2], by: f64) -> bool {
    // A sideways-only turn comes as 0; Java's wheel is one step per event, up when positive.
    if by == 0.0 {
        return false;
    }
    let Some((k, stack)) = CUI.with_borrow(|u| (u.data.len() >= 64 && u.data[50] != 0 && data_base(&u.data) != Base::Redstone).then(|| u.view.clone()).flatten()).map(|(k, s, _)| (k, s)) else {
        return false;
    };
    match super::enderio_filters::wheel_press(&stack, at, by > 0.0) {
        Some(a) if a >= 0 => super::enderio_ui::send_button(game, B_FILTER + 200 * k as i32 + a),
        Some(_) => {}
        None => return false,
    }
    true
}

// The settings screen's clicks: a tab, the check boxes, the panel's buttons; each change goes to the server.
pub(in crate::game) fn pressed(game: &Game<'_>, at: [f32; 2], right: bool) -> bool {
    let data = CUI.with_borrow(|u| u.data.clone());
    let data = &data[..];
    if data.len() < 64 {
        return false;
    }
    let ts = tabs(data);
    let Some(t) = active(game, data) else { return false };
    if let Some((k, stack, sticky_ok)) = CUI.with_borrow(|u| u.view.clone()).filter(|_| data[50] != 0) {
        let hit = if data_base(data) == Base::Redstone {
            super::enderio_signal_filters::press(&stack, data, at, right)
        } else {
            super::enderio_filters::press(&stack, sticky_ok, at, right)
        };
        match hit {
            Some(super::enderio_filters::A_CLOSE) => super::enderio_ui::send_button(game, B_FILTER_CLOSE),
            Some(a) if a >= 0 => super::enderio_ui::send_button(game, B_FILTER + 200 * k as i32 + a),
            Some(_) => {}
            None => return false,
        }
        return true;
    }
    for k in 0..2 {
        if !right && super::enderio_ui::inside(GEARS[k], at) && gear_shown(game, data, k) {
            super::enderio_ui::send_button(game, B_GEAR + k as i32);
            return true;
        }
    }
    if t == 3 && super::enderio_signal_filters::panel_press(game, data, at, right) {
        return true;
    }
    let send = |k: i32| super::enderio_ui::send_button(game, 100 + 20 * t as i32 + k);
    let pan = panel_of(data, t);
    if let Some(p) = CUI.with_borrow(|u| u.picker) {
        CUI.with_borrow_mut(|u| u.picker = None);
        if let Some((btn, _)) = panel_of(data, p).rs {
            let cur = data[2 + p * 10 + 2];
            if super::enderio_ui::inside(super::enderio_ui::picker_bounds(btn, cur), at) {
                if !right {
                    if let Some(i) = super::enderio_ui::picker_cell(btn, cur, at) {
                        super::enderio_ui::send_button(game, 100 + 20 * p as i32 + B_RS + i);
                    }
                }
                return true;
            }
        }
    }
    for i in 0..ts.len() {
        if super::enderio_ui::inside(tab_rect(i), at) && !right {
            CUI.with_borrow_mut(|u| u.tab = i);
            // The server's slots follow the tab (setInOutSlotsVisible).
            super::enderio_ui::send_button(game, B_TAB + ts[i] as i32);
            return true;
        }
    }
    let step_by = |next: i32, prev: i32| if right { prev } else { next };
    if !right && super::enderio_ui::inside(CHECK_INSERT, at) {
        send(B_INSERT);
    } else if !right && super::enderio_ui::inside(CHECK_EXTRACT, at) {
        send(B_EXTRACT);
    } else if pan.rs.is_some_and(|(r, _)| super::enderio_ui::inside(r, at)) {
        CUI.with_borrow_mut(|u| u.picker = Some(t));
    } else if color_shown(data, t) && pan.rs.is_some_and(|(_, c)| super::enderio_ui::inside(c, at)) {
        send(step_by(B_COLOR_NEXT, B_COLOR_PREV));
    } else if pan.ender && super::enderio_ui::inside(ITEM_INSERT_CH, at) {
        send(step_by(B_INSERT_CH_NEXT, B_INSERT_CH_PREV));
    } else if pan.ender && super::enderio_ui::inside(ITEM_EXTRACT_CH, at) {
        send(step_by(B_EXTRACT_CH_NEXT, B_EXTRACT_CH_PREV));
    } else if pan.ender && !right && super::enderio_ui::inside(ITEM_RR, at) {
        send(B_ROUND_ROBIN);
    } else if pan.ender && !right && super::enderio_ui::inside(ITEM_LOOP, at) {
        send(B_SELF_FEED);
    } else if pan.ender && !right && super::enderio_ui::inside(ITEM_PRI_UP, at) {
        send(B_PRIORITY_UP);
    } else if pan.ender && !right && super::enderio_ui::inside(ITEM_PRI_DOWN, at) {
        send(B_PRIORITY_DOWN);
    } else {
        return false;
    }
    true
}

// ---- Fluid conduits (LiquidConduit, AdvancedLiquidConduit, EnderLiquidConduit and their networks)

// ConduitConfig: tier1/2/3 extractRate and maxIO, mB.
const FLUID_EXTRACT: [i32; 3] = [50, 100, 200];
const FLUID_MAX_IO: [i32; 3] = [200, 400, 800];

fn fluid_external(game: &Game<'_>, pos: Pos, d: usize) -> bool {
    let n = step(pos, d);
    !is_bundle(game, n) && game.native_fluid_tanks(game.native_dimension(), n, FACES[opposite(d)]).is_some()
}

// IFluidWrapper over Horizon's fluid helpers: the neighbour at `n` through `face`.
fn ext_available(game: &Game<'_>, n: Pos, face: Direction) -> Option<(String, i32)> {
    let tanks = game.native_fluid_tanks(game.native_dimension(), n, face)?;
    tanks.into_iter().find(|t| t.can_drain && t.amount > 0 && t.fluid.is_some()).map(|t| (t.fluid.unwrap_or_default(), t.amount))
}

// offer: what a fill would take (the room of the fillable tanks of that fluid or empty).
fn ext_offer(game: &Game<'_>, n: Pos, face: Direction, fluid: &str, amount: i32) -> i32 {
    let Some(tanks) = game.native_fluid_tanks(game.native_dimension(), n, face) else { return 0 };
    let room: i64 = tanks.iter().filter(|t| t.can_fill && t.fluid.as_deref().is_none_or(|f| f == fluid)).map(|t| i64::from((t.capacity - t.amount).max(0))).sum();
    room.min(i64::from(amount.max(0))) as i32
}

fn ext_fill(game: &mut Game<'_>, n: Pos, face: Direction, fluid: &str, amount: i32) -> i32 {
    if amount <= 0 {
        return 0;
    }
    amount - game.native_fluid_insert(n, face, fluid, amount)
}

// drain(FluidStack): only that fluid (another drained by mistake goes back).
fn ext_drain(game: &mut Game<'_>, n: Pos, face: Direction, fluid: &str, amount: i32) -> i32 {
    let dim = game.native_dimension().to_owned();
    let Some((f, got)) = game.native_fluid_extract(&dim, n, face, amount) else { return 0 };
    if f != fluid {
        game.native_fluid_insert(n, face, &f, got);
        return 0;
    }
    got
}

impl Conduit {
    // ConduitTank, kept as `tank` {FluidName, Amount}.
    pub(in crate::game) fn tank(&self) -> (Option<String>, i32) {
        match super::enderio::gc(&self.nbt, "tank") {
            Some(t) => (super::enderio::gs(t, "FluidName").filter(|s| !s.is_empty()), gi(t, "Amount", 0).max(0)),
            None => (None, 0),
        }
    }
    fn set_tank(&mut self, fluid: Option<&str>, amount: i32) {
        let mut t = Compound::new();
        if let Some(f) = fluid.filter(|_| amount > 0) {
            super::enderio::ss(&mut t, "FluidName", f);
            si(&mut t, "Amount", amount);
        }
        self.nbt.insert(js("tank"), Tag::Compound(t));
    }
    fn fluid_locked(&self) -> bool {
        gi(&self.nbt, "fluidLocked", 0) != 0
    }
    // LiquidConduit.updateTank: 250 mB per connection.
    fn tank_capacity(&self) -> i32 {
        250 * (self.conns.count_ones() + self.ext.count_ones()) as i32
    }
    // autoExtractForDir: extract mode and the redstone condition (read from the cached signal).
    fn auto_extract(&self, game: &mut Game<'_>, pos: Pos, d: usize) -> bool {
        accepts_input(self.mode(d)) && rs_met(game, pos, self.rs_mode(d), self.rs_color(d))
    }
    // EnderLiquidConduit.getExtractSpeedMultiplier: 1; speed upgrades 1 + min(n, 15); downgrades 0.25 x n.
    fn fluid_speed(&self, d: usize) -> f32 {
        match self.stack("functionUpgrades", d) {
            Some(s) if &*s.item == SPEED_UPGRADE => 1.0 + (s.count as f32).min(15.0),
            Some(s) if &*s.item == SPEED_DOWNGRADE => 0.25 * s.count as f32,
            _ => 1.0,
        }
    }
}

// The fluid filter (FluidFilter): its fluids and blacklist, from the filter item's custom_data `filter`.
fn fluid_filter_passes(c: &Conduit, d: usize, input: bool, fluid: &str) -> bool {
    let key = if input { "inputFilterUpgrades" } else { "outputFilterUpgrades" };
    let Some(s) = c.stack(key, d).filter(|s| &*s.item == super::enderio_filters::FLUID_FILTER) else { return true };
    // FluidFilter.matchesFilter: an empty filter passes all; isValid is !blacklist && empty, so an empty blacklist
    // is a valid filter that passes all too.
    let (fluids, black) = super::enderio_filters::read_fluids(&s);
    if fluids.iter().all(Option::is_none) {
        return true;
    }
    fluids.iter().flatten().any(|x| x == fluid) != black
}

#[derive(Default)]
struct FluidNet {
    tier: usize,
    dirty: bool,
    cons: HashMap<Pos, Conduit>,
    fluid: Option<String>,
    locked: bool,
    ticks_empty: i32,
    // Normal: each conduit's tank, its push direction and token, its failed-extract counter.
    tanks: HashMap<Pos, i32>,
    start_dir: HashMap<Pos, usize>,
    token_seen: HashMap<Pos, i32>,
    token: i32,
    last_flow: usize,
    extract_ticks: HashMap<Pos, i32>,
    // Pressurized: the network's one tank, its outputs and their round robin.
    amount: i32,
    outputs: Vec<(Pos, usize)>,
    out_at: usize,
    // Ender: the network tanks by priority, each one's round robin.
    etanks: Vec<(Pos, usize)>,
    iters: HashMap<(Pos, usize), i32>,
}

fn rebuild_fluid(game: &Game<'_>, id: u32) {
    let Some(members) = NETS.with_borrow(|n| n.nets.get(&id).map(|x| x.members.clone())) else { return };
    let mut cons: HashMap<Pos, Conduit> = HashMap::new();
    for p in &members {
        if let Some(b) = bundle_at(game, *p) {
            if let Some(i) = find(&b, Base::Fluid) {
                cons.insert(*p, b[i].clone());
            }
        }
    }
    NETS.with_borrow_mut(|n| {
        let Some(f) = n.nets.get_mut(&id).and_then(|x| x.fluid.as_mut()) else { return };
        // Pressurized: the outputs are the faces that take output and don't auto-extract (refreshInputs).
        f.outputs = members.iter().flat_map(|p| {
            let c = cons.get(p);
            (0..6).filter_map(move |d| c.filter(|c| c.has_ext(d) && accepts_output(c.mode(d))).map(|_| (*p, d)))
        }).collect();
        // Ender: NetworkTank per external connection, priority high first.
        let mut et: Vec<(Pos, usize, i32)> = members.iter().flat_map(|p| {
            let c = cons.get(p);
            (0..6).filter_map(move |d| c.filter(|c| c.has_ext(d)).map(|c| (*p, d, c.priority(d))))
        }).collect();
        et.sort_by(|a, b| b.2.cmp(&a.2));
        f.etanks = et.into_iter().map(|(p, d, _)| (p, d)).collect();
        for (p, c) in &cons {
            let cap = c.tank_capacity();
            if let Some(v) = f.tanks.get_mut(p) {
                *v = (*v).min(cap);
            }
        }
        f.cons = cons;
        f.dirty = false;
    });
}

fn fluid_net_new(game: &Game<'_>, members: &[Pos], own: &[Conduit], pos: Pos) -> FluidNet {
    let mut f = FluidNet { dirty: true, ..FluidNet::default() };
    for p in members {
        let b = if *p == pos { Some(own.to_vec()) } else { bundle_at(game, *p) };
        let Some(b) = b else { continue };
        let Some(i) = find(&b, Base::Fluid) else { continue };
        let c = &b[i];
        f.tier = c.def.tier;
        let (fl, amount) = c.tank();
        if f.fluid.is_none() {
            f.fluid = fl.clone();
        }
        f.locked |= c.fluid_locked();
        if fl.is_some() && fl == f.fluid {
            f.tanks.insert(*p, amount);
            f.amount += amount;
        }
    }
    f.amount = f.amount.min(1000 * members.len() as i32);
    f
}

// The share each conduit keeps (written into its block entity each tick; AdvancedLiquidConduitNetwork.setConduitVolumes).
fn fluid_share(f: &FluidNet, members: &[Pos], pos: Pos) -> (Option<String>, i32) {
    if f.tier == 1 {
        let n = members.len().max(1) as i32;
        let i = members.iter().position(|p| *p == pos).unwrap_or(0) as i32;
        let v = f.amount / n + i32::from(i < f.amount % n);
        return (f.fluid.clone(), v);
    }
    (f.fluid.clone(), f.tanks.get(&pos).copied().unwrap_or(0))
}

fn fluid_tick(game: &mut Game<'_>, id: u32) {
    if NETS.with_borrow(|n| n.nets.get(&id).and_then(|x| x.fluid.as_ref()).is_some_and(|f| f.dirty)) {
        rebuild_fluid(game, id);
    }
    let Some((tier, members)) = NETS.with_borrow(|n| n.nets.get(&id).and_then(|x| x.fluid.as_ref().map(|f| (f.tier, x.members.clone())))) else { return };
    // Each conduit's doExtract: while it succeeds every tick, after a failure every 16th.
    for p in &members {
        let Some(c) = NETS.with_borrow(|n| n.nets.get(&id).and_then(|x| x.fluid.as_ref()).and_then(|f| f.cons.get(p).cloned())) else { continue };
        // LiquidConduit.doExtract: updateStartPushDir every tick, before hasExtractableMode and the 16-tick gate.
        if tier == 0 {
            advance_push_dir(id, *p, &c);
        }
        if !(0..6).any(|d| c.has_ext(d) && accepts_input(c.mode(d))) {
            continue;
        }
        let t = NETS.with_borrow_mut(|n| {
            let f = n.nets.get_mut(&id)?.fluid.as_mut()?;
            let e = f.extract_ticks.entry(*p).or_insert(0);
            let v = *e;
            *e = e.wrapping_add(1);
            Some(v)
        }).unwrap_or(1);
        if t & 15 != 0 {
            continue;
        }
        for d in (0..6).filter(|d| c.has_ext(*d)) {
            if c.auto_extract(game, *p, d) {
                let ok = match tier {
                    0 => normal_extract(game, id, *p, &c, d),
                    1 => pressurized_extract(game, id, *p, d),
                    _ => ender_extract(game, id, *p, &c, d),
                };
                if ok {
                    NETS.with_borrow_mut(|n| {
                        if let Some(f) = n.nets.get_mut(&id).and_then(|x| x.fluid.as_mut()) {
                            f.extract_ticks.insert(*p, 0);
                        }
                    });
                }
            }
        }
    }
    match tier {
        0 => normal_flow(game, id, &members),
        1 => pressurized_push(game, id),
        _ => {}
    }
}

// ---- Normal fluid conduits: each its own tank, pushed by token, flowing down and levelling (LiquidConduitNetwork)

fn advance_push_dir(id: u32, p: Pos, c: &Conduit) {
    NETS.with_borrow_mut(|n| {
        let Some(f) = n.nets.get_mut(&id).and_then(|x| x.fluid.as_mut()) else { return };
        let start = f.start_dir.get(&p).copied().unwrap_or(0);
        let mut v = (start + 1) % 6;
        while v != start && !c.connected(v) {
            v = (v + 1) % 6;
        }
        f.start_dir.insert(p, v);
    });
}

// pushLiquid: into this conduit's tank, then each direction from its start (not back, not an auto-extracting face):
// neighbour conduits recursively, external tanks filled.
fn push_liquid(game: &mut Game<'_>, id: u32, p: Pos, from: Option<usize>, fluid: &str, amount: i32, token: i32, depth: u32) -> i32 {
    let Some((c, seen, start, have)) = NETS.with_borrow(|n| {
        let f = n.nets.get(&id)?.fluid.as_ref()?;
        Some((f.cons.get(&p)?.clone(), f.token_seen.get(&p).copied(), f.start_dir.get(&p).copied().unwrap_or(0), f.tanks.get(&p).copied().unwrap_or(0)))
    }) else { return 0 };
    if seen == Some(token) || amount <= 0 || depth > 512 {
        return 0;
    }
    let cap = c.tank_capacity();
    let local = amount.min(cap - have).max(0);
    NETS.with_borrow_mut(|n| {
        if let Some(f) = n.nets.get_mut(&id).and_then(|x| x.fluid.as_mut()) {
            f.token_seen.insert(p, token);
            *f.tanks.entry(p).or_insert(0) += local;
            if f.fluid.is_none() && local > 0 {
                f.fluid = Some(fluid.to_owned());
            }
        }
    });
    let mut pushed = local;
    let mut left = amount - local;
    let mut d = start;
    loop {
        if left <= 0 {
            break;
        }
        // canOutputToDir: insert allowed, not auto-extracting, a connection there.
        let out_ok = Some(d) != from && accepts_output(c.mode(d)) && !c.auto_extract(game, p, d) && c.connected(d);
        if out_ok {
            if c.has_conn(d) {
                let got = push_liquid(game, id, step(p, d), Some(opposite(d)), fluid, left, token, depth + 1);
                left -= got;
                pushed += got;
            } else if c.has_ext(d) {
                let got = ext_fill(game, step(p, d), FACES[opposite(d)], fluid, left);
                left -= got;
                pushed += got;
            }
        }
        d = (d + 1) % 6;
        if d == start {
            break;
        }
    }
    pushed
}

// LiquidConduit.doExtract: up to 50 mB of what the neighbour has, if this conduit's tank takes that fluid.
fn normal_extract(game: &mut Game<'_>, id: u32, p: Pos, c: &Conduit, d: usize) -> bool {
    let n = step(p, d);
    let face = FACES[opposite(d)];
    let Some((fluid, avail)) = ext_available(game, n, face) else { return false };
    let compatible = NETS.with_borrow(|x| x.nets.get(&id).and_then(|x| x.fluid.as_ref()).is_none_or(|f| f.fluid.as_deref().is_none_or(|g| g == fluid)));
    if !accepts_input(c.mode(d)) || !compatible {
        return false;
    }
    let amount = avail.min(FLUID_EXTRACT[0]);
    let token = NETS.with_borrow_mut(|x| x.nets.get_mut(&id).and_then(|x| x.fluid.as_mut()).map_or(0, |f| {
        f.token += 1;
        f.token
    }));
    let used = push_liquid(game, id, p, Some(d), &fluid, amount, token, 0);
    if used > 0 {
        // What was pushed is drained from the source (pushLiquid's tanks already hold it).
        let got = ext_drain(game, n, face, &fluid, used);
        if got < used {
            // The source gave less than it offered: the difference comes out of this conduit's tank.
            NETS.with_borrow_mut(|x| {
                if let Some(f) = x.nets.get_mut(&id).and_then(|x| x.fluid.as_mut()) {
                    let t = f.tanks.entry(p).or_insert(0);
                    *t = (*t - (used - got)).max(0);
                }
            });
        }
        return true;
    }
    false
}

fn normal_flow(game: &mut Game<'_>, id: u32, members: &[Pos]) {
    let Some((fluid, locked, empty)) = NETS.with_borrow(|n| n.nets.get(&id).and_then(|x| x.fluid.as_ref()).map(|f| (f.fluid.clone(), f.locked, f.tanks.values().all(|v| *v <= 0)))) else { return };
    if empty {
        // An empty, unlocked network forgets its fluid after 40 ticks.
        NETS.with_borrow_mut(|n| {
            if let Some(f) = n.nets.get_mut(&id).and_then(|x| x.fluid.as_mut()) {
                if !locked && f.fluid.is_some() {
                    f.ticks_empty += 1;
                    if f.ticks_empty > 40 {
                        f.fluid = None;
                        f.ticks_empty = 0;
                    }
                }
            }
        });
        return;
    }
    let Some(fluid) = fluid else { return };
    // Every max(1000, viscosity) / 500 ticks: water and the like every 2nd tick, lava (6000) every 12th.
    let visc = if fluid == "minecraft:lava" { 6000 } else { 1000 };
    if game.world.game_time % i64::from(visc / 500) != 0 {
        return;
    }
    let mut actions: Vec<(Pos, Pos, i32)> = Vec::new();
    let count = members.len().min(10);
    for _ in 0..count {
        let i = NETS.with_borrow_mut(|n| n.nets.get_mut(&id).and_then(|x| x.fluid.as_mut()).map_or(0, |f| {
            if f.last_flow >= members.len() {
                f.last_flow = 0;
            }
            let i = f.last_flow;
            f.last_flow += 1;
            i
        }));
        flow_from(game, id, members[i], &fluid, &mut actions);
    }
    for (from, to, amount) in actions {
        NETS.with_borrow_mut(|n| {
            let Some(f) = n.nets.get_mut(&id).and_then(|x| x.fluid.as_mut()) else { return };
            let cap_to = f.cons.get(&to).map_or(0, |c| c.tank_capacity());
            let have_from = f.tanks.get(&from).copied().unwrap_or(0);
            let have_to = f.tanks.get(&to).copied().unwrap_or(0);
            let actual = amount.min(have_from).min(cap_to - have_to).max(0);
            *f.tanks.entry(from).or_insert(0) -= actual;
            *f.tanks.entry(to).or_insert(0) += actual;
        });
    }
    // When every conduit holds under 10 mB, each drains into the nearest external tank that takes it.
    let low = NETS.with_borrow(|n| n.nets.get(&id).and_then(|x| x.fluid.as_ref()).is_some_and(|f| members.iter().all(|p| f.tanks.get(p).copied().unwrap_or(0) < 10)));
    if !low {
        return;
    }
    let cons = NETS.with_borrow(|n| n.nets.get(&id).and_then(|x| x.fluid.as_ref()).map(|f| f.cons.clone()).unwrap_or_default());
    let mut externals: Vec<(Pos, usize)> = Vec::new();
    for p in members {
        let Some(c) = cons.get(p) else { continue };
        for d in (0..6).filter(|d| c.has_ext(*d)) {
            if accepts_output(c.mode(d)) && !c.auto_extract(game, *p, d) {
                externals.push((*p, d));
            }
        }
    }
    for p in members {
        let have = NETS.with_borrow(|n| n.nets.get(&id).and_then(|x| x.fluid.as_ref()).and_then(|f| f.tanks.get(p).copied())).unwrap_or(0);
        if have <= 0 {
            continue;
        }
        let mut best: Option<(i64, Pos, usize)> = None;
        for (q, d) in &externals {
            let n = step(*q, *d);
            let dist = i64::from(n[0] - p[0]).pow(2) + i64::from(n[1] - p[1]).pow(2) + i64::from(n[2] - p[2]).pow(2);
            if best.is_none_or(|b| dist < b.0) && ext_offer(game, n, FACES[opposite(*d)], &fluid, have) > 0 {
                best = Some((dist, *q, *d));
            }
        }
        if let Some((_, q, d)) = best {
            let got = ext_fill(game, step(q, d), FACES[opposite(d)], &fluid, have);
            NETS.with_borrow_mut(|n| {
                if let Some(f) = n.nets.get_mut(&id).and_then(|x| x.fluid.as_mut()) {
                    *f.tanks.entry(*p).or_insert(0) -= got;
                }
            });
        }
    }
}

// flowFrom: down first, then the external outputs (an even share, 20 mB at most each), then level with the joined
// conduits that aren't higher and less full.
fn flow_from(game: &mut Game<'_>, id: u32, p: Pos, fluid: &str, actions: &mut Vec<(Pos, Pos, i32)>) {
    let max_flow = 20;
    let Some((c, cons, mut tanks)) = NETS.with_borrow(|n| n.nets.get(&id).and_then(|x| x.fluid.as_ref()).and_then(|f| Some((f.cons.get(&p)?.clone(), f.cons.clone(), f.tanks.clone())))) else { return };
    let have = tanks.get(&p).copied().unwrap_or(0);
    if have <= 0 {
        return;
    }
    let cap = c.tank_capacity();
    if c.has_conn(0) {
        let down = step(p, 0);
        if let Some(dc) = cons.get(&down) {
            let room = dc.tank_capacity() - tanks.get(&down).copied().unwrap_or(0);
            let moved = have.min(room).min(FLUID_MAX_IO[0]).max(0);
            *tanks.entry(p).or_insert(0) -= moved;
            *tanks.entry(down).or_insert(0) += moved;
        }
    }
    let mut total = tanks.get(&p).copied().unwrap_or(0);
    if total > 0 {
        let mut requested = 0;
        let mut requests = 0;
        let outs: Vec<usize> = (0..6).filter(|d| c.has_ext(*d) && accepts_output(c.mode(*d))).collect();
        let outs: Vec<usize> = outs.into_iter().filter(|d| !c.auto_extract(game, p, *d)).collect();
        for d in &outs {
            let a = ext_offer(game, step(p, *d), FACES[opposite(*d)], fluid, total);
            if a > 0 {
                requested += a;
                requests += 1;
            }
        }
        if requests > 0 {
            let per = (total.min(requested) / requests).min(max_flow);
            for d in &outs {
                let got = ext_fill(game, step(p, *d), FACES[opposite(*d)], fluid, per);
                if got > 0 {
                    *tanks.entry(p).or_insert(0) -= got;
                }
            }
        }
        total = tanks.get(&p).copied().unwrap_or(0);
    }
    if total > 0 {
        let ratio = |q: Pos, t: &HashMap<Pos, i32>| {
            let cc = cons.get(&q).map_or(0, |c| c.tank_capacity());
            if cc <= 0 { 0.0 } else { t.get(&q).copied().unwrap_or(0) as f32 / cc as f32 }
        };
        let my = ratio(p, &tanks);
        let joined: Vec<Pos> = (0..6).filter(|d| c.has_conn(*d)).map(|d| step(p, d)).filter(|q| cons.contains_key(q)).collect();
        let can = |q: Pos| q[1] <= p[1] && ratio(q, &tanks) < my;
        let (mut amount, mut capacity) = (total, cap);
        for q in joined.iter().filter(|q| can(**q)) {
            amount += tanks.get(q).copied().unwrap_or(0);
            capacity += cons.get(q).map_or(0, |c| c.tank_capacity());
        }
        if capacity > 0 {
            let target = amount as f32 / capacity as f32;
            let flow = (((target - my) * cap as f32).floor() as i32).min(max_flow);
            if flow.abs() >= 2 {
                for q in joined.iter().filter(|q| can(**q)) {
                    let qc = cons.get(q).map_or(0, |c| c.tank_capacity());
                    let v = ((target - ratio(*q, &tanks)) * qc as f32).floor() as i32;
                    if v > 0 {
                        actions.push((p, *q, v));
                    } else if v < 0 {
                        actions.push((*q, p, -v));
                    }
                }
            }
        }
    }
    NETS.with_borrow_mut(|n| {
        if let Some(f) = n.nets.get_mut(&id).and_then(|x| x.fluid.as_mut()) {
            f.tanks = tanks;
        }
    });
}

// ---- Pressurized: one tank, 1000 mB a conduit, offered to the outputs in turn (AdvancedLiquidConduitNetwork)

fn pressurized_extract(game: &mut Game<'_>, id: u32, p: Pos, d: usize) -> bool {
    let n = step(p, d);
    let face = FACES[opposite(d)];
    let Some((members, fluid, amount)) = NETS.with_borrow(|x| x.nets.get(&id).and_then(|x| x.fluid.as_ref().map(|f| (x.members.len(), f.fluid.clone(), f.amount)))) else { return false };
    let cap = 1000 * members as i32;
    if amount >= cap {
        return false;
    }
    let max = FLUID_EXTRACT[1].min(cap - amount);
    let fluid = match fluid.filter(|_| amount > 0) {
        Some(f) => f,
        None => match ext_available(game, n, face) {
            Some((f, a)) if a > 0 => f,
            _ => return false,
        },
    };
    let got = ext_drain(game, n, face, &fluid, max);
    if got <= 0 {
        return false;
    }
    NETS.with_borrow_mut(|x| {
        if let Some(f) = x.nets.get_mut(&id).and_then(|x| x.fluid.as_mut()) {
            f.fluid = Some(fluid);
            f.amount += got;
        }
    });
    true
}

fn pressurized_push(game: &mut Game<'_>, id: u32) {
    let Some((fluid, amount, locked, outputs, at, cons)) = NETS.with_borrow(|n| n.nets.get(&id).and_then(|x| x.fluid.as_ref()).map(|f| (f.fluid.clone(), f.amount, f.locked, f.outputs.clone(), f.out_at, f.cons.clone()))) else { return };
    if fluid.is_none() || amount <= 0 || outputs.is_empty() {
        if amount <= 0 && !locked && fluid.is_some() {
            NETS.with_borrow_mut(|n| {
                if let Some(f) = n.nets.get_mut(&id).and_then(|x| x.fluid.as_mut()) {
                    f.ticks_empty += 1;
                    if f.ticks_empty > 40 {
                        f.fluid = None;
                        f.ticks_empty = 0;
                    }
                }
            });
        }
        return;
    }
    let fluid = fluid.unwrap_or_default();
    let mut left = amount;
    let mut at = at;
    for _ in 0..outputs.len() {
        if left <= 0 {
            break;
        }
        if at >= outputs.len() {
            at = 0;
        }
        let (p, d) = outputs[at];
        at += 1;
        // An auto-extracting face takes no output (canInputToDir).
        if cons.get(&p).is_some_and(|c| c.auto_extract(game, p, d)) {
            continue;
        }
        left -= ext_fill(game, step(p, d), FACES[opposite(d)], &fluid, left);
    }
    NETS.with_borrow_mut(|n| {
        if let Some(f) = n.nets.get_mut(&id).and_then(|x| x.fluid.as_mut()) {
            f.amount = left.max(0);
            f.out_at = at;
            f.ticks_empty = 0;
        }
    });
}

// ---- Ender: no storage, straight from a source to the targets (EnderLiquidConduitNetwork)

fn ender_extract(game: &mut Game<'_>, id: u32, p: Pos, c: &Conduit, d: usize) -> bool {
    let n = step(p, d);
    let face = FACES[opposite(d)];
    let limit = (FLUID_EXTRACT[2] as f32 * c.fluid_speed(d)) as i32;
    let Some(tanks) = game.native_fluid_tanks(game.native_dimension(), n, face) else { return false };
    // Multi-tank sources drain per tank.
    let mut left = limit;
    for t in tanks.into_iter().filter(|t| t.can_drain && t.amount > 0) {
        let Some(fluid) = t.fluid.clone() else { continue };
        if !fluid_filter_passes(c, d, true, &fluid) {
            continue;
        }
        let want = t.amount.min(left);
        let accepted = ender_fill_from(game, id, (p, d), &fluid, want, false);
        if accepted <= 0 {
            continue;
        }
        let got = ext_drain(game, n, face, &fluid, want.min(accepted));
        if got > 0 {
            let filled = ender_fill_from(game, id, (p, d), &fluid, got, true);
            if filled < got {
                game.native_fluid_insert(n, face, &fluid, got - filled);
            }
            left -= filled;
        }
        if left <= 0 {
            return true;
        }
    }
    left < limit
}

// fillFrom: to the targets (not itself without self feed, insert allowed, NetworkTank's colours: the target's insert
// channel (getOutputColor) = the source's extract channel (getInputColor), the filters), up to 800 x the speed; round robin keeps its place, else it starts over each fill.
fn ender_fill_from(game: &mut Game<'_>, id: u32, src: (Pos, usize), fluid: &str, amount: i32, execute: bool) -> i32 {
    let Some((tanks, cons, start)) = NETS.with_borrow(|n| n.nets.get(&id).and_then(|x| x.fluid.as_ref()).map(|f| (f.etanks.clone(), f.cons.clone(), f.iters.get(&src).copied().unwrap_or(-1)))) else { return 0 };
    let Some(sc) = cons.get(&src.0) else { return 0 };
    if !fluid_filter_passes(sc, src.1, true, fluid) {
        return 0;
    }
    let mut remaining = amount.min((FLUID_MAX_IO[2] as f32 * sc.fluid_speed(src.1)) as i32);
    let mut filled = 0;
    let mut idx = start;
    let len = tanks.len() as i32;
    for _ in 0..len {
        idx += 1;
        if idx >= len {
            idx = 0;
        }
        let (tp, td) = tanks[idx as usize];
        let Some(tc) = cons.get(&tp) else { continue };
        if ((tp, td) == src && !sc.self_feed(src.1)) || !accepts_output(tc.mode(td)) || tc.out_color(td) != sc.in_color(src.1) || !fluid_filter_passes(tc, td, false, fluid) {
            continue;
        }
        let n = step(tp, td);
        let face = FACES[opposite(td)];
        let vol = if execute { ext_fill(game, n, face, fluid, remaining) } else { ext_offer(game, n, face, fluid, remaining) };
        remaining -= vol;
        filled += vol;
        if remaining <= 0 || (!execute && filled > 0) {
            break;
        }
    }
    if execute {
        let keep = if sc.round_robin(src.1) { idx } else { -1 };
        NETS.with_borrow_mut(|n| {
            if let Some(f) = n.nets.get_mut(&id).and_then(|x| x.fluid.as_mut()) {
                f.iters.insert(src, keep);
            }
        });
    }
    filled
}

// ---- The bundle's faces as other blocks' fluid helpers reach them (the custom fluid capability)

pub(in crate::game) fn custom_block_fluids(block: &str, custom: &mut bool) {
    if block == BUNDLE {
        *custom = true;
    }
}

fn face_fluid_conduit(game: &Game<'_>, pos: Pos, side: Option<Direction>) -> Option<(Conduit, usize, Option<u32>)> {
    let d = side? as usize;
    let b = bundle_at(game, pos)?;
    let c = b[find(&b, Base::Fluid)?].clone();
    c.has_ext(d).then(|| (c, d, net_of(game, pos, Base::Fluid)))
}

pub(in crate::game) fn custom_fluid_tanks(game: &Game<'_>, pos: Pos, side: Option<Direction>, out: &mut Option<Vec<horizon_client::mod_api::NativeFluidTankView>>) {
    if out.is_some() || !is_bundle(game, pos) {
        return;
    }
    let Some((c, d, id)) = face_fluid_conduit(game, pos, side) else {
        *out = Some(Vec::new());
        return;
    };
    let view = id.and_then(|id| NETS.with_borrow(|n| n.nets.get(&id).and_then(|x| x.fluid.as_ref().map(|f| (f.fluid.clone(), if f.tier == 1 { f.amount } else { f.tanks.get(&pos).copied().unwrap_or(0) }, x.members.len())))));
    let (fluid, amount, members) = view.unwrap_or((c.tank().0, c.tank().1, 1));
    let capacity = match c.def.tier {
        0 => c.tank_capacity(),
        1 => 1000 * members as i32,
        _ => 0,
    };
    let m = c.mode(d);
    *out = Some(if c.def.tier == 2 {
        Vec::new()
    } else {
        vec![horizon_client::mod_api::NativeFluidTankView {
            tank: 0,
            content: fluid.filter(|_| amount > 0).map(|f| horizon_client::mod_api::NativeFluidStack { fluid: f, amount, components: Compound::new() }),
            capacity,
            can_fill: c.def.tier == 1 && accepts_input(m),
            can_drain: accepts_output(m),
        }]
    });
}

pub(in crate::game) fn custom_fluid_fill(game: &mut Game<'_>, pos: Pos, side: Option<Direction>, resource: &horizon_client::mod_api::NativeFluidStack, action: horizon_client::mod_api::NativeFluidAction, out: &mut Option<i32>) {
    if out.is_some() || !is_bundle(game, pos) {
        return;
    }
    *out = Some(0);
    let Some((c, d, Some(id))) = face_fluid_conduit(game, pos, side) else { return };
    if !accepts_input(c.mode(d)) || resource.amount <= 0 || !resource.components.is_empty() {
        return;
    }
    let execute = action == horizon_client::mod_api::NativeFluidAction::Execute;
    match c.def.tier {
        // LiquidConduit.fill: 0 (normal conduits only pull).
        0 => {}
        // AdvancedLiquidConduitNetwork.fill: 400 a call, into the one tank.
        1 => {
            let got = NETS.with_borrow_mut(|n| {
                let x = n.nets.get_mut(&id)?;
                let members = x.members.len() as i32;
                let f = x.fluid.as_mut()?;
                if f.fluid.as_deref().is_some_and(|g| g != resource.fluid && f.amount > 0) {
                    return Some(0);
                }
                let take = resource.amount.min(FLUID_MAX_IO[1]).min(1000 * members - f.amount).max(0);
                if execute && take > 0 {
                    f.amount += take;
                    f.fluid = Some(resource.fluid.clone());
                }
                Some(take)
            });
            *out = Some(got.unwrap_or(0));
        }
        _ => {
            *out = Some(ender_fill_from(game, id, (pos, d), &resource.fluid, resource.amount, execute));
        }
    }
}

pub(in crate::game) fn custom_fluid_drain(game: &mut Game<'_>, pos: Pos, side: Option<Direction>, request: &horizon_client::mod_api::NativeFluidDrain, action: horizon_client::mod_api::NativeFluidAction, out: &mut Option<horizon_client::mod_api::NativeFluidStack>) {
    if out.is_some() || !is_bundle(game, pos) {
        return;
    }
    let Some((c, d, Some(id))) = face_fluid_conduit(game, pos, side) else { return };
    if !accepts_output(c.mode(d)) || c.def.tier == 2 {
        return;
    }
    let execute = action == horizon_client::mod_api::NativeFluidAction::Execute;
    let (want, want_fluid) = match request {
        horizon_client::mod_api::NativeFluidDrain::Any(n) => (*n, None),
        horizon_client::mod_api::NativeFluidDrain::Exact(s) => (s.amount, Some(s.fluid.clone())),
    };
    let got = NETS.with_borrow_mut(|n| {
        let f = n.nets.get_mut(&id)?.fluid.as_mut()?;
        let fluid = f.fluid.clone()?;
        if want_fluid.as_ref().is_some_and(|w| *w != fluid) {
            return None;
        }
        let have = if f.tier == 1 { f.amount } else { f.tanks.get(&pos).copied().unwrap_or(0) };
        let take = want.min(have).min(if f.tier == 1 { FLUID_MAX_IO[1] } else { i32::MAX }).max(0);
        if take <= 0 {
            return None;
        }
        if execute {
            if f.tier == 1 {
                f.amount -= take;
            } else if let Some(t) = f.tanks.get_mut(&pos) {
                *t -= take;
            }
        }
        Some(horizon_client::mod_api::NativeFluidStack { fluid, amount: take, components: Compound::new() })
    });
    *out = got;
}

// RenderUtil.getStillTexture: the fluid's still sprite and its colour.
fn fluid_sprite(f: &str) -> (&'static str, u32) {
    match f {
        "minecraft:water" => ("minecraft:block/water_still", 0x3F_76E4),
        "minecraft:lava" => ("minecraft:block/lava_still", 0xFF_FFFF),
        _ => {
            let name = f.strip_prefix("enderio:").unwrap_or(f);
            // Interned once per fluid: the drawn quads keep &'static sprite names.
            thread_local! {
                static NAMES: RefCell<HashMap<String, &'static str>> = RefCell::new(HashMap::new());
            }
            let s = NAMES.with_borrow_mut(|m| *m.entry(name.to_owned()).or_insert_with(|| Box::leak(format!("enderio:block/fluid_{name}_still").into_boxed_str())));
            (s, 0xFF_FFFF)
        }
    }
}

// ---- The insulated redstone conduit (InsulatedRedstoneConduit, RedstoneConduitNetwork, BundledSignal)

impl Conduit {
    // forceConnectionMode: a face the wrench set, whatever the neighbour is.
    fn forced(&self, d: usize) -> Option<u8> {
        let v = gi(&self.nbt, &format!("forcedConnection.{}", DIR_NAMES[d]), -1);
        (0..=4).contains(&v).then_some(v as u8)
    }
    fn force(&mut self, d: usize, m: u8) {
        let k = format!("forcedConnection.{}", DIR_NAMES[d]);
        if m == NOT_SET {
            self.nbt.remove(&js(&k));
        } else {
            si(&mut self.nbt, &k, i32::from(m));
        }
        self.set_mode(d, if m == NOT_SET { OUTPUT } else { m });
    }
    pub(in crate::game) fn rs_input_color(&self, d: usize) -> i32 {
        self.dir_i("inputSignalColor", d, RED).clamp(0, 15)
    }
    pub(in crate::game) fn rs_output_color(&self, d: usize) -> i32 {
        self.dir_i("outputSignalColor", d, GREEN).clamp(0, 15)
    }
    pub(in crate::game) fn rs_strong(&self, d: usize) -> bool {
        self.dir_i("signalStrong", d, 0) != 0
    }
}

// ConnectivityTool.shouldAutoConnectRedstone: the redstone-aware blocks (its list, and what canConnectRedstone).
fn redstone_aware(block: &str) -> bool {
    let b = block.strip_prefix("minecraft:").unwrap_or(block);
    matches!(b, "redstone_lamp" | "dispenser" | "dropper" | "note_block" | "piston" | "sticky_piston" | "tnt" | "redstone_wire"
        | "repeater" | "comparator" | "lever" | "redstone_torch" | "redstone_wall_torch" | "redstone_block" | "observer"
        | "daylight_detector" | "target" | "hopper" | "powered_rail" | "activator_rail" | "detector_rail" | "trapped_chest"
        | "tripwire_hook" | "light_weighted_pressure_plate" | "heavy_weighted_pressure_plate" | "command_block" | "crafter")
        || b.ends_with("_button") || b.ends_with("_pressure_plate") || b.ends_with("_door") || b.ends_with("_trapdoor") || b.ends_with("_fence_gate")
}

fn redstone_external(game: &Game<'_>, pos: Pos, c: &Conduit, d: usize) -> bool {
    match c.forced(d) {
        Some(DISABLED) => false,
        Some(IN_OUT | OUTPUT | INPUT) => true,
        _ => {
            let n = game.native_block(step(pos, d));
            n != BUNDLE && redstone_aware(&n)
        }
    }
}

thread_local! {
    // Set while a conduit reads the world: the network's own output is 0 then (setNetworkEnabled(false)).
    static READING: std::cell::Cell<bool> = const { std::cell::Cell::new(false) };
    // Bundles whose neighbours changed: their redstone network reads its inputs again.
    static RS_DIRTY: RefCell<HashSet<Pos>> = RefCell::new(HashSet::new());
}

#[derive(Default)]
struct RedstoneNet {
    dirty: bool,
    // BundledSignal: each colour's strength (the sum of its signals, 15 at most).
    bundled: [i32; 16],
    // Each emitting face: (weak, strong).
    outputs: HashMap<(Pos, usize), (i32, i32)>,
    // A member has an input signal filter (timer, sensor): the network ticks it (RedstoneConduitNetwork.tickEnd).
    ticking: bool,
}

// getExternalPowerLevel of face `d`: the signal the neighbour gives towards the conduit (its strong power first, a
// wire's level). Horizon has no per-face signal read: the vanilla sources answer from their state, anything else
// from what reaches that block (the network's own output held at 0 while it reads).
fn face_input(game: &mut Game<'_>, pos: Pos, d: usize) -> i32 {
    let n = step(pos, d);
    let block = game.native_block(n);
    let b = block.strip_prefix("minecraft:").unwrap_or(&block).to_owned();
    let prop = |game: &Game<'_>, k: &str| game.native_block_property(n, k);
    let powered = |game: &Game<'_>| prop(game, "powered").as_deref() == Some("true");
    let power = |game: &Game<'_>| prop(game, "power").and_then(|v| v.parse::<i32>().ok()).unwrap_or(0);
    match b.as_str() {
        "air" | "cave_air" | "void_air" => 0,
        "redstone_wire" | "daylight_detector" | "target" | "light_weighted_pressure_plate" | "heavy_weighted_pressure_plate" => power(game),
        "redstone_block" => 15,
        "redstone_torch" | "redstone_wall_torch" => if prop(game, "lit").as_deref() == Some("true") { 15 } else { 0 },
        "lever" | "observer" | "tripwire_hook" | "detector_rail" => if powered(game) { 15 } else { 0 },
        "repeater" | "comparator" => {
            // A diode gives out of its front only: its facing is the direction from the reader to it.
            let facing = prop(game, "facing").unwrap_or_default();
            if facing == PROP_NAMES[d] && powered(game) { 15 } else { 0 }
        }
        _ if b.ends_with("_button") || b.ends_with("_pressure_plate") => if powered(game) { 15 } else { 0 },
        _ => {
            let dim = game.native_dimension().to_owned();
            READING.with(|r| r.set(true));
            let v = game.native_neighbor_signal(&dim, n);
            READING.with(|r| r.set(false));
            v
        }
    }
}

fn redstone_update(game: &mut Game<'_>, id: u32) {
    let Some(members) = NETS.with_borrow(|n| n.nets.get(&id).map(|x| x.members.clone())) else { return };
    let mut bundled = [0i32; 16];
    let mut outs: HashMap<(Pos, usize), (i32, i32)> = HashMap::new();
    let mut cons: Vec<(Pos, Conduit)> = Vec::new();
    for p in &members {
        let Some(b) = bundle_at(game, *p) else { continue };
        let Some(i) = find(&b, Base::Redstone) else { continue };
        cons.push((*p, b[i].clone()));
    }
    for (p, c) in &cons {
        for d in (0..6).filter(|d| c.has_ext(*d) && accepts_output(c.mode(*d))) {
            // acceptSignalsForDir: not from another redstone conduit.
            if bundle_at(game, step(*p, d)).is_some_and(|nb| find(&nb, Base::Redstone).is_some()) {
                continue;
            }
            let raw = face_input(game, *p, d);
            let level = super::enderio_signal_filters::apply_input(game, c, *p, d, raw);
            let col = c.rs_input_color(d) as usize;
            bundled[col] = (bundled[col] + level).min(15);
        }
    }
    // The stateful output filters' new item NBT (Java keeps it on the filter in the conduit).
    let mut states: Vec<(Pos, usize, ItemStack)> = Vec::new();
    let mut ticking = false;
    for (p, c) in &cons {
        for d in (0..6).filter(|d| accepts_input(c.mode(*d))) {
            let (weak, state) = super::enderio_signal_filters::apply_output(game, c, *p, d, c.rs_output_color(d), &bundled);
            states.extend(state.map(|s| (*p, d, s)));
            outs.insert((*p, d), (weak, if c.rs_strong(d) { weak } else { 0 }));
        }
        ticking |= (0..6).any(|d| super::enderio_signal_filters::input_filter(c, d).is_some());
    }
    for (p, d, s) in states {
        game.native_with_mod_block_entity(p, |e| {
            let mut b = load(&e.nbt);
            if let Some(i) = find(&b, Base::Redstone) {
                b[i].set_stack("inputFilterUpgrades", d, Some(&s));
                save(&b, &mut e.nbt);
            }
        });
    }
    let changed = NETS.with_borrow_mut(|n| {
        let Some(r) = n.nets.get_mut(&id).and_then(|x| x.redstone.as_mut()) else { return false };
        let changed = r.outputs != outs || r.bundled != bundled;
        r.outputs = outs;
        r.bundled = bundled;
        r.dirty = false;
        r.ticking = ticking;
        changed
    });
    if changed {
        // notifyNeigborsOfSignalUpdate: every member's neighbours, and theirs.
        let dim = game.native_dimension().to_owned();
        for p in &members {
            game.native_update_signal_neighbors(&dim, *p);
        }
        // The other conduits' redstone control reads the channels.
        for p in &members {
            receptors_changed(game, *p);
        }
    }
}

fn redstone_tick(game: &mut Game<'_>, id: u32) {
    let members = NETS.with_borrow(|n| n.nets.get(&id).map(|x| x.members.clone())).unwrap_or_default();
    let touched = RS_DIRTY.with_borrow(|s| members.iter().any(|p| s.contains(p)));
    let (dirty, ticking) = NETS.with_borrow(|n| n.nets.get(&id).and_then(|x| x.redstone.as_ref()).map_or((false, false), |r| (r.dirty, r.ticking)));
    // checkTickingFilters: each member's timers count; one at its time (or the sensor) reads the inputs again.
    let mut due = false;
    if ticking {
        for p in &members {
            let Some(b) = bundle_at(game, *p) else { continue };
            let Some(i) = find(&b, Base::Redstone) else { continue };
            for d in 0..6 {
                due |= super::enderio_signal_filters::tick(game, &b[i], *p, d);
            }
        }
    }
    if dirty || touched || due {
        redstone_update(game, id);
    }
}

// getInternalRedstoneSignalForColor: the bundle's redstone network's channel.
fn internal_signal(game: &Game<'_>, pos: Pos, color: i32) -> i32 {
    let Some(id) = net_of(game, pos, Base::Redstone) else { return 0 };
    NETS.with_borrow(|n| n.nets.get(&id).and_then(|x| x.redstone.as_ref()).map_or(0, |r| r.bundled[color.clamp(0, 15) as usize]))
}

// `block_signal`: isProvidingWeakPower / isProvidingStrongPower of the bundle's face, from the network's answer.
// Asked for every block's signal: the dimension's name is only made for a bundle.
pub(in crate::game) fn block_signal_at(block: &str, level: impl FnOnce() -> String, pos: Pos, face: Direction, direct: bool, signal: &mut i32) {
    if block == BUNDLE {
        block_signal(&level(), pos, block, face, direct, signal);
    }
}

// `level` is the level's Dimension, as Debug names it (Overworld, Nether, End).
pub(in crate::game) fn block_signal(level: &str, pos: Pos, block: &str, face: Direction, direct: bool, signal: &mut i32) {
    if block != BUNDLE || READING.with(|r| r.get()) {
        return;
    }
    let dim = match level {
        "Nether" => "minecraft:the_nether",
        "End" => "minecraft:the_end",
        _ => "minecraft:overworld",
    };
    let Some(id) = NETS.with_borrow(|n| n.of.get(&(dim.to_owned(), pos, Base::Redstone)).copied()) else {
        *signal = 0;
        return;
    };
    let d = face as usize;
    let v = NETS.with_borrow(|n| n.nets.get(&id).and_then(|x| x.redstone.as_ref()).and_then(|r| r.outputs.get(&(pos, d)).copied()));
    *signal = v.map_or(0, |(weak, strong)| if direct { strong } else { weak });
}

// `is_signal_source`: redstone wire beside a bundle turns to it (canConnectRedstone; Horizon asks by state only).
pub(in crate::game) fn is_signal_source(block: &str, source: &mut bool) {
    if block == BUNDLE {
        *source = true;
    }
}

// InsulatedRedstoneConduit.onBlockActivated: a dye on an arm sets its colour (the output one where it emits); the
// wrench joins (both faces NOT_SET first), forces an external face IN_OUT, cycles it, or disconnects.
fn redstone_activated(game: &mut Game<'_>, pos: Pos, b: &mut Vec<Conduit>, i: usize, held: Option<&ItemStack>, hit: Hit) -> bool {
    let wrench = held.is_some_and(|h| &*h.item == WRENCH);
    if let (Some(col), Some(d)) = (held.and_then(dye_of), hit.comp.dir.filter(|_| hit.comp.kind != Ck::Core)) {
        let key = if accepts_input(b[i].mode(d)) { "outputSignalColor" } else { "inputSignalColor" };
        b[i].set_dir_i(key, d, col);
        write_bundle(game, pos, b);
        rs_dirty(game, pos);
        return true;
    }
    if !wrench {
        return false;
    }
    match hit.comp.dir.filter(|_| hit.comp.kind != Ck::Core) {
        None => {
            let d = hit.face;
            let n = step(pos, d);
            if let Some(mut nb) = bundle_at(game, n) {
                if let Some(j) = find(&nb, Base::Redstone) {
                    if nb[j].mode(opposite(d)) == DISABLED {
                        nb[j].force(opposite(d), NOT_SET);
                        write_bundle(game, n, &nb);
                    }
                }
                b[i].force(d, NOT_SET);
                return connect(game, pos, b, i, d);
            }
            b[i].force(d, IN_OUT);
            b[i].ext |= 1 << d;
            write_bundle(game, pos, b);
            destroy_at(game, pos, Base::Redstone);
            true
        }
        Some(d) => {
            if b[i].has_ext(d) {
                let next = (b[i].mode(d) + 1) % 5;
                b[i].force(d, next);
                if !redstone_external(game, pos, &b[i], d) {
                    b[i].ext &= !(1 << d);
                }
                write_bundle(game, pos, b);
                destroy_at(game, pos, Base::Redstone);
            } else if b[i].has_conn(d) {
                disconnect(game, pos, b, i, d);
            }
            true
        }
    }
}

fn rs_dirty(game: &Game<'_>, pos: Pos) {
    if let Some(id) = net_of(game, pos, Base::Redstone) {
        NETS.with_borrow_mut(|n| {
            if let Some(r) = n.nets.get_mut(&id).and_then(|x| x.redstone.as_mut()) {
                r.dirty = true;
            }
        });
    }
}

// ---- The conduit probe and the Yeta wrench's display mode (ItemConduitProbe, ToolTickHandler, ConduitDisplayMode)

const CHANNEL: &str = "enderio:conduit_tool";

// The probe's mode (meta 0 probe, 1 copy/paste), kept as custom_data `probeMode`.
fn probe_mode(s: &ItemStack) -> i32 {
    super::enderio::custom_data(s).map_or(0, |c| gi(c, "probeMode", 0)).clamp(0, 1)
}

// The item model of each mode (1.12's item_conduit_probe variants `probe` and `copy`): the item definition reads
// custom_model_data flag 0, set here from the mode.
fn sync_probe_model(stack: &mut ItemStack) {
    let copy = probe_mode(stack) == 1;
    let extra = stack.extra.get_or_insert_with(Default::default);
    extra.other.retain(|(k, _)| k != "minecraft:custom_model_data");
    if copy {
        let mut data = Compound::new();
        data.insert(js("flags"), Tag::List(vec![Tag::Byte(1)]));
        extra.other.push(("minecraft:custom_model_data".to_owned(), Tag::Compound(data)));
        extra.other.sort_by(|a, b| a.0.cmp(&b.0));
    }
}

// `wheel`: sneak + wheel with the wrench or the probe in hand changes its mode (PacketConduitProbeMode /
// PacketYetaWrenchDisplayMode), on the server.
pub(in crate::game) fn wheel(game: &mut Game<'_>, steps: f32, claimed: &mut bool) {
    if *claimed || steps == 0.0 || !game.native_playing() || !game.native_has_shift_down() {
        return;
    }
    let Some(held) = game.held() else { return };
    if &*held.item != WRENCH && &*held.item != PROBE {
        return;
    }
    *claimed = true;
    let dir: u8 = if steps > 0.0 { 1 } else { 0 };
    let slot = game.selected as u8;
    let _ = game.native_net_send_to_server(CHANNEL, &[dir, slot]);
}

pub(in crate::game) fn net_from_client(game: &mut Game<'_>, player: u128, channel: &str, data: &[u8]) {
    if channel != CHANNEL || data.len() != 2 || data[1] > 8 {
        return;
    }
    let Some(held) = game.native_held_stack(player) else { return };
    let forward = data[0] == 0;
    let mut new = held.clone();
    let mut c = super::enderio::custom_data(&held).cloned().unwrap_or_default();
    if &*held.item == WRENCH {
        // ConduitDisplayMode.next / previous over NEUTRAL, NONE, ALL and the types.
        let cur = display_mode(Some(&held));
        let i = MODES.iter().position(|m| *m == cur).unwrap_or(2);
        let n = if forward { (i + 1) % MODES.len() } else { (i + MODES.len() - 1) % MODES.len() };
        super::enderio::ss(&mut c, "DISPLAYMODE", MODES[n].name());
    } else if &*held.item == PROBE {
        let m = 1 - probe_mode(&held);
        si(&mut c, "probeMode", m);
    } else {
        return;
    }
    super::enderio::set_custom_data(&mut new, c);
    if &*held.item == PROBE {
        sync_probe_model(&mut new);
    }
    let kind = crate::game::native_inventories::NativeInventoryKind::Player;
    let _ = game.native_set_inventory_slot(player, kind, data[1] as usize, Some(&held), Some(new));
}

fn rf(a: i64, b: i64) -> String {
    format!("{} / {}", super::enderio_ui::commas(a), super::enderio::lang_power_pub(b))
}

// getConduitProbeInformation of each conduit (PacketConduitProbe): sent to the player's chat.
fn probe(game: &mut Game<'_>, pos: Pos, uuid: u128) {
    let Some(b) = bundle_at(game, pos) else { return };
    let l = super::enderio_ui::lang;
    let mut lines: Vec<String> = Vec::new();
    for c in &b {
        let net = net_of(game, pos, c.def.base);
        if net.is_none() {
            lines.push(format!("\u{a7}c{}", l("enderio.gui.conduit.network.unloaded", "Conduit partially in unloaded chunk(s)")));
        }
        match c.def.base {
            Base::Energy => {
                let max = ENERGY_MAX_IO[c.def.tier.min(2)];
                let info = net.and_then(|id| NETS.with_borrow(|n| n.nets.get(&id).and_then(|x| x.energy.as_ref()).map(|e| {
                    (e.stored.get(&pos).copied().unwrap_or(0), e.energy, e.max, e.in_banks, e.max_banks, e.receptors.iter().map(|r| (r.pos, r.d)).collect::<Vec<_>>(), Tracker::avg(&e.tracker.sent), Tracker::avg(&e.tracker.recv))
                })));
                let stored = info.as_ref().map_or(c.energy(), |i| i.0);
                lines.push(format!("\u{a7}a{}", l("enderio.gui.conduit_probe.power.tracked_conduit.line1", "Energy Conduit")));
                lines.push(format!("\u{a7}9{}", l("enderio.gui.conduit_probe.power.tracked_conduit.line2", " - Internal Buffer: %s").replacen("%s", &rf(i64::from(stored), i64::from(max)), 1)));
                if let Some((_, energy, emax, banks, bmax, receptors, sent, recv)) = info {
                    // getPowerInReceptors: each receptor's buffer once.
                    let mut seen = HashSet::new();
                    let (mut r, mut rmax) = (0i64, 0i64);
                    let dim = game.native_dimension().to_owned();
                    for (p, d) in receptors {
                        let n = step(p, d);
                        if seen.insert(n) {
                            if let Some(e) = game.native_energy(&dim, n, FACES[opposite(d)]) {
                                r += i64::from(e.amount);
                                rmax += i64::from(e.capacity);
                            }
                        }
                    }
                    lines.push(format!("\u{a7}a{}", l("enderio.gui.conduit_probe.power.tracked_network.line1", "Energy Network")));
                    lines.push(format!("\u{a7}1{}", l("enderio.gui.conduit_probe.power.tracked_network.line2", " - Conduit Storage: %s").replacen("%s", &rf(energy, emax), 1)));
                    lines.push(format!("\u{a7}9{}", l("enderio.gui.conduit_probe.power.tracked_network.line3", " - Capacitor Bank Storage: %s").replacen("%s", &rf(banks, bmax), 1)));
                    lines.push(format!("\u{a7}b{}", l("enderio.gui.conduit_probe.power.tracked_network.line4", " - Machine Buffers: %s").replacen("%s", &rf(r, rmax), 1)));
                    lines.push(format!("\u{a7}9{}", l("enderio.gui.conduit_probe.power.tracked_network.line5", " - Average output over 5 seconds: %s").replacen("%s", &super::enderio_ui::rft(sent.round() as i64), 1)));
                    lines.push(format!("\u{a7}9{}", l("enderio.gui.conduit_probe.power.tracked_network.line6", " - Average input over 5 seconds: %s").replacen("%s", &super::enderio_ui::rft(recv.round() as i64), 1)));
                }
            }
            Base::Redstone => {
                if c.ext == 0 {
                    lines.push(format!("\u{a7}a{}", l("enderio.gui.conduit_probe.redstone.heading.no_connections", "Redstone Conduit has no external connections")));
                }
                for d in (0..6).filter(|d| c.has_ext(*d)) {
                    let name = game.native_block(step(pos, d));
                    lines.push(format!("\u{a7}a{}", l("enderio.gui.conduit_probe.redstone.heading", "Redstone Conduit connection to: %s").replacen("%s", &name, 1)));
                    let out = net.and_then(|id| NETS.with_borrow(|n| n.nets.get(&id).and_then(|x| x.redstone.as_ref()).and_then(|r| r.outputs.get(&(pos, d)).copied()))).unwrap_or((0, 0));
                    if accepts_input(c.mode(d)) {
                        lines.push(format!("\u{a7}9{}", l("enderio.gui.conduit_probe.redstone.strong", " - Provides strong signal level %s").replacen("%s", &out.1.to_string(), 1)));
                        lines.push(format!("\u{a7}9{}", l("enderio.gui.conduit_probe.redstone.weak", " - Provides weak signal level %s").replacen("%s", &out.0.to_string(), 1)));
                    }
                    if accepts_output(c.mode(d)) {
                        let v = face_input(game, pos, d);
                        lines.push(format!("\u{a7}9{}", l("enderio.gui.conduit_probe.redstone.external", " - Reads redstone level %s").replacen("%s", &v.to_string(), 1)));
                    }
                }
            }
            Base::Item => {
                if c.ext == 0 {
                    lines.push(format!("\u{a7}6{}", l("enderio.gui.conduit_probe.item.heading.no_connections", "Item Conduit has no external connections")));
                }
                // The item asked about: the off hand's (the probe is in the main hand).
                let input = game.native_player_equipment(uuid, horizon_world::player_inventory::EquipmentSlot::OffHand);
                let item_name = input.as_ref().map(|s| s.item.to_string()).unwrap_or_default();
                let (invs, cons) = net.and_then(|id| NETS.with_borrow(|n| n.nets.get(&id).and_then(|x| x.item.as_ref()).map(|i| (i.invs.clone(), i.cons.clone())))).unwrap_or_default();
                let at_name = |game: &Game<'_>, q: Pos, d: usize| {
                    let n = step(q, d);
                    format!("{} ({}, {}, {})", game.native_block(n), n[0], n[1], n[2])
                };
                for d in (0..6).filter(|d| c.has_ext(*d)) {
                    let facing = l(&format!("enderio.facing.{}", PROP_NAMES[d]), PROP_NAMES[d]);
                    lines.push(format!("\u{a7}a{}", l("enderio.gui.conduit_probe.item.heading", "Item Conduit connection to: %s").replacen("%s", &facing, 1)));
                    let me = invs.iter().position(|v| v.pos == pos && v.d == d);
                    if net.is_some() && accepts_input(c.mode(d)) {
                        // getTargetsForExtraction: this face's send order, by the item through each target's filter.
                        let targets: Vec<String> = me.map(|i| invs[i].targets.iter().filter(|j| {
                            let t = &invs[**j];
                            input.as_ref().is_none_or(|it| cons.get(&t.pos).and_then(|tc| valid_filter(tc, "outputFilterUpgrades", t.d)).is_none_or(|f| f.with_inventory(game, t.pos, t.d).passes(game, it)))
                        }).map(|j| at_name(game, invs[*j].pos, invs[*j].d)).collect()).unwrap_or_default();
                        let key = match (input.is_none(), targets.is_empty()) {
                            (true, true) => ("enderio.gui.conduit_probe.item.extract.no_item.no_targets", " - Extracted items have no valid outputs"),
                            (true, false) => ("enderio.gui.conduit_probe.item.extract.no_item.targets", " - Extracted items will be inserted into:"),
                            (false, true) => ("enderio.gui.conduit_probe.item.extract.item.no_targets", " - Item %s has no valid outputs"),
                            (false, false) => ("enderio.gui.conduit_probe.item.extract.item.targets", " - Item %s will be inserted into:"),
                        };
                        lines.push(format!("\u{a7}9{}", l(key.0, key.1).replacen("%s", &item_name, 1)));
                        lines.extend(targets.into_iter().map(|t| format!("  -> {t}")));
                    }
                    if net.is_some() && accepts_output(c.mode(d)) {
                        // getInputSourcesFor: the faces whose send order holds this one.
                        let sources: Vec<String> = me.map(|m| invs.iter().filter(|v| v.targets.contains(&m)).filter(|v| {
                            input.as_ref().is_none_or(|it| cons.get(&v.pos).and_then(|sc| valid_filter(sc, "inputFilterUpgrades", v.d)).is_none_or(|f| f.with_inventory(game, v.pos, v.d).passes(game, it)))
                        }).map(|v| at_name(game, v.pos, v.d)).collect()).unwrap_or_default();
                        let key = match (input.is_none(), sources.is_empty()) {
                            (true, true) => ("enderio.gui.conduit_probe.item.receive.no_items", " - Can not receive items"),
                            (false, true) => ("enderio.gui.conduit_probe.item.receive.no_item", " - Can not receive item %s"),
                            (true, false) => ("enderio.gui.conduit_probe.item.receive.items", " - Can receive items from:"),
                            (false, false) => ("enderio.gui.conduit_probe.item.receive.item", " - Can receive item %s from:"),
                        };
                        lines.push(format!("\u{a7}9{}", l(key.0, key.1).replacen("%s", &item_name, 1)));
                        lines.extend(sources.into_iter().map(|t| format!("  -> {t}")));
                    }
                }
            }
            Base::Fluid => {}
        }
    }
    for line in lines {
        game.native_tell(uuid, horizon_client::text::Text::legacy(&line));
    }
}

// YetaWrenchOverlayRenderer, overlay mode 0: after a change the mode in the middle (the previous and next beside it,
// 12 big) for 20 ticks, then in the bottom right corner.
const MODE_ICONS: [[f32; 2]; 7] = [[48.0, 224.0], [64.0, 192.0], [0.0, 192.0], [16.0, 128.0], [0.0, 128.0], [0.0, 112.0], [16.0, 112.0]];

thread_local! {
    static OVERLAY: RefCell<(Option<Mode>, i32, i64)> = const { RefCell::new((None, 0, 0)) };
}

pub(in crate::game) fn hud(game: &Game<'_>, batch: &mut horizon_client::hud::HudBatch) {
    let Some(held) = game.held().filter(|h| &*h.item == WRENCH) else { return };
    let mode = display_mode(Some(held));
    let now = game.world.game_time;
    let show = OVERLAY.with_borrow_mut(|o| {
        if o.0 != Some(mode) {
            *o = (Some(mode), 20, now);
        }
        if o.1 > 0 && o.2 < now {
            o.2 += 1;
            o.1 -= 1;
        }
        o.1
    });
    let i = MODES.iter().position(|m| *m == mode).unwrap_or(2);
    let (w, h) = (batch.width, batch.height);
    let icon = |batch: &mut horizon_client::hud::HudBatch, k: usize, at: [f32; 2], size: f32| {
        super::enderio_ui::icon(game, batch, [0.0, 0.0], at, MODE_ICONS[k], [16.0, 16.0], [size, size]);
    };
    if show > 0 {
        let (x, y) = ((w / 2.0).floor() - 8.0, (h / 2.0).floor() - 24.0);
        icon(batch, i, [x, y], 16.0);
        icon(batch, (i + MODES.len() - 1) % MODES.len(), [x - 18.0 + 4.0, y + 4.0], 12.0);
        icon(batch, (i + 1) % MODES.len(), [x + 18.0, y + 4.0], 12.0);
    } else {
        icon(batch, i, [w - 20.0, h - 20.0], 16.0);
    }
}
