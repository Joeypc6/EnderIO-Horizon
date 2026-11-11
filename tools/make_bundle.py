"""Writes the Ender IO bundle's horizon-mixin.toml and its one-line hook helpers, and packs the content.

Usage: python make_bundle.py
"""

import json
import pathlib
import subprocess
import sys

ROOT = pathlib.Path(__file__).resolve().parent.parent
BUNDLE = ROOT / "mixins" / "enderio"
BUILD = ROOT.parent / "native-build" / "main" / "enderio"
JAR = ROOT.parent / "EnderIO-1.12.2-5.3.72.jar"
if not JAR.exists():
    JAR = ROOT.parent / "Ported" / JAR.name
HORIZON = "fdc87aecdbd116bf1e08adcd062fcbaea8c2e2ca"
MIXINS = "5a64af938ec86b00d53d16a408aabc0038fb3e57"
VERSION = "5.3.72"
M = "crate::game::enderio"
UI = "crate::game::enderio_ui"
M2 = "crate::game::enderio_machines"
C = "crate::game::enderio_conduits"

GAME_MUT = ("game", "&mut crate::game::Game<'_>", "&mut *self")
GAME = ("game", "&crate::game::Game<'_>", "&*self")
ENTITY = ("entity", "&mut horizon_client::containers::ModBlockEntity", "&mut *entity")
SCREEN = ("screen", "&crate::game::native_hooks::NativeScreenView<'_>", "screen")
CANCELED = ("canceled", "&mut bool", "&mut *canceled")

# The modules: (id, file they go in, module name, source).
MODULES = [
    ("enderio", "crates/horizon-client/src/game.rs", "enderio", "enderio.rs"),
    ("enderio-ui", "crates/horizon-client/src/game.rs", "enderio_ui", "enderio_ui.rs"),
    # Phase 2: the generators, vat, soul binder, slice'n'splice, enchanter, tanks, chargers, painter.
    ("enderio-machines", "crates/horizon-client/src/game.rs", "enderio_machines", "enderio_machines.rs"),
    # Phase 3: the conduit bundle and its conduits.
    ("enderio-conduits", "crates/horizon-client/src/game.rs", "enderio_conduits", "enderio_conduits.rs"),
    # The filter items: their NBT, the server's edits, their screens; the redstone signal filters.
    ("enderio-filters", "crates/horizon-client/src/game.rs", "enderio_filters", "enderio_filters.rs"),
    ("enderio-signal-filters", "crates/horizon-client/src/game.rs", "enderio_signal_filters", "enderio_signal_filters.rs"),
    # The conduit facades: painted, placed on a bundle, drawn as their paint.
    ("enderio-facades", "crates/horizon-client/src/game.rs", "enderio_facades", "enderio_facades.rs"),
    # In explosion.rs: the blast's rays with a hardened facade's 10 x resistance.
    ("enderio-blast", "crates/horizon-client/src/game/explosion.rs", "enderio_blast", "enderio_blast.rs"),
    # In native_hooks.rs: the bundle's open screen laid out again (the filter screens, the tabs' slots).
    ("enderio-screen", "crates/horizon-client/src/game/native_hooks.rs", "enderio_screen", "enderio_screen.rs"),
    # In the client library: the menus' slot rules and the machines' smoke.
    ("enderio-rules", "crates/horizon-client/src/menu.rs", "enderio_rules", "enderio_rules.rs"),
    ("enderio-particles", "crates/horizon-client/src/animate_tick.rs", "enderio_particles", "enderio_particles.rs"),
]

