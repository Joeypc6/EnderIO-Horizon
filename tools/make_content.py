"""Ender IO 5.3.72 (Minecraft 1.12.2) as a Horizon content mod for Minecraft 26.3.

Usage: python make_content.py <EnderIO jar> <out.hmod> [--report report.json]

What it writes (spec-content.md is the reference for every mapping):
- blocks and items flattened as flattening.json says, one id per 1.12 (name, meta[, NBT]): each block's declaration
  (`data/enderio/horizon/block/`: the vanilla template of its Java class's family, strength from its constructor,
  its state properties), 26.3 block states and models (eio_blocks), item declarations (stack sizes from the Java
  classes) and item definitions and models (layers as 1.12's);
- textures moved to `textures/block`, `textures/item`; the colours of the fused glass in the pixels;
- lang files: every 1.12 `.lang` of the jar as 26.3 JSON, the name keys on the flattened ids, the rest kept;
- recipes from the XML (eio_recipes): crafting and smelting as vanilla JSON, the machines' as `enderio:<kind>`;
- tags (the ore dictionary Ender IO registers, as `c:` and `enderio:` tags), loot tables, advancements, creative tabs,
  sounds, the W guide (enderio_guide), and a report.

Not as blocks: Ender IO's fluids (Horizon keeps a mod's fluids in tanks, the report lists them).
"""

import io
import json
import os
import pathlib
import re
import sys
import zipfile

HERE = pathlib.Path(__file__).resolve().parent
sys.path.insert(0, str(HERE))

import eio_blocks  # noqa: E402
from eio_blocks import BlockBuilder  # noqa: E402
from eio_models import ID, Report, forge_parts, lenient_json, merge_parts, texture, texture_file  # noqa: E402
from eio_recipes import Converter, Universe  # noqa: E402
from eio_tables import COLORS, DYE_112  # noqa: E402

PORT = HERE.parent


def white_png(size):
    """An opaque white RGBA PNG of `size` x `size`."""
    import struct
    import zlib

    def chunk(kind, data):
        return struct.pack(">I", len(data)) + kind + data + struct.pack(">I", zlib.crc32(kind + data) & 0xFFFFFFFF)
    raw = b"".join(b"\x00" + b"\xff" * 4 * size for _ in range(size))
    return (b"\x89PNG\r\n\x1a\n" + chunk(b"IHDR", struct.pack(">IIBBBBB", size, size, 8, 6, 0, 0, 0))
            + chunk(b"IDAT", zlib.compress(raw)) + chunk(b"IEND", b""))


SPEC = PORT / "spec-content.md"
FLATTENING = PORT / "flattening.json"
HORIZON_26 = pathlib.Path(os.environ.get("APPDATA", "C:/Users/joey_/AppData/Roaming")) / \
    "horizon-launcher/versions/nightly-2026-11-08-fdc87ae/.local/horizon-client/26.3"
VANILLA_ASSETS = HORIZON_26 / "assets/minecraft"
VANILLA_DATA = HORIZON_26 / "data/minecraft"

MANIFEST = """# Ender IO's blocks, items, recipes and textures, from EnderIO-1.12.2-5.3.72.jar by
# enderio-horizon/tools/make_content.py.
id = "enderio"
name = "Ender IO (blocks and items)"
version = "5.3.72"
description = "Ender IO's blocks, items, recipes, textures and guide."
authors = ["CrazyPants", "tterrag", "HenryLoenwind", "MatthiasM", "CyanideX", "EpicSquid"]
api = "0.2"
side = "both"
capabilities = ["registries"]
"""

TABS = {"main": "itemGroup.enderio.main", "items": "itemGroup.enderio.items", "materials": "itemGroup.enderio.materials",
        "machines": "itemGroup.enderio.machines", "mobs": "itemGroup.enderio.mobs",
        "conduits": "itemGroup.enderio.conduits", "invpanel": "itemGroup.enderio.invpanel"}
TAB_ICONS = {"main": "enderio:fused_quartz", "items": "enderio:dark_steel_pickaxe", "materials": "enderio:dark_steel_ingot",
             "machines": "enderio:alloy_smelter", "mobs": "enderio:filled_soul_vial", "conduits": "enderio:item_conduit",
             "invpanel": "enderio:inventory_panel"}


# ----------------------------------------------------------------------------------------------------------------
# The flattening table (spec section 7)

def load_rows():
    text = SPEC.read_text(encoding="utf-8").split("## 7. Full flattening table", 1)[1]
    rows = []
    for line in text.splitlines():
        if not line.startswith("| ") or line.startswith("| module") or line.startswith("|---"):
            continue
        c = [x.strip() for x in line.strip().strip("|").split("|")]
        if len(c) < 8:
            continue
        rows.append({"module": c[0], "name": c[1].strip("`"), "meta": c[2], "id": c[3].strip("`"), "kind": c[4],
                     "stack": c[5], "be": c[6], "notes": c[7]})
    return rows


# ----------------------------------------------------------------------------------------------------------------
# Items

CONDUIT_MODELS = {"item_power_conduit": ["item_power_conduit", "item_power_conduit_enhanced", "item_power_conduit_ender"],
                  "item_liquid_conduit": ["item_liquid_conduit", "item_liquid_conduit_advanced", "item_liquid_conduit_ender"],
                  "item_redstone_conduit": ["item_redstone_conduit_insulated"],
                  "item_inventory_remote": ["item_inventory_remote_basic", "item_inventory_remote_advanced",
                                            "item_inventory_remote_ender"]}
# Item templates: what the item is in vanilla's terms (each item class's parent; stack size and damage come with it).
ITEM_TEMPLATES = [
    (r"_sword$", {"dark_steel": "minecraft:diamond_sword", "end_steel": "minecraft:netherite_sword"}),
    (r"_pickaxe$", {"dark_steel": "minecraft:diamond_pickaxe", "end_steel": "minecraft:netherite_pickaxe"}),
    (r"_axe$", {"dark_steel": "minecraft:diamond_axe", "end_steel": "minecraft:netherite_axe"}),
    (r"_helmet$", {"dark_steel": "minecraft:diamond_helmet", "end_steel": "minecraft:netherite_helmet"}),
    (r"_chestplate$", {"dark_steel": "minecraft:diamond_chestplate", "end_steel": "minecraft:netherite_chestplate"}),
    (r"_leggings$", {"dark_steel": "minecraft:diamond_leggings", "end_steel": "minecraft:netherite_leggings"}),
    (r"_boots$", {"dark_steel": "minecraft:diamond_boots", "end_steel": "minecraft:netherite_boots"}),
    (r"_bow$", "minecraft:bow"), (r"_shield$", "minecraft:shield"), (r"_shears$", "minecraft:shears"),
    (r"_backhoe$", "minecraft:diamond_hoe"), (r"^enderios$", "minecraft:mushroom_stew"), (r"^owl_egg$", "minecraft:egg"),
]
TAB_OF_ITEM = [
    (r"_conduit$|extract_speed|conduit_facade", "conduits"), (r"inventory_panel_remote", "invpanel"),
    (r"soul_vial|broken_spawner", "mobs"),
    (r"_ingot$|_nugget$|_grinding_ball$|capacitor$|capacitor_alt$|^owl_egg$", "materials"),
]


