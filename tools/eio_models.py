"""Ender IO's 1.12 models and Forge block states as 26.3 block states and models.

- `Models` copies the jar's model files it is asked for into 26.3 form (texture paths `blocks/`->`block/`,
  `items/`->`item/`, vanilla 1.12 names renamed, Forge-only keys dropped) and bakes "a model with these textures" into a
  model file of its own.
- `forge_parts` evaluates a Forge (`forge_marker: 1`) or vanilla 1.12 block state for one assignment of its properties:
  the merged defaults and property entries, as the list of models (main model and submodels) it draws, each with its
  own rotation. A 26.3 block state with one part per state is written as `variants`, several as `multipart`.
- `merge_parts` folds rotated parts into one model with the rotations applied to the elements (an item model can't
  rotate its parts).
"""

import copy
import hashlib
import json
import re

from eio_tables import VANILLA_PARENTS, VANILLA_TEXTURES

ID = "enderio"
NULL_MODELS = {"enderio:null", "enderio:block/null"}
FORGE_KEYS = {"model", "textures", "submodel", "x", "y", "uvlock", "transform", "weight", "custom", "smooth_lighting",
              "gui3d"}
SIDES = ("north", "south", "east", "west", "up", "down")


def lenient_json(data):
    """1.12's JSON files: some carry // comments or trailing commas, which Gson allowed."""
    text = data.decode("utf-8-sig") if isinstance(data, bytes) else data
    try:
        return json.loads(text)
    except ValueError:
        pass
    text = re.sub(r"(?m)^\s*//.*$", "", text)
    text = re.sub(r"/\*.*?\*/", "", text, flags=re.S)
    text = re.sub(r",(\s*[}\]])", r"\1", text)
    return json.loads(text)


class Report:
    def __init__(self):
        self.unknown_textures = set()
        self.missing_models = set()
        self.notes = []


def texture(ref, report=None):
    """A 1.12 texture reference as 26.3's."""
    if not isinstance(ref, str) or ref.startswith("#"):
        return ref
    if ref == "endercore:blank" or ref == "endercore:blocks/blank":
        return f"{ID}:block/blank"
    ns, path = ref.split(":", 1) if ":" in ref else ("minecraft", ref)
    if ns == "minecraft":
        if path in VANILLA_TEXTURES:
            return VANILLA_TEXTURES[path]
        if path.startswith(("block/", "item/")):
            return f"minecraft:{path}"
        if report is not None:
            report.unknown_textures.add(ref)
        return "minecraft:" + path.replace("blocks/", "block/", 1).replace("items/", "item/", 1)
    if ns != ID:
        # enderiomachines' DSU icons and the like: their textures move under Ender IO's namespace.
        path = path.replace("items/", f"item/", 1).replace("blocks/", "block/", 1)
        return f"{ID}:{path}"
    if path.startswith("blocks/"):
        return f"{ID}:block/" + path[len("blocks/"):]
    if path.startswith("items/"):
        return f"{ID}:item/" + path[len("items/"):]
    return f"{ID}:{path}"


def texture_file(asset_path):
    """A texture file's path in the jar (assets/<ns>/textures/...) as its path in the pack, or None to drop it."""
    m = re.match(r"assets/([a-z_]+)/textures/(.+)$", asset_path)
    if not m:
        return None
    ns, rest = m.groups()
    if ns == "enderiointegrationforestry":
        return None
    if rest.startswith("blocks/"):
        rest = "block/" + rest[len("blocks/"):]
    elif rest.startswith("items/"):
        rest = "item/" + rest[len("items/"):]
    return f"assets/{ID}/textures/{rest}"