# (id, hook, bindings, body); a `target:` hook is a function the tool finds by path. A body starting with `ui::`
# calls the screens' module (enderio_ui.rs), the rest the machines' (enderio.rs); None is SLOT_RULES.
HOOKS = [
    ("be", "block_entity", [("block", "&str", "block"), ("entity", "&mut Option<(String, horizon_world::nbt::Compound)>", "&mut *entity")], "block_entity(block, entity)"),
    ("be-ticks", "block_entity_ticks", [("block", "&str", "block"), ("ticks", "&mut bool", "&mut *ticks")], "block_entity_ticks(block, ticks)"),
    ("be-container", "block_entity_container", [("block", "&str", "block"), ("container", "&mut Option<horizon_client::containers::ModContainer>", "&mut *container")], "block_entity_container(block, container)"),
    ("be-energy", "block_entity_energy", [("block", "&str", "block"), ("energy", "&mut Option<horizon_client::containers::ModEnergyBuffer>", "&mut *energy")], "block_entity_energy(block, energy)"),
    ("be-tick", "block_entity_tick", [GAME_MUT, ("pos", "[i32; 3]", "pos"), ENTITY], "block_entity_tick(game, pos, entity)"),
    ("accepts", "target:horizon_client::game::native_hooks::Game<'_>::native_container_accepts", [GAME, ("entity", "&horizon_client::containers::ModBlockEntity", "&*entity"), ("slot", "usize", "slot"), ("stack", "&horizon_client::items::ItemStack", "stack"), ("face", "Option<horizon_core::geometry::Direction>", "face"), ("cancel", "&mut bool", "&mut *cancel")], "container_accepts(game, entity, slot, stack, face, cancel)"),
    ("slot-rules", "menu_slot_rules", [("menu", "&str", "menu"), ("slot", "usize", "slot"), ("stack", "&ItemStack", "stack"), ("may_place", "&mut bool", "&mut *may_place"), ("max_stack", "&mut u32", "&mut *max_stack")], None),
    ("menu-button", "menu_button_v2", [GAME_MUT, ("pos", "[i32; 3]", "pos"), ENTITY, ("id", "i32", "id"), ("carried", "&mut Option<ItemStack>", "&mut *carried")], "menu_button(game, pos, entity, id, carried)"),
    ("quick-move", "menu_quick_move", [GAME_MUT, ENTITY, ("container_slots", "usize", "container_slots"), ("slot", "usize", "slot"), ("stack", "&mut horizon_client::items::ItemStack", "&mut *stack"), ("handled", "&mut bool", "&mut *handled")], "quick_move(game, entity, container_slots, slot, stack, handled)"),
    ("menu-clicked", "menu_clicked", [("uuid", "u128", "uuid"), ("slot", "i32", "slot")], "menu_clicked(uuid, slot)"),
    ("right-click", "right_click_block", [GAME_MUT, ("pos", "[i32; 3]", "pos"), ("face", "[i32; 3]", "face"), CANCELED], "right_click(game, pos, face, canceled)"),
    ("placed", "place_block", [GAME_MUT, ("stack", "&ItemStack", "stack"), ("pos", "[i32; 3]", "pos")], "placed(game, stack, pos)"),
    ("destroy", "destroy_block", [GAME_MUT, ("pos", "[i32; 3]", "pos")], "destroyed(game, pos)"),
    ("neighbor", "neighbor_changed", [("level", "&GameLevel<'_>", "&*self"), ("pos", "BlockPos", "pos"), ("state", "BlockStateId", "state")], "neighbor_changed(&level.facts.get(state).name, [pos.x, pos.y, pos.z])"),
    ("placement", "target:horizon_client::game::native_hooks::GameLevel<'_>::native_state_for_placement", [("level", "&crate::game::GameLevel<'_>", "&*self"), ("state", "horizon_world::registry::BlockStateId", "state"), ("context", "&horizon_world::level::PlacementContext", "context"), ("answer", "&mut Option<Option<horizon_world::registry::BlockStateId>>", "&mut *answer")], "state_for_placement(level, state, context, answer)"),
    ("level-tick", "level_tick_post", [GAME_MUT, ("dimension", "&str", "dimension")], "level_tick(game, dimension)"),
    ("item-energy", "item_energy_buffer", [("item", "&str", "item"), ("buffer", "&mut Option<crate::game::native_item_energy::ItemEnergyBuffer>", "&mut *buffer")], "item_energy_buffer(item, buffer)"),
    ("tooltip", "tooltip", [("stack", "&ItemStack", "stack"), ("lines", "&mut Vec<(Text, u32)>", "&mut *lines")], "tooltip(stack, lines)"),
    ("update-tag", "block_entity_update_tag", [("entity", "&horizon_client::containers::ModBlockEntity", "&*entity"), ("tag", "&mut Option<horizon_world::nbt::Compound>", "&mut *tag")], "update_tag(entity, tag)"),
    ("screen", "screen", [("block", "&str", "block"), ("screen", "&mut Option<crate::game::native_hooks::NativeScreen>", "&mut *screen")], "ui::screen(block, screen)"),
    ("screen-draw", "screen_draw", [GAME, ("block", "&str", "block"), ("data", "&[i32]", "data"), ("batch", "&mut horizon_client::hud::HudBatch", "&mut *batch"), ("origin", "[f32; 2]", "origin")], "ui::screen_draw(game, block, data, batch, origin)"),
    ("screen-tooltip", "screen_tooltip", [GAME, ("block", "&str", "block"), ("data", "&[i32]", "data"), ("at", "[f32; 2]", "at"), ("lines", "&mut Vec<String>", "&mut *lines")], "ui::screen_tooltip(game, block, data, at, lines)"),
    ("screen-pressed", "screen_mouse_button_pressed_pre", [GAME, SCREEN, ("button", "i32", "button"), ("modifiers", "u32", "modifiers"), CANCELED], "ui::pressed(game, screen, button, modifiers, canceled)"),
    ("screen-released", "screen_mouse_button_released_pre", [CANCELED], "ui::released(canceled)"),
    ("screen-dragged", "screen_mouse_dragged_pre", [GAME, ("delta", "[f32; 2]", "delta"), CANCELED], "ui::dragged(game, delta, canceled)"),
    # The wheel: the screens' own, and a filter ghost slot's +1 / -1 sent as a button the server checks.
    ("screen-scrolled", "screen_mouse_scrolled_pre", [GAME, SCREEN, ("scroll", "[f64; 2]", "scroll"), CANCELED], "ui::scrolled(game, screen, scroll, canceled)"),
    ("screen-key", "screen_key_pressed_pre", [GAME, SCREEN, ("key", "&str", "key"), CANCELED], "ui::key(game, screen, key, canceled)"),
    ("screen-char", "screen_char_typed_pre", [GAME, SCREEN, ("c", "char", "c"), CANCELED], "ui::char_typed(game, screen, c, canceled)"),
    ("server-tick", "server_tick", [GAME_MUT], "ui::register_jade(game)"),
    ("kit-frame", "frame", [GAME_MUT], "kit_frame(game)"),
    # Phase 2: the soul vial on a mob, the fluid tank's item as a fluid container.
    ("interact-entity", "interact_entity", [GAME_MUT, ("entity", "i32", "entity"), ("kind", "&str", "kind"), ("stack", "Option<&horizon_client::items::ItemStack>", "stack"), ("claimed", "&mut bool", "&mut *claimed")], "m2::interact_entity(game, entity, kind, stack, claimed)"),
    ("item-fluid-tank", "item_fluid_tank", [("item", "&str", "item"), ("capacity", "&mut Option<i32>", "&mut *capacity")], "m2::item_fluid_tank(item, capacity)"),
    # Phase 3: the conduit bundle drawn, and a player's break taking one conduit at a time.
    ("submit", "block_entity_submit", [GAME, ("pos", "[i32; 3]", "pos"), ("block", "&str", "block"), ("entity", "&horizon_client::containers::ModBlockEntity", "&*entity"), ("draw", "&mut crate::game::native_block_entity_draw::BlockEntityDraw", "&mut *draw")], "c::submit(game, pos, block, entity, draw)"),
    ("conduit-destroy", "destroy_block", [GAME_MUT, ("pos", "[i32; 3]", "pos"), CANCELED], "c::destroy_block(game, pos, canceled)"),
    # The insulated redstone conduit's output, and redstone wire turning to a bundle.
    ("signal", "block_signal", [("level", "&crate::game::GameLevel<'_>", "&*self"), ("dimension", "crate::game::LevelDimension", "dimension"), ("pos", "horizon_core::position::BlockPos", "pos"), ("state", "horizon_world::registry::BlockStateId", "state"), ("face", "horizon_core::geometry::Direction", "face"), ("direct", "bool", "direct"), ("signal", "&mut i32", "&mut *signal")], "c::block_signal_at(&level.facts.get(state).name, || format!(\"{dimension:?}\"), [pos.x, pos.y, pos.z], face, direct, signal)"),
    # The Yeta wrench's and the probe's mode: sneak + wheel on the client, decided on the server.
    ("wheel", "wheel", [GAME_MUT, ("steps", "f32", "steps"), ("claimed", "&mut bool", "&mut *claimed")], "c::wheel(game, steps, claimed)"),
    ("net", "net_from_client", [GAME_MUT, ("player", "u128", "player"), ("channel", "&str", "channel"), ("data", "&[u8]", "data")], "c::net_from_client(game, player, channel, data)"),
    ("hud", "hud", [GAME_MUT, ("batch", "&mut horizon_client::hud::HudBatch", "&mut *batch")], "c::hud(game, batch)"),
    # A hardened facade: BlockConduitBundle.getBlockHardness x 10.
    ("break-speed", "target:horizon_client::game::native_hooks::Game<'_>::native_break_speed", [GAME, ("pos", "[i32; 3]", "pos"), ("speed", "&mut f32", "&mut *speed")], "fa::break_speed(game, pos, speed)"),
    ("signal-source", "is_signal_source", [("level", "&crate::game::GameLevel<'_>", "&*self"), ("state", "horizon_world::registry::BlockStateId", "state"), ("source", "&mut bool", "&mut *source")], "c::is_signal_source(&level.facts.get(state).name, source)"),
]

