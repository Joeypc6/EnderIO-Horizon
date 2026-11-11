"""The W guide's pages from Ender IO's in-game book (assets/enderio/eiobook, Mantle's book format), in every
language the book is written in.

As tconstruct_guide does for Tinkers' books: the guide's first page lists the book's sections, a section's page
lists its pages, and each page of the book is a short page of the guide (its title, its text, its picture: a
structure page becomes a 3D scene, a crafting page its result). Pages without a title of their own continue the
page before them. Translations go to `ae2guide/_<lang>/`, titles to the index's `titles_<lang>`.

The jar's other book (assets/enderio/book) is a page for Tinkers' Construct's modifier book (the TiC integration,
skipped with it).

Usage (from make_content.py): enderio_guide.write(files, jar, langs)
"""

import json
import re

import guide_index

ID = "enderio"
PREFIX = f"assets/{ID}/eiobook/"


def read_json(z, path):
    try:
        return json.loads(z.read(path).decode("utf-8-sig"))
    except (KeyError, ValueError):
        return None


def read_lang(z, path):
    out = {}
    try:
        text = z.read(path).decode("utf-8-sig")
    except KeyError:
        return out
    for line in text.splitlines():
        s = line.strip()
        if "=" in s and not s.startswith(("#", "//")):
            k, v = s.split("=", 1)
            out[k.strip()] = v.strip()
    return out


def text_of(segments):
    """Mantle's text segments as short Markdown: bold and italic kept, `paragraph` a new paragraph."""
    if isinstance(segments, str):
        return segments
    out = ""
    for s in segments or []:
        if isinstance(s, str):
            out += s
            continue
        t = re.sub(r"\$\([^)]*\)", "", s.get("text", ""))
        t = t.replace("---", "—")
        if s.get("bold") and t.strip():
            t = f"**{t.strip()}** "
        if s.get("italic") and t.strip():
            t = f"*{t.strip()}* "
        if s.get("paragraph") and out:
            out = out.rstrip(" ") + "\n\n"
        out += t
        if s.get("linebreak"):
            out += "\n"
    return re.sub(r"\n{3,}", "\n\n", out).strip()


def page_body(files, z, lang, entry, page, scene_name):
    """(Markdown body, item ids) of one book page."""
    lines, items = [], []
    if entry.get("type") == "structure":
        data = page
        if isinstance(page, dict) and "data" in page and "structure" not in page:
            data = read_json(z, f"{PREFIX}{lang}/{page['data']}") or read_json(z, f"{PREFIX}en_us/{page['data']}") or {}
        blocks = []
        for part in data.get("structure", []):
            (x0, y0, z0), (x1, y1, z1) = part["pos"], part.get("endPos", part["pos"])
            for x in range(min(x0, x1), max(x0, x1) + 1):
                for y in range(min(y0, y1), max(y0, y1) + 1):
                    for zz in range(min(z0, z1), max(z0, z1) + 1):
                        blocks.append((x, y, zz, part["block"]))
        if blocks:
            lines.append(guide_index.scene(files, ID, scene_name, blocks))
    if entry.get("type") == "crafting" and isinstance(page, dict):
        res = page.get("result", {})
        if isinstance(res, dict) and res.get("id"):
            items.append(res["id"])
            lines.append(f'<ItemImage id="{res["id"]}" scale="2" />')
            lines.append("")
        text = text_of(page.get("description"))
        if text:
            lines.append(text)
    text = text_of(page.get("text")) if isinstance(page, dict) else ""
    if text:
        lines.append(text)
    return "\n".join(lines).strip() + "\n", items


def front(title, items):
    head = f"---\nnavigation:\n  title: {title}\nitem_ids:"
    head += "".join(f"\n- {i}" for i in items) if items else " []"
    return head + "\n---\n\n"