def bs_model_ref(ref):
    """A model named in a 1.12 block state ("enderio:block_alloy_smelter", "anvil") as (namespace, path under models/).
    Forge and vanilla 1.12 both put block states' models under models/block/."""
    ns, path = ref.split(":", 1) if ":" in ref else ("minecraft", ref)
    if ns == "minecraft" and ("block/" + path) in VANILLA_PARENTS:
        return "minecraft", VANILLA_PARENTS["block/" + path].split(":", 1)[1]
    if ref in VANILLA_PARENTS:
        return "minecraft", VANILLA_PARENTS[ref].split(":", 1)[1]
    if path.startswith(("block/", "item/")) and ns == "minecraft":
        return ns, path
    return ns, "block/" + path


def parent_ref(ref):
    """A 1.12 model's `parent` as (namespace, path under models/)."""
    if ref in VANILLA_PARENTS:
        return tuple(VANILLA_PARENTS[ref].split(":", 1))
    ns, path = ref.split(":", 1) if ":" in ref else ("minecraft", ref)
    if ns != "minecraft" and not path.startswith(("block/", "item/")):
        path = "block/" + path
    return ns, path


class Models:
    """The jar's models as 26.3 model files, written on demand into `files`."""

    def __init__(self, jar, files, vanilla_assets, report):
        self.jar = jar
        self.names = set(jar.namelist())
        self.files = files
        self.vanilla = vanilla_assets
        self.report = report
        self.done = {}
        self.baked = {}
        self.baked_names = set()
        self.cache = {}

    # -- the jar's model files -------------------------------------------------------------------------------
    def exists(self, ns, path):
        if ns == "minecraft":
            return (self.vanilla / "models" / (path + ".json")).exists()
        return f"assets/{ns}/models/{path}.json" in self.names

    def raw(self, ns, path):
        key = (ns, path)
        if key in self.cache:
            return self.cache[key]
        if ns == "minecraft":
            p = self.vanilla / "models" / (path + ".json")
            doc = json.loads(p.read_text(encoding="utf-8")) if p.exists() else None
        else:
            name = f"assets/{ns}/models/{path}.json"
            doc = lenient_json(self.jar.read(name)) if name in self.names else None
        self.cache[key] = doc
        return doc

    def ensure(self, ns, path):
        """Writes the jar's model (and its parents) in 26.3 form; returns its 26.3 id."""
        if ns == "minecraft":
            if not self.exists(ns, path):
                self.report.missing_models.add(f"{ns}:{path}")
            return f"minecraft:{path}"
        out_id = f"{ID}:{path}"
        if (ns, path) in self.done:
            return self.done[(ns, path)]
        self.done[(ns, path)] = out_id
        doc = self.raw(ns, path)
        if doc is None:
            self.report.missing_models.add(f"{ns}:{path}")
            self.files[f"assets/{ID}/models/{path}.json"] = json.dumps({}).encode()
            return out_id
        self.files[f"assets/{ID}/models/{path}.json"] = json.dumps(self.convert(doc), indent=1).encode()
        return out_id

    def convert(self, doc):
        out = {}
        for k, v in doc.items():
            if k.startswith("__") or k in ("credit", "loader", "custom", "overrides", "gui3d", "groups"):
                continue
            if k == "parent":
                if v == "builtin/entity":
                    out["parent"] = "minecraft:item/generated"
                    continue
                pns, ppath = parent_ref(v)
                out["parent"] = self.ensure(pns, ppath)
            elif k == "textures":
                out["textures"] = {tk: texture(tv, self.report) for tk, tv in v.items()}
            elif k == "elements":
                out["elements"] = [clean_element(e) for e in v]
            else:
                out[k] = v
        return out

    # -- resolved views (parent chain followed) ---------------------------------------------------------------
    def resolved(self, model_id):
        """(elements, textures, display) of a 26.3 model id, parents followed: the files written here and vanilla's."""
        textures, elements, display = {}, None, {}
        seen = 0
        while model_id and seen < 20:
            seen += 1
            ns, path = model_id.split(":", 1) if ":" in model_id else ("minecraft", model_id)
            if ns == ID:
                data = self.files.get(f"assets/{ID}/models/{path}.json")
                doc = json.loads(data) if data else {}
            else:
                doc = self.raw("minecraft", path) or {}
            for k, v in doc.get("textures", {}).items():
                textures.setdefault(k, v)
            if elements is None and "elements" in doc:
                elements = doc["elements"]
            for k, v in doc.get("display", {}).items():
                display.setdefault(k, v)
            model_id = doc.get("parent")
            if model_id and ":" not in model_id:
                model_id = "minecraft:" + model_id
        return elements or [], textures, display

    # -- baking a model with block-state textures ---------------------------------------------------------------
    def bake(self, model112, textures, hint):
        """The 1.12 block state model `model112` (a block-state reference) drawn with `textures` (1.12 refs): a 26.3
        model id, a file of its own when the textures change anything."""
        ns, path = bs_model_ref(model112)
        base = self.ensure(ns, path)
        if base in NULL_MODELS:
            return None
        tex = resolve_refs(textures)
        tex = {k: texture(v, self.report) for k, v in tex.items()}
        if not tex:
            return base
        key = (base, tuple(sorted(tex.items())))
        if key in self.baked:
            return self.baked[key]
        # A variable the model's faces use that neither it nor the block state sets (block_enhanced_sagmill's
        # #frontonoff): 1.12 drew the missing texture there; the front (or the particle) here.
        els, chain_tex, _ = self.resolved(base)
        used = {f.get("texture", "")[1:] for e in els for f in e.get("faces", {}).values() if f.get("texture", "").startswith("#")}
        for var in sorted(used):
            if var not in chain_tex and var not in tex:
                fb = "front" if var.startswith("front") and ("front" in tex or "front" in chain_tex) else "particle"
                if fb in tex or fb in chain_tex:
                    tex[var] = "#" + fb
                    self.report.notes.append(f"{base}: #{var} unset in 1.12, drawn as #{fb}")
        key = (base, tuple(sorted(tex.items())))
        if key in self.baked:
            return self.baked[key]
        name = f"block/{hint}"
        n = 2
        while name in self.baked_names or f"assets/{ID}/models/{name}.json" in self.names:
            name = f"block/{hint}_{n}"
            n += 1
        self.baked_names.add(name)
        self.files[f"assets/{ID}/models/{name}.json"] = json.dumps({"parent": base, "textures": tex}, indent=1).encode()
        self.baked[key] = f"{ID}:{name}"
        return self.baked[key]

    def free(self, name):
        """A model path for a generated model that no model of the jar (written under its 1.12 path) takes."""
        base, n = name, 2
        while f"assets/{ID}/models/{name}.json" in self.names or name in self.baked_names:
            name = f"{base}_{'state' if n == 2 else n}"
            n += 1
        self.baked_names.add(name)
        return name

    def write(self, name, doc):
        name = self.free(name)
        self.files[f"assets/{ID}/models/{name}.json"] = json.dumps(doc, indent=1).encode()
        return f"{ID}:{name}"


