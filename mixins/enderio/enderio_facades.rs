// Ender IO 5.3.72's conduit facades (crazypants.enderio.base.conduit.facade, CC0 / Unlicense, the Ender IO team) for
// Horizon: the four facade items (EnumFacadeType BASIC, HARDENED, TRANSPARENT, TRANSPARENT_HARDENED, one item each
// here), painted in the painter (FacadePainterRecipe: the paint on the item's custom_data `paintSource`), placed on a
// bundle or as a new one (ItemConduitFacade.onItemUse, TileConduitBundle.handleFacadeClick), kept on the bundle's
// block entity as `facadeType` and `paintSource`, drawn as the paint block's own model (native_draw_block), and taken
// first when a player breaks the bundle.
use super::Game;
use super::enderio::{custom_data, gi, gs, js, set_custom_data, si, ss, Pos};
use horizon_client::items::ItemStack;
use horizon_world::nbt::Compound;

pub(in crate::game) const FACADES: [&str; 4] = [
    "enderio:conduit_facade",
    "enderio:hardened_conduit_facade",
    "enderio:transparent_conduit_facade",
    "enderio:transparent_hardened_conduit_facade",
];

// EnumFacadeType's ordinal by item.
pub(in crate::game) fn facade_type(item: &str) -> Option<i32> {
    FACADES.iter().position(|f| *f == item).map(|p| p as i32)
}

pub(in crate::game) fn is_hardened(t: i32) -> bool {
    t == 1 || t == 3
}

pub(in crate::game) fn is_transparent(t: i32) -> bool {
    t == 2 || t == 3
}

// BlockConduitBundle.getExplosionResistance: super's (1.12's setResistance(10): 30 / 5 = 6, the content's 6.0) x 10
// when the bundle's facadeType is hardened (HARDENED or TRANSPARENT_HARDENED), so 60. The explosion's rays ask this of
// a bundle block only (enderio_blast.rs).
pub(in crate::game) fn hardened_bundle_at(game: &Game<'_>, pos: (i32, i32, i32)) -> bool {
    matches!(game.world.block_entities.get(&pos), Some(horizon_client::containers::BlockEntity::Mod(m))
        if m.id == super::enderio_conduits::BUNDLE && is_hardened(gi(&m.nbt, "facadeType", 0)))
}

// Whether a hardened facade's bundle may lie on one of a blast's rays: a ray starts with at most 1.3 x radius of
// power and loses at least 0.225 every 0.3 blocks, so none gets 1.74 x radius + 1 from the centre. Only the mod
// block entities of the chunks in reach are looked at (none at all in a world without mod block entities).
pub(in crate::game) fn hardened_bundle_near(game: &Game<'_>, center: [f64; 3], radius: f32) -> bool {
    let be = &game.world.block_entities;
    if !be.has_mods() {
        return false;
    }
    let reach = f64::from(radius) * 1.74 + 2.0;
    let near = |c: i32, at: f64| (f64::from(c) + 0.5 - at).abs() <= reach;
    be.mods_near(center[0], center[2], reach).any(|p| near(p.0, center[0]) && near(p.1, center[1]) && near(p.2, center[2]) && hardened_bundle_at(game, *p))
}

// `break_speed`: BlockConduitBundle.getBlockHardness is 10 x for a hardened facade, so digging it is 10 x slower.
pub(in crate::game) fn break_speed(game: &Game<'_>, pos: Pos, speed: &mut f32) {
    if let Some(horizon_client::containers::BlockEntity::Mod(m)) = game.world.block_entities.get(&(pos[0], pos[1], pos[2])) {
        if m.id == super::enderio_conduits::BUNDLE && bundle_facade(&m.nbt).is_some_and(|(t, _)| is_hardened(t)) {
            *speed /= 10.0;
        }
    }
}

// PaintUtil.getSourceBlock of a stack.
pub(in crate::game) fn paint_of(stack: &ItemStack) -> Option<String> {
    custom_data(stack).and_then(|c| gs(c, "paintSource")).filter(|s| !s.is_empty())
}

// The bundle's facade: (type, paint block).
pub(in crate::game) fn bundle_facade(nbt: &Compound) -> Option<(i32, String)> {
    let paint = gs(nbt, "paintSource").filter(|s| !s.is_empty())?;
    Some((gi(nbt, "facadeType", 0).clamp(0, 3), paint))
}

pub(in crate::game) fn set_bundle_facade(nbt: &mut Compound, f: Option<(i32, &str)>) {
    match f {
        Some((t, paint)) => {
            si(nbt, "facadeType", t);
            ss(nbt, "paintSource", paint);
        }
        None => {
            nbt.remove(&js("facadeType"));
            nbt.remove(&js("paintSource"));
        }
    }
}