SLOT_RULES = """// In the client library: the rules the game module published (enderio_rules.rs).
fn enderio_slot_rules(menu: &str, slot: usize, stack: &ItemStack, may_place: &mut bool, max_stack: &mut u32) {
    if let Some((ok, most)) = enderio_rules::rule(menu, slot, &stack.item) {
        *may_place = *may_place && ok;
        *max_stack = (*max_stack).min(most);
    }
}
"""

# Injections into Horizon's own functions: the energy reads and moves (an Ender IO block's buffer is its instance's:
# capacitor, IO modes, the bank's network), hoppers taking (per-face IO), drops, the client's ambient particles.
# (id, target, helper file, cancellable, bindings, text)
TARGETS = [
    ("energy", "horizon_client::game::machines::native_energy::Game<'_>::native_energy", "energy_view.rs", True,
     [GAME, ("dimension", "&str", "dimension"), ("pos", "[i32; 3]", "pos"), ("face", "Direction", "face")],
     """// The energy of an Ender IO block as a face reaches it.
fn enderio_energy(game: &crate::game::Game<'_>, dimension: &str, pos: [i32; 3], face: Direction) -> core::ops::ControlFlow<Option<NativeEnergy>> {
    match crate::game::enderio::energy_view(game, dimension, pos, face) {
        None => core::ops::ControlFlow::Continue(()),
        Some(v) => core::ops::ControlFlow::Break(v.map(|(amount, capacity, max_insert, max_extract, can_insert, can_extract)| NativeEnergy { amount, capacity, max_insert, max_extract, can_insert, can_extract })),
    }
}
"""),
    ("energy-transfer", "horizon_client::game::machines::native_energy::Game<'_>::native_energy_transfer", "energy_transfer.rs", True,
     [GAME_MUT, ("dimension", "&str", "dimension"), ("pos", "[i32; 3]", "pos"), ("face", "Direction", "face"), ("amount", "i32", "amount"), ("insert", "bool", "insert"), ("simulate", "bool", "simulate")],
     """// Energy into an Ender IO block: the intake per call and per face per tick, the IO modes, the bank's network.
fn enderio_energy_transfer(game: &mut crate::game::Game<'_>, dimension: &str, pos: [i32; 3], face: Direction, amount: i32, insert: bool, simulate: bool) -> core::ops::ControlFlow<i32> {
    match crate::game::enderio::energy_transfer(game, dimension, pos, face, amount, insert, simulate) {
        None => core::ops::ControlFlow::Continue(()),
        Some(v) => core::ops::ControlFlow::Break(v),
    }
}
"""),
    ("can-take", "horizon_client::game::machines::Containers<'_>::can_take", "can_take.rs", True,
     [("c", "&Containers<'_>", "&*self"), ("view", "&View", "view"), ("slot", "usize", "slot"), ("stack", "&ItemStack", "stack"), ("face", "Direction", "face")],
     """// LegacyMachineWrapper.extractItem: outputs only, through a face whose IO mode may output.
fn enderio_can_take(c: &Containers<'_>, view: &View, slot: usize, stack: &ItemStack, face: Direction) -> core::ops::ControlFlow<bool> {
    if view.kind == Kind::Mod
        && let Some(BlockEntity::Mod(m)) = c.world.block_entities.get(&view.parts[0].0)
        && m.id.starts_with("enderio:")
    {
        let fuel = c.items.get(&stack.item).is_some_and(|i| i.fuel.is_some());
        if let Some(v) = crate::game::enderio::can_take(m, slot, stack, face, fuel) {
            return core::ops::ControlFlow::Break(v);
        }
    }
    core::ops::ControlFlow::Continue(())
}
"""),
    ("drops", "horizon_client::game::native_hooks::Game<'_>::native_block_drops", "drops.rs", False,
     [GAME_MUT, ("pos", "[i32; 3]", "pos"), ("state", "BlockStateId", "state"), ("explosion", "Option<f32>", "explosion"), ("drops", "&mut Vec<ItemStack>", "&mut *drops")],
     """// BlockEnder.getDrops: the machine's item keeps its NBT, the bank's its energy; an enhanced top drops nothing; an
// explosion drops them with chance 1 / size.
fn enderio_drops(game: &mut crate::game::Game<'_>, pos: [i32; 3], state: BlockStateId, explosion: Option<f32>, drops: &mut Vec<ItemStack>) {
    let block = game.res.facts().get(state).name.to_string();
    if block.starts_with("enderio:") {
        crate::game::enderio::block_drops(game, pos, &block, explosion, drops);
    }
}
"""),
    ("be-removed", "horizon_client::game::<GameLevel<'_> as LevelAccess>::remove_block_entity", "be_removed.rs", False,
     [("level", "&mut crate::game::GameLevel<'_>", "&mut *self"), ("pos", "horizon_core::position::BlockPos", "pos")],
     """// A machine's items stay on its dropped item: none spill when its block entity leaves the world.
fn enderio_be_removed(level: &mut crate::game::GameLevel<'_>, pos: horizon_core::position::BlockPos) {
    crate::game::enderio::block_entity_removed(level, [pos.x, pos.y, pos.z]);
}
"""),
    # Phase 2: the machines' tanks as other blocks reach them (SmartTankFluidMachineHandler, per face IO mode).
    ("custom-fluids", "horizon_client::game::native_hooks::Game<'_>::native_custom_block_fluids", "custom_fluids.rs", False,
     [("block", "&str", "block"), ("custom", "&mut bool", "&mut *custom")],
     """fn enderio_custom_fluids(block: &str, custom: &mut bool) {
    crate::game::enderio_machines::custom_block_fluids(block, custom);
}
"""),
    ("custom-tanks", "horizon_client::game::native_hooks::Game<'_>::native_custom_fluid_tanks", "custom_tanks.rs", False,
     [GAME, ("pos", "[i32; 3]", "pos"), ("side", "Option<horizon_core::geometry::Direction>", "side"), ("out", "&mut Option<Vec<horizon_client::mod_api::NativeFluidTankView>>", "&mut *out")],
     """fn enderio_custom_tanks(game: &crate::game::Game<'_>, pos: [i32; 3], side: Option<horizon_core::geometry::Direction>, out: &mut Option<Vec<horizon_client::mod_api::NativeFluidTankView>>) {
    crate::game::enderio_machines::custom_fluid_tanks(game, pos, side, out);
}
"""),
    ("custom-fill", "horizon_client::game::native_hooks::Game<'_>::native_custom_fluid_fill", "custom_fill.rs", False,
     [GAME_MUT, ("pos", "[i32; 3]", "pos"), ("side", "Option<horizon_core::geometry::Direction>", "side"), ("resource", "&horizon_client::mod_api::NativeFluidStack", "resource"), ("action", "horizon_client::mod_api::NativeFluidAction", "action"), ("out", "&mut Option<i32>", "&mut *out")],
     """fn enderio_custom_fill(game: &mut crate::game::Game<'_>, pos: [i32; 3], side: Option<horizon_core::geometry::Direction>, resource: &horizon_client::mod_api::NativeFluidStack, action: horizon_client::mod_api::NativeFluidAction, out: &mut Option<i32>) {
    crate::game::enderio_machines::custom_fluid_fill(game, pos, side, resource, action, out);
}
"""),
    ("custom-drain", "horizon_client::game::native_hooks::Game<'_>::native_custom_fluid_drain", "custom_drain.rs", False,
     [GAME_MUT, ("pos", "[i32; 3]", "pos"), ("side", "Option<horizon_core::geometry::Direction>", "side"), ("request", "&horizon_client::mod_api::NativeFluidDrain", "request"), ("action", "horizon_client::mod_api::NativeFluidAction", "action"), ("out", "&mut Option<horizon_client::mod_api::NativeFluidStack>", "&mut *out")],
     """fn enderio_custom_drain(game: &mut crate::game::Game<'_>, pos: [i32; 3], side: Option<horizon_core::geometry::Direction>, request: &horizon_client::mod_api::NativeFluidDrain, action: horizon_client::mod_api::NativeFluidAction, out: &mut Option<horizon_client::mod_api::NativeFluidStack>) {
    crate::game::enderio_machines::custom_fluid_drain(game, pos, side, request, action, out);
}
"""),
    ("animate", "horizon_client_lib::animate_tick::animate_tick", "animate.rs", False,
     [("engine", "&mut ParticleEngine", "&mut *engine"), ("ctx", "&AnimateContext<'_>", "ctx"), ("player", "[i32; 3]", "player")],
     """fn enderio_animate(engine: &mut ParticleEngine, ctx: &AnimateContext<'_>, player: [i32; 3]) {
    enderio_particles::animate(engine, ctx, player);
}
"""),
]

