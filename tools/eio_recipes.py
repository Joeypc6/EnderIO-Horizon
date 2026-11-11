"""Ender IO's XML recipes (assets/enderio/config/recipes/*.xml) as 26.3 recipes and Ender IO's own machine data.

The reading follows Ender IO 5.3.72's RecipeLoader / RecipeFactory and EnderCore's StringThing (spec section 3):
- aliases first (all files), then the core files' recipes; a <recipe> keeps its first variant whose conditions hold
  and whose items all resolve; `disabled` ones and the ones a missing mod leaves without items drop out;
- an item name is a comma list (an OR as an input, the first that resolves as an output, `-x` excluded); an alias
  is replaced by its item string; `oredict:`, `item:`, `block:` force the kind; `mod:name[:meta]` is a registry id;
  a bare name is `minecraft:<name>` (1.12's registry) before it is an ore dictionary name.
Crafting and smelting become vanilla JSON; the machines' recipes `enderio:<kind>` JSON under data/enderio/recipe/<machine>/
(schema in PLAN.md, "Machine recipe data").
"""

import json
import re
import xml.etree.ElementTree as ET

from eio_tables import COLORS, ORE_COLOR, ORE_PREFIX, V112, V112_SUBTYPED, VANILLA_ORE

ID = "enderio"
NS = ""  # the root is enderio:recipes; its children are in no namespace

CORE_FILES = [
    # (priority, file) as each IEnderIOAddon.getRecipeFiles lists them
    (0, "aliases"), (1, "materials"), (1, "items"), (1, "base"), (1, "balls"), (1, "hiding_base"),
    (1, "darksteel_upgrades"), (1, "fuels"), (1, "glass"), (1, "generated"), (1, "misc"), (9, "capacitor"),
    (2, "machines"), (2, "sagmill"), (3, "sagmill_modded"), (3, "sagmill_ores"), (3, "sagmill_metals"),
    (3, "sagmill_vanilla"), (3, "sagmill_vanilla2modded"), (3, "sagmill_silentgems"), (3, "vat"), (3, "enchanter"),
    (3, "spawner"), (3, "integration_railcraft_recipes"), (3, "soulbinder"), (3, "tank"), (3, "hiding_machines"),
    (3, "darksteel_upgrades_machines"), (3, "alloying"), (3, "alloying_modded"), (9, "capacitor_machines"),
    (2, "conduits"), (2, "hiding_conduits"), (2, "invpanel"), (9, "capacitor_invpanel"), (9, "hiding_invpanel"),
    (2, "powertools"),
]
INTEGRATION_FILES = ["conduits-applied-energistics", "conduits-opencomputers", "conduits-refined-storage",
                     "integration-forestry", "darksteel_upgrades_forestry"]
KINDS = ["crafting", "smelting", "alloying", "sagmilling", "slicing", "soulbinding", "fermenting", "tanking",
         "enchanting", "spawning", "fuel", "coolant", "hiding", "casting", "brewing"]
MACHINE_DIR = {"alloying": ("alloy_smelter", "alloy_smelting"), "sagmilling": ("sag_mill", "sag_milling"),
               "slicing": ("slice_and_splice", "slicing"), "soulbinding": ("soul_binder", "soul_binding"),
               "fermenting": ("vat", "fermenting"), "tanking": ("tank", "tanking"),
               "enchanting": ("enchanter", "enchanting"), "spawning": ("powered_spawner", "spawner_entity"),
               "fuel": ("combustion_generator", "combustion_fuel"), "coolant": ("combustion_generator", "combustion_coolant")}
# 1.12 entity names that 26.3 renamed.
ENTITY_RENAME = {"minecraft:villager_golem": "minecraft:iron_golem", "minecraft:zombie_pigman": "minecraft:zombified_piglin",
                 "minecraft:evocation_illager": "minecraft:evoker", "minecraft:vindication_illager": "minecraft:vindicator",
                 "minecraft:snowman": "minecraft:snow_golem", "minecraft:illusion_illager": "minecraft:illusioner",
                 "minecraft:ender_crystal": "minecraft:end_crystal"}
ENCHANT_RENAME = {"minecraft:sweeping": "minecraft:sweeping_edge"}
# Ender IO's own enchantments (base/enchantment): not data in this pack yet, the native mod registers them.
EIO_ENCHANTMENTS = {"enderio:repellent", "enderio:soulbound", "enderio:witherarrow", "enderio:witherweapon",
                    "enderio:shimmer"}


def snake(s):
    return re.sub(r"(?<!^)(?=[A-Z])", "_", s).lower()


def safe_name(s):
    s = re.sub(r"[^a-z0-9/._-]+", "_", s.lower()).strip("_")
    return s or "recipe"