def write(files, z, langs_json):
    names = z.namelist()
    langs = sorted({n[len(PREFIX):].split("/")[0] for n in names if n.startswith(PREFIX)
                    and re.fullmatch(r"[a-z]{2}_[a-z]{2}", n[len(PREFIX):].split("/")[0])})
    import eio_guide_systems
    # The book is English only; the Systems pages are also in the languages Ender IO itself ships.
    langs = sorted(set(langs) | set(eio_guide_systems.LANGS))
    appearance = read_json(z, f"{PREFIX}appearance.json") or {}
    index = read_json(z, f"{PREFIX}index.json") or []
    book_lang = {lang: read_lang(z, f"{PREFIX}{lang}/language.lang") for lang in langs}
    pages, translated, titles, order, parents = {}, {}, {}, [], {}

    def add(name, title_by, body_by, items, parent):
        order.append(name)
        if parent:
            parents[name] = parent
        en = title_by.get("en_us") or name
        # Only Ender IO's own items open these pages with W (the book's crafting page shows a vanilla stand-in).
        items = [i for i in items if i.startswith(f"{ID}:")]
        pages[name] = front(en, items) + f"# {en}\n\n" + body_by.get("en_us", "")
        for lang in langs:
            if lang == "en_us":
                continue
            t = title_by.get(lang)
            if t and t != en:
                titles.setdefault(lang, {})[name] = t
            b = body_by.get(lang)
            if b:
                translated.setdefault(lang, {})[name] = f"# {t or en}\n\n{b}"

    book_title = appearance.get("title", "Ender IO")
    sections = []
    for sec in index:
        if not isinstance(sec, dict) or not sec.get("data"):
            continue
        listing = read_json(z, f"{PREFIX}{sec['data']}")
        if isinstance(listing, list):
            sections.append((sec, listing))

    def sec_title(lang, sec):
        return book_lang.get(lang, {}).get(sec["name"]) or book_lang["en_us"].get(sec["name"]) or sec["name"].replace("_", " ").title()

    systems = eio_guide_systems.SECTION
    root_body = {lang: f"- [{systems.get(lang, systems['en_us'])}](systems/index.md)\n"
                 + "\n".join(f"- [{sec_title(lang, s)}]({s['name']}/index.md)" for s, _ in sections) + "\n" for lang in langs}
    subtitle = appearance.get("subtitle")
    if subtitle:
        root_body = {lang: f"*{subtitle}*\n\n" + b for lang, b in root_body.items()}
    add("index.md", {lang: book_title for lang in langs}, root_body, [f"{ID}:enderface"], None)
    for sec, listing in sections:
        sec_page = f"{sec['name']}/index.md"
        leaves = []   # (name, {lang: title}, {lang: body}, items)
        for n, entry in enumerate(listing):
            if not isinstance(entry, dict) or entry.get("type") == "blank" or not entry.get("data"):
                continue
            ename = entry.get("name") or entry["data"].rsplit("/", 1)[-1].removesuffix(".json")
            t_by, b_by, items = {}, {}, []
            for lang in langs:
                page = read_json(z, f"{PREFIX}{lang}/{entry['data']}")
                if page is None:
                    continue
                body, its = page_body(files, z, lang, entry, page, f"{sec['name']}_{ename}")
                b_by[lang] = body
                if page.get("title"):
                    t_by[lang] = page["title"]
                if lang == "en_us":
                    items = its
            if not b_by:
                continue
            if not t_by and leaves:
                # A page without a title goes on with the one before it.
                prev = leaves[-1]
                for lang, b in b_by.items():
                    prev[2][lang] = (prev[2].get(lang, "") + "\n" + b) if prev[2].get(lang) else b
                prev[3].extend(i for i in items if i not in prev[3])
                continue
            leaves.append((ename, t_by, b_by, items))
        sec_body = {lang: "\n".join(f"- [{l[1].get(lang) or l[1].get('en_us') or l[0]}]({l[0]}.md)" for l in leaves) + "\n"
                    for lang in langs}
        icon = sec.get("icon", {}).get("item", {})
        sec_items = []
        if isinstance(icon, dict) and icon.get("id"):
            iid = icon["id"]
            if iid == "enderio:item_material" and icon.get("damage", 0) == 0:
                iid = f"{ID}:simple_machine_chassis"
            sec_items.append(iid)
        add(sec_page, {lang: sec_title(lang, sec) for lang in langs}, sec_body, sec_items, "index.md")
        for ename, t_by, b_by, items in leaves:
            add(f"{sec['name']}/{ename}.md", t_by or {"en_us": ename}, b_by, items, sec_page)
    eio_guide_systems.add_pages(add, files, guide_index)
    idx = guide_index.write(files, ID, pages)
    idx["parents"] = parents
    idx["positions"] = {name: str(at * 10) for at, name in enumerate(order)}
    for lang, by_page in titles.items():
        idx[f"titles_{lang}"] = by_page
    files[f"data/{ID}/ae2guide/index.json"] = json.dumps(idx, indent=1).encode()
    for lang, by_page in translated.items():
        for name, body in by_page.items():
            files[f"data/{ID}/ae2guide/_{lang}/{name}"] = body.encode()
    return len(pages), sorted(translated)
