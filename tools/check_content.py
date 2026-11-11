"""Static checks of the Ender IO content mod (no game needed).

Usage: python check_content.py <enderio-content.hmod>

- only Horizon's paths (assets/enderio, data/enderio, c: and minecraft: tags, horizon-mod.toml) and no stray files;
- every JSON parses; every block has a declaration, a block state and (unless `item: false`) an item definition;
  every item a declaration (block items excepted) and a definition;
- every model a block state or item definition names resolves, with its parents, and every texture it uses is a
  file in the pack or a vanilla texture; texture variables resolve;
- block states name only the properties the block has (its template's and its own) with valid values;
- templates are vanilla blocks; declared state properties are not the template's;
- recipe, loot, advancement and creative tab ids are our items or 26.3 vanilla items; tags are ours or vanilla's;
- every block and item has its name in en_us.
"""

import json
import os
import pathlib
import re
import sys
import zipfile

HORIZON_26 = pathlib.Path(os.environ.get("APPDATA", "")) / "horizon-launcher/versions/nightly-2026-11-08-fdc87ae/.local/horizon-client/26.3"
VA = HORIZON_26 / "assets/minecraft"
VD = HORIZON_26 / "data/minecraft"
ID = "enderio"


def main(path):
    z = zipfile.ZipFile(path)
    names = z.namelist()
    nameset = set(names)
    errors, warnings = [], []
    reg = json.loads((HORIZON_26 / "registries.json").read_text(encoding="utf-8"))
    vitems = set(reg["minecraft:item"]["entries"])
    vblocks = set(reg["minecraft:block"]["entries"])
    vfluids = set(reg["minecraft:fluid"]["entries"])

    # -- paths --------------------------------------------------------------------------------------------------
    for n in names:
        ok = (n == "horizon-mod.toml" or re.match(r"assets/enderio/(blockstates|models|items|textures|lang|sounds)/", n)
              or n == "assets/enderio/sounds.json" or re.match(r"data/enderio/", n)
              or re.match(r"data/(c|minecraft)/tags/(item|block|fluid)/", n))
        if not ok:
            errors.append(f"path outside Horizon's rules: {n}")
        if n.startswith("assets/enderio/") and n.count("/") == 2 and n != "assets/enderio/sounds.json":
            errors.append(f"stray file: {n}")
        if not re.search(r"\.(json|png|mcmeta|ogg|md|toml)$", n):
            errors.append(f"stray file type: {n}")
    docs = {}
    for n in names:
        if n.endswith(".json") or n.endswith(".mcmeta"):
            try:
                docs[n] = json.loads(z.read(n).decode("utf-8"))
            except ValueError as e:
                errors.append(f"bad JSON {n}: {e}")

    def jdoc(n):
        return docs.get(n)

    # -- models and textures ------------------------------------------------------------------------------------
    model_cache = {}

    def model(mid):
        if mid in model_cache:
            return model_cache[mid]
        ns, p = mid.split(":", 1) if ":" in mid else ("minecraft", mid)
        if ns == ID:
            d = jdoc(f"assets/{ID}/models/{p}.json")
        elif ns == "minecraft":
            f = VA / "models" / (p + ".json")
            d = json.loads(f.read_text(encoding="utf-8")) if f.exists() else None
        else:
            d = None
        model_cache[mid] = d
        return d

    def texture_exists(t):
        ns, p = t.split(":", 1) if ":" in t else ("minecraft", t)
        if ns == ID:
            return f"assets/{ID}/textures/{p}.png" in nameset
        if ns == "minecraft":
            return (VA / "textures" / (p + ".png")).exists()
        return False

    checked = set()

    def check_model(mid, where):
        if mid in checked:
            return
        checked.add(mid)
        chain, textures, elements = [], {}, None
        cur = mid
        while cur:
            if cur.startswith("builtin/"):
                break
            d = model(cur if ":" in cur else "minecraft:" + cur)
            if d is None:
                errors.append(f"missing model {cur} (from {where})")
                return
            chain.append(cur)
            for k, v in d.get("textures", {}).items():
                textures.setdefault(k, v)
            if elements is None and "elements" in d:
                elements = d["elements"]
            cur = d.get("parent")
            if len(chain) > 30:
                errors.append(f"parent loop {mid}")
                return

        def res(v, depth=0):
            while isinstance(v, str) and v.startswith("#") and depth < 10:
                v = textures.get(v[1:])
                depth += 1
            return v
        for k, v in textures.items():
            r = res(v)
            if r is None:
                continue  # an unresolved variable in a template the child fills
            if not texture_exists(r):
                errors.append(f"missing texture {r} (model {mid}, from {where})")
        for e in elements or []:
            for f, face in e.get("faces", {}).items():
                t = face.get("texture")
                r = res(t)
                if r is None or (isinstance(r, str) and r.startswith("#")):
                    if mid.startswith(f"{ID}:"):
                        errors.append(f"unresolved texture variable {t} in {mid} (from {where})")
                elif not texture_exists(r):
                    errors.append(f"missing texture {r} (model {mid})")

    def models_in(obj):
        if isinstance(obj, dict):
            if "model" in obj and isinstance(obj["model"], str):
                yield obj["model"]
            for k in ("base",):
                if isinstance(obj.get(k), str):
                    yield obj[k]
            for v in obj.values():
                yield from models_in(v)
        elif isinstance(obj, list):
            for v in obj:
                yield from models_in(v)

    # -- model elements as 26.3 reads them ---------------------------------------------------------------------
    for n in names:
        if not (n.startswith(f"assets/{ID}/models/") and n.endswith(".json")):
            continue
        for e in (jdoc(n) or {}).get("elements", []):
            if any(c < -16 or c > 32 for c in e["from"] + e["to"]):
                errors.append(f"{n}: element outside -16..32")
            r = e.get("rotation")
            if r and (r.get("axis") not in ("x", "y", "z") or float(r.get("angle", 0)) not in (-45, -22.5, 0, 22.5, 45)):
                errors.append(f"{n}: element rotation {r}")
            for k in e:
                if k not in ("from", "to", "faces", "rotation", "shade", "light_emission"):
                    errors.append(f"{n}: element key {k}")
            for f, face in e.get("faces", {}).items():
                if f not in ("north", "south", "east", "west", "up", "down"):
                    errors.append(f"{n}: face {f}")
                if face.get("rotation", 0) not in (0, 90, 180, 270):
                    errors.append(f"{n}: face rotation {face['rotation']}")
                if "cullface" in face and face["cullface"] not in ("north", "south", "east", "west", "up", "down"):
                    errors.append(f"{n}: cullface {face['cullface']}")
                if "uv" in face and len(face["uv"]) != 4:
                    errors.append(f"{n}: uv {face['uv']}")

    # -- blocks -------------------------------------------------------------------------------------------------
    decls = {n.split("/")[-1][:-5]: jdoc(n) for n in names if n.startswith(f"data/{ID}/horizon/block/")}
    item_decls = {n.split("/")[-1][:-5]: jdoc(n) for n in names if n.startswith(f"data/{ID}/horizon/item/")}
    item_defs = {n.split("/")[-1][:-5]: jdoc(n) for n in names if n.startswith(f"assets/{ID}/items/")}
    blockstates = {n.split("/")[-1][:-5]: jdoc(n) for n in names if n.startswith(f"assets/{ID}/blockstates/")}
    allowed_props = {"hardness", "explosion_resistance", "light", "requires_tool", "friction", "map_color", "sounds"}
    allowed_decl = {"template", "properties", "state_properties", "default_state", "shapes", "item"}

    def template_props(t):
        f = VA / "blockstates" / (t.split(":", 1)[1] + ".json")
        props = {}
        if not f.exists():
            return props
        bs = json.loads(f.read_text(encoding="utf-8"))
        for k in bs.get("variants", {}):
            for part in filter(None, k.split(",")):
                a, b = part.split("=")
                props.setdefault(a, set()).add(b)
        for p in bs.get("multipart", []):
            w = p.get("when", {})
            for c in w.get("OR", w.get("AND", [w])):
                for a, b in c.items():
                    props.setdefault(a, set()).update(str(b).split("|"))
        return props

    for bid, d in decls.items():
        for k in d:
            if k not in allowed_decl:
                errors.append(f"block {bid}: unknown key {k}")
        for k in d.get("properties", {}):
            if k not in allowed_props:
                warnings.append(f"block {bid}: property {k} not in the documented list")
        t = d["template"]
        if t not in vblocks:
            errors.append(f"block {bid}: template {t} is not a vanilla block")
        tprops = template_props(t)
        own = d.get("state_properties", {})
        for p in own:
            if p in tprops:
                errors.append(f"block {bid}: declares {p}, which {t} has")
        if set(d.get("default_state", {})) - set(own):
            errors.append(f"block {bid}: default_state names undeclared properties")
        bs = blockstates.get(bid)
        if bs is None:
            errors.append(f"block {bid}: no block state")
            continue
        props = {k: set(v) for k, v in tprops.items()}
        for p, spec in own.items():
            props[p] = {"true", "false"} if spec["type"] == "boolean" else set(spec["values"])
        keys = []
        for k in bs.get("variants", {}):
            for part in filter(None, k.split(",")):
                keys.append(part.split("="))
        for p in bs.get("multipart", []):
            w = p.get("when", {})
            for c in w.get("OR", [w]):
                for a, b in c.items():
                    for v in str(b).split("|"):
                        keys.append((a, v))
        for a, b in keys:
            if a not in props:
                errors.append(f"block state {bid}: property {a} not on the block")
            elif b not in props[a]:
                errors.append(f"block state {bid}: {a}={b} not a value")
        for m in models_in(bs):
            check_model(m, f"blockstate {bid}")
        if d.get("item") is not False and bid not in item_defs:
            errors.append(f"block {bid}: no item definition")
    for bid in blockstates:
        if bid not in decls:
            errors.append(f"block state {bid} without a declaration")
    for iid, d in item_defs.items():
        if iid not in decls and iid not in item_decls:
            errors.append(f"item definition {iid} without a declaration")
        for m in models_in(d):
            check_model(m, f"item {iid}")
    for iid, d in item_decls.items():
        for k in d:
            if k not in ("template", "max_stack_size", "rarity"):
                errors.append(f"item {iid}: unknown key {k}")
        if d["template"] not in vitems:
            errors.append(f"item {iid}: template {d['template']} not a vanilla item")
        if iid not in item_defs:
            errors.append(f"item {iid}: no item definition")
    ours = {f"{ID}:{i}" for i in item_defs}
    all_items = ours | vitems

    # -- tags -----------------------------------------------------------------------------------------------------
    our_tags = {}
    for n in names:
        m = re.match(r"data/([a-z]+)/tags/item/(.+)\.json$", n)
        if m:
            our_tags[f"{m.group(1)}:{m.group(2)}"] = jdoc(n)
    vtags = {"minecraft:" + p.relative_to(VD / "tags/item").as_posix()[:-5] for p in (VD / "tags/item").rglob("*.json")}
    for t, d in our_tags.items():
        for v in d.get("values", []):
            v = v["id"] if isinstance(v, dict) else v
            if v.startswith("#"):
                if v[1:] not in our_tags and v[1:] not in vtags:
                    errors.append(f"tag {t}: unknown tag {v}")
            elif v not in all_items:
                errors.append(f"tag {t}: unknown item {v}")

    def check_ing(x, where):
        if isinstance(x, list):
            for y in x:
                check_ing(y, where)
            return
        if not isinstance(x, str):
            errors.append(f"{where}: odd ingredient {x}")
            return
        if x.startswith("#"):
            if x[1:] not in our_tags and x[1:] not in vtags:
                errors.append(f"{where}: unknown tag {x}")
        elif x not in all_items:
            errors.append(f"{where}: unknown item {x}")

    # -- recipes ----------------------------------------------------------------------------------------------------
    rtypes = {}
    for n in names:
        if not n.startswith(f"data/{ID}/recipe/"):
            continue
        d = jdoc(n)
        t = d.get("type")
        rtypes[t] = rtypes.get(t, 0) + 1
        if t in ("minecraft:crafting_shaped", "minecraft:crafting_shapeless"):
            if "category" not in d:
                errors.append(f"{n}: no category")
            for k, v in d.get("key", {}).items():
                check_ing(v, n)
            for v in d.get("ingredients", []):
                check_ing(v, n)
            if d["result"]["id"] not in all_items:
                errors.append(f"{n}: unknown result {d['result']['id']}")
            if t == "minecraft:crafting_shaped":
                used = set("".join(d["pattern"])) - {" "}
                if used != set(d["key"]):
                    errors.append(f"{n}: pattern and key disagree")
        elif t == "minecraft:smelting":
            check_ing(d["ingredient"], n)
            if d["result"]["id"] not in all_items:
                errors.append(f"{n}: unknown result")
        elif t and t.startswith(f"{ID}:"):
            def walk(o, key=None):
                if isinstance(o, dict):
                    for k, v in o.items():
                        if k in ("ingredient", "input") and not isinstance(v, dict):
                            check_ing(v, n)
                        elif k in ("id", "item") and isinstance(v, str):
                            if v not in all_items:
                                errors.append(f"{n}: unknown item {v}")
                        elif k in ("fluid", "input_fluid", "output_fluid") and isinstance(v, str):
                            if v not in vfluids and not v.startswith(f"{ID}:"):
                                errors.append(f"{n}: unknown fluid {v}")
                        else:
                            walk(v, k)
                elif isinstance(o, list):
                    for v in o:
                        if key == "inputs" and isinstance(v, (str, list)):
                            check_ing(v, n)
                        else:
                            walk(v, key)
            walk(d)
        else:
            errors.append(f"{n}: unknown recipe type {t}")

    # -- loot, advancements, creative tabs ----------------------------------------------------------------------------
    def loot_items(o):
        if isinstance(o, dict):
            if o.get("type") == "minecraft:item" and "name" in o:
                yield o["name"]
            for v in o.values():
                yield from loot_items(v)
        elif isinstance(o, list):
            for v in o:
                yield from loot_items(v)
    for n in names:
        if n.startswith(f"data/{ID}/loot_table/"):
            for i in loot_items(jdoc(n)):
                if i not in all_items:
                    errors.append(f"{n}: unknown item {i}")
        if n.startswith(f"data/{ID}/advancement/"):
            d = jdoc(n)
            for c, crit in d.get("criteria", {}).items():
                for p in crit.get("conditions", {}).get("items", []):
                    it = p.get("items")
                    if it is None:
                        errors.append(f"{n}#{c}: an item predicate without items (matches everything)")
                    else:
                        check_ing(it, f"{n}#{c}")
            icon = d.get("display", {}).get("icon", {}).get("id")
            if icon and icon not in all_items:
                errors.append(f"{n}: unknown icon {icon}")
            par = d.get("parent")
            if par and par.startswith(f"{ID}:") and f"data/{ID}/advancement/{par.split(':', 1)[1]}.json" not in nameset:
                errors.append(f"{n}: unknown parent {par}")
        if n.startswith(f"data/{ID}/horizon/creative_tab/"):
            d = jdoc(n)
            for i in d["items"] + [d["icon"]]:
                if i not in ours:
                    errors.append(f"{n}: unknown item {i}")

    # -- old (1.20-) forms that 26.3 reads as nothing: string conditions, block_state_property, list players --
    def old_forms(o, where):
        if isinstance(o, dict):
            if isinstance(o.get("condition"), str) and not o["condition"].startswith(("minecraft:tool/",)):
                errors.append(f"{where}: condition as a string ({o['condition']}): 26.3 wants {{\"type\": ...}}")
            if o.get("type") == "minecraft:block_state_property":
                errors.append(f"{where}: block_state_property is match_block in 26.3")
            if "functions" in o or "function" in o or "conditions" in o and where.startswith(f"data/{ID}/loot_table"):
                errors.append(f"{where}: 1.20 loot keys (functions/conditions)")
            if isinstance(o.get("player"), list):
                errors.append(f"{where}: player conditions as a list (26.3: one condition object)")
            if "predicate" in o and isinstance(o["predicate"], dict) and "location" in o["predicate"]:
                errors.append(f"{where}: entity predicate key `location` (26.3: minecraft:location)")
            for v in o.values():
                old_forms(v, where)
        elif isinstance(o, list):
            for v in o:
                old_forms(v, where)
    for n in names:
        if n.startswith((f"data/{ID}/loot_table/", f"data/{ID}/advancement/")):
            old_forms(jdoc(n), n)

    # -- lang ------------------------------------------------------------------------------------------------------
    en = jdoc(f"assets/{ID}/lang/en_us.json") or {}
    for bid, d in decls.items():
        if f"block.{ID}.{bid}" not in en:
            errors.append(f"no en_us name for block {bid}")
    for iid in item_decls:
        if f"item.{ID}.{iid}" not in en:
            errors.append(f"no en_us name for item {iid}")

    # -- counts ----------------------------------------------------------------------------------------------------
    print(f"files {len(names)}; blocks {len(decls)} (with item {sum(1 for d in decls.values() if d.get('item') is not False)}); "
          f"item definitions {len(item_defs)}; item declarations {len(item_decls)}; block states {len(blockstates)}")
    print(f"models checked {len(checked)}; tags {len(our_tags)}; recipe types {rtypes}")
    for w in warnings[:20]:
        print("warning:", w)
    print(f"{len(errors)} errors")
    for e in errors[:200]:
        print("  ", e)
    return 1 if errors else 0


if __name__ == "__main__":
    sys.exit(main(sys.argv[1]))
