"""Ender IO's blocks (flattened) as Horizon block declarations, 26.3 block states, models and block-item models.

Each 1.12 block goes through the family its Java class belongs to (see eio_tables.MACHINES, PAINTED and the
dispatch in `BlockBuilder.build`). The states and parts of each family copy what 1.12 drew: the Forge block state
evaluated for the state's properties (eio_models.forge_parts), the body part MachineRenderMapper puts under a
machine, vanilla's own block state (retextured) where the Java class extends a vanilla block.
"""

import io
import json
import re

from eio_models import (ID, Models, blockstate_json, forge_parts, lenient_json, merge_parts, part_dict,
                        property_values, rot_of, texture, transform_matrix, turn_sequence, turned_by)
from eio_tables import (COLORS, DEFAULT_STRENGTH, MACHINE_STRENGTH, MACHINES, MAP_COLOR, NO_FACING, OBELISK_SIDE,
                        PAINTED, PLATE_TEXTURES, PRESSURE_PLATES, SOUND, STRENGTH, TESR, ALLOY_HARDNESS)

HORIZONTAL = ["north", "south", "west", "east"]
SIX = ["down", "up", "north", "south", "west", "east"]
ROT_SUFFIX = {"north": "", "east": "_east", "south": "_south", "west": "_west"}
FUSED_TYPES = None  # filled from the table


def px(*b):
    lo = [min(b[i], b[i + 3]) / 16 for i in range(3)]
    hi = [max(b[i], b[i + 3]) / 16 for i in range(3)]
    return [round(v, 5) for v in lo + hi]