class Universe:
    """What exists in the 26.3 game: items (vanilla's registry and ours), the ore dictionary as tags, fluids,
    entities, enchantments, and the ids the flattening gives each 1.12 name."""

    def __init__(self, flattening, items26, vanilla_items, vanilla_tags, fluids26, entities26, enchants26, rows):
        self.flat = flattening
        self.items = items26
        self.vanilla_items = vanilla_items
        self.vanilla_tags = vanilla_tags      # minecraft item tag -> members (ids)
        self.fluids = fluids26
        self.entities = entities26
        self.enchants = enchants26
        self.by_name = {}
        for k, v in flattening.items():
            if k.startswith("fluid:") or v.startswith("("):
                continue
            name, meta = k.split("#", 1)
            self.by_name.setdefault(name, []).append((meta, v))
        self.ore = {}          # name -> (tag or None, [members])
        self.eio_ore = {}      # name -> [eio members] (registered by Ender IO itself)
        self.build_ore(rows)

    # -- the ore dictionary ---------------------------------------------------------------------------------
    def tag_members(self, tag):
        if tag.startswith("minecraft:"):
            return self.vanilla_tags.get(tag, [])
        return []

    def add_eio(self, name, members):
        self.eio_ore.setdefault(name, [])
        for m in members:
            if m not in self.eio_ore[name]:
                self.eio_ore[name].append(m)

    def build_ore(self, rows):
        f = self.flat
        mats = {}
        for r in rows:
            if r["name"] == "item_material":
                v = re.search(r"variant=([a-z_0-9]+)", r["notes"]).group(1)
                mats[v] = r["id"]
        self.material_ids = mats
        # Material.java: baseName (camelCase) and its oredict, and the dependency for five of them.
        from material_oredicts import MATERIALS
        self.dependency = {}
        for base, ore, dep in MATERIALS:
            v = snake(base)
            if v in mats:
                self.add_eio(ore, [mats[v]])
                if dep:
                    self.dependency[ore] = dep
        alloys = ["electrical_steel", "energetic_alloy", "vibrant_alloy", "redstone_alloy", "conductive_iron",
                  "pulsating_iron", "dark_steel", "soularium", "end_steel", "construction_alloy", "crude_steel",
                  "crystalline_alloy", "melodic_alloy", "stellar_alloy", "crystalline_pink_slime", "energetic_silver",
                  "vivid_alloy"]
        for a in alloys:
            cap = "".join(w.capitalize() for w in a.split("_"))
            self.add_eio("block" + cap, [f"{ID}:{a}_block"])
            self.add_eio("ingot" + cap, [f"{ID}:{a}_ingot"])
            self.add_eio("nugget" + cap, [f"{ID}:{a}_nugget"])
            self.add_eio("ball" + cap, [f"{ID}:{a}_grinding_ball"])
        glass_white = f"{ID}:fused_glass"
        fused_glass = [glass_white] + [f"{ID}:fused_glass_{c}" for c in COLORS[1:]]
        fused_quartz = [f"{ID}:fused_quartz"] + [f"{ID}:fused_quartz_{c}" for c in COLORS[1:]]
        self.add_eio("blockGlass", fused_glass)
        self.add_eio("blockGlassColorless", [glass_white])
        self.add_eio("blockGlassHardened", fused_quartz)
        for i, c in enumerate(COLORS):
            self.add_eio("blockGlass" + ORE_COLOR[c], [fused_glass[i]])
            self.add_eio("blockGlassHardened" + ORE_COLOR[c], [fused_quartz[i]])
        for r in rows:
            m = re.match(r"block_([a-z_]*fused_(quartz|glass))$", r["name"])
            if m and r["kind"] == "block" and not r["name"].startswith("block_painted"):
                ore = re.sub(r"_([a-z])", lambda x: x.group(1).upper(), m.group(1))
                self.add_eio(ore, [r["id"]])
        self.add_eio("itemSkull", [f"minecraft:{s}" for s in ("skeleton_skull", "wither_skeleton_skull", "zombie_head",
                                                               "player_head", "creeper_head", "dragon_head")] + [f"{ID}:enderman_head"])
        self.add_eio("toolHoe", [f"minecraft:{m}_hoe" for m in ("wooden", "stone", "iron", "golden", "diamond")])
        self.add_eio("toolShears", ["minecraft:shears", f"{ID}:dark_steel_shears"])
        self.add_eio("toolTreetap", [f"{ID}:dark_steel_treetap"])
        self.add_eio("egg", [f"{ID}:owl_egg"])
        self.add_eio("eggOwl", [f"{ID}:owl_egg"])
        self.add_eio("compressed1xDustBedrock", [f"{ID}:infinity_block"])
        self.add_eio("compressed2xDustBedrock", [f"{ID}:compressed_infinity_block"])
        self.add_eio("compressed3xDustBedrock", [f"{ID}:double_compressed_infinity_block"])
        # The ore dictionary: vanilla's entries, then Ender IO's (materials with a dependency only when it resolves).
        for name, (tag, members) in VANILLA_ORE.items():
            if members is None:
                members = self.tag_members(tag)
            self.ore[name] = (tag, [m for m in members if m in self.items])
        for name, members in self.eio_ore.items():
            dep = self.dependency.get(name)
            if dep and not self.ore_members(dep, False):
                continue
            tag, old = self.ore.get(name, (None, []))
            self.ore[name] = (tag or self.tag_for(name), members + [m for m in old if m not in members])

    def ore_members(self, name, eio=True):
        t = self.ore.get(name)
        return t[1] if t else []

    def tag_for(self, name):
        """The 26.3 tag an oredict name becomes (spec 4): c: for convention shapes, enderio: for Ender IO's own
        kinds, None for single-item Ender IO names (the recipes name the item)."""
        special = {"itemSilicon": "c:silicon", "gearWood": "c:gears/wood", "gearStone": "c:gears/stone",
                   "gearIronInfinity": "c:gears/iron", "gearEnergized": "c:gears/energized",
                   "gearVibrant": "c:gears/vibrant", "gearDark": "c:gears/dark_bimetal",
                   "dustNetherQuartz": "c:dusts/quartz", "nuggetEnderpearl": "c:nuggets/ender_pearl",
                   "dustEnderPearl": "c:dusts/ender_pearl", "ingotBrickNetherGlazed": None, "blockGlassHardened": "c:glass_blocks/hardened",
                   "dyeMachine": "enderio:dyes/machine", "dyeSoulMachine": "enderio:dyes/soul_machine",
                   "dyeEnhancedMachine": "enderio:dyes/enhanced_machine", "itemSkull": "minecraft:skulls",
                   "toolHoe": "minecraft:hoes", "toolShears": "c:tools/shears", "toolTreetap": "enderio:treetaps",
                   "egg": "c:eggs", "blockGlass": "c:glass_blocks", "blockGlassColorless": "c:glass_blocks/colorless"}
        if name in special:
            return special[name]
        for c in COLORS:
            if name == "blockGlass" + ORE_COLOR[c]:
                return f"c:dyed/{c}"
            if name == "blockGlassHardened" + ORE_COLOR[c]:
                return f"c:glass_blocks/hardened/{c}"
        if re.match(r"^(not)?([A-Z]?[a-z]+)?(Holy|Unholy|Pasture)?[A-Za-z]*Fused(Quartz|Glass)$", name) or name in (
                "fusedQuartz", "fusedGlass"):
            return f"{ID}:{snake(name)}"
        if name.startswith("ball"):
            return f"{ID}:grinding_balls/{snake(name[4:])}"
        for prefix, folder in ORE_PREFIX:
            if name.startswith(prefix) and len(name) > len(prefix) and name[len(prefix)].isupper():
                if prefix in ("ingot", "nugget", "dust", "block", "ore", "gem", "plate", "rod", "crop", "seed", "dye"):
                    mat = snake(name[len(prefix):])
                    if mat == "aluminium":
                        mat = "aluminum"
                    if name in ("ingotEnderiumBase",) or name.startswith(("dustTiny",)):
                        return None
                    return f"c:{folder}/{mat}"
        return None

    # -- resolving names ------------------------------------------------------------------------------------
    def lookup_id(self, mod, name, meta, nbt=None):
        """A registry id (with a meta, '*' or None) as 26.3 atoms, [] when the game has none."""
        if mod == ID:
            key = f"{ID}:{name}"
            entries = self.by_name.get(key)
            if not entries:
                return []
            if nbt and "enderio:dsu" in nbt:
                k = f"{key}#{meta if meta not in (None, '*') else 1}{{enderio:dsu={nbt['enderio:dsu']}}}"
                v = self.flat.get(k)
                return [("item", v)] if v and v in self.items else []
            if meta in (None, "*"):
                ids = []
                for m, v in entries:
                    if "{" in m and meta is None:
                        continue
                    if v in self.items and v not in ids:
                        ids.append(v)
                return [("item", i) for i in ids]
            v = self.flat.get(f"{key}#{meta}") or self.flat.get(f"{key}#*")
            return [("item", v)] if v and v in self.items else []
        if mod == "minecraft":
            m = None if meta in (None, "*") else int(meta)
            v = V112.get((name, m))
            if v is None and m is not None:
                if name in V112_SUBTYPED:
                    return []
                if m == 0 or name not in V112_SUBTYPED:
                    v = V112.get((name, None))
            if v is None:
                v = f"minecraft:{name}" if f"minecraft:{name}" in self.vanilla_items else None
            if v is None:
                return []
            if isinstance(v, list):
                return [("item", x) for x in v if x in self.items]
            if v.startswith("#"):
                return [("tag", v[1:], self.tag_members(v[1:]))]
            return [("item", v)] if v in self.items else []
        return []

    def lookup_ore(self, name):
        if name not in self.ore:
            return []
        tag, members = self.ore[name]
        if not members:
            return []
        if tag:
            return [("tag", tag, members)]
        return [("item", m) for m in members]

    def thing(self, text, aliases, nbt=None, depth=0):
        """A ThingsString as atoms ('item', id) / ('tag', tag, members): all of them (an input's choices; an output
        takes the first)."""
        out, minus = [], []
        for entry in [e.strip() for e in text.split(",") if e.strip()]:
            neg = entry.startswith("-")
            entry = entry.lstrip("+-")
            atoms = self.one(entry, aliases, nbt, depth)
            (minus if neg else out).extend(atoms)
        if minus:
            drop = {a[1] for a in minus if a[0] == "item"} | {a[1] for a in minus if a[0] == "tag"}
            kept = []
            for a in out:
                if a[0] == "item" and a[1] in drop:
                    continue
                if a[0] == "tag" and a[1] in drop:
                    continue
                if a[0] == "tag":
                    # A tag minus some items: its remaining members.
                    rest = [m for m in a[2] if m not in drop]
                    if len(rest) != len(a[2]):
                        kept.extend(("item", m) for m in rest)
                        continue
                kept.append(a)
            out = kept
        return out

    def one(self, entry, aliases, nbt, depth):
        if depth < 10 and entry in aliases:
            return self.thing(aliases[entry], aliases, nbt, depth + 1)
        if entry.startswith("oredict:"):
            return self.lookup_ore(entry[len("oredict:"):])
        if entry.startswith(("item:", "block:")):
            entry = entry.split(":", 1)[1]
            if ":" not in entry:
                entry = "minecraft:" + entry
        if ":" in entry:
            parts = entry.split(":")
            mod, name = parts[0], parts[1]
            meta = parts[2] if len(parts) > 2 else None
            return self.lookup_id(mod, name, meta, nbt)
        # a bare name: minecraft's block or item first, then the ore dictionary
        v = self.lookup_id("minecraft", entry, None, nbt) if f"minecraft:{entry}" in self.vanilla_items or (entry, None) in V112 else []
        if v:
            return v
        return self.lookup_ore(entry)