SPECIAL_HELPERS = {
    "embedded_content.rs": """// Ender IO is one file on the registry: its data and textures are the content mod in this bundle's data/
// (made by tools/make_content.py from the jar). It is written out once and loaded as a dev mod is.
fn enderio_embedded_content(root: &std::path::Path, enabled: &std::collections::BTreeSet<String>, dev: &[std::path::PathBuf]) -> core::ops::ControlFlow<std::path::PathBuf> {
    static CONTENT: &[u8] = horizon_client::native_data!("enderio", "enderio-content.hmod");
    let dir = std::env::current_dir().unwrap_or_default().join(".horizon-cache");
    let file = dir.join("enderio-embedded-content.hmod");
    if dev.contains(&file) {
        return core::ops::ControlFlow::Continue(());
    }
    let same = std::fs::read(&file).is_ok_and(|on_disk| on_disk == CONTENT);
    if !same && std::fs::create_dir_all(&dir).and_then(|()| std::fs::write(&file, CONTENT)).is_err() {
        log::warn!("Horizon: Ender IO's content could not be written to {}", file.display());
        return core::ops::ControlFlow::Continue(());
    }
    let mut with = dev.to_vec();
    with.push(file);
    core::ops::ControlFlow::Break(content_root(root, enabled, &with))
}
""",
    "has_analog.rs": """fn enderio_has_analog(level: &crate::game::GameLevel<'_>, state: horizon_world::registry::BlockStateId, output: &mut bool) {
    if crate::game::enderio::is_bank(&level.facts.get(state).name) || matches!(&*level.facts.get(state).name, "enderio:fluid_tank" | "enderio:pressurized_fluid_tank") {
        *output = true;
    }
}
""",
    "analog.rs": """// TileCapBank.getComparatorInputOverride: this block's own energy; TileTank.getComparatorOutput: its fluid.
fn enderio_analog(level: &crate::game::GameLevel<'_>, pos: horizon_core::position::BlockPos, state: horizon_world::registry::BlockStateId, output: &mut i32) {
    if !level.facts.get(state).name.starts_with("enderio:") {
        return;
    }
    if let Some(horizon_client::containers::BlockEntity::Mod(m)) = level.world.block_entities.get(&(pos.x, pos.y, pos.z))
        && let Some(v) = crate::game::enderio::comparator(m).or_else(|| crate::game::enderio_machines::tank_comparator(m))
    {
        *output = v;
    }
}
""",
    "list.rs": """fn enderio_list(mods: &mut Vec<crate::game::native_hooks::NativeMod>) {
    mods.push(crate::game::native_hooks::NativeMod {
        id: "enderio".into(),
        title: "Ender IO 5.3.72".into(),
        description: "Machines, power, conduits and travel, as in Ender IO.".into(),
        keys: Vec::new(),
    });
}
""",
}