def resolve_refs(textures):
    """Block-state textures that name another of the same map (`"front": "#fronton"`) take its value."""
    out = dict(textures)
    for _ in range(4):
        changed = False
        for k, v in out.items():
            if isinstance(v, str) and v.startswith("#") and v[1:] in out and out[v[1:]] != v:
                out[k] = out[v[1:]]
                changed = True
        if not changed:
            break
    return out


def clean_element(e):
    out = {}
    for k, v in e.items():
        if k.startswith("__"):
            continue
        if k == "faces":
            faces = {}
            for f, face in v.items():
                if f not in SIDES:
                    continue
                face = {fk: fv for fk, fv in face.items() if not fk.startswith("__")}
                if face.get("cullface") == "bottom":
                    face["cullface"] = "down"
                if face.get("cullface") == "top":
                    face["cullface"] = "up"
                if "cullface" in face and face["cullface"] not in SIDES:
                    face.pop("cullface")  # "none": Gson ignored it, 26.3 refuses it
                faces[f] = face
            out["faces"] = faces
        elif k not in ("from", "to", "rotation", "shade", "light_emission"):
            continue  # exporters' extras ("type": "cube", "name"): not part of a 26.3 element
        elif k == "rotation" and isinstance(v, dict):
            r = dict(v)
            # 1.12 allows only -45..45 in 22.5 steps, as 26.3 does; keep it.
            out["rotation"] = r
        else:
            out[k] = v
    return out


