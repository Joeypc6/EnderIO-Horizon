// In explosion.rs: the blast's rays with a hardened facade's 10 x resistance (BlockConduitBundle
// .getExplosionResistance; Ender IO 5.3.72, CC0 / Unlicense, the Ender IO team). The resistance lives on the bundle's
// block entity, and Game::exploded_positions reads it from the block alone, inside a body the mixin tool can only
// prepend to; so a blast that may reach such a bundle runs this copy of exploded_positions, which differs only in the
// bundle's resistance. Any other blast takes the engine's own at once.
use horizon_core::random::RandomSource;

// Game::exploded_positions (Explosion.calculateExplodedPositions) line for line, the bundle's resistance x 10 under a
// hardened facade. None when no hardened facade's bundle is in reach.
pub(in crate::game) fn exploded_positions(game: &mut crate::game::Game<'_>, center: [f64; 3], radius: f32, resists: Option<([i32; 3], f32)>) -> Option<Vec<[i32; 3]>> {
    if !crate::game::enderio_facades::hardened_bundle_near(game, center, radius) {
        return None;
    }
    let (min_y, max_y) = game.with_level(|_, level, _| {
        use horizon_world::level::LevelAccess;
        (level.min_y(), level.max_y())
    });
    let cap = game.combat.blast_resistance_cap;
    let wither_immune = cap.and_then(|_| game.res.items().block_tags.get("minecraft:wither_immune").cloned());
    let mut set: horizon_world::java_hash_map::JavaHashMap<super::Key, ()> = horizon_world::java_hash_map::JavaHashMap::default();
    for xx in 0..16 {
        for yy in 0..16 {
            for zz in 0..16 {
                if !(xx == 0 || xx == 15 || yy == 0 || yy == 15 || zz == 0 || zz == 15) {
                    continue;
                }
                let mut xd = f64::from(xx as f32 / 15.0 * 2.0 - 1.0);
                let mut yd = f64::from(yy as f32 / 15.0 * 2.0 - 1.0);
                let mut zd = f64::from(zz as f32 / 15.0 * 2.0 - 1.0);
                let d = (xd * xd + yd * yd + zd * zd).sqrt();
                xd /= d;
                yd /= d;
                zd /= d;
                let mut power = radius * (0.7 + game.world.random.next_float() * 0.6);
                let (mut xp, mut yp, mut zp) = (center[0], center[1], center[2]);
                while power > 0.0 {
                    let pos = [xp.floor() as i32, yp.floor() as i32, zp.floor() as i32];
                    if pos[1] < min_y
                        || pos[1] >= max_y
                        || !(-30_000_000..30_000_000).contains(&pos[0])
                        || !(-30_000_000..30_000_000).contains(&pos[2])
                    {
                        break;
                    }
                    let state = game.world.block(pos[0], pos[1], pos[2]);
                    let f = game.res.facts().get(state);
                    let fluid = if f.fluid != horizon_client::blocks::Fluid::None { 100.0f32 } else { 0.0 };
                    let spared = game.combat.rail_proof_blast
                        && (game.rail(pos[0], pos[1], pos[2]).is_some() || game.rail(pos[0], pos[1] + 1, pos[2]).is_some());
                    match resists {
                        Some((at, resistance)) if at == pos => {
                            power -= (resistance + 0.3) * 0.3;
                        }
                        _ if !game.res.facts().is_air(state) || f.fluid != horizon_client::blocks::Fluid::None => {
                            // The one change: a hardened facade's bundle resists 10 x.
                            let own = if &*f.name == crate::game::enderio_conduits::BUNDLE
                                && crate::game::enderio_facades::hardened_bundle_at(game, (pos[0], pos[1], pos[2]))
                            {
                                f.explosion_resistance * 10.0
                            } else {
                                f.explosion_resistance
                            };
                            let base = if spared { 0.0 } else { own.max(fluid) };
                            let immune = wither_immune.as_ref().is_some_and(|t| t.contains(&*f.name));
                            power -= (super::blast_resistance(base, cap, immune) + 0.3) * 0.3;
                        }
                        _ => {}
                    }
                    if power > 0.0 && !spared {
                        set.put(super::Key(pos), ());
                    }
                    xp += xd * f64::from(0.3f32);
                    yp += yd * f64::from(0.3f32);
                    zp += zd * f64::from(0.3f32);
                    power -= 0.225_000_01;
                }
            }
        }
    }
    Some(set.entries().map(|(k, _)| k.0).collect())
}