// The facade item a bundle's facade drops as.
pub(in crate::game) fn facade_stack(t: i32, paint: &str) -> ItemStack {
    let mut s = ItemStack::new(FACADES[t.clamp(0, 3) as usize], 1);
    let mut c = Compound::new();
    ss(&mut c, "paintSource", paint);
    set_custom_data(&mut s, c);
    s
}

// YetaUtil.shouldHeldItemHideFacades: the Yeta wrench in a display mode other than NONE and NEUTRAL, the probe.
pub(in crate::game) fn hides_facades(held: Option<&ItemStack>) -> bool {
    let Some(h) = held else { return false };
    if &*h.item == super::enderio_conduits::PROBE {
        return true;
    }
    if &*h.item == super::enderio_conduits::WRENCH {
        let m = super::enderio_conduits::display_mode(Some(h));
        return m != super::enderio_conduits::Mode::None && m != super::enderio_conduits::Mode::Neutral;
    }
    false
}

// Block.isOpaqueCube of the paint (ConduitRenderMapper: an opaque paint or a transparent facade hides the conduits).
pub(in crate::game) fn paint_opaque(game: &Game<'_>, paint: &str) -> bool {
    game.native_default_state(paint).is_some_and(|s| {
        let f = game.res.facts().get(s);
        f.solid_render && f.full_collision
    })
}

// FacadePainterRecipe: a facade item with any valid paint (PaintUtil.isValid) becomes a copy of itself with that
// paint; the painter's other templates don't take facades. None for a target that isn't a facade.
pub(in crate::game) fn paint_facade(game: &Game<'_>, target: &ItemStack, paint: &ItemStack) -> Option<Option<ItemStack>> {
    facade_type(&target.item)?;
    let valid = paint_valid(game, paint);
    if !valid {
        return Some(None);
    }
    let mut out = ItemStack { count: 1, ..target.clone() };
    let mut c = custom_data(target).cloned().unwrap_or_default();
    ss(&mut c, "paintSource", &paint.item);
    set_custom_data(&mut out, c);
    Some(Some(out))
}

// PaintUtil.isValid(paint, null): a block item that isn't painted itself (and isn't a facade).
pub(in crate::game) fn paint_valid(game: &Game<'_>, paint: &ItemStack) -> bool {
    if paint_of(paint).is_some() || paint.item.starts_with("enderio:painted_") || facade_type(&paint.item).is_some() || &*paint.item == super::enderio_conduits::BUNDLE {
        return false;
    }
    game.native_default_state(&paint.item).is_some()
}

// The painter's isPartialRecipe for the facade template.
pub(in crate::game) fn facade_partial(game: &Game<'_>, target: Option<&ItemStack>, paint: Option<&ItemStack>) -> bool {
    target.is_none_or(|t| facade_type(&t.item).is_some()) && paint.is_none_or(|p| paint_valid(game, p)) && (target.is_some() || paint.is_some())
}

// The click outcome for a facade in hand on `at` (a bundle or air): TileConduitBundle.handleFacadeClick, or a new
// bundle holding only the facade (ItemConduitFacade.onItemUse). Some(drop) of the facade it replaced when it was
// placed (None when nothing happened).
pub(in crate::game) fn place(game: &mut Game<'_>, at: Pos, held: &ItemStack, sneaking: bool) -> Option<Option<ItemStack>> {
    let t = facade_type(&held.item)?;
    let paint = paint_of(held)?;
    if sneaking {
        return None;
    }
    let block = game.native_block(at);
    if block == super::enderio_conduits::BUNDLE {
        let current = game.native_with_mod_block_entity(at, |e| bundle_facade(&e.nbt)).flatten();
        let mut drop = None;
        if let Some((ct, cp)) = current {
            // A facade not drawn solid (the wrench shows through it), or the same one, stays.
            if !super::enderio_conduits::facade_solid_for(game) || (ct == t && cp == paint) {
                return None;
            }
            drop = Some(facade_stack(ct, &cp));
        }
        game.native_with_mod_block_entity(at, |e| set_bundle_facade(&mut e.nbt, Some((t, &paint))));
        return Some(drop);
    }
    let replaceable = matches!(block.as_str(), "minecraft:air" | "minecraft:cave_air" | "minecraft:void_air");
    if !replaceable {
        return None;
    }
    let dim = game.native_dimension().to_owned();
    let state = game.native_default_state(super::enderio_conduits::BUNDLE)?;
    game.native_set_block(&dim, at, state, 3);
    game.native_with_mod_block_entity(at, |e| set_bundle_facade(&mut e.nbt, Some((t, &paint))));
    Some(None)
}