# ------------------------------------------------------------------------------------------------------------------
# Forge block states

def is_property_map(v):
    """A `variants` entry that maps property values to variants (Forge v1), as opposed to a variant of its own."""
    if not isinstance(v, dict) or not v:
        return False
    if any(k in FORGE_KEYS for k in v):
        return False
    return all(isinstance(x, (dict, list)) for x in v.values())


def property_values(bs):
    """{property: [values]} of a Forge block state's per-property entries."""
    out = {}
    for k, v in bs.get("variants", {}).items():
        if is_property_map(v) and "=" not in k:
            out[k] = list(v.keys())
    return out


def _merge(state, entry):
    if isinstance(entry, list):
        entry = entry[0] if entry else {}
    if not isinstance(entry, dict):
        return
    if "model" in entry:
        state["model"] = entry["model"]
    if "textures" in entry:
        state["textures"].update(entry["textures"])
    for k in ("x", "y", "uvlock", "transform"):
        if k in entry:
            state[k] = entry[k]
    sub = entry.get("submodel")
    if isinstance(sub, str):
        state["submodel"]["_"] = {"model": sub}
    elif isinstance(sub, dict):
        for name, s in sub.items():
            state["submodel"][name] = s if isinstance(s, dict) else {"model": s}


def forge_parts(bs, assign, flat=()):
    """The models a 1.12 block state draws for `assign` ({property: value}) and the flat variant keys in `flat`
    ("normal", "inventory"): [(model ref, textures, x, y, uvlock)]."""
    assign = {k: str(v) for k, v in assign.items()}
    if bs.get("forge_marker") != 1:
        return vanilla_parts(bs, assign, flat)
    d = bs.get("defaults", {})
    state = {"model": d.get("model"), "textures": dict(d.get("textures", {})), "x": d.get("x", 0), "y": d.get("y", 0),
             "uvlock": d.get("uvlock", False), "transform": d.get("transform"), "submodel": {}}
    _merge(state, {"submodel": d.get("submodel")} if d.get("submodel") else {})
    for key, val in bs.get("variants", {}).items():
        if "=" in key:
            pairs = dict(p.split("=", 1) for p in key.split(","))
            if all(assign.get(k) == v for k, v in pairs.items()):
                _merge(state, val)
        elif is_property_map(val):
            if key in assign:
                entry = val.get(assign[key])
                if entry is None:
                    raise KeyError(f"{key}={assign[key]}")
                _merge(state, entry)
        elif key in flat:
            _merge(state, val)
    x, y = state["x"], state["y"]
    if state["transform"]:
        x, y = transform_rotation(state["transform"], x, y)
    parts = []
    if state["model"]:
        parts.append((state["model"], state["textures"], x, y, state["uvlock"]))
    for name, s in state["submodel"].items():
        if not s.get("model"):
            continue
        tex = dict(state["textures"])
        tex.update(s.get("textures", {}))
        # Forge's MultiModel bakes a submodel with ModelStateComposition(variant state, its own): both turns.
        sx, sy = compose((x, y), (s.get("x", 0), s.get("y", 0)))
        parts.append((s["model"], tex, sx, sy, s.get("uvlock", False)))
    return parts


_X = [[1, 0, 0], [0, 0, 1], [0, -1, 0]]   # x=90: north to down (y' = z, z' = -y about the centre)
_Y = [[0, 0, -1], [0, 1, 0], [1, 0, 0]]   # y=90: north to east
_I = [[1, 0, 0], [0, 1, 0], [0, 0, 1]]


