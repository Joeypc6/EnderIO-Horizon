// Ender IO's machines' randomDisplayTick (AbstractMachineBlock, BlockSagMill, BlockStirlingGenerator): smoke while
// the block shows `active`, sampled as ClientLevel.animateTick picks its blocks (667 within 16 and 32), with a random
// of its own so vanilla's draws are left as they are.
use super::{AnimateContext, ParticleEngine, ParticleOptions, T};
use horizon_core::random::{LegacyRandom, RandomSource};
use horizon_world::nbt::{JavaString, Tag};
use std::cell::RefCell;

thread_local! {
    static RANDOM: RefCell<LegacyRandom> = RefCell::new(LegacyRandom::new(0x454e_4445_5249_4f));
}

fn int(r: &mut LegacyRandom, bound: i32) -> i32 {
    r.next_int_bound(bound).unwrap_or(0)
}

pub(super) fn animate(engine: &mut ParticleEngine, ctx: &AnimateContext<'_>, center: [i32; 3]) {
    RANDOM.with_borrow_mut(|r| {
        for _ in 0..667 {
            for radius in [16, 32] {
                let x = center[0] + int(r, radius) - int(r, radius);
                let y = center[1] + int(r, radius) - int(r, radius);
                let z = center[2] + int(r, radius) - int(r, radius);
                if !ctx.world.loaded(x, z) {
                    continue;
                }
                let state = ctx.state([x, y, z]);
                let name = ctx.facts.get(state).name.clone();
                if let Some(path) = name.strip_prefix("enderio:") {
                    block(engine, ctx, r, state, path, [x, y, z]);
                }
            }
        }
    });
}

const MACHINES: [&str; 17] = ["simple_powered_furnace", "simple_alloy_smelter", "alloy_smelter", "enhanced_alloy_smelter", "simple_sag_mill", "sag_mill", "enhanced_sag_mill", "simple_stirling_generator", "stirling_generator", "vat", "enhanced_vat", "soul_binder", "slice_and_splice", "painting_machine", "combustion_generator", "enhanced_combustion_generator", "zombie_generator"];
const ZOMBIES: [&str; 3] = ["zombie_generator", "franken_zombie_generator", "ender_generator"];

fn block(engine: &mut ParticleEngine, ctx: &AnimateContext<'_>, r: &mut LegacyRandom, state: horizon_world::registry::BlockStateId, path: &str, pos: [i32; 3]) {
    if !MACHINES.contains(&path) && !ZOMBIES.contains(&path) {
        return;
    }
    let Some(info) = ctx.registry.state(state) else { return };
    if info.properties.get("active").is_none_or(|v| v != "true") {
        return;
    }
    let facing = info.properties.get("facing").map_or("north".to_owned(), |f| f.to_string());
    let (fx, fz) = match facing.as_str() {
        "south" => (0.0, 1.0),
        "west" => (-1.0, 0.0),
        "east" => (1.0, 0.0),
        _ => (0.0, -1.0),
    };
    let [x, y, z] = pos.map(f64::from);
    let smoke = ParticleOptions::simple(T::Smoke);
    if path.ends_with("sag_mill") {
        for _ in 0..3 {
            let (mut px, mut pz) = (x + 0.5 + fx * 0.51, z + 0.5 + fz * 0.51);
            let py = y + f64::from(r.next_float()) * 0.8 + 0.1;
            let (mut vx, mut vz) = (0.0, 0.0);
            if fx != 0.0 {
                pz += f64::from(r.next_float()) * 0.8 - 0.4;
                vx = fx * 0.05;
            } else {
                px += f64::from(r.next_float()) * 0.8 - 0.4;
                vz = fz * 0.05;
            }
            engine.add(&smoke, [px, py, pz], [vx, 0.0, vz]);
        }
        return;
    }
    if path.ends_with("stirling_generator") {
        let lava = match ctx.world.block_entities.get(&(pos[0], pos[1], pos[2])) {
            Some(crate::containers::BlockEntity::Mod(e)) => matches!(e.nbt.get(&JavaString::from("isLavaFired")), Some(Tag::Int(v)) if *v != 0),
            _ => false,
        };
        for _ in 0..2 {
            let (mut px, mut pz) = (x + 0.5 + fx * 0.6, z + 0.5 + fz * 0.6);
            let (mut vx, mut vz) = (0.0, 0.0);
            if fx != 0.0 {
                pz += f64::from(r.next_float()) * 0.9 - 0.45;
                vx = fx * 0.05;
            } else {
                px += f64::from(r.next_float()) * 0.9 - 0.45;
                vz = fz * 0.05;
            }
            if lava && int(r, 40) == 0 {
                engine.add(&ParticleOptions::simple(T::Lava), [px, y + 0.1, pz], [0.0; 3]);
            }
            engine.add(&smoke, [px, y + 0.1, pz], [vx, 0.0, vz]);
        }
        return;
    }
    if ZOMBIES.contains(&path) {
        // BlockZombieGenerator: one display tick in three, two bubbles rising from the base (BubbleFX: drawn here
        // with vanilla's bubble pop, which lives out of water).
        if int(r, 3) == 0 {
            for _ in 0..2 {
                let px = x + 0.5 + f64::from(r.next_float()) * 0.6 - 0.3;
                let pz = z + 0.5 + f64::from(r.next_float()) * 0.6 - 0.3;
                engine.add(&ParticleOptions::simple(T::BubblePop), [px, y + 0.1, pz], [0.0, 0.05, 0.0]);
            }
        }
        return;
    }
    if path.ends_with("combustion_generator") {
        // BlockCombustionGenerator: puffs over the top, one cluster of two (the enhanced one three of five).
        let (clusters, puffs) = if path.starts_with("enhanced") { (3, 5) } else { (1, 2) };
        let top = if path.starts_with("enhanced") { 1.6875 } else { 1.0 };
        for _ in 0..clusters {
            let cx = x + 0.5 + f64::from(r.next_float()) * 0.5 - 0.25;
            let cz = z + 0.5 + f64::from(r.next_float()) * 0.5 - 0.25;
            for _ in 0..puffs {
                let px = cx + f64::from(r.next_float()) * 0.1 - 0.05;
                let pz = cz + f64::from(r.next_float()) * 0.1 - 0.05;
                engine.add(&smoke, [px, y + top, pz], [0.0, 0.02, 0.0]);
            }
        }
        return;
    }
    // AbstractMachineBlock: four puffs near the top's +x +z corner.
    for _ in 0..4 {
        let ox = -0.2 - f64::from(r.next_float()) * 0.6;
        let oy = -0.1 + f64::from(r.next_float()) * 0.2;
        let oz = -0.2 - f64::from(r.next_float()) * 0.6;
        engine.add(&smoke, [x + 1.0 + ox, y + 1.0 + oy, z + 1.0 + oz], [0.0; 3]);
    }
}