def parse_nbt(text):
    """The few Mojangson strings the recipes use: {"enderio:dsu":"enderio:carpet"}, {Potion:"minecraft:water"}."""
    if not text:
        return None
    out = {}
    for k, v in re.findall(r'"?([A-Za-z_:]+)"?\s*:\s*"([^"]*)"', text):
        out[k] = v
    return out


class Converter:
    def __init__(self, jar, universe, files, report):
        self.jar = jar
        self.u = universe
        self.files = files
        self.report = report
        self.aliases = {}
        self.tags_used = {}       # tag -> members
        self.counts = {k: {"xml": 0, "written": 0} for k in KINDS + ["grindingball", "capacitor", "alias"]}
        self.skipped = []
        self.native = {"upgrade_recipes": [], "nbt_dropped": [], "enchantments_native": set()}
        self.names_used = set()
        self.unresolved = []

    # -- reading ----------------------------------------------------------------------------------------------
    def root(self, f):
        data = self.jar.read(f"assets/enderio/config/recipes/{f}.xml")
        return ET.fromstring(data)

    @staticmethod
    def tag(e):
        return e.tag.replace(NS, "")

    def conditions_ok(self, el):
        """<dependency> and <config> children of `el`, and its `disabled`."""
        if el.get("disabled") == "true":
            return False, "disabled"
        for c in el:
            t = self.tag(c)
            if t == "disabled":
                ok, _ = self.conditions_ok(c) if len(c) else (True, None)
                if ok:
                    return False, "disabled"
            elif t == "dependency":
                met = True
                why = None
                if c.get("mod"):
                    met = False
                    why = f"mod {c.get('mod')} absent"
                if met and c.get("item") and not self.u.thing(c.get("item"), self.aliases):
                    met = False
                    why = f"item {c.get('item')} absent"
                if met and c.get("upgrade"):
                    up = c.get("upgrade")
                    key = f"enderio:item_dark_steel_upgrade#1{{enderio:dsu={up}}}"
                    if key not in self.u.flat or self.u.flat[key].startswith("("):
                        met = False
                        why = f"upgrade {up} absent"
                if c.get("reverse") == "true":
                    met = not met
                    why = why or f"reverse dependency {c.attrib} met"
                if not met:
                    return False, why
            elif t == "config":
                return False, f"config {c.attrib}"
        return True, None

    def load_aliases(self, files):
        for f in files:
            for el in self.root(f):
                if self.tag(el) != "alias":
                    continue
                self.counts["alias"]["xml"] += 1
                ok, _ = self.conditions_ok(el)
                if ok:
                    self.aliases[el.get("name")] = el.get("item")
                    self.counts["alias"]["written"] += 1
        # MaterialOredicts.registerOredict: the organic dyes' aliases
        for base, idx in (("ORGANIC_GREEN_DYE", 48), ("ORGANIC_BROWN_DYE", 49), ("ORGANIC_BLACK_DYE", 50)):
            self.aliases[base] = f"enderio:item_material:{idx}"

    # -- ingredients and results --------------------------------------------------------------------------------
    def ingredient(self, text, nbt=None, potion=None):
        atoms = self.u.thing(text, self.aliases, parse_nbt(nbt))
        if not atoms:
            self.unresolved.append(text)
            return None
        if (nbt and "enderio:dsu" not in (nbt or "")) or potion:
            self.native["nbt_dropped"].append(f"{text} {nbt or potion}")
        out = []
        for a in atoms:
            if a[0] == "item":
                v = a[1]
            else:
                v = "#" + a[1]
                if not a[1].startswith("minecraft:"):
                    self.tags_used.setdefault(a[1], [])
                    for m in a[2]:
                        if m not in self.tags_used[a[1]]:
                            self.tags_used[a[1]].append(m)
            if v not in out:
                out.append(v)
        return out[0] if len(out) == 1 else out

    def result(self, text, amount=1, nbt=None):
        atoms = self.u.thing(text, self.aliases, parse_nbt(nbt))
        if not atoms:
            self.unresolved.append(text)
            return None
        a = atoms[0]
        rid = a[1] if a[0] == "item" else self.first_member(a)
        if not rid:
            return None
        out = {"id": rid}
        count = int(float(amount or 1))
        if count != 1:
            out["count"] = count
        n = parse_nbt(nbt)
        if n and "Potion" in n:
            out["components"] = {"minecraft:potion_contents": {"potion": n["Potion"]}}
        return out

    def first_member(self, atom):
        members = atom[2]
        eio = [m for m in members if m.startswith(f"{ID}:")]
        return (eio or members or [None])[0]

    def output(self, el):
        """A machine output: an item (or, for an ore dictionary name with no Ender IO item, its tag)."""
        text = el.get("name")
        atoms = self.u.thing(text, self.aliases, parse_nbt(el.get("nbt")))
        amount = el.get("amount", "1")
        count = int(float(amount))
        if not atoms:
            return None
        a = atoms[0]
        if a[0] == "item":
            o = {"item": a[1]}
        else:
            eio = [m for m in a[2] if m.startswith(f"{ID}:")]
            o = {"item": eio[0]} if eio else {"tag": a[1]}
            if "tag" in o:
                self.tags_used.setdefault(a[1], [])
                for m in a[2]:
                    if m not in self.tags_used[a[1]]:
                        self.tags_used[a[1]].append(m)
        o["count"] = count
        return o

    def unresolved_output_tag(self, text):
        """An optional output that resolves to nothing: its tag name, kept so another mod's items fill it."""
        name = text.split(",")[0].strip()
        if name in self.aliases:
            name = self.aliases[name].split(",")[0].strip()
        if name.startswith("oredict:"):
            name = name[len("oredict:"):]
        if ":" in name:
            return None
        return self.u.tag_for(name)

    # -- writing ----------------------------------------------------------------------------------------------
    def write(self, path, doc):
        base = path
        n = 2
        while path in self.names_used:
            path = f"{base}_{n}"
            n += 1
        self.names_used.add(path)
        self.files[f"data/{ID}/recipe/{path}.json"] = json.dumps(doc, indent=1).encode()

    # -- one <recipe> -------------------------------------------------------------------------------------------
    def recipe(self, el, fname):
        name = el.get("name") or "unnamed"
        kinds = [c for c in el if self.tag(c) in KINDS]
        kind_names = sorted({self.tag(c) for c in kinds})
        for k in kind_names:
            self.counts[k]["xml"] += 1
        for c in kinds:
            self.counts[self.tag(c)]["variants"] = self.counts[self.tag(c)].get("variants", 0) + 1
        ok, why = self.conditions_ok(el)
        if not ok:
            self.skipped.append({"recipe": name, "file": fname, "kind": ",".join(kind_names), "reason": why})
            return
        reasons = []
        self.unresolved = []
        for v in kinds:
            k = self.tag(v)
            ok, why = self.conditions_ok(v)
            if not ok:
                reasons.append(why)
                continue
            if k in ("casting", "brewing"):
                reasons.append("Tinkers' casting" if k == "casting" else "brewing")
                continue
            if k == "hiding":
                reasons.append("JEI visibility rules (the JEI kit's own data)")
                continue
            fn = getattr(self, "k_" + k)
            doc = fn(v, name)
            if isinstance(doc, str):
                reasons.append(doc)
                continue
            if doc is None:
                reasons.append("no item for " + ", ".join(sorted(set(self.unresolved))) if self.unresolved
                               else "an item does not resolve")
                self.unresolved = []
                continue
            path, body = doc
            self.write(path, body)
            self.counts[k]["written"] += 1
            return
        self.skipped.append({"recipe": name, "file": fname, "kind": ",".join(kind_names),
                             "reason": "; ".join(sorted(set(r for r in reasons if r))) or "no variant",
                             "required": el.get("required") == "true"})

    def k_crafting(self, v, name):
        grid = v.find(NS + "grid")
        shapeless = v.find(NS + "shapeless")
        outs = v.findall(NS + "output")
        if not outs:
            return None
        res = None
        for o in outs:
            ok, _ = self.conditions_ok(o)
            if ok:
                res = self.result(o.get("name"), o.get("amount", 1), o.get("nbt"))
                if res:
                    break
        if not res:
            return None
        if grid is not None:
            w, h = (int(x) for x in grid.get("size", "3x3").split("x"))
            cells = [it.get("name") for it in grid.findall(NS + "item")]
            keys, key_of, rows = {}, {}, []
            letters = iter("ABCDEFGHIJKLMNOPQRSTUVWXYZ")
            for r in range(h):
                row = ""
                for c in range(w):
                    i = r * w + c
                    cell = cells[i] if i < len(cells) else None
                    if not cell:
                        row += " "
                        continue
                    ing = self.ingredient(cell, grid.findall(NS + "item")[i].get("nbt"))
                    if ing is None:
                        return None
                    k = json.dumps(ing, sort_keys=True)
                    if k not in key_of:
                        key_of[k] = next(letters)
                        keys[key_of[k]] = ing
                    row += key_of[k]
                rows.append(row)
            doc = {"type": "minecraft:crafting_shaped", "category": "misc", "pattern": rows, "key": keys, "result": res}
            if v.get("upgrade") == "true":
                self.native["upgrade_recipes"].append(name)
        elif shapeless is not None:
            ings = []
            for it in shapeless.findall(NS + "item"):
                if not it.get("name"):
                    continue
                ing = self.ingredient(it.get("name"), it.get("nbt"))
                if ing is None:
                    return None
                ings.append(ing)
            doc = {"type": "minecraft:crafting_shapeless", "category": "misc", "ingredients": ings, "result": res}
            if v.get("upgrade") == "true":
                self.native["upgrade_recipes"].append(name)
        else:
            return None
        return safe_name(name), doc

    def k_smelting(self, v, name):
        if v.get("vanilla", "true") == "false":
            return "Tinkers' smeltery only (vanilla=false)"
        i = v.find(NS + "input")
        o = v.find(NS + "output")
        ing = self.ingredient(i.get("name"), i.get("nbt"))
        res = self.result(o.get("name"), o.get("amount", 1), o.get("nbt"))
        if ing is None or res is None:
            return None
        exp = v.get("exp")
        if exp is None:
            exp = self.vanilla_smelting_exp.get(res["id"], 0.0)
        doc = {"type": "minecraft:smelting", "ingredient": ing, "result": res, "experience": float(exp), "cookingtime": 200}
        return "smelting/" + safe_name(name), doc

    def machine_path(self, kind, name):
        d, _ = MACHINE_DIR[kind]
        n = safe_name(name)
        for prefix in ("alloy_", "sagmill_", "sag_mill_", "soul_binder_", "slice_n_splice_", "vat_", "tank_",
                       "enchanter_", "fuel_", "coolant_"):
            if n.startswith(prefix) and len(n) > len(prefix):
                n = n[len(prefix):]
                break
        return f"{d}/{n}"

    def counted(self, el, ing_attr="name"):
        ing = self.ingredient(el.get(ing_attr), el.get("nbt"), el.get("potion"))
        if ing is None:
            return None
        out = {"ingredient": ing}
        amount = el.get("amount")
        out["count"] = int(float(amount)) if amount else 1
        return out

    def k_alloying(self, v, name):
        inputs = []
        for i in v.findall(NS + "input"):
            x = self.counted(i)
            if x is None:
                return None
            inputs.append(x)
        o = v.find(NS + "output")
        res = self.result(o.get("name"), o.get("amount", 1), o.get("nbt"))
        if res is None:
            return None
        res.setdefault("count", 1)
        doc = {"type": f"{ID}:alloy_smelting", "energy": int(v.get("energy")), "experience": float(v.get("exp", 0)),
               "inputs": inputs, "output": res}
        return self.machine_path("alloying", name), doc

    def k_sagmilling(self, v, name):
        i = v.find(NS + "input")
        ing = self.ingredient(i.get("name"), i.get("nbt"))
        if ing is None:
            return None
        outs = []
        for o in v.findall(NS + "output"):
            ok, _ = self.conditions_ok(o)
            if not ok:
                continue
            required = o.get("required", "true") != "false"
            x = self.output(o)
            chance = float(o.get("chance", 1))
            if x is None:
                if required:
                    return None
                tag = self.unresolved_output_tag(o.get("name"))
                if not tag:
                    continue
                x = {"tag": tag, "count": int(float(o.get("amount", 1)))}
            x["chance"] = chance
            if not required:
                x["optional"] = True
            outs.append(x)
        if not outs:
            return None
        doc = {"type": f"{ID}:sag_milling", "energy": int(v.get("energy")), "bonus": v.get("bonus", "multiply_output"),
               "input": ing, "outputs": outs}
        return self.machine_path("sagmilling", name), doc

    def k_slicing(self, v, name):
        inputs = []
        for i in v.findall(NS + "input"):
            if not i.get("name"):
                inputs.append(None)
                continue
            ing = self.ingredient(i.get("name"), i.get("nbt"))
            if ing is None:
                return None
            inputs.append(ing)
        o = v.find(NS + "output")
        res = self.result(o.get("name"), o.get("amount", 1), o.get("nbt"))
        if res is None:
            return None
        res.setdefault("count", 1)
        return self.machine_path("slicing", name), {"type": f"{ID}:slicing", "energy": int(v.get("energy")),
                                                     "inputs": inputs, "output": res}

    def entity(self, n):
        n = ENTITY_RENAME.get(n, n)
        return n if n in self.u.entities else None

    def k_soulbinding(self, v, name):
        i = v.find(NS + "input")
        ing = self.counted(i)
        if ing is None:
            return None
        souls = []
        for s in v.findall(NS + "soul"):
            e = self.entity(s.get("name"))
            if e:
                souls.append(e)
        mode = v.get("souls", "LISTED")
        if mode == "LISTED" and not souls:
            return "its souls are other mods' mobs"
        o = v.find(NS + "output")
        res = self.result(o.get("name"), o.get("amount", 1), o.get("nbt"))
        if res is None:
            return None
        res.setdefault("count", 1)
        return self.machine_path("soulbinding", name), {
            "type": f"{ID}:soul_binding", "energy": int(v.get("energy")), "levels": int(v.get("levels")),
            "logic": v.get("logic", "NONE").lower(), "souls_mode": mode.lower(), "souls": souls, "input": ing, "output": res}

    def fluid(self, n):
        if n in ("water", "lava"):
            return f"minecraft:{n}"
        v = self.u.flat.get(f"fluid:{n}")
        return v if v and not v.startswith("(") else None

    def k_fermenting(self, v, name):
        groups = []
        for g in v.findall(NS + "inputgroup"):
            items = []
            for i in g.findall(NS + "input"):
                ing = self.ingredient(i.get("name"), i.get("nbt"))
                if ing is None:
                    continue
                items.append({"ingredient": ing, "multiplier": float(i.get("multiplier", 1))})
            if not items:
                return None
            groups.append(items)
        fi = v.find(NS + "inputfluid")
        fo = v.find(NS + "outputfluid")
        a, b = self.fluid(fi.get("name")), self.fluid(fo.get("name"))
        if not a or not b:
            return None
        return self.machine_path("fermenting", name), {
            "type": f"{ID}:fermenting", "energy": int(v.get("energy")), "input_groups": groups,
            "input_fluid": {"fluid": a, "multiplier": float(fi.get("multiplier", 1))}, "output_fluid": b}

    def k_tanking(self, v, name):
        i = v.find(NS + "input")
        ing = self.ingredient(i.get("name"), i.get("nbt"), i.get("potion"))
        if ing is None:
            return None
        f = v.find(NS + "fluid")
        fl = self.fluid(f.get("name"))
        if not fl:
            return None
        doc = {"type": f"{ID}:tanking", "mode": v.get("type").lower(), "logic": v.get("logic", "NONE").lower(),
               "input": ing, "fluid": {"fluid": fl, "amount": int(float(f.get("amount", 1000)))}}
        o = v.find(NS + "output")
        if o is not None:
            res = self.result(o.get("name"), o.get("amount", 1), o.get("nbt"))
            if res is None:
                return None
            res.setdefault("count", 1)
            doc["output"] = res
        return self.machine_path("tanking", name), doc

    def k_enchanting(self, v, name):
        i = v.find(NS + "input")
        ing = self.ingredient(i.get("name"), i.get("nbt"))
        if ing is None:
            return None
        e = v.find(NS + "enchantment")
        en = ENCHANT_RENAME.get(e.get("name"), e.get("name"))
        if en not in self.u.enchants and en not in EIO_ENCHANTMENTS:
            return f"enchantment {en} absent"
        if en in EIO_ENCHANTMENTS:
            self.native["enchantments_native"].add(en)
        return self.machine_path("enchanting", name), {
            "type": f"{ID}:enchanting", "input": ing, "count": int(float(i.get("amount", 1))), "enchantment": en,
            "cost_multiplier": float(e.get("costMultiplier", 1))}

    def k_spawning(self, v, name):
        ents = []
        for e in v.findall(NS + "entity"):
            n = e.get("name")
            if n in ("*", "*boss*") or n.endswith(":*"):
                ent = n
                if n.endswith(":*") and not n.startswith("minecraft:"):
                    continue
            else:
                ent = self.entity(n)
                if not ent:
                    continue
            ents.append({"entity": ent, "cost_multiplier": float(e.get("costMultiplier", 1)),
                         "disabled": e.get("disabled", "false") == "true", "clone": e.get("clone", "false") == "true",
                         "soul_vial": e.get("soulvial", "true") == "true"})
        if not ents:
            return "its entities are other mods'"
        return self.machine_path("spawning", name), {"type": f"{ID}:spawner_entity", "entities": ents}

    def k_fuel(self, v, name):
        fl = self.fluid(v.get("fluid"))
        if not fl:
            return f"fluid {v.get('fluid')} absent"
        return self.machine_path("fuel", "fuel_" + name), {
            "type": f"{ID}:combustion_fuel", "fluid": fl, "power_per_tick": int(v.get("pertick")), "ticks": int(v.get("ticks"))}

    def k_coolant(self, v, name):
        fl = self.fluid(v.get("fluid"))
        if not fl:
            return f"fluid {v.get('fluid')} absent"
        return self.machine_path("coolant", "coolant_" + name), {
            "type": f"{ID}:combustion_coolant", "fluid": fl, "degrees_per_mb": float(v.get("amount"))}

    # -- top-level grinding balls and capacitors -----------------------------------------------------------------
    def grindingball(self, el, fname):
        self.counts["grindingball"]["xml"] += 1
        ok, why = self.conditions_ok(el)
        if not ok:
            self.skipped.append({"recipe": el.get("name"), "file": fname, "kind": "grindingball", "reason": why})
            return
        it = el.find(NS + "item")
        atoms = self.u.thing(it.get("name"), self.aliases)
        if not atoms:
            self.skipped.append({"recipe": el.get("name"), "file": fname, "kind": "grindingball",
                                 "reason": "its item does not resolve"})
            return
        a = atoms[0]
        item = a[1] if a[0] == "item" else self.first_member(a)
        self.write("grinding_ball/" + safe_name(el.get("name")), {
            "type": f"{ID}:grinding_ball", "item": item, "grinding": float(el.get("grinding", 1)),
            "chance": float(el.get("chance", 1)), "power": float(el.get("power", 1)),
            "durability": int(el.get("durability"))})
        self.counts["grindingball"]["written"] += 1

    def capacitor(self, el, fname):
        self.counts["capacitor"]["xml"] += 1
        ok, why = self.conditions_ok(el)
        if not ok:
            self.skipped.append({"recipe": el.get("key"), "file": fname, "kind": "capacitor", "reason": why})
            return
        key = el.get("key")
        scaler = el.get("scaler")
        doc = {"type": f"{ID}:capacitor", "key": key, "base": int(el.get("base"))}
        sc = el.find(NS + "scaler")
        ix = el.find(NS + "indexed")
        if scaler:
            doc["scaler"] = scaler
        elif sc is not None:
            doc["scaler"] = sc.get("name")
        elif ix is not None:
            doc["scaler"] = {"indexed": {"step": float(ix.get("step", 1)),
                                         "data": [float(d.get("value")) for d in ix.findall(NS + "data")]}}
        ns, path = key.split(":", 1)
        self.write("capacitor/" + safe_name(f"{ns}_{path}"), doc)
        self.counts["capacitor"]["written"] += 1

    # -- all files -------------------------------------------------------------------------------------------------
    def run(self):
        files = [f for _, f in sorted(CORE_FILES, key=lambda x: x[0])]
        self.load_aliases(files)
        for f in files:
            for el in self.root(f):
                t = self.tag(el)
                if t == "recipe":
                    self.recipe(el, f)
                elif t == "grindingball":
                    self.grindingball(el, f)
                elif t == "capacitor":
                    self.capacitor(el, f)
        integration = {}
        for f in INTEGRATION_FILES:
            n = sum(1 for el in self.root(f) if self.tag(el) == "recipe")
            integration[f] = n
        return integration