# The eio-blast-resistance helper: a blast that may reach a hardened facade's bundle runs enderio_blast's rays.
BLAST_HELPER = """// A blast that may reach a hardened facade's bundle: the rays with its 10 x resistance (enderio_blast.rs).
fn enderio_blast_resistance(game: &mut crate::game::Game<'_>, center: [f64; 3], radius: f32, resists: Option<([i32; 3], f32)>) -> core::ops::ControlFlow<Vec<[i32; 3]>> {
    match crate::game::explosion::enderio_blast::exploded_positions(game, center, radius, resists) {
        Some(v) => core::ops::ControlFlow::Break(v),
        None => core::ops::ControlFlow::Continue(()),
    }
}
"""

def binding_lines(bindings):
    return "".join(f'  {{ name = "{n}", ty = "{t}", value = "{v}" }},\n' for n, t, v in bindings)


def injection(hook_id, target, helper, bindings, cancellable=False):
    where = [f'target = "{target[7:]}"'] if target.startswith("target:") else [f'hook = "{target}"', "hook_version = 1"]
    return ["[[injection]]", f'id = "eio-{hook_id}"', 'kind = "HEAD"', *(["cancellable = true"] if cancellable else []), *where,
            f'helper = "{helper}"', "bindings = [", binding_lines(bindings).rstrip("\n"), "]", ""]