def item_template(iid):
    for pat, t in ITEM_TEMPLATES:
        if re.search(pat, iid):
            if isinstance(t, dict):
                return t["end_steel" if iid.startswith("end_steel") else "dark_steel"]
            return t
    return "minecraft:paper"


def item_tab(row, iid):
    if row["name"] == "item_material":
        return "materials"
    for pat, tab in TAB_OF_ITEM:
        if re.search(pat, iid):
            return tab
    return "items"


def variant_of(row, iid):
    m = re.search(r"variant=([a-z_0-9]+)", row["notes"])
    if m:
        return m.group(1)
    n = row["name"]
    if n.startswith("item_alloy"):
        return re.sub(r"_(ingot|nugget|grinding_ball)$", "", iid)
    return None


class Items:
    def __init__(self, jar, files, models, report):
        self.jar = jar
        self.names = set(jar.namelist())
        self.files = files
        self.models = models
        self.report = report
        self.decls = {}
        self.defs = {}       # id -> item definition
        self.tabs = {}
        self.needs_renderer = {}
        self.bs_cache = {}

    def bs(self, name, ns="enderio"):
        path = f"assets/{ns}/blockstates/{name}.json"
        if path not in self.bs_cache:
            self.bs_cache[path] = lenient_json(self.jar.read(path)) if path in self.names else None
        return self.bs_cache[path]

    def generated(self, iid, layers, parent="minecraft:item/generated"):
        return self.models.write(f"item/{iid}", {"parent": parent, "textures": {
            f"layer{i}": texture(l, self.report) for i, l in enumerate(layers)}})

    def from_part(self, iid, parts):
        """An item model from a 1.12 item block state's variant: its model and every submodel (item_material's
        machine chassis is machine_body plus a machine_front submodel), folded into one model when there are
        several or they turn."""
        if isinstance(parts, tuple):
            parts = [parts]
        model, tex, x, y, uv = parts[0]
        if model in ("enderio:item_generated",) and len(parts) == 1:
            layers = [tex[k] for k in sorted(tex) if k.startswith("layer")]
            return self.generated(iid, layers)
        baked = []
        for i, (model, tex, x, y, uv) in enumerate(parts):
            mid = self.models.bake(model, tex, f"item_{iid}" if i == 0 else f"item_{iid}_{i}")
            if mid:
                baked.append((mid, x, y))
        if len(baked) == 1 and not baked[0][1] and not baked[0][2]:
            return self.models.write(f"item/{iid}", {"parent": baked[0][0]})
        # The display (hand / GUI transforms) of the main model, the elements of all parts.
        return self.models.write(f"item/{iid}", merge_parts(self.models, baked, display_parent=baked[0][0]))

    def model_file(self, iid, name):
        """models/item/<name>.json of the jar as our item/<id>."""
        src = self.models.ensure("enderio", f"item/{name}")
        return self.models.write(f"item/{iid}", {"parent": src})

    def add(self, row, iid, model_id, definition=None):
        bid = iid.split(":", 1)[1]
        if bid in self.defs:
            return
        self.defs[bid] = definition or {"model": {"type": "minecraft:model", "model": model_id}}
        decl = {"template": item_template(bid)}
        stack = row["stack"]
        decl["max_stack_size"] = int(stack) if stack.isdigit() else 64
        self.decls[bid] = decl
        self.tabs[bid] = item_tab(row, bid)

    def build(self, row):
        name, iid = row["name"], row["id"]
        bid = iid.split(":", 1)[1]
        if bid in self.defs:
            return
        if name == "forge:bucketfilled":
            return self.bucket(row)
        bs = self.bs(name)
        if name == "item_dark_steel_upgrade":
            m = re.search(r"enderio:dsu=([a-z_]+):([a-z_0-9]+)", row["meta"])
            if m:
                ns = m.group(1)
                src = self.bs(name, "enderiomachines" if ns == "enderiomachines" else "enderio")
                part = forge_parts(src, {"upgrade": m.group(2)})
            else:
                part = forge_parts(bs, {}, flat=("inventory",))
            return self.add(row, iid, self.from_part(bid, part))
        if bs is not None and name not in CONDUIT_MODELS:
            pv = {k: v for k, v in bs.get("variants", {}).items() if isinstance(v, dict)}
            prop = next((k for k in pv if k not in ("inventory", "normal")), None)
            value = variant_of(row, bid)
            if name == "item_conduit_facade":
                prop, value = "type", ["basic", "hardened", "transparent", "transparent_hardened"][int(row["meta"])]
            elif name == "item_soul_vial":
                value = ["empty", "full"][int(row["meta"])]
            elif name == "item_conduit_probe":
                value = "probe"
            elif name == "item_enderface":
                value = "items"
            elif name == "item_ender_food":
                value = "enderios"
            part = forge_parts(bs, {prop: value})
            mid = self.from_part(bid, part)
            if name == "item_conduit_probe":
                # Meta 1, copy / paste mode, is the `copy` variant. The native mod keeps the mode in custom_data
                # `probeMode` and mirrors it as custom_model_data flag 0 (enderio_conduits.rs sync_probe_model).
                copy = self.from_part(f"{bid}_copy", forge_parts(bs, {prop: "copy"}))
                m = lambda x: {"type": "minecraft:model", "model": x}
                return self.add(row, iid, mid, {"model": {"type": "minecraft:condition", "property":
                                                          "minecraft:custom_model_data", "index": 0,
                                                          "on_true": m(copy), "on_false": m(mid)}})
            return self.add(row, iid, mid)
        if name in CONDUIT_MODELS:
            model = CONDUIT_MODELS[name][int(row["meta"])]
            return self.add(row, iid, self.model_file(bid, model))
        if f"assets/enderio/models/item/{name}.json" in self.names:
            doc = lenient_json(self.jar.read(f"assets/enderio/models/item/{name}.json"))
            if doc.get("parent") == "builtin/entity":
                return self.shield(row, iid, name)
            mid = self.model_file(bid, name)
            definition = None
            if any("pulling" in o.get("predicate", {}) for o in doc.get("overrides", [])):
                definition = self.bow(bid, name, mid)
            return self.add(row, iid, mid, definition)
        raise KeyError(f"no model for {name} ({iid})")

    def bow(self, bid, name, mid):
        pulls = [self.models.ensure("enderio", f"item/{name}_pulling_{i}") for i in range(3)]
        m = lambda x: {"type": "minecraft:model", "model": x}
        return {"model": {"type": "minecraft:condition", "property": "minecraft:using_item", "on_false": m(mid),
                          "on_true": {"type": "minecraft:range_dispatch", "property": "minecraft:use_duration",
                                      "scale": 0.05, "entries": [{"threshold": 0.65, "model": m(pulls[1])},
                                                                 {"threshold": 0.9, "model": m(pulls[2])}],
                                      "fallback": m(pulls[0])}}}

    def shield(self, row, iid, name):
        """ItemDarkSteelShield draws vanilla's shield with its own base texture (a TEISR): vanilla's shield until the
        native mod's renderer takes the texture."""
        bid = iid.split(":", 1)[1]
        mid = self.models.write(f"item/{bid}", {"parent": "minecraft:item/shield"})
        vanilla = json.loads((VANILLA_ASSETS / "items/shield.json").read_text(encoding="utf-8"))
        self.needs_renderer[iid] = f"shield texture (textures/models/armor/{bid.replace('_shield', '')}_shield_base.png)"
        self.add(row, iid, mid, vanilla)

    def bucket(self, row):
        """Forge's universal bucket drew the fluid in code: a bucket texture of its own per fluid, the water of
        vanilla's water bucket recoloured to the fluid's still texture's average colour."""
        from PIL import Image
        iid = row["id"]
        bid = iid.split(":", 1)[1]
        fluid = re.search(r"FluidName=([a-z_]+)", row["meta"]).group(1)
        still = Image.open(io.BytesIO(self.jar.read(f"assets/enderio/textures/blocks/fluid_{fluid}_still.png"))).convert("RGBA")
        frame = still.crop((0, 0, still.width, still.width))
        pixels = [p for p in frame.get_flattened_data() if p[3] > 0]
        avg = tuple(sum(p[i] for p in pixels) // len(pixels) for i in range(3))
        bucket = Image.open(VANILLA_ASSETS / "textures/item/bucket.png").convert("RGBA")
        water = Image.open(VANILLA_ASSETS / "textures/item/water_bucket.png").convert("RGBA")
        out = bucket.copy()
        bp, wp, op = bucket.load(), water.load(), out.load()
        for y in range(out.height):
            for x in range(out.width):
                if wp[x, y] != bp[x, y] and wp[x, y][3] > 0:
                    r, g, b, a = wp[x, y]
                    shade = (r + g + b) / (3 * 160)
                    op[x, y] = tuple(min(255, int(c * shade)) for c in avg) + (255,)
        buf = io.BytesIO()
        out.save(buf, "PNG")
        self.files[f"assets/{ID}/textures/item/bucket/{fluid}.png"] = buf.getvalue()
        mid = self.models.write(f"item/{bid}", {"parent": "minecraft:item/generated", "textures": {
            "layer0": f"{ID}:item/bucket/{fluid}"}})
        self.add(row, iid, mid)


# ----------------------------------------------------------------------------------------------------------------
# Lang

def read_lang(data):
    text = data.decode("utf-8-sig")
    escapes = text.lstrip().startswith("#PARSE_ESCAPES")
    out = {}
    for line in text.splitlines():
        if not line.strip() or line.lstrip().startswith("#") or "=" not in line:
            continue
        k, v = line.split("=", 1)
        k = k.strip()
        if escapes:
            v = re.sub(r"\\u([0-9a-fA-F]{4})", lambda m: chr(int(m.group(1), 16)), v)
        out[k] = v
    return out


DSU_KEYS = {"anvil": "anvil.0", "anvil1": "anvil.1", "anvil2": "anvil.2", "carpet": "carpet", "depth": "depth",
            "direct": "direct", "elytra": "elytra", "energyupgrade": "empowered_one", "energyupgrade1": "empowered_two",
            "energyupgrade2": "empowered_three", "energyupgrade3": "empowered_four", "energyupgrade4": "empowered_five",
            "glide": "glider", "hoe": "hoe", "inv": "inv.0", "inv1": "inv.1", "inv2": "inv.2", "jumpboost1": "jump_one",
            "jumpboost2": "jump_two", "jumpboost3": "jump_three", "nightvision": "nightVision", "padding": "padding",
            "sounddetector": "sound", "speedboost1": "speed_one", "speedboost2": "speed_two", "speedboost3": "speed_three",
            "spoon": "spoon", "step_assist": "step_assist", "swim": "swim", "tnt": "tnt.0", "tnt1": "tnt.1", "tnt2": "tnt.2",
            "tnt3": "tnt.3", "tnt4": "tnt.4", "travel": "travel", "solar": "solar_0", "solar1": "solar_1",
            "solar2": "solar_2", "solar3": "solar_3"}
ANVIL_DAMAGE = ["intact", "slightlyDamaged", "veryDamaged"]
LIGHT_KEYS = ["electric", "electric.inverted", None, "inverted", "wireless", "wireless.inverted"]
PLATE_KEYS = ["wood", "stone", "iron", "gold", "darksteel", "soularium", "tuned"]


def name_keys(row):
    """The 1.12 unlocalised name keys that may name a row's 26.3 id, best first; or a composer (lang -> name)."""
    n, meta = row["name"], row["meta"]
    v = variant_of(row, row["id"].split(":", 1)[1])
    block = row["kind"].startswith("block")
    pre = "tile" if block else "item"
    m = int(meta) if meta.isdigit() else None
    keys = []
    if n == "block_infinity":
        keys.append(f"tile.block_infinity_{meta}.name")
    elif n in ("block_dark_steel_anvil", "block_dark_paper_anvil"):
        keys.append(f"tile.{n}.{ANVIL_DAMAGE[m]}.name")
    elif n == "block_electric_light":
        k = LIGHT_KEYS[m]
        keys.append(f"tile.{n}.{k}.name" if k else f"tile.{n}.name")
    elif n == "block_painted_pressure_plate":
        keys.append(f"tile.{n}.{PLATE_KEYS[m >> 1]}{'.silent' if m & 1 else ''}.name")
    elif n == "block_buffer":
        keys.append(f"tile.{n}.{['item', 'power', 'omni', 'creative'][m]}.name")
    elif n == "block_cap_bank":
        keys.append(f"tile.{n}.{['creative', 'simple', 'activated', 'vibrant'][m]}.name")
    elif n == "block_solar_panel":
        keys.append(f"tile.{n}.{['simple', None, 'advanced', 'vibrant'][m]}.name" if m != 1 else f"tile.{n}.name")
    elif n == "block_tank":
        keys.append(f"tile.{n}.advanced.name" if m == 1 else f"tile.{n}.name")
    elif n == "block_enderman_skull":
        keys.append(f"tile.{n}.{['base', 'reanimated', 'tormented', 'reanimated_tormented'][m]}.name")
    elif n in ("block_decoration1", "block_decoration2", "block_decoration3"):
        keys.append(f"tile.{n}.type{m:02d}.name")
    elif n.startswith("block_alloy"):
        keys.append(f"tile.{n}.{row['id'].split(':', 1)[1].removesuffix('_block')}.name")
    elif n.startswith("item_alloy"):
        form = {"item_alloy_ingot": "ingot", "item_alloy_nugget": "nugget", "item_alloy_ball": "ball",
                "item_alloy_endergy_ingot": "ingot", "item_alloy_endergy_nugget": "nugget", "item_alloy_endergy_ball": "ball"}[n]
        keys.append(f"item.{n}_{v}.name")
    elif n == "item_conduit_facade":
        keys.append(["item.item_conduit_facade.name", "item.item_conduit_facade.hardened.name",
                     "item.item_conduit_facade.transparent.name", "item.item_conduit_facade.transparent.hardened.name"][m])
    elif n == "item_inventory_remote":
        keys.append(f"item.item_inventory_remote_{['basic', 'advanced', 'ender'][m]}.name")
    elif n in CONDUIT_MODELS and n != "item_inventory_remote":
        keys.append(f"enderio.{CONDUIT_MODELS[n][m]}.name")
    elif n in ("item_item_conduit", "item_data_conduit"):
        keys.append(f"enderio.{n}.name")
    elif n == "item_soul_vial":
        keys.append("item.item_soul_vial.name")
    elif n.startswith("block_inventory_chest"):
        keys.append(f"tile.{n}.name")
    elif n == "item_ender_food":
        keys.append("item.item_ender_food.enderios.name")
    elif n == "block_dark_steel_door":
        keys.append("item.block_dark_steel_door.name")
    if v:
        keys.append(f"{pre}.{n}.{v}.name")
    keys.append(f"{pre}.{n}.name")
    return keys


def build_lang(jar, rows, item_ids, block_ids, report):
    langs = {}
    for name in jar.namelist():
        m = re.match(r"assets/enderio/lang/([a-z]{2}_[a-z]{2})\.lang$", name)
        if m:
            langs[m.group(1)] = read_lang(jar.read(name))
    en = langs["en_us"]
    out = {}
    renamed = set()
    missing = []
    for loc, src in langs.items():
        dst = {}
        for k, v in src.items():
            dst[k] = v
        out[loc] = dst
    done = set()
    for row in rows:
        iid = row["id"]
        if iid.startswith("("):
            continue
        bid = iid.split(":", 1)[1]
        is_block = bid in block_ids or row["name"].startswith("block_fluid_")
        target = f"block.{ID}.{bid}" if is_block else f"item.{ID}.{bid}"
        if target in done:
            continue
        done.add(target)
        n = row["name"]
        for loc, src in langs.items():
            name = None
            if n == "item_dark_steel_upgrade":
                m = re.search(r"dsu=[a-z_]+:([a-z_0-9]+)", row["meta"])
                if not m:
                    name = src.get("item.item_dark_steel_upgrade.name")
                else:
                    up = src.get(f"enderio.darksteel.upgrade.{DSU_KEYS[m.group(1)]}.name")
                    w = src.get("item.item_dark_steel_upgrade.with")
                    if up and w:
                        name = w.replace("%s", up)
            elif n == "forge:bucketfilled":
                fluid = re.search(r"FluidName=([a-z_]+)", row["meta"]).group(1)
                f = src.get(f"tile.fluid.{fluid}.name") or src.get(f"tile.{fluid}.name")
                if f and loc == "en_us":
                    name = f"{f} Bucket"
                elif f:
                    name = f
            elif n.startswith("block_fluid_"):
                fluid = n[len("block_fluid_"):]
                name = src.get(f"tile.fluid.{fluid}.name") or src.get(f"tile.{fluid}.name")
            elif n == "item_basic_capacitor" and row["meta"] in ("3", "4"):
                # ItemCapacitor's loot capacitors: named in code from their stats ("Hungry Capacitor"...)
                name = src.get("enderio.loot.capacitor.name")
            elif re.match(r"block_[a-z_]*fused_(quartz|glass)$", n) and not n.startswith("block_painted"):
                base = src.get(f"tile.{n}.name")
                c = COLORS[int(row["meta"])]
                if base and c != "white":
                    col = src.get(f"enderio.color.{DYE_112.get(c, c) if c != 'light_blue' else 'lightBlue'}")
                    name = f"{col} {base}" if col else None
                else:
                    name = base
            elif n.startswith("block_painted_fused_quartz"):
                kind = bid.replace("painted_", "")
                for k in (f"tile.block_{kind}.name", f"tile.{kind}.name"):
                    if k in src:
                        p = src.get("tile.block_painted_fused_quartz.name", "")
                        name = src[k] if not p else f"{p} ({src[k]})"
                        break
            else:
                for k in name_keys(row):
                    if k in src:
                        name = src[k]
                        break
            if name:
                if len(name) > 2 and name[0] == name[-1] == '"':
                    name = name[1:-1]
                out[loc][target] = name
                renamed.add(target)
        if target not in out["en_us"]:
            missing.append(target)
            out["en_us"][target] = bid.replace("_", " ").title()
    # buckets and fluids: fluid_type keys for the tank screens
    for k, v in list(en.items()) + [("tile.fluid.vapor_of_levity.name", en.get("tile.vapor_of_levity.name"))]:
        m = re.match(r"tile\.fluid\.([a-z_]+)\.name$", k)
        if m:
            fid = {"xpjuice": "xp_juice"}.get(m.group(1), m.group(1))
            for loc, src in langs.items():
                val = src.get(k) or (src.get("tile.vapor_of_levity.name") if m.group(1) == "vapor_of_levity" else None)
                if val:
                    out[loc][f"fluid_type.{ID}.{fid}"] = val
    report["lang_missing_names"] = missing
    report["lang_locales"] = sorted(out)
    return out


# ----------------------------------------------------------------------------------------------------------------
# Loot, advancements

def upgrade_loot_entry(e, flat, report):
    """A 1.12 chest loot entry in 26.3's shape. Ender IO's own functions (random capacitor stats, random dark steel
    upgrades, random energy) aren't vanilla's: the entry keeps its item, and the native mod adds them (report)."""
    out = {"type": "minecraft:item" if e.get("type") == "item" else e.get("type", "minecraft:item")}
    name = e.get("name")
    meta = None
    mods = []
    native = []
    for f in e.get("functions", []):
        fn = f.get("function")
        if fn == "minecraft:set_data":
            d = f.get("data", 0)
            if isinstance(d, dict):
                # a random meta (3..4: the two loot capacitors): one of the ids, evenly
                lo, hi = int(d.get("min", 0)), int(d.get("max", 0))
                kids = []
                for i, m in enumerate(range(lo, hi + 1)):
                    one = dict(e, functions=[g for g in e.get("functions", []) if g is not f] +
                               [{"function": "minecraft:set_data", "data": m}], conditions=[])
                    one.pop("weight", None)
                    one.pop("quality", None)
                    child, nat = upgrade_loot_entry(one, flat, report)
                    left = hi - m + 1
                    if left > 1:
                        child["condition"] = {"type": "minecraft:random_chance", "chance": round(1 / left, 4)}
                    kids.append(child)
                    native += [x for x in nat if x not in native]
                out = {"type": "minecraft:alternatives", "children": kids}
                if "weight" in e:
                    out["weight"] = e["weight"]
                conds = [{"type": "minecraft:random_chance", "chance": c["chance"]} for c in e.get("conditions", [])
                         if c.get("condition") == "minecraft:random_chance"]
                if conds:
                    out["condition"] = conds[0]
                return out, native
            meta = int(d)
        elif fn == "minecraft:set_count":
            c = f.get("count")
            if isinstance(c, dict):
                mods.append({"type": "minecraft:set_count", "count": {"type": "minecraft:uniform",
                             "min": c.get("min", 1), "max": c.get("max", 1)}})
            elif c not in (1, 1.0):
                mods.append({"type": "minecraft:set_count", "count": int(c)})
        elif fn == "minecraft:set_damage":
            d = f.get("damage")
            mods.append({"type": "minecraft:set_damage", "damage": {"type": "minecraft:uniform", "min": d["min"], "max": d["max"]}
                         if isinstance(d, dict) else d})
        elif fn == "minecraft:set_nbt" and name == "forge:bucketfilled":
            fluid = re.search(r'FluidName:"([a-z_]+)"', f.get("tag", ""))
            if fluid:
                name = flat.get(f"forge:bucketfilled#0{{FluidName={fluid.group(1)}}}", name)
        else:
            native.append(fn)
    if name and name.startswith("enderio:"):
        key = f"{name}#{meta or 0}"
        if key not in flat:
            key = f"{name}#*"
        name = flat.get(key, name)
    elif name and name.startswith("minecraft:") and meta:
        from eio_tables import V112
        v = V112.get((name.split(":", 1)[1], meta))
        if isinstance(v, str) and not v.startswith("#"):
            name = v
    out["name"] = name
    if "weight" in e:
        out["weight"] = e["weight"]
    if "quality" in e:
        out["quality"] = e["quality"]
    conds = []
    for c in e.get("conditions", []):
        if c.get("condition") == "minecraft:random_chance":
            conds.append({"type": "minecraft:random_chance", "chance": c["chance"]})
    if conds:
        out["condition"] = conds[0] if len(conds) == 1 else {"type": "minecraft:all_of", "terms": conds}
    if mods:
        out["modifier"] = mods
    return out, native


def convert_chest_loot(doc, flat, name, report):
    pools = []
    for p in doc.get("pools", []):
        entries = []
        for e in p.get("entries", []):
            ne, native = upgrade_loot_entry(e, flat, report)
            entries.append(ne)
            if native:
                what = ne.get("name") or "/".join(c["name"] for c in ne.get("children", []))
                report.setdefault("loot_functions_native", []).append(f"{name}: {what} ({', '.join(native)})")
        rolls = p.get("rolls", 1)
        if isinstance(rolls, dict):
            rolls = {"type": "minecraft:uniform", "min": rolls["min"], "max": rolls["max"]}
        pools.append({"rolls": rolls, "entries": entries})
    return {"type": "minecraft:chest", "pools": pools}


def item_predicate(p, flat, ore):
    """A 1.12 advancement item predicate in 26.3's form, or None when it can't be (NBT): such a criterion must not
    match everything by accident."""
    if not isinstance(p, dict):
        return None
    if p.get("type") == "forge:ore_dict":
        o = ore.get(p.get("ore"))
        if not o or not o[1]:
            return None
        tag, members = o
        return {"items": "#" + tag} if tag else {"items": members[0] if len(members) == 1 else members}
    if "nbt" in p or "potion" in p or "enchantments" in p:
        return None
    if "item" in p:
        name = p["item"]
        if name.startswith("enderio:"):
            v = flat.get(f"{name}#{p.get('data', 0)}") or flat.get(f"{name}#*")
            if not v or v.startswith("("):
                return None
            return {"items": v}
        return {"items": name}
    return None


def advancement(doc, name, flat, ore, report):
    for cname, crit in list(doc.get("criteria", {}).items()):
        trig = crit.get("trigger")
        cond = crit.get("conditions", {}) or {}
        ok = True
        if trig == "minecraft:inventory_changed":
            preds = [item_predicate(p, flat, ore) for p in cond.get("items", [])]
            if not preds or any(x is None for x in preds):
                ok = False
            else:
                crit["conditions"] = {"items": preds}
        elif trig == "minecraft:location":
            loc = {}
            if "dimension" in cond:
                loc["dimension"] = "minecraft:" + {"overworld": "overworld", "the_nether": "the_nether",
                                                   "the_end": "the_end"}.get(cond["dimension"], cond["dimension"])
            if "position" in cond:
                loc["position"] = cond["position"]
            if "biome" in cond:
                loc["biomes"] = cond["biome"]
            # 26.3's form (vanilla's nether/find_fortress): one entity_properties condition under `type`, the
            # predicate's sub-predicates namespaced.
            crit["conditions"] = {"player": {"type": "minecraft:entity_properties", "entity": "this",
                                             "predicate": {"minecraft:location": loc}}}
        else:
            ok = False
        if not ok:
            doc["criteria"][cname] = {"trigger": "minecraft:impossible"}
            report.setdefault("advancement_criteria_native", []).append(f"{name}#{cname}")
    d = doc.get("display")
    if d:
        icon = d.get("icon", {})
        if "item" in icon:
            it = icon.pop("item")
            data = icon.pop("data", 0)
            if ":" not in it:
                it = "minecraft:" + it
            if it.startswith("enderio:"):
                # item_enderface's icon metas (16...) are TESR icons of the one enderface item
                it = flat.get(f"{it}#{data}") or flat.get(f"{it}#*") or flat.get(f"{it}#0") or it
            else:
                from eio_tables import V112
                v = V112.get((it.split(":", 1)[1], int(data))) if data else None
                if isinstance(v, str) and not v.startswith("#"):
                    it = v
            icon["id"] = it
        bg = d.get("background")
        if isinstance(bg, str):
            bg = bg.replace("textures/", "", 1).removesuffix(".png").replace("blocks/", "block/")
            d["background"] = bg
    if "parent" in doc:
        doc["parent"] = re.sub(r"^enderio(machines|conduits)?:", lambda m: f"{ID}:" + (m.group(1) + "/" if m.group(1) else ""), doc["parent"])
    doc.setdefault("requirements", [[k] for k in doc.get("criteria", {})])
    return doc


# ----------------------------------------------------------------------------------------------------------------

def main(jar_path, out_path, report_path=None):
    jar = zipfile.ZipFile(jar_path)
    names = jar.namelist()
    files = {}
    report = {"skipped": {}, "needs_renderer": {}}
    mreport = Report()
    rows = load_rows()
    flat = json.loads(FLATTENING.read_text(encoding="utf-8"))
    for r in rows:
        key = f"enderio:{r['name']}#{r['meta']}" if ":" not in r["name"] else f"{r['name']}#{r['meta']}"
        if key in flat and flat[key] != r["id"] and not r["id"].startswith("("):
            report.setdefault("flattening_mismatch", []).append([key, flat[key], r["id"]])

    # -- blocks ---------------------------------------------------------------------------------------------------
    eio_blocks.FUSED_TYPES = {r["name"] for r in rows if re.match(r"block_[a-z_]*fused_(quartz|glass)$", r["name"])}
    bb = BlockBuilder(jar, files, VANILLA_ASSETS, mreport)
    by_name = {}
    for r in rows:
        if r["kind"].startswith("block"):
            by_name.setdefault(r["name"], []).append(r)
    for name, rs in by_name.items():
        bb.build(name, rs)
    for bid, decl in bb.decls.items():
        files[f"data/{ID}/horizon/block/{bid}.json"] = json.dumps(decl, indent=1).encode()
    for bid, loot in bb.loot.items():
        files[f"data/{ID}/loot_table/blocks/{bid}.json"] = json.dumps(loot, indent=1).encode()
    # block items
    item_defs, item_decls, tabs = {}, {}, {}
    block_rows = {r["id"].split(":", 1)[1]: r for r in rows if r["kind"] == "block"}
    for bid, model in bb.item_models.items():
        if bb.decls[bid].get("item") is False:
            continue
        item_defs[bid] = {"model": {"type": "minecraft:model", "model": model}}
        tabs.setdefault(bb.tabs[bid], []).append(bid)
    # -- items ------------------------------------------------------------------------------------------------
    it = Items(jar, files, bb.models, mreport)
    for r in rows:
        if r["kind"] == "item":
            it.build(r)
    for bid, d in it.defs.items():
        item_defs[bid] = d
        tabs.setdefault(it.tabs[bid], []).append(bid)
    for bid, d in it.decls.items():
        files[f"data/{ID}/horizon/item/{bid}.json"] = json.dumps(d, indent=1).encode()
    for bid, d in item_defs.items():
        files[f"assets/{ID}/items/{bid}.json"] = json.dumps(d, indent=1).encode()
    # Item-only code models (spec 2.7): rotating gears, armour models, the filled vial's soul, facades' paint.
    for bid in item_defs:
        why = None
        if bid in ("wood_gear", "stone_gear", "iron_gear", "energized_gear", "vibrant_gear", "dark_bimetal_gear"):
            why = "rotating gear (ItemModelRegistry.registerRotating)"
        elif re.match(r"(dark|end)_steel_(helmet|chestplate|leggings|boots)$", bid):
            why = "armour model when worn (textures/models/armor)"
        elif bid == "filled_soul_vial":
            why = "the captured mob in the vial"
        elif bid.endswith("conduit_facade"):
            why = "paint (the facade's paint source)"
        elif bid.endswith("_bucket"):
            why = "fluid in the bucket (Forge's dynamic bucket; a static recoloured texture here)"
        elif bid in ("fluid_tank", "pressurized_fluid_tank") or bid.endswith("capacitor_bank"):
            why = (bb.needs_renderer.get(f"{ID}:{bid}", "") + "; item shows its contents").lstrip("; ")
        if why:
            it.needs_renderer[f"{ID}:{bid}"] = why if f"{ID}:{bid}" not in bb.needs_renderer or "contents" in why                 else bb.needs_renderer[f"{ID}:{bid}"] + "; " + why
    report["needs_renderer"] = dict(sorted({**bb.needs_renderer, **it.needs_renderer}.items()))
    report.setdefault("advancement_criteria_native", [])

    # -- textures, sounds -----------------------------------------------------------------------------------------
    for n in names:
        if "/textures/" in n and n.endswith((".png", ".mcmeta")):
            out = texture_file(n)
            if out:
                data = jar.read(n)
                if n.endswith(".mcmeta"):
                    # Gson read trailing commas; 26.3's parser doesn't.
                    data = json.dumps(lenient_json(data), indent=1).encode()
                files.setdefault(out, data)
    if f"assets/{ID}/textures/block/blank.png" not in files:
        raise SystemExit("blank texture missing")
    # 1.12's sounds.json: drop `category` (26.3 sets it in code), keep the sounds.
    sounds = lenient_json(jar.read("assets/enderio/sounds.json"))
    for k, v in sounds.items():
        v.pop("category", None)
    files[f"assets/{ID}/sounds.json"] = json.dumps(sounds, indent=1).encode()
    for n in names:
        if n.startswith("assets/enderio/sounds/") and n.endswith(".ogg"):
            files[n] = jar.read(n)

    # -- lang ---------------------------------------------------------------------------------------------------
    block_ids = set(bb.decls)
    langs = build_lang(jar, rows, set(item_defs), block_ids, report)
    for loc, d in langs.items():
        files[f"assets/{ID}/lang/{loc}.json"] = json.dumps(dict(sorted(d.items())), indent=1, ensure_ascii=False).encode("utf-8")

    # -- recipes ------------------------------------------------------------------------------------------------
    reg = json.loads((HORIZON_26 / "registries.json").read_text(encoding="utf-8"))
    vanilla_items = set(reg["minecraft:item"]["entries"])
    vanilla_blocks = set(reg["minecraft:block"]["entries"])
    our_items = {f"{ID}:{b}" for b in item_defs}
    vtags = {}
    tag_dir = VANILLA_DATA / "tags/item"

    def tag_values(t, seen=()):
        p = tag_dir / (t.split(":", 1)[1] + ".json")
        if not p.exists() or t in seen:
            return []
        vals = []
        for v in json.loads(p.read_text(encoding="utf-8")).get("values", []):
            v = v["id"] if isinstance(v, dict) else v
            vals += tag_values(v[1:], seen + (t,)) if v.startswith("#") else [v]
        return vals
    for p in tag_dir.rglob("*.json"):
        t = "minecraft:" + p.relative_to(tag_dir).as_posix()[:-5]
        vtags[t] = tag_values(t)
    entities = set(reg["minecraft:entity_type"]["entries"])
    enchants = {"minecraft:" + p.stem for p in (VANILLA_DATA / "enchantment").glob("*.json")}
    fluids = set(reg["minecraft:fluid"]["entries"])
    universe = Universe(flat, vanilla_items | our_items, vanilla_items, vtags, fluids, entities, enchants, rows)
    conv = Converter(jar, universe, files, report)
    exp = {}
    for p in (VANILLA_DATA / "recipe").glob("*.json"):
        d = json.loads(p.read_text(encoding="utf-8"))
        if d.get("type") == "minecraft:smelting":
            exp.setdefault(d["result"]["id"], d.get("experience", 0.0))
    conv.vanilla_smelting_exp = exp
    integration = conv.run()

    # -- tags: the ore dictionary Ender IO registers, and every c:/enderio: tag the recipes name ---------------------
    tags = {}
    for oname in universe.eio_ore:
        t = universe.ore.get(oname)
        if not t or not t[0] or t[0].startswith("minecraft:"):
            continue
        tags.setdefault(t[0], [])
        for m in t[1]:
            if m not in tags[t[0]]:
                tags[t[0]].append(m)
    for t, members in conv.tags_used.items():
        tags.setdefault(t, [])
        for m in members:
            if m not in tags[t]:
                tags[t].append(m)
    # Ender IO's own minecraft: tag members (its skull, its shears and hoe-likes) go in vanilla's tags.
    extra_vanilla = {"minecraft:skulls": [f"{ID}:enderman_head"]}
    for t, members in extra_vanilla.items():
        tags[t] = members
    for t, members in sorted(tags.items()):
        if not members:
            continue
        ns, path = t.split(":", 1)
        files[f"data/{ns}/tags/item/{path}.json"] = json.dumps({"replace": False, "values": members}, indent=1).encode()
    report["tags_written"] = sorted(t for t, m in tags.items() if m)

    # -- chest loot, advancements --------------------------------------------------------------------------------
    for n in names:
        m = re.match(r"assets/enderio/loot_tables/(chests/[a-z_]+)\.json$", n)
        if m:
            doc = lenient_json(jar.read(n))
            files[f"data/{ID}/loot_table/{m.group(1)}.json"] = json.dumps(
                convert_chest_loot(doc, flat, m.group(1), report), indent=1).encode()
    for n in names:
        m = re.match(r"assets/(enderio|enderiomachines|enderioconduits)/advancements/(.+)\.json$", n)
        if not m:
            continue
        sub = {"enderio": "", "enderiomachines": "machines/", "enderioconduits": "conduits/"}[m.group(1)]
        doc = lenient_json(jar.read(n))
        rel = sub + m.group(2)
        files[f"data/{ID}/advancement/{rel}.json"] = json.dumps(
            advancement(doc, rel, flat, universe.ore, report), indent=1).encode()

    # -- creative tabs --------------------------------------------------------------------------------------------
    order = [r["id"].split(":", 1)[1] for r in rows if not r["id"].startswith("(")]
    pos = {}
    for i, b in enumerate(order):
        pos.setdefault(b, i)
    for tab, ids in tabs.items():
        hidden = {"enderface", "reanimated_enderman_head", "tormented_enderman_head", "reanimated_tormented_enderman_head",
                  "loot_capacitor", "loot_capacitor_alt"}
        ids = sorted(set(ids), key=lambda b: pos.get(b, 10 ** 6))
        icon = TAB_ICONS[tab] if TAB_ICONS[tab].split(":", 1)[1] in item_defs else f"{ID}:{ids[0]}"
        files[f"data/{ID}/horizon/creative_tab/{tab}.json"] = json.dumps(
            {"title": TABS[tab], "icon": icon, "items": [f"{ID}:{b}" for b in ids if b not in hidden]}, indent=1).encode()

    # -- the W guide --------------------------------------------------------------------------------------------
    import enderio_guide
    guide_pages, guide_langs = enderio_guide.write(files, jar, langs)

    # -- the native mod's menus (enderio.rs): each machine's container slots, outputs taken only, 64 data slots ------
    menus = {"simple_powered_furnace": (4, [3]), "simple_alloy_smelter": (4, [3]), "alloy_smelter": (5, [3]),
             "enhanced_alloy_smelter": (5, [3]), "simple_sag_mill": (6, [2, 3, 4, 5]), "sag_mill": (7, [2, 3, 4, 5]),
             "enhanced_sag_mill": (7, [2, 3, 4, 5]), "simple_stirling_generator": (1, []), "stirling_generator": (2, []),
             "creative_capacitor_bank": (1, []), "basic_capacitor_bank": (1, []), "capacitor_bank": (1, []),
             "vibrant_capacitor_bank": (1, []),
             # Phase 2.
             "combustion_generator": (1, []), "enhanced_combustion_generator": (1, []), "zombie_generator": (1, []),
             "franken_zombie_generator": (1, []), "ender_generator": (1, []), "vat": (3, []), "enhanced_vat": (3, []),
             "soul_binder": (5, [2, 3]), "slice_and_splice": (10, [8]), "enchanter": (4, [3]), "fluid_tank": (5, [3, 4]),
             "pressurized_fluid_tank": (5, [3, 4]), "simple_wired_charger": (2, [1]), "wired_charger": (3, [1]),
             "enhanced_wired_charger": (3, [1]), "painting_machine": (4, [2]),
             # The conduit bundle's per-side settings: the insert filter, the extract filter, the function upgrade.
             "conduit_bundle": (3, [])}
    for menu, (slots, results) in menus.items():
        files[f"data/{ID}/horizon/menu/{menu}.json"] = json.dumps({"slots": slots, "data_slots": 64, "results": results}, indent=2).encode()
    # EnderCore's button frames (gui/widgets.png: BUTTON, BUTTON_HIGHLIGHT, BUTTON_DOWN...), which Ender IO's screens draw.
    endercore = PORT.parent / "deps" / "endercore" / "EnderCore-1.12.2-0.5.78.jar"
    with zipfile.ZipFile(endercore) as ec:
        # Under Ender IO's namespace (Horizon's check flags a texture of another namespace).
        files[f"assets/{ID}/textures/gui/endercore_widgets.png"] = ec.read("assets/endercore/textures/gui/widgets.png")
    # Forge's ModelLoader.White: a plain white sprite in the block atlas, tinted for the conduits' colour bands.
    files[f"assets/{ID}/textures/block/white.png"] = white_png(16)

    files["horizon-mod.toml"] = MANIFEST.encode()

    # -- report ---------------------------------------------------------------------------------------------------
    rec = {k: v for k, v in conv.counts.items() if v["xml"] or v["written"]}
    report.update({
        "blocks": len(bb.decls),
        "blocks_with_item": sum(1 for d in bb.decls.values() if d.get("item") is not False),
        "block_entities_beacon_template": sum(1 for d in bb.decls.values() if d["template"] == "minecraft:beacon"),
        "items": len(item_defs),
        "item_declarations": len(it.decls),
        "fluids": sorted({v for k, v in flat.items() if k.startswith("fluid:")}),
        "fluid_blocks_not_declared": sorted(bb.fluid_blocks),
        "recipes": rec,
        "recipes_integration_files_skipped": integration,
        "recipes_upgrade_copy_components": conv.native["upgrade_recipes"],
        "recipes_nbt_dropped": sorted(set(conv.native["nbt_dropped"])),
        "enchantments_native": sorted(conv.native["enchantments_native"]),
        "guide": {"pages": guide_pages, "languages": guide_langs},
        "files": len(files),
    })
    # Block entities: the table's BE column against the templates here (beacon = a block entity the native mod
    # gives; the rest keep their vanilla template's behaviour and need the native mod to attach theirs).
    be_ids = sorted({r["id"].split(":", 1)[1] for r in rows if r["be"] and not r["id"].startswith("(")})
    report["block_entities"] = {
        "in_table": len(be_ids),
        "on_beacon_template": sum(1 for b in be_ids if bb.decls.get(b, {}).get("template") == "minecraft:beacon"),
        "on_vanilla_templates": {b: bb.decls[b]["template"] for b in be_ids
                                 if b in bb.decls and bb.decls[b]["template"] != "minecraft:beacon"},
    }
    report["counts_vs_spec"] = {
        "blocks (declared + fluid blocks kept in tanks)": [len(bb.decls) + len(bb.fluid_blocks), 950],
        "glass blocks": [sum(1 for b in bb.decls if "fused_" in b), 714],
        "items": [len(item_defs), 1204],
        "crafting variants": [conv.counts["crafting"].get("variants", 0), 1821],
        "smelting": [conv.counts["smelting"]["xml"], 93], "alloying": [conv.counts["alloying"]["xml"], 77],
        "sagmilling": [conv.counts["sagmilling"]["xml"], 523], "slicing": [conv.counts["slicing"]["xml"], 6],
        "soulbinding": [conv.counts["soulbinding"]["xml"], 13], "fermenting": [conv.counts["fermenting"]["xml"], 9],
        "tanking": [conv.counts["tanking"]["xml"], 71], "enchanting": [conv.counts["enchanting"]["xml"], 50],
        "spawning": [conv.counts["spawning"]["xml"], 15], "fuel": [conv.counts["fuel"]["xml"], 31],
        "coolant": [conv.counts["coolant"]["xml"], 2], "hiding": [conv.counts["hiding"]["xml"], 8],
        "casting": [conv.counts["casting"]["xml"], 36], "grindingball": [conv.counts["grindingball"]["xml"], 14],
        "capacitor keys": [conv.counts["capacitor"]["xml"], 273], "aliases (XML)": [conv.counts["alias"]["xml"], 157],
        "block entities": [len(be_ids), 175],
    }
    report["skipped"]["recipes"] = conv.skipped
    report["skipped"]["modules"] = [
        "enderioconduitsappliedenergistics (AE2): item_me_conduit #0, #1",
        "enderioconduitsopencomputers: item_opencomputers_conduit",
        "enderioconduitsrefinedstorage: item_refined_storage_conduit, item_rs_crafting_upgrade, item_rs_crafting_speed_upgrade, item_rs_crafting_speed_downgrade",
        "enderiointegrationforestry: item_species_item_filter, DSUs apiarist_armor_*, naturalist_eye",
        "enderiointegrationtic: item_eio_book (Mantle), molten-alloy fluids, casting",
        "DSUs enderio:top (The One Probe), thaumaturge_robes_*, gogglesofrevealing (Thaumcraft)",
        "Endergy: solar panel metas 4-6, capacitors item_capacitor_silver/energetic_silver/vivid",
        "block_machine_base, block_machine_io: render dummies (their models are used as parts)",
        "the book/ folder (Tinkers' modifier book page for the TiC integration)",
    ]
    report["notes"] = sorted(set(mreport.notes)) + [
        "Block items take their stack size from the block's item: the death urn stacks to 16 in 1.12 (64 here until the native mod sets it).",
        "Materials with hasEffect (crystals, powders, tokens) had the enchantment glint in 1.12; item declarations have no glint key.",
        "Dark fused glass blocked light (opacity 255); block declarations have no light-opacity key.",
        "Painted blocks, painted glass, pressure plates and detectors keep their vanilla template (stairs, doors, plates...): the paint block entity is the native mod's.",
        "minecraft:skull meta 2 is the zombie head (1.12 ItemSkull order), not the creeper head the spec's example names.",
        "Water-bottle and withering-arrow inputs lose their potion (vanilla ingredients can't hold components): see recipes_nbt_dropped.",
        "Ender IO's enchantments (repellent, soulbound, wither arrow/weapon) are named by the enchanter recipes; the native mod registers them.",
    ]
    report["skipped"]["model_textures_unknown"] = sorted(mreport.unknown_textures)
    report["skipped"]["models_missing"] = sorted(mreport.missing_models)

    buf = io.BytesIO()
    with zipfile.ZipFile(buf, "w", zipfile.ZIP_DEFLATED) as z:
        for name in sorted(files):
            z.writestr(name, files[name])
    pathlib.Path(out_path).parent.mkdir(parents=True, exist_ok=True)
    pathlib.Path(out_path).write_bytes(buf.getvalue())
    if report_path:
        pathlib.Path(report_path).write_text(json.dumps(report, indent=1, ensure_ascii=False), encoding="utf-8")
    print(f"{len(files)} files, {report['blocks']} blocks, {report['items']} items, {len(buf.getvalue())} bytes")
    for k, v in rec.items():
        print(f"  {k}: {v['written']} written of {v['xml']}")


if __name__ == "__main__":
    args = sys.argv[1:]
    rep = None
    if "--report" in args:
        i = args.index("--report")
        rep = args[i + 1]
        del args[i:i + 2]
    main(args[0], args[1], rep)