class BlockBuilder:
    def __init__(self, jar, files, vanilla_assets, report):
        self.jar = jar
        self.names = set(jar.namelist())
        self.files = files
        self.vanilla = vanilla_assets
        self.report = report
        self.models = Models(jar, files, vanilla_assets, report)
        self.decls = {}          # id -> block declaration
        self.item_models = {}    # block id -> item model id (26.3)
        self.needs_renderer = {} # id -> reason
        self.loot = {}           # id -> loot table (default: drops itself)
        self.tabs = {}           # id -> tab name
        self.fluid_blocks = []
        self.skipped = []
        self.bs_cache = {}

    # -- helpers ---------------------------------------------------------------------------------------------
    def bs(self, name, ns="enderio"):
        key = (ns, name)
        if key not in self.bs_cache:
            path = f"assets/{ns}/blockstates/{name}.json"
            self.bs_cache[key] = lenient_json(self.jar.read(path)) if path in self.names else None
        return self.bs_cache[key]

    def vanilla_json(self, kind, name):
        p = self.vanilla / kind / (name + ".json")
        return json.loads(p.read_text(encoding="utf-8")) if p.exists() else None

    def strength(self, name):
        if name in STRENGTH:
            return STRENGTH[name]
        if name in MACHINES:
            return MACHINE_STRENGTH
        return DEFAULT_STRENGTH

    def properties(self, name, **over):
        h, r, snd, tool, light = self.strength(name)
        p = {"hardness": h, "explosion_resistance": r, "sounds": SOUND[snd], "requires_tool": tool, "light": light}
        p.update(over)
        return p

    def bake_parts(self, parts, hint):
        """forge_parts output as 26.3 parts [(model id, x, y, uvlock)], null models dropped."""
        out = []
        for i, (model, tex, x, y, uv) in enumerate(parts):
            mid = self.models.bake(model, tex, hint if i == 0 else f"{hint}_{i}")
            if mid:
                out.append((mid, x, y, uv))
        return out

    def item_from(self, bid, parts, display=None):
        """The block item's model: the one unturned part itself, or the parts folded into one model."""
        if not parts:
            return None
        if len(parts) == 1 and not parts[0][1] and not parts[0][2] and not display:
            mid = parts[0][0]
            return self.models.write(f"item/{bid}", {"parent": mid})
        doc = merge_parts(self.models, [(m, x, y) for m, x, y, _ in parts],
                          display_parent=display or f"{ID}:block/transform/block")
        return self.models.write(f"item/{bid}", doc)

    def add(self, bid, decl, blockstate, item_model, tab, renderer=None, loot=True):
        self.decls[bid] = decl
        self.files[f"assets/{ID}/blockstates/{bid}.json"] = json.dumps(blockstate, indent=1).encode()
        if item_model:
            self.item_models[bid] = item_model
        self.tabs[bid] = tab
        if renderer:
            self.needs_renderer[f"{ID}:{bid}"] = renderer
        if loot is True:
            self.loot[bid] = {"type": "minecraft:block", "pools": [{"rolls": 1, "entries": [
                {"type": "minecraft:item", "name": f"{ID}:{bid}"}], "condition": {"type": "minecraft:survives_explosion"}}],
                "random_sequence": f"{ID}:blocks/{bid}"}
        elif isinstance(loot, dict):
            self.loot[bid] = loot

    def states_json(self, props, states, hint):
        """[(cond, forge parts)] baked and written as a 26.3 block state; returns the block state."""
        out = []
        for cond, parts in states:
            baked = self.bake_parts(parts, hint)
            out.append((cond, [part_dict(m, x, y, uv) for m, x, y, uv in baked]))
        return blockstate_json(props, out)

    # -- vanilla block states, retextured ---------------------------------------------------------------------
    def vanilla_copy(self, template, bid, tex_map=None):
        """`template`'s own 26.3 block state, its models retextured by `tex_map` ({vanilla texture: ours})."""
        name = template.split(":", 1)[1]
        bs = self.vanilla_json("blockstates", name)
        if not tex_map:
            return bs
        done = {}

        def fix(apply):
            if isinstance(apply, list):
                return [fix(a) for a in apply]
            a = dict(apply)
            a["model"] = self.retexture(a["model"], bid, tex_map, done)
            return a
        if "variants" in bs:
            bs = {"variants": {k: fix(v) for k, v in bs["variants"].items()}}
        else:
            bs = {"multipart": [dict(p, apply=fix(p["apply"])) for p in bs["multipart"]]}
        return bs

    def retexture(self, model_id, bid, tex_map, done):
        if model_id in done:
            return done[model_id]
        _, tex, _ = self.models.resolved(model_id if ":" in model_id else "minecraft:" + model_id)
        over = {}
        for k, v in tex.items():
            vv = v if ":" in v or v.startswith("#") else "minecraft:" + v
            if vv in tex_map:
                over[k] = tex_map[vv]
        if not over:
            done[model_id] = model_id
            return model_id
        base = model_id.split("/")[-1]
        mid = self.models.write(f"block/{bid}/{base}", {"parent": model_id, "textures": over})
        done[model_id] = mid
        return mid

    def vanilla_item(self, template, bid, tex_map=None):
        """The template's item model, retextured, as our block item's."""
        name = template.split(":", 1)[1]
        item = self.vanilla_json("items", name)
        model = None
        if item and item.get("model", {}).get("type") == "minecraft:model":
            model = item["model"]["model"]
        if model is None:
            model = f"minecraft:block/{name}"
        if tex_map:
            model = self.retexture(model, bid, tex_map, {})
            if model.startswith(f"{ID}:block/{bid}/"):
                pass
        return self.models.write(f"item/{bid}", {"parent": model})

    # -- the families ------------------------------------------------------------------------------------------
    def build(self, name, rows):
        if name.startswith("block_fluid_"):
            for r in rows:
                self.fluid_blocks.append(r["id"])
            return
        if name in ("block_machine_base", "block_machine_io"):
            return
        if name.startswith("block_painted_fused_quartz"):
            return self.painted_glass(name, rows)
        if FUSED_TYPES and name in FUSED_TYPES:
            return self.glass(name, rows)
        if name == "block_painted_pressure_plate":
            return self.pressure_plates(name, rows)
        if name in PAINTED or name in ("block_painted_double_slab", "block_painted_stone_double_slab"):
            return self.painted(name, rows)
        if name in MACHINES:
            return self.machine(name, rows)
        fn = getattr(self, "b_" + name, None)
        if fn:
            return fn(name, rows)
        raise NotImplementedError(name)

    # glass ----------------------------------------------------------------------------------------------------
    def glass_texture(self, base, color):
        """Fused quartz / glass tinted by the dye's map colour (BlockFusedQuartz.getBlockTint), as a texture file:
        26.3 tints blocks only in code, so the colour is in the pixels."""
        if color == "white":
            return f"{ID}:block/block_fused_{base}"
        out = f"assets/{ID}/textures/block/fused/{base}_{color}.png"
        if out not in self.files:
            from PIL import Image
            img = Image.open(io.BytesIO(self.jar.read(f"assets/enderio/textures/blocks/block_fused_{base}.png"))).convert("RGBA")
            c = MAP_COLOR[color]
            r, g, b = (c >> 16) & 255, (c >> 8) & 255, c & 255
            px_ = img.load()
            for yy in range(img.height):
                for xx in range(img.width):
                    pr, pg, pb, pa = px_[xx, yy]
                    px_[xx, yy] = (pr * r // 255, pg * g // 255, pb * b // 255, pa)
            buf = io.BytesIO()
            img.save(buf, "PNG")
            self.files[out] = buf.getvalue()
        return f"{ID}:block/fused/{base}_{color}"

    def glass(self, name, rows):
        for r in rows:
            bid = r["id"].split(":", 1)[1]
            color = COLORS[int(r["meta"])]
            base = "quartz" if name.endswith("quartz") else "glass"
            kind = name[len("block_"):]
            mid = f"{ID}:block/fused_{base}_{color}"
            if f"assets/{ID}/models/block/fused_{base}_{color}.json" not in self.files:
                self.models.write(f"block/fused_{base}_{color}", {"parent": "minecraft:block/cube_all", "textures": {
                    "all": self.glass_texture(base, color)}})
            enlightened = "enlightened" in kind
            decl = {"template": "minecraft:glass" if color == "white" else f"minecraft:{color}_stained_glass",
                    "properties": {"hardness": 0.5, "explosion_resistance": 1200.0 if base == "quartz" else 0.5,
                                   "sounds": SOUND["glass"], "requires_tool": False, "light": 15 if enlightened else 0}}
            item = self.models.write(f"item/{bid}", {"parent": mid})
            self.add(bid, decl, {"variants": {"": {"model": mid}}}, item, "main",
                     renderer="connected glass borders (FusedQuartzBlockRenderMapper)")

    def painted_glass(self, name, rows):
        for r in rows:
            bid = r["id"].split(":", 1)[1]
            base = "quartz" if bid.endswith("quartz") else "glass"
            mid = f"{ID}:block/fused_{base}_white"
            if f"assets/{ID}/models/block/fused_{base}_white.json" not in self.files:
                self.models.write(f"block/fused_{base}_white", {"parent": "minecraft:block/cube_all", "textures": {
                    "all": self.glass_texture(base, "white")}})
            decl = {"template": "minecraft:glass",
                    "properties": {"hardness": 0.5, "explosion_resistance": 1200.0 if base == "quartz" else 0.5,
                                   "sounds": SOUND["glass"], "requires_tool": False,
                                   "light": 15 if "enlightened" in bid else 0}}
            item = self.models.write(f"item/{bid}", {"parent": mid})
            self.add(bid, decl, {"variants": {"": {"model": mid}}}, item, "main",
                     renderer="paint (the paint source's model)")

    # painted vanilla-shaped blocks ----------------------------------------------------------------------------
    def painted(self, name, rows):
        if name in ("block_painted_double_slab", "block_painted_stone_double_slab"):
            return  # merged into the slab's type=double state
        template = PAINTED[name]
        bid = rows[0]["id"].split(":", 1)[1]
        tex_map = None
        if "dark_steel_trapdoor" in name:
            tex_map = {"minecraft:block/iron_trapdoor": f"{ID}:block/block_dark_steel_trapdoor"}
        elif "dark_steel_door" in name:
            tex_map = {"minecraft:block/iron_door_bottom": f"{ID}:block/block_dark_steel_door_lower",
                       "minecraft:block/iron_door_top": f"{ID}:block/block_dark_steel_door_upper",
                       "minecraft:item/iron_door": f"{ID}:item/item_dark_steel_door"}
        elif "reinforced_obsidian" in name:
            tex_map = {"minecraft:block/obsidian": f"{ID}:block/block_reinforced_obsidian"}
        bs = self.vanilla_copy(template, bid, tex_map)
        item = self.vanilla_item(template, bid, tex_map)
        h, r, snd, tool, light = self.strength(name)
        decl = {"template": template, "properties": self.properties(name)}
        loot = True
        if name in ("block_painted_slab", "block_painted_stone_slab"):
            loot = {"type": "minecraft:block", "pools": [{"rolls": 1, "entries": [{"type": "minecraft:item",
                "name": f"{ID}:{bid}", "modifier": [{"type": "minecraft:set_count", "count": 2, "condition": {
                    "type": "minecraft:match_block", "blocks": f"{ID}:{bid}", "state": {"type": "double"}}},
                    {"type": "minecraft:explosion_decay"}]}]}], "random_sequence": f"{ID}:blocks/{bid}"}
        if "door" in name and "trapdoor" not in name:
            loot = {"type": "minecraft:block", "pools": [{"rolls": 1, "entries": [{"type": "minecraft:item",
                "name": f"{ID}:{bid}"}], "condition": {"type": "minecraft:match_block", "blocks": f"{ID}:{bid}",
                "state": {"half": "lower"}}}], "random_sequence": f"{ID}:blocks/{bid}"}
        self.add(bid, decl, bs, item, "main", renderer="paint (the paint source's model)", loot=loot)

    def pressure_plates(self, name, rows):
        for r in rows:
            bid = r["id"].split(":", 1)[1]
            kind = bid.replace("silent_", "").replace("painted_", "").replace("_pressure_plate", "")
            template = PRESSURE_PLATES[kind]
            tex_map = None
            if kind in PLATE_TEXTURES:
                tex_map = {"minecraft:block/stone": f"{ID}:block/{PLATE_TEXTURES[kind]}"}
            bs = self.vanilla_copy(template, bid, tex_map)
            item = self.vanilla_item(template, bid, tex_map)
            decl = {"template": template, "properties": self.properties(name,
                    sounds=SOUND["wood" if kind == "wooden" else "stone" if kind == "stone" else "metal"])}
            self.add(bid, decl, bs, item, "main", renderer="paint (the paint source's model)")

    # machines -----------------------------------------------------------------------------------------------------
    def machine(self, name, rows):
        body, mode = MACHINES[name]
        bs = self.bs(name)
        base = self.bs("block_machine_base")
        for r in rows:
            bid = r["id"].split(":", 1)[1]
            extra = {}
            pv = property_values(bs)
            for p in pv:
                if p == "render":
                    continue
                vals = pv[p]
                if p == "type":   # BufferType: item, power, omni, creative
                    extra[p] = ["item", "power", "omni", "creative"][int(r["meta"])]
                elif p == "kind":  # EnumTankType
                    extra[p] = ["normal", "advanced"][int(r["meta"])]
                elif p == "size":
                    extra[p] = bid.replace("inventory_chest_", "")
                elif p == "blocktype":
                    extra[p] = "single"
                else:
                    extra[p] = vals[0]

            def front(render):
                a = dict(extra)
                a["render"] = render
                return forge_parts(bs, a)

            def body_parts(facing, rotate=True):
                if body is None:
                    return []
                sub = body + (ROT_SUFFIX[facing] if rotate else "")
                return forge_parts(base, {"sub": sub})

            props, states = {}, []
            decl = {"template": "minecraft:beacon", "properties": self.properties(name)}
            if mode == "obelisk":
                return self.obelisk(name, bid, decl)
            if mode == "telepad":
                return self.telepad(name, bid, decl, bs)
            if mode == "invpanel":
                props = {"facing": SIX, "active": ["false", "true"]}
                six = {"down": "", "up": "_up", "north": "_north", "south": "_south", "west": "_west", "east": "_east"}
                for f in SIX:
                    for a in ("false", "true"):
                        states.append(({"facing": f, "active": a}, front(("front_on" if a == "true" else "front") + six[f])))
                # InvPanelRenderMapper.mapItemRender: FRONT_ON_NORTH (the screen lit, facing north).
                item_parts = front("front_on_north")
            else:
                facing = name not in NO_FACING
                props = {"facing": HORIZONTAL} if facing else {}
                props["active"] = ["false", "true"]
                for f in (HORIZONTAL if facing else ["north"]):
                    for a in ("false", "true"):
                        on = a == "true"
                        if mode == "rot":
                            parts = body_parts(f) + front(("front_on" if on else "front") + ROT_SUFFIX[f])
                        elif mode == "xp":
                            parts = front(("front_on" if on else "front") + ROT_SUFFIX[f])
                        elif mode == "fixed":
                            parts = front("front")
                        elif mode == "layers":
                            parts = front("front") + front("front_on")
                        elif mode == "farm":
                            parts = front("front") + (front("front_on") if on else [])
                        elif mode == "soulbinder":
                            parts = body_parts(f) + ([] if on else front("front" + ROT_SUFFIX[f]))
                        elif mode == "wireless":
                            parts = front("front_on" if on else "front")
                        elif mode == "tank":
                            parts = front("front") + front("front_on")
                        else:
                            raise ValueError(mode)
                        cond = {"active": a}
                        if facing:
                            cond["facing"] = f
                        states.append((cond, parts))
                # mapItemRender of each mapper
                if mode in ("rot", "soulbinder"):
                    item_parts = body_parts("north", rotate=False) + front("front")
                elif mode == "xp":
                    item_parts = front("front_on_south")
                elif mode == "fixed":
                    item_parts = front("front_south" if name in ("block_vacuum_chest", "block_travel_anchor") else "front")
                elif mode in ("layers", "tank"):
                    item_parts = front("front") + front("front_on")
                elif mode == "wireless":
                    item_parts = front("front_on")
                else:
                    item_parts = front("front")
            blockstate = self.states_json(props, states, bid)
            decl["state_properties"] = {k: ({"type": "boolean"} if v == ["false", "true"] else {"type": "enum", "values": v})
                                        for k, v in props.items()}
            decl["default_state"] = {k: ("false" if v == ["false", "true"] else "north" if "north" in v else v[0])
                                     for k, v in props.items()}
            display = None
            d = forge_parts(bs, dict(extra, render="defaults"))
            if d and d[0][0].startswith("enderio:transform/"):
                display = self.models.ensure("enderio", "block/" + d[0][0].split(":", 1)[1])
            item = self.item_from(bid, self.bake_parts(item_parts, f"{bid}_item"), display)
            tab = "invpanel" if "inventory" in name else "machines"
            reason = "IO overlays and paint (MachineRenderMapper)"
            if name in TESR:
                reason += "; " + TESR[name]
            self.add(bid, decl, blockstate, item, tab, renderer=reason)

    def obelisk(self, name, bid, decl):
        """ObeliskBakery's quads as a model: two crossed tapered plates (drawn as boxes 0.9 wide at the foot) on a
        foot plate, the side texture idle or `_on` when active."""
        side = OBELISK_SIDE[name]
        def model(on):
            s = f"{ID}:block/{side}_on" if on else f"{ID}:block/{side}"
            faces = lambda: {f: {"texture": "#side"} for f in ("north", "south", "east", "west")}
            els = [
                {"from": [3.95, 0, 0.8], "to": [12.05, 7.6, 15.2], "faces": dict(faces(), up={"texture": "#top"})},
                {"from": [0.8, 0, 3.95], "to": [15.2, 7.6, 12.05], "faces": dict(faces(), up={"texture": "#top"})},
                {"from": [0.8, 0, 0.8], "to": [15.2, 0.01, 15.2], "faces": {"down": {"texture": "#bottom", "cullface": "down"}}},
            ]
            return self.models.write(f"block/{bid}{'_on' if on else ''}", {"parent": f"{ID}:block/transform/block", "textures": {
                "particle": f"{ID}:block/block_soul_machine_bottom", "side": s, "top": f"{ID}:block/block_soul_machine_top",
                "bottom": f"{ID}:block/obelisk_bottom"}, "elements": els})
        off, on = model(False), model(True)
        props = {"facing": HORIZONTAL, "active": ["false", "true"]}
        blockstate = {"variants": {f"active={a},facing={f}": {"model": on if a == "true" else off} for a in ("false", "true") for f in HORIZONTAL}}
        decl["state_properties"] = {"facing": {"type": "enum", "values": HORIZONTAL}, "active": {"type": "boolean"}}
        decl["default_state"] = {"facing": "north", "active": "false"}
        decl["shapes"] = {"default": [px(0, 0, 0, 16, 7.6, 16)]}
        item = self.models.write(f"item/{bid}", {"parent": on})
        self.add(bid, decl, blockstate, item, "machines", renderer="obelisk quads (ObeliskBakery), IO overlays")

    def telepad(self, name, bid, decl, bs):
        parts_types = ["single", "master", "n", "ne", "e", "se", "s", "sw", "w", "nw"]
        states = []
        for t in parts_types:
            if t == "single":
                parts = forge_parts(bs, {"render": "front", "blocktype": t})
            elif t == "master":
                parts = forge_parts(bs, {"render": "front_east", "blocktype": t})
            else:
                parts = []
            states.append(({"part": t}, parts))
        blockstate = self.states_json({"part": parts_types}, states, bid)
        decl["state_properties"] = {"part": {"type": "enum", "values": parts_types}}
        decl["default_state"] = {"part": "single"}
        item = self.item_from(bid, self.bake_parts(forge_parts(bs, {"render": "front_south", "blocktype": "single"}), f"{bid}_item"))
        self.add(bid, decl, blockstate, item, "machines", renderer="telepad blades and 3x3 formed look (TelePadRenderMapper)")

    # merging (connected) blocks --------------------------------------------------------------------------------------
    def merging(self, name, rows, kind_of, tab):
        bs = self.bs(name)
        pv = property_values(bs)
        borders = [v for v in pv["render"] if v not in ("auto", "defaults", "sides")]
        for r in rows:
            bid = r["id"].split(":", 1)[1]
            a = {}
            if "kind" in pv:
                a["kind"] = kind_of(r)
            parts = []
            for v in ["sides"] + borders:
                parts += forge_parts(bs, dict(a, render=v))
            blockstate = self.states_json({}, [({}, parts)], bid)
            baked = self.bake_parts(parts, f"{bid}_item")
            d = forge_parts(bs, dict(a, render="defaults"))
            display = self.models.ensure("enderio", "block/" + d[0][0].split(":", 1)[1]) if d else None
            item = self.item_from(bid, baked, display)
            props = {}
            decl = {"template": "minecraft:beacon", "properties": self.properties(name)}
            if name == "block_solar_panel":
                decl["shapes"] = {"default": [px(0, 0, 0, 16, 2.5, 16)]}
            reason = "connected borders (EnumMergingBlockRenderMode)"
            if name in TESR and TESR[name] not in reason:
                reason += "; " + TESR[name]
            self.add(bid, decl, blockstate, item, tab, renderer=reason)

    def b_block_solar_panel(self, name, rows):
        kinds = ["simple", "normal", "advanced", "vibrant"]
        self.merging(name, rows, lambda r: kinds[int(r["meta"])], "machines")

    def b_block_cap_bank(self, name, rows):
        kinds = ["creative", "simple", "activated", "vibrant"]
        self.merging(name, rows, lambda r: kinds[int(r["meta"])], "machines")

    def b_block_reservoir(self, name, rows):
        self.merging(name, rows, None, "machines")

    b_block_omni_reservoir = b_block_reservoir

    # plain variant blocks ------------------------------------------------------------------------------------------
    def variant_blocks(self, name, rows, prop, value_of, template, tab, props_of=None):
        bs = self.bs(name)
        for r in rows:
            bid = r["id"].split(":", 1)[1]
            parts = forge_parts(bs, {prop: value_of(r)})
            blockstate = self.states_json({}, [({}, parts)], bid)
            item = self.item_from(bid, self.bake_parts(parts, bid))
            p = props_of(r) if props_of else self.properties(name)
            t = template
            if item is None:
                # decoration1 type15, the "Invisible Block": nothing to draw, solid and unseen as vanilla's barrier.
                # Its item is as 1.12 has it: an empty model, so the item draws nothing either (it is still named and
                # still has its tooltip in the slot).
                item = self.models.write(f"item/{bid}", {"elements": []})
                t = "minecraft:barrier"
            self.add(bid, {"template": t, "properties": p}, blockstate, item, tab)

    def b_block_alloy(self, name, rows):
        def value(r):
            return r["id"].split(":", 1)[1].removesuffix("_block")
        def props(r):
            h = ALLOY_HARDNESS[value(r)]
            return {"hardness": h, "explosion_resistance": h * 2, "sounds": SOUND["metal"], "requires_tool": True, "light": 0}
        self.variant_blocks(name, rows, "variant", value, "minecraft:iron_block", "materials", props)

    b_block_alloy_endergy = b_block_alloy

    def b_block_infinity(self, name, rows):
        self.variant_blocks(name, rows, "level", lambda r: r["meta"], "minecraft:gravel", "materials")

    def b_block_decoration1(self, name, rows):
        def props(r):
            p = self.properties(name)
            return p
        self.variant_blocks(name, rows, "type", lambda r: f"type{int(r['meta']):02d}", "minecraft:stone", "main", props)

    def b_block_decoration2(self, name, rows):
        bs = self.bs(name)
        for r in rows:
            bid = r["id"].split(":", 1)[1]
            t = f"type{int(r['meta']):02d}"
            states = []
            for f in HORIZONTAL:
                for a in ("false", "true"):
                    states.append(({"facing": f, "active": a}, forge_parts(bs, {"type": t, "facing": f, "active": a})))
            blockstate = self.states_json({"facing": HORIZONTAL, "active": ["false", "true"]}, states, bid)
            item = self.item_from(bid, self.bake_parts(forge_parts(bs, {"type": t, "facing": "north", "active": "false"}), bid))
            decl = {"template": "minecraft:carved_pumpkin", "properties": self.properties(name),
                    "state_properties": {"active": {"type": "boolean"}}, "default_state": {"active": "false"}}
            self.add(bid, decl, blockstate, item, "main")

    b_block_decoration3 = b_block_decoration2

    # vanilla families -----------------------------------------------------------------------------------------
    def vanilla_family(self, name, rows, template, tex_map, tab, item_tex=None, extra=None, loot=True):
        bid = rows[0]["id"].split(":", 1)[1]
        bs = self.vanilla_copy(template, bid, tex_map)
        if item_tex:
            item = self.models.write(f"item/{bid}", {"parent": "minecraft:item/generated", "textures": {"layer0": item_tex}})
        else:
            item = self.vanilla_item(template, bid, tex_map)
        decl = {"template": template, "properties": self.properties(name)}
        if extra:
            decl.update(extra)
        self.add(bid, decl, bs, item, tab, loot=loot)
        return bid

    def b_block_dark_steel_ladder(self, name, rows):
        # BlockDarkSteelLadder extends BlockLadder (its states and shape) but draws its own model: six elements
        # (rungs and rails standing off the wall), not vanilla's flat ladder. The item is 1.12's
        # models/item/block_dark_steel_ladder (a sprite).
        bs = self.bs(name)
        bid = rows[0]["id"].split(":", 1)[1]
        states = [({"facing": f}, forge_parts(bs, {"facing": f})) for f in HORIZONTAL]
        blockstate = self.states_json({"facing": HORIZONTAL}, states, bid)
        item = self.models.write(f"item/{bid}", {"parent": "minecraft:item/generated", "textures": {
            "layer0": f"{ID}:block/block_dark_steel_ladder"}})
        self.add(bid, {"template": "minecraft:ladder", "properties": self.properties(name)}, blockstate, item, "main")

    def b_block_dark_iron_bars(self, name, rows):
        t = "block_dark_iron_bars" if name == "block_dark_iron_bars" else "block_end_iron_bars"
        self.vanilla_family(name, rows, "minecraft:iron_bars", {"minecraft:block/iron_bars": f"{ID}:block/{t}"}, "main",
                            item_tex=f"{ID}:block/{t}")

    b_block_end_iron_bars = b_block_dark_iron_bars

    def b_block_dark_steel_door(self, name, rows):
        bid = rows[0]["id"].split(":", 1)[1]
        loot = {"type": "minecraft:block", "pools": [{"rolls": 1, "entries": [{"type": "minecraft:item", "name": f"{ID}:{bid}"}],
                "condition": {"type": "minecraft:match_block", "blocks": f"{ID}:{bid}", "state": {"half": "lower"}}}],
                "random_sequence": f"{ID}:blocks/{bid}"}
        self.vanilla_family(name, rows, "minecraft:iron_door", {
            "minecraft:block/iron_door_bottom": f"{ID}:block/block_dark_steel_door_lower",
            "minecraft:block/iron_door_top": f"{ID}:block/block_dark_steel_door_upper"}, "main",
            item_tex=f"{ID}:item/item_dark_steel_door", loot=loot)

    def b_block_dark_steel_trapdoor(self, name, rows):
        self.vanilla_family(name, rows, "minecraft:iron_trapdoor",
                            {"minecraft:block/iron_trapdoor": f"{ID}:block/block_dark_steel_trapdoor"}, "main")

    def b_block_exit_rail(self, name, rows):
        # BlockExitRail extends BlockRailBase(false): straight shapes only, as the activator rail's.
        self.vanilla_family(name, rows, "minecraft:activator_rail", {
            "minecraft:block/activator_rail": f"{ID}:block/block_exit_rail",
            "minecraft:block/activator_rail_on": f"{ID}:block/block_exit_rail"}, "main",
            item_tex=f"{ID}:block/block_exit_rail")

    def anvils(self, name, rows, base_tex, top_prefix):
        for r, template in zip(rows, ["minecraft:anvil", "minecraft:chipped_anvil", "minecraft:damaged_anvil"]):
            dmg = int(r["meta"])
            vanilla_top = {"minecraft:anvil": "minecraft:block/anvil_top", "minecraft:chipped_anvil": "minecraft:block/chipped_anvil_top",
                           "minecraft:damaged_anvil": "minecraft:block/damaged_anvil_top"}[template]
            tex = {"minecraft:block/anvil": base_tex, vanilla_top: f"{top_prefix}{dmg}"}
            self.vanilla_family(name, [r], template, tex, "main")

    def b_block_dark_steel_anvil(self, name, rows):
        self.anvils(name, rows, f"{ID}:block/anvil/base", f"{ID}:block/anvil/")

    def b_block_dark_paper_anvil(self, name, rows):
        bs = self.bs(name)
        base = bs["defaults"]["textures"].get("body", "enderio:blocks/anvil/paper_base")
        tops = {int(k): v["textures"]["top"] for k, v in bs["variants"]["damage"].items()}
        for r, template in zip(rows, ["minecraft:anvil", "minecraft:chipped_anvil", "minecraft:damaged_anvil"]):
            dmg = int(r["meta"])
            vanilla_top = {"minecraft:anvil": "minecraft:block/anvil_top", "minecraft:chipped_anvil": "minecraft:block/chipped_anvil_top",
                           "minecraft:damaged_anvil": "minecraft:block/damaged_anvil_top"}[template]
            self.vanilla_family(name, [r], template, {"minecraft:block/anvil": texture(base), vanilla_top: texture(tops[dmg])}, "main")

    def b_block_broken_anvil(self, name, rows):
        bs = self.bs(name)
        tex = bs["defaults"].get("textures", {})
        bid = rows[0]["id"].split(":", 1)[1]
        states = [({"facing": f}, forge_parts(bs, {"facing": f})) for f in HORIZONTAL]
        blockstate = self.states_json({"facing": HORIZONTAL}, states, bid)
        decl = {"template": "minecraft:damaged_anvil", "properties": self.properties(name), "item": False}
        self.add(bid, decl, blockstate, None, "main",
                 loot={"type": "minecraft:block", "pools": [], "random_sequence": f"{ID}:blocks/{bid}"})

    def b_block_self_resetting_lever5(self, name, rows):
        # BlockSelfResettingLever extends BlockLever and draws vanilla's lever.
        self.vanilla_family(name, rows, "minecraft:lever", None, "main", item_tex="minecraft:block/lever")

    for _n in ("5i", "10", "10i", "30", "30i", "60", "60i", "300", "300i"):
        locals()["b_block_self_resetting_lever" + _n] = b_block_self_resetting_lever5

    def b_block_confusion_charge(self, name, rows):
        # BlockConfusionCharge extends BlockTNT: tnt's states (explode -> unstable) with the charge's sided cube.
        bs = self.bs(name)
        bid = rows[0]["id"].split(":", 1)[1]
        parts = forge_parts(bs, {"explode": "false"}, flat=("inventory",))
        blockstate = self.states_json({}, [({}, parts)], bid)
        item = self.item_from(bid, self.bake_parts(parts, bid))
        self.add(bid, {"template": "minecraft:tnt", "properties": self.properties(name)}, blockstate, item, "main",
                 renderer=None)

    b_block_concussion_charge = b_block_confusion_charge
    b_block_ender_charge = b_block_confusion_charge

    def b_block_cold_fire(self, name, rows):
        bid = rows[0]["id"].split(":", 1)[1]
        bs = self.vanilla_copy("minecraft:fire", bid)
        decl = {"template": "minecraft:fire", "properties": self.properties(name), "item": False}
        self.add(bid, decl, bs, None, "main", loot={"type": "minecraft:block", "pools": [], "random_sequence": f"{ID}:blocks/{bid}"})

    def b_block_reinforced_obsidian(self, name, rows):
        bs = self.bs(name)
        bid = rows[0]["id"].split(":", 1)[1]
        parts = forge_parts(bs, {}, flat=("normal",))
        blockstate = self.states_json({}, [({}, parts)], bid)
        item = self.item_from(bid, self.bake_parts(forge_parts(bs, {}, flat=("inventory",)), f"{bid}_item"))
        self.add(bid, {"template": "minecraft:obsidian", "properties": self.properties(name)}, blockstate, item, "main")

    def b_block_industrial_insulation(self, name, rows):
        # Material.SPONGE, and like the sponge it soaks up liquids around it.
        bs = self.bs(name)
        bid = rows[0]["id"].split(":", 1)[1]
        parts = forge_parts(bs, {}, flat=("normal",))
        blockstate = self.states_json({}, [({}, parts)], bid)
        item = self.item_from(bid, self.bake_parts(parts, bid))
        self.add(bid, {"template": "minecraft:sponge", "properties": self.properties(name)}, blockstate, item, "main")

    def fog(self, name, rows, props):
        """The fogs are particles in 1.12 (model item_generated: nothing in the world): an invisible block without
        collision, its item the 1.12 sprite."""
        bs = self.bs(name)
        bid = rows[0]["id"].split(":", 1)[1]
        layer = bs["defaults"]["textures"]["layer0"]
        item = self.models.write(f"item/{bid}", {"parent": "minecraft:item/generated", "textures": {"layer0": texture(layer)}})
        null = self.models.write(f"block/{bid}", {"textures": {"particle": texture(layer)}})
        decl = {"template": "minecraft:structure_void", "properties": self.properties(name)}
        if props:
            decl["state_properties"] = props
            decl["default_state"] = {k: ("false" if v["type"] == "boolean" else v["values"][0]) for k, v in props.items()}
        self.add(bid, decl, {"variants": {"": {"model": null}}}, item, "materials", renderer="fog particles")

    def b_block_infinity_fog(self, name, rows):
        self.fog(name, rows, {"age": {"type": "enum", "values": [str(i) for i in range(8)]}, "harmless": {"type": "boolean"}})

    def b_block_holy_fog(self, name, rows):
        self.fog(name, rows, {"amount": {"type": "enum", "values": [str(i) for i in range(16)]}})

    def b_block_holier_fog(self, name, rows):
        self.fog(name, rows, None)

    def b_block_light_node(self, name, rows):
        bid = rows[0]["id"].split(":", 1)[1]
        null = self.models.write(f"block/{bid}", {"textures": {"particle": f"{ID}:block/block_reinforced_obsidian"}})
        decl = {"template": "minecraft:light", "properties": self.properties(name), "item": False,
                "state_properties": {"active": {"type": "boolean"}}, "default_state": {"active": "false"}}
        self.add(bid, decl, {"variants": {"": {"model": null}}}, None, "machines",
                 loot={"type": "minecraft:block", "pools": [], "random_sequence": f"{ID}:blocks/{bid}"})

    def b_block_enderman_skull(self, name, rows):
        bs = self.bs(name)
        for r in rows:
            bid = r["id"].split(":", 1)[1]
            variant = ["base", "reanimated", "tormented", "reanimated_tormented"][int(r["meta"])]
            parts = forge_parts(bs, {"variant": variant})
            # TileEndermanSkull's yaw as 16 steps; a block state turns in quarters only (the renderer does the rest).
            rot = [str(i) for i in range(16)]
            states = [({"rotation": i}, [(p[0], p[1], 0, ((int(i) + 2) // 4 % 4) * 90, False) for p in parts]) for i in rot]
            blockstate = self.states_json({"rotation": rot}, states, bid)
            item = self.item_from(bid, self.bake_parts(parts, bid))
            decl = {"template": "minecraft:beacon", "properties": self.properties(name),
                    "state_properties": {"rotation": {"type": "enum", "values": rot}}, "default_state": {"rotation": "0"},
                    "shapes": {"default": [px(4, 0, 4, 12, 8, 12)]}}
            self.add(bid, decl, blockstate, item, "main", renderer="skull rotation in 22.5 degree steps")

    def b_block_death_pouch(self, name, rows):
        bs = self.bs(name)
        bid = rows[0]["id"].split(":", 1)[1]
        layer = bs["defaults"]["textures"]["layer0"]
        item = self.models.write(f"item/{bid}", {"parent": "minecraft:item/generated", "textures": {"layer0": texture(layer)}})
        null = self.models.write(f"block/{bid}", {"textures": {"particle": texture(layer)}})
        decl = {"template": "minecraft:beacon", "properties": self.properties(name),
                "shapes": {"default": [px(4, 0, 4, 12, 10, 12)]}}
        self.add(bid, decl, {"variants": {"": {"model": null}}}, item, "items", renderer="death urn (TileGrave TESR)")

    def b_block_conduit_bundle(self, name, rows):
        bid = rows[0]["id"].split(":", 1)[1]
        # The native mod draws everything (enderio_conduits.rs); the model only gives the break particles.
        core = self.models.write(f"block/{bid}", {"textures": {"particle": f"{ID}:block/conduit_core_0"}})
        # The shape: the core and an arm towards each side a conduit reaches (the native mod sets down..east from
        # its arms). Java's per-component boxes need a block-entity shape (PLAN: blocked by Horizon).
        arms = ["down", "east", "north", "south", "up", "west"]
        arm_box = {"down": px(5, 0, 5, 11, 5, 11), "up": px(5, 11, 5, 11, 16, 11), "north": px(5, 5, 0, 11, 11, 5),
                   "south": px(5, 5, 11, 11, 11, 16), "west": px(0, 5, 5, 5, 11, 11), "east": px(11, 5, 5, 16, 11, 11)}
        # `opaque` (the native mod sets it while the bundle holds a facade): the full block.
        values = {}
        for opaque in ("false", "true"):
            for mask in range(64):
                on = [arms[i] for i in range(6) if mask & (1 << i)]
                shape = [px(0, 0, 0, 16, 16, 16)] if opaque == "true" else [px(5, 5, 5, 11, 11, 11)] + [arm_box[a] for a in on]
                values[",".join([opaque] + ["true" if a in on else "false" for a in arms])] = shape
        props = {"opaque": {"type": "boolean"}, **{a: {"type": "boolean"} for a in arms}}
        decl = {"template": "minecraft:beacon", "properties": self.properties(name), "item": False,
                "state_properties": props, "default_state": {k: "false" for k in props},
                "shapes": {"properties": ["opaque"] + arms, "values": values}}
        self.add(bid, decl, {"variants": {"": {"model": core}}}, None, "conduits",
                 renderer="conduits: cores, connectors, in/out arrows, facades, fluid/energy levels (ConduitBundleRenderer)",
                 loot={"type": "minecraft:block", "pools": [], "random_sequence": f"{ID}:blocks/{bid}"})

    def single(self, name, rows, template="minecraft:beacon", tab="machines", renderer=None, flat=("normal",), shapes=None,
               item_flat=("inventory",)):
        bs = self.bs(name)
        bid = rows[0]["id"].split(":", 1)[1]
        parts = forge_parts(bs, {}, flat=flat)
        blockstate = self.states_json({}, [({}, parts)], bid)
        item = self.item_from(bid, self.bake_parts(forge_parts(bs, {}, flat=item_flat), f"{bid}_item"))
        decl = {"template": template, "properties": self.properties(name)}
        if shapes:
            decl["shapes"] = {"default": shapes}
        self.add(bid, decl, blockstate, item, tab, renderer=renderer)

    def b_block_gauge(self, name, rows):
        self.single(name, rows, renderer="energy level (TileGauge TESR)")

    def b_block_normal_wireless_charger(self, name, rows):
        self.single(name, rows)

    def b_block_enhanced_wireless_charger(self, name, rows):
        bs = self.bs(name)
        bid = rows[0]["id"].split(":", 1)[1]
        dirs = ["up", "east", "west", "north", "south"]
        states = []
        import itertools
        for combo in itertools.product(["dish", "connector"], repeat=5):
            a = dict(zip(dirs, combo))
            states.append((a, forge_parts(bs, a)))
        props = {d: ["dish", "connector"] for d in dirs}
        blockstate = self.states_json(props, states, bid)
        # The item is the block state's `inventory` variant: the base model and its `down` base submodel, five dishes.
        item = self.item_from(bid, self.bake_parts(forge_parts(bs, {}, flat=("inventory",)), f"{bid}_item"))
        decl = {"template": "minecraft:beacon", "properties": self.properties(name),
                "state_properties": {d: {"type": "enum", "values": ["dish", "connector"]} for d in dirs},
                "default_state": {d: "dish" for d in dirs}}
        self.add(bid, decl, blockstate, item, "machines")

    def b_block_wireless_charger_extension(self, name, rows):
        bs = self.bs(name)
        bid = rows[0]["id"].split(":", 1)[1]
        vals = ["down", "east", "west", "north", "south"]
        states = [({"facing": v}, forge_parts(bs, {"base": v})) for v in vals]
        blockstate = self.states_json({"facing": vals}, states, bid)
        # The item is the block state's `inventory` variant: a dish on all six sides.
        item = self.item_from(bid, self.bake_parts(forge_parts(bs, {}, flat=("inventory",)), f"{bid}_item"))
        decl = {"template": "minecraft:iron_block", "properties": self.properties(name),
                "state_properties": {"facing": {"type": "enum", "values": vals}}, "default_state": {"facing": "down"}}
        self.add(bid, decl, blockstate, item, "machines")

    def b_block_dialing_device(self, name, rows):
        bs = self.bs(name)
        bid = rows[0]["id"].split(":", 1)[1]
        vals = list(bs["variants"]["facing"].keys())
        faces = []
        for v in vals:
            f, t = v.split("_to")
            faces.append((f, t))
        face_vals = SIX
        to_vals = SIX
        states = []
        exact = {}
        for f in face_vals:
            for t in to_vals:
                key = f"{f}_to{t}"
                if key not in vals:
                    key = vals[0]
                parts = forge_parts(bs, {"facing": key}, flat=("normal",))
                v = bs["variants"]["facing"][key]
                m = transform_matrix(v.get("transform"), v.get("x", 0), v.get("y", 0))
                if [list(r) for r in m] != [list(r) for r in rot_of(parts[0][2], parts[0][3])]:
                    # DialerFacing's sideways turns (north_toeast...) roll the model about the wall it stands on,
                    # which no block-state x / y does; the dial's 22.5-degree tilt shows it. The elements turned
                    # exactly, in a model of their own.
                    if key not in exact:
                        base = self.models.bake(parts[0][0], parts[0][1], bid)
                        els, _, _ = self.models.resolved(base)
                        exact[key] = self.models.write(f"block/{bid}_{key}", {
                            "parent": base, "elements": turned_by(els, turn_sequence(m))})
                    states.append(({"face": f, "facing": t}, [part_dict(exact[key])]))
                    continue
                states.append(({"face": f, "facing": t}, [part_dict(mid, x, y, uv) for mid, x, y, uv in
                                                          self.bake_parts(parts, bid)]))
        blockstate = blockstate_json({"face": face_vals, "facing": to_vals}, states)
        item = self.item_from(bid, self.bake_parts(forge_parts(bs, {"facing": "down_tonorth"}, flat=("inventory",)), f"{bid}_item"))
        decl = {"template": "minecraft:beacon", "properties": self.properties(name),
                "state_properties": {"face": {"type": "enum", "values": face_vals}, "facing": {"type": "enum", "values": to_vals}},
                "default_state": {"face": "down", "facing": "north"}}
        self.add(bid, decl, blockstate, item, "machines", renderer="DialerFacing turns (Forge TRSR) approximated")

    def b_block_electric_light(self, name, rows):
        bs = self.bs(name)
        types = ["electric", "electric_inv", "basic", "basic_inv", "wireless", "wireless_inv"]
        boxes = {"down": px(4, 0, 4, 12, 2, 12), "up": px(4, 14, 4, 12, 16, 12), "north": px(4, 4, 0, 12, 12, 2),
                 "south": px(4, 4, 14, 12, 12, 16), "west": px(0, 4, 4, 2, 12, 12), "east": px(14, 4, 4, 16, 12, 12)}
        for r in rows:
            bid = r["id"].split(":", 1)[1]
            t = types[int(r["meta"])]
            states = []
            for f in SIX:
                for p in ("false", "true"):
                    states.append(({"face": f, "powered": p}, forge_parts(bs, {"type": t, "face": f, "active": p})))
            blockstate = self.states_json({"face": SIX, "powered": ["false", "true"]}, states, bid)
            item = self.item_from(bid, self.bake_parts(forge_parts(bs, {"type": t, "face": "down", "active": "true"}), f"{bid}_item"))
            decl = {"template": "minecraft:beacon", "properties": self.properties(name),
                    "state_properties": {"face": {"type": "enum", "values": SIX}, "powered": {"type": "boolean"}},
                    "default_state": {"face": "down", "powered": "false"},
                    "shapes": {"property": "face", "values": {f: [boxes[f]] for f in SIX}}}
            self.add(bid, decl, blockstate, item, "machines")

    def b_block_detector_block(self, name, rows):
        # BlockDetector: painted, `on` and six facings; the observer's states (facing, powered) and part.
        bs = self.bs(name)
        bid = rows[0]["id"].split(":", 1)[1]
        states = []
        for f in SIX:
            for p in ("false", "true"):
                states.append(({"facing": f, "powered": p}, forge_parts(bs, {"facing": f, "on": p})))
        blockstate = self.states_json({"facing": SIX, "powered": ["false", "true"]}, states, bid)
        item = self.item_from(bid, self.bake_parts(forge_parts(bs, {"facing": "up", "on": "false"}), f"{bid}_item"))
        self.add(bid, {"template": "minecraft:observer", "properties": self.properties(name)}, blockstate, item, "machines",
                 renderer="paint (the paint source's model)")

    b_block_detector_block_silent = b_block_detector_block

    def top(self, name, rows, be):
        """BlockMachineExtension: the upper half of a two-high machine (AxisAlignedBB 0..0.6875 high), drawn by the
        machine below it."""
        bid = rows[0]["id"].split(":", 1)[1]
        bs = self.bs(name)
        particle = bs["defaults"]["textures"]["particle"]
        null = self.models.write(f"block/{bid}", {"textures": {"particle": texture(particle)}})
        decl = {"template": "minecraft:beacon" if be else "minecraft:iron_block", "properties": self.properties(name),
                "item": False, "shapes": {"default": [px(0, 0, 0, 16, 11, 16)]}}
        lower = bid.removesuffix("_top")
        self.add(bid, decl, {"variants": {"": {"model": null}}}, None, "machines",
                 renderer="drawn with the machine below (two-high machine)",
                 loot={"type": "minecraft:block", "pools": [], "random_sequence": f"{ID}:blocks/{bid}"})

    def b_block_enhanced_alloy_smelter_top(self, name, rows):
        self.top(name, rows, False)

    b_block_enhanced_combustion_generator_top = b_block_enhanced_alloy_smelter_top
    b_block_enhanced_sag_mill_top = b_block_enhanced_alloy_smelter_top
    b_block_enhanced_vat_top = b_block_enhanced_alloy_smelter_top

    def b_block_enhanced_wired_charger_top(self, name, rows):
        self.top(name, rows, True)