def main():
    BUNDLE.mkdir(parents=True, exist_ok=True)
    out = [
        "[mod]",
        'id = "enderio"',
        f'version = "{VERSION}"',
        "",
        "# The Horizon commit this bundle was built and tested on.",
        "[compatibility]",
        f'horizon_revision = "{HORIZON}"',
        "",
    ]
    for mod_id, file, name, source in MODULES:
        out += ["[[module]]", f'id = "{mod_id}"', f'file = "{file}"', f'name = "{name}"', f'source = "{source}"', ""]
    for hook_id, hook, bindings, body in HOOKS:
        helper = f"h_{hook_id.replace('-', '_')}.rs"
        args = ", ".join(f"{n}: {t}" for n, t, _ in bindings)
        if body is None:
            text = SLOT_RULES
        else:
            call = (f"{UI}::{body[4:]}" if body.startswith("ui::") else f"{M2}::{body[4:]}" if body.startswith("m2::")
                    else f"{C}::{body[3:]}" if body.startswith("c::")
                    else f"crate::game::enderio_facades::{body[4:]}" if body.startswith("fa::") else f"{M}::{body}")
            text = f"fn enderio_{hook_id.replace('-', '_')}({args}) {{\n    {call};\n}}\n"
        (BUNDLE / helper).write_text(text, encoding="utf8")
        out += injection(hook_id, hook, helper, bindings)
    for hook_id, target, helper, cancellable, bindings, text in TARGETS:
        (BUNDLE / helper).write_text(text, encoding="utf8")
        out += injection(hook_id, "target:" + target, helper, bindings, cancellable)
    for name, text in SPECIAL_HELPERS.items():
        (BUNDLE / name).write_text(text, encoding="utf8")
    level = ("level", "&crate::game::GameLevel<'_>", "&*self")
    state = ("state", "horizon_world::registry::BlockStateId", "state")
    out += injection("has-analog", "has_analog_output", "has_analog.rs", [level, state, ("output", "&mut bool", "&mut *output")])
    out += injection("analog", "analog_output", "analog.rs", [level, ("pos", "horizon_core::position::BlockPos", "pos"), state, ("output", "&mut i32", "&mut *output")])
    out += injection("list", "mods", "list.rs", [("mods", "&mut Vec<crate::game::native_hooks::NativeMod>", "&mut *mods")])
    out += injection("embedded-content", "target:horizon_client::game::mods::content_root", "embedded_content.rs",
                     [("root", "&std::path::Path", "root"), ("enabled", "&std::collections::BTreeSet<String>", "enabled"), ("dev", "&[std::path::PathBuf]", "dev")], True)
    # BlockConduitBundle.getExplosionResistance: x 10 under a hardened facade, which lives on the block entity. The
    # engine reads a block's resistance inside Game::exploded_positions, whose body only takes a HEAD (a nested `use`
    # refuses interior hooks); so a blast that may reach a hardened facade's bundle runs the enderio-blast module's
    # copy of it (the bundle's resistance its one change), and any other blast the engine's own. Priority 50: after
    # the default 100, so another mod's own blast (Tinkers' EFLN) still comes first.
    out += ["[[injection]]", 'id = "eio-blast-resistance"', "priority = 50", 'kind = "HEAD"', "cancellable = true",
            "target = \"horizon_client::game::explosion::Game<'_>::exploded_positions\"",
            'helper = "blast_resistance.rs"', "bindings = [",
            binding_lines([GAME_MUT, ("center", "[f64; 3]", "center"), ("radius", "f32", "radius"),
                           ("resists", "Option<([i32; 3], f32)>", "resists")]).rstrip("\n"), "]", ""]
    (BUNDLE / "blast_resistance.rs").write_text(BLAST_HELPER, encoding="utf8")
    (BUNDLE / "horizon-mixin.toml").write_text("\n".join(out), encoding="utf8")
    subprocess.run([sys.executable, str(ROOT.parent / "jei-kit" / "apply.py"), "enderio", str(BUNDLE)], check=True)
    for fixer in ("unused_helper_params.py", "use_helper_params.py"):
        subprocess.run([sys.executable, str(ROOT.parent / "tools" / fixer), str(BUNDLE)], check=True)
    (BUNDLE / "data").mkdir(exist_ok=True)
    subprocess.run([sys.executable, "-I", str(ROOT / "tools" / "make_content.py"), str(JAR), str(BUNDLE / "data" / "enderio-content.hmod"),
                    "--report", str(ROOT / "content" / "report.json")], check=True)
    # EIO_NO_BUILD_COPY=1: write the bundle for the gate only, leaving native-build's copy (what a game build uses).
    if __import__("os").environ.get("EIO_NO_BUILD_COPY"):
        print("bundle written (not copied to native-build):", len(HOOKS) + len(TARGETS) + 4, "injections")
        return
    target = BUILD / "mixins" / "enderio"
    target.mkdir(parents=True, exist_ok=True)
    for old in target.rglob("*"):
        if old.is_file():
            old.unlink()
    for f in BUNDLE.rglob("*"):
        if f.is_file():
            to = target / f.relative_to(BUNDLE)
            to.parent.mkdir(parents=True, exist_ok=True)
            to.write_bytes(f.read_bytes())
    (BUILD / "horizon-native.json").write_text(json.dumps({"game": "26.3", "horizon": HORIZON, "id": "enderio", "mixins": MIXINS, "schema": 1, "version": VERSION}, indent=2), encoding="utf8")
    print("bundle written:", len(HOOKS) + len(TARGETS) + 4, "injections")


if __name__ == "__main__":
    main()