def _pow(m, n):
    out = _I
    for _ in range(n % 4):
        out = _mul(m, out)
    return out


def rot_of(x, y):
    """A vanilla (x, y) turn as a matrix: x first, then y."""
    return _mul(_pow(_Y, (y // 90)), _pow(_X, (x // 90)))


def nearest_xy(m, keep_up=False):
    """The vanilla (x, y) nearest to the turn `m`. Vanilla's x / y reach 16 of the 24 turns; with `keep_up` the
    model's up axis must land where `m` puts it (the dialing device stands on that face; its turn about it is the
    part a block state can't always give)."""
    best = None
    for x in (0, 90, 180, 270):
        for y in (0, 90, 180, 270):
            mm = rot_of(x, y)
            score = sum(abs(mm[i][j] - m[i][j]) * (10 if keep_up and j == 1 else 1) for i in range(3) for j in range(3))
            if best is None or score < best[0]:
                best = (score, x, y)
    return best[1], best[2]


def compose(outer, inner):
    """The turn `inner` then `outer`, as one vanilla (x, y)."""
    if outer[0] % 360 == 0 and inner[0] % 360 == 0:
        return 0, (outer[1] + inner[1]) % 360
    if outer[1] % 360 == 0 and inner[1] % 360 == 0:
        return (outer[0] + inner[0]) % 360, 0
    if inner == (0, 0):
        return outer
    if outer == (0, 0):
        return inner
    return nearest_xy(_mul(rot_of(*outer), rot_of(*inner)))


def vanilla_parts(bs, assign, flat=()):
    variants = bs.get("variants", {})
    for key, val in variants.items():
        if key in flat or (key and "=" in key and all(assign.get(k) == v for k, v in (p.split("=", 1) for p in key.split(",")))):
            val = val[0] if isinstance(val, list) else val
            return [(val["model"], {}, val.get("x", 0), val.get("y", 0), val.get("uvlock", False))]
    return []


def _mul(a, b):
    return [[sum(a[i][k] * b[k][j] for k in range(3)) for j in range(3)] for i in range(3)]


_Z = [[0, -1, 0], [1, 0, 0], [0, 0, 1]]


def transform_rotation(transform, x0=0, y0=0):
    """A Forge TRSR `transform` made of 90-degree rotations as the vanilla (x, y) pair that turns a model the same
    way, or the nearest one (vanilla has no z turn of its own; z180 is x180 then y180)."""
    rots = []
    if isinstance(transform, dict):
        r = transform.get("rotation")
        if isinstance(r, dict):
            r = [r]
        for one in r or []:
            for axis, deg in one.items():
                rots.append((axis, int(deg)))
    if not rots:
        return x0, y0
    # Forge's TRSR rotation: each entry a right-handed turn about its axis (x+90 takes down to north), the list
    # multiplied in order (q1 * q2 ...), so the last entry turns the model first. _X and _Y are vanilla's block
    # state turns (the other way round), _Z is right-handed.
    right = {"x": _pow(_X, 3), "y": _pow(_Y, 3), "z": _Z}
    t = _I
    for axis, deg in rots:
        t = _mul(t, _pow(right[axis], (deg // 90) % 4))
    return nearest_xy(_mul(t, rot_of(x0, y0)), keep_up=True)


# ------------------------------------------------------------------------------------------------------------------
# Rotating elements (for item models made of several turned parts)

FACE_TURN = {"x": {"north": "down", "down": "south", "south": "up", "up": "north", "east": "east", "west": "west"},
             "y": {"north": "east", "east": "south", "south": "west", "west": "north", "up": "up", "down": "down"}}
# The texture turn each face takes when the element turns 90 degrees (worked out from vanilla's default UVs).
UV_TURN = {"x": {"north": 180, "up": 180, "south": 0, "down": 0, "east": 90, "west": 270},
           "y": {"up": 90, "down": 270, "north": 0, "east": 0, "south": 0, "west": 0}}


def auto_uv(face, f, t):
    return {"down": [f[0], 16 - t[2], t[0], 16 - f[2]], "up": [f[0], f[2], t[0], t[2]],
            "north": [16 - t[0], 16 - t[1], 16 - f[0], 16 - f[1]], "south": [f[0], 16 - t[1], t[0], 16 - f[1]],
            "west": [f[2], 16 - t[1], t[2], 16 - f[1]], "east": [16 - t[2], 16 - t[1], 16 - f[2], 16 - f[1]]}[face]


def _turn_point(p, axis):
    x, y, z = p
    if axis == "x":
        return [x, z, 16 - y]
    return [16 - z, y, x]


def turn_element(e, axis):
    """One element turned 90 degrees as a block state's x (north to down) or y (north to east) turns it."""
    f, t = e["from"], e["to"]
    a, b = _turn_point(f, axis), _turn_point(t, axis)
    out = dict(e)
    out["from"] = [round(min(a[i], b[i]), 4) for i in range(3)]
    out["to"] = [round(max(a[i], b[i]), 4) for i in range(3)]
    faces = {}
    for name, face in e.get("faces", {}).items():
        face = dict(face)
        if "uv" not in face:
            face["uv"] = auto_uv(name, f, t)
        new = FACE_TURN[axis][name]
        rot = (face.get("rotation", 0) + UV_TURN[axis][name]) % 360
        if rot:
            face["rotation"] = rot
        else:
            face.pop("rotation", None)
        if "cullface" in face and face["cullface"] in FACE_TURN[axis]:
            face["cullface"] = FACE_TURN[axis][face["cullface"]]
        faces[new] = face
    out["faces"] = faces
    r = e.get("rotation")
    if isinstance(r, dict) and "origin" in r:
        r = dict(r)
        r["origin"] = _turn_point(r["origin"], axis)
        if axis == "y":
            r["axis"], flip = {"x": ("z", False), "z": ("x", True), "y": ("y", False)}[r.get("axis", "y")]
        else:
            r["axis"], flip = {"y": ("z", True), "z": ("y", False), "x": ("x", False)}[r.get("axis", "x")]
        if flip:
            r["angle"] = -r.get("angle", 0)
        out["rotation"] = r
    return out


def transform_matrix(transform, x0=0, y0=0):
    """The exact turn of a Forge TRSR `transform` (after the variant's x / y), as a matrix."""
    right = {"x": _pow(_X, 3), "y": _pow(_Y, 3), "z": _Z}
    t = _I
    r = transform.get("rotation") if isinstance(transform, dict) else None
    for one in ([r] if isinstance(r, dict) else r or []):
        for axis, deg in one.items():
            t = _mul(t, _pow(right[axis], (int(deg) // 90) % 4))
    return _mul(t, rot_of(x0, y0))


def turn_sequence(m):
    """The shortest list of element turns ("x": north to down, "y": north to east, as turn_element does them) that
    makes the turn `m`: any of the 24, where a block state's x / y reach only 16."""
    start = tuple(map(tuple, _I))
    goal = tuple(map(tuple, m))
    seen, todo = {start: []}, [start]
    while todo:
        cur = todo.pop(0)
        if cur == goal:
            return seen[cur]
        for axis, g in (("x", _X), ("y", _Y)):
            nxt = tuple(map(tuple, _mul(g, [list(r) for r in cur])))
            if nxt not in seen:
                seen[nxt] = seen[cur] + [axis]
                todo.append(nxt)
    raise ValueError(m)


def turned_by(elements, seq):
    out = [copy.deepcopy(e) for e in elements]
    for axis in seq:
        out = [turn_element(e, axis) for e in out]
    return out


def turned(elements, x, y):
    out = [copy.deepcopy(e) for e in elements]
    for _ in range((x // 90) % 4):
        out = [turn_element(e, "x") for e in out]
    for _ in range((y // 90) % 4):
        out = [turn_element(e, "y") for e in out]
    return out


def merge_parts(models, parts, display_parent=f"{ID}:block/transform/block", extra_elements=()):
    """Parts [(26.3 model id, x, y)] folded into one model document (its elements turned, its texture variables
    kept apart per part)."""
    elements, textures = [], {}
    particle = None
    for i, (mid, x, y) in enumerate(parts):
        els, tex, _ = models.resolved(mid)
        tex = {k: v for k, v in tex.items()}
        prefix = f"p{i}_"
        def fix(ref):
            if isinstance(ref, str) and ref.startswith("#"):
                return "#" + prefix + ref[1:]
            return ref
        for k, v in tex.items():
            textures[prefix + k] = fix(v)
        if particle is None and "particle" in tex:
            particle = "#" + prefix + "particle"
        for e in turned(els, x, y):
            e = copy.deepcopy(e)
            for face in e.get("faces", {}).values():
                face["texture"] = fix(face.get("texture"))
            elements.append(e)
    if particle:
        textures["particle"] = particle
    doc = {"parent": display_parent, "textures": textures, "elements": elements + list(extra_elements)}
    return doc


def blockstate_json(props, states):
    """A 26.3 block state from [(condition {prop: value}, [part dicts])]: variants when each state draws one model and
    the conditions are all whole states, else multipart."""
    if all(len(p) == 1 for _, p in states) and all(set(c) == set(props) for c, _ in states):
        variants = {}
        for cond, parts in states:
            key = ",".join(f"{k}={cond[k]}" for k in sorted(cond))
            variants[key] = parts[0]
        return {"variants": variants}
    if all(len(p) == 0 for _, p in states):
        return {"variants": {"": {"model": f"{ID}:block/null"}}}
    # multipart: each distinct part with the states that draw it
    by_part = {}
    order = []
    for cond, parts in states:
        for p in parts:
            key = json.dumps(p, sort_keys=True)
            if key not in by_part:
                by_part[key] = []
                order.append(key)
            by_part[key].append(cond)
    total = len(states)
    multipart = []
    for key in order:
        conds = by_part[key]
        part = json.loads(key)
        # An empty condition is every state (a block without properties): the part is always drawn. Equal
        # conditions count once.
        unique = []
        for c in conds:
            if c not in unique:
                unique.append(c)
        conds = unique
        if any(not c for c in conds):
            multipart.append({"apply": part})
            continue
        if len(conds) == total:
            multipart.append({"apply": part})
        elif len(conds) == 1:
            c = conds[0]
            multipart.append({"when": {k: c[k] for k in sorted(c)} if c else {}, "apply": part} if c else {"apply": part})
        else:
            # one property may cover them all
            shared = _single_property(conds, props)
            if shared:
                multipart.append({"when": shared, "apply": part})
            else:
                multipart.append({"when": {"OR": [{k: c[k] for k in sorted(c)} for c in conds]}, "apply": part})
    return {"multipart": multipart}


def _single_property(conds, props):
    """A `when` naming one property with `|` values that matches exactly the conditions given, or None."""
    full = [c for c in conds]
    for prop in props:
        vals = sorted({c.get(prop) for c in full if prop in c})
        if None in vals:
            continue
        # does {prop in vals} select exactly these states?
        others = [p for p in props if p != prop]
        n_other = 1
        for p in others:
            n_other *= len(props[p])
        if len(full) == len(vals) * n_other and all(prop in c for c in full):
            return {prop: "|".join(vals)}
    return None


def part_dict(model_id, x=0, y=0, uvlock=False):
    d = {"model": model_id}
    if x % 360:
        d["x"] = x % 360
    if y % 360:
        d["y"] = y % 360
    if uvlock:
        d["uvlock"] = True
    return d


def digest(obj):
    return hashlib.sha1(json.dumps(obj, sort_keys=True).encode()).hexdigest()[:8]
