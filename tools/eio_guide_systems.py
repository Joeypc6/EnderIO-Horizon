"""The W guide's "Systems" section: short pages on how Ender IO's parts work together, each with a 3D scene of the
blocks connected, in English and in the languages Ender IO itself is translated into (de_de, ru_ru, ja_jp).

Ender IO 5.3's own book is a stub ("the most incomplete guide to Ender IO"), so these pages carry what a player needs:
power and where it goes, machines and their upgrades, conduits, travel. Numbers are Ender IO's (spec-machines.md).
"""

ID = "enderio"
LANGS = ["en_us", "de_de", "ru_ru", "ja_jp"]
SECTION = {"en_us": "Systems", "de_de": "Systeme", "ru_ru": "Системы", "ja_jp": "システム"}

PAGES = [
    {
        "name": "power",
        "title": {"en_us": "Power", "de_de": "Energie", "ru_ru": "Энергия", "ja_jp": "電力"},
        "items": ["stirling_generator", "combustion_generator", "zombie_generator", "solar_panel"],
        "body": {
            "en_us": "Generators make energy (µI, the same as RF). A **Stirling Generator** burns furnace fuel for 60 µI/t; "
                     "a **Combustion Generator** burns fuel fluids and needs a coolant; **Solar Panels** work in daylight.\n\n"
                     "Energy conduits carry it to **Capacitor Banks** and machines:",
            "de_de": "Generatoren erzeugen Energie (µI, wie RF). Ein **Stirling-Generator** verbrennt Ofenbrennstoff für 60 µI/t; "
                     "ein **Verbrennungsgenerator** verbrennt flüssige Brennstoffe und braucht Kühlmittel; **Solarpaneele** "
                     "arbeiten bei Tageslicht.\n\nEnergieleitungen bringen sie zu **Kondensatorbänken** und Maschinen:",
            "ru_ru": "Генераторы производят энергию (µI, то же, что RF). **Генератор Стирлинга** сжигает печное топливо, "
                     "60 µI/т; **генератор внутреннего сгорания** сжигает жидкое топливо и требует охладитель; "
                     "**солнечные панели** работают днём.\n\nЭнергопроводы несут её к **банкам конденсаторов** и машинам:",
            "ja_jp": "発電機がエネルギー（µI、RFと同じ）を作ります。**スターリング発電機**はかまどの燃料を燃やして60 µI/t、"
                     "**燃焼発電機**は液体燃料を燃やし冷却材が必要、**ソーラーパネル**は昼間に動きます。\n\n"
                     "エネルギーコンジットが**キャパシタバンク**と機械へ運びます：",
        },
        "scene": [(0, 0, 0, "stirling_generator"), (1, 0, 0, "conduit_bundle"), (2, 0, 0, "basic_capacitor_bank"),
                  (3, 0, 0, "conduit_bundle"), (4, 0, 0, "alloy_smelter")],
    },
    {
        "name": "capacitor_bank",
        "title": {"en_us": "Capacitor Banks", "de_de": "Kondensatorbänke", "ru_ru": "Банки конденсаторов",
                  "ja_jp": "キャパシタバンク"},
        "items": ["basic_capacitor_bank", "capacitor_bank", "vibrant_capacitor_bank", "creative_capacitor_bank"],
        "body": {
            "en_us": "Banks of one tier placed side by side join into one: they share their energy and their gauge.\n\n"
                     "- Basic: 1,000,000 µI, 1,000 µI/t in and out\n- Normal: 5,000,000 µI, 5,000 µI/t\n"
                     "- Vibrant: 25,000,000 µI, 25,000 µI/t\n\nEach face can be set to take energy in, give it out, or neither.",
            "de_de": "Bänke einer Stufe nebeneinander verbinden sich: Sie teilen Energie und Anzeige.\n\n"
                     "- Einfach: 1.000.000 µI, 1.000 µI/t ein und aus\n- Normal: 5.000.000 µI, 5.000 µI/t\n"
                     "- Vibrierend: 25.000.000 µI, 25.000 µI/t\n\nJede Seite nimmt Energie auf, gibt sie ab, oder keins von beiden.",
            "ru_ru": "Банки одного уровня, поставленные рядом, объединяются: общая энергия и общий индикатор.\n\n"
                     "- Простой: 1 000 000 µI, 1 000 µI/т на вход и выход\n- Обычный: 5 000 000 µI, 5 000 µI/т\n"
                     "- Вибрирующий: 25 000 000 µI, 25 000 µI/т\n\nКаждая сторона принимает энергию, отдаёт её или выключена.",
            "ja_jp": "同じ段階のバンクを並べると一つにまとまり、エネルギーとゲージを共有します。\n\n"
                     "- 基本：1,000,000 µI、入出力 1,000 µI/t\n- 通常：5,000,000 µI、5,000 µI/t\n"
                     "- ヴァイブラント：25,000,000 µI、25,000 µI/t\n\n各面は入力・出力・無効に設定できます。",
        },
        "scene": [(x, y, z, "capacitor_bank") for x in range(2) for y in range(2) for z in range(2)],
    },
    {
        "name": "capacitors",
        "title": {"en_us": "Capacitors", "de_de": "Kondensatoren", "ru_ru": "Конденсаторы", "ja_jp": "キャパシタ"},
        "items": ["basic_capacitor", "double_layer_capacitor", "octadic_capacitor"],
        "body": {
            "en_us": "A machine's capacitor slot sets how fast it works and how much energy it holds and takes in. "
                     "A **Double-Layer Capacitor** runs an alloy smelter at 90 µI/t instead of 30, an **Octadic** at 150.",
            "de_de": "Der Kondensatorplatz einer Maschine bestimmt, wie schnell sie arbeitet und wie viel Energie sie speichert. "
                     "Ein **Doppelschicht-Kondensator** lässt einen Legierungsofen mit 90 statt 30 µI/t laufen, ein **Oktadischer** mit 150.",
            "ru_ru": "Слот конденсатора задаёт скорость машины и сколько энергии она хранит и принимает. "
                     "**Двухслойный конденсатор** разгоняет плавильню сплавов до 90 µI/т вместо 30, **октадический** — до 150.",
            "ja_jp": "機械のキャパシタスロットで速さと蓄えられるエネルギー量が決まります。"
                     "**二重層キャパシタ**で合金炉は30から90 µI/tに、**オクタディック**では150になります。",
        },
        "scene": [(0, 0, 0, "alloy_smelter"), (2, 0, 0, "sag_mill")],
    },
    {
        "name": "alloy_smelter",
        "title": {"en_us": "Alloy Smelter", "de_de": "Legierungsofen", "ru_ru": "Плавильня сплавов", "ja_jp": "合金炉"},
        "items": ["alloy_smelter", "simple_alloy_smelter"],
        "body": {
            "en_us": "Melts up to three inputs into one alloy: **Electrical Steel**, **Energetic Alloy**, **Vibrant Alloy**, "
                     "**Dark Steel** and the rest. It also smelts like a furnace. Feed it energy from any side.",
            "de_de": "Schmilzt bis zu drei Zutaten zu einer Legierung: **Elektrostahl**, **Energetische Legierung**, "
                     "**Vibrierende Legierung**, **Dunkelstahl** und mehr. Schmilzt auch wie ein Ofen. Energie von jeder Seite.",
            "ru_ru": "Сплавляет до трёх ингредиентов в один сплав: **электротехническая сталь**, **энергетический сплав**, "
                     "**вибрирующий сплав**, **тёмная сталь** и другие. Плавит и как печь. Энергия — с любой стороны.",
            "ja_jp": "最大3つの材料を合金にします：**エレクトリカルスチール**、**エナジェティック合金**、**ヴァイブラント合金**、"
                     "**ダークスチール**など。かまどとしても使えます。エネルギーはどの面からでも。",
        },
        "scene": [(0, 0, 0, "alloy_smelter"), (1, 0, 0, "conduit_bundle"), (2, 0, 0, "stirling_generator")],
    },
    {
        "name": "sag_mill",
        "title": {"en_us": "SAG Mill", "de_de": "SAG-Mühle", "ru_ru": "Мельница SAG", "ja_jp": "SAGミル"},
        "items": ["sag_mill", "simple_sag_mill", "dark_steel_grinding_ball"],
        "body": {
            "en_us": "Grinds ores into two dusts, with a chance of extras. A **Grinding Ball** in its side slot multiplies "
                     "the output and the chances, and wears down as the mill runs.",
            "de_de": "Mahlt Erze zu zwei Stäuben, mit Chance auf mehr. Eine **Mahlkugel** im Seitenplatz vervielfacht "
                     "Ertrag und Chancen und nutzt sich beim Mahlen ab.",
            "ru_ru": "Перемалывает руду в две пыли, иногда с бонусом. **Мелющий шар** в боковом слоте умножает выход "
                     "и шансы и изнашивается при работе.",
            "ja_jp": "鉱石を2つの粉に砕き、追加の可能性もあります。横のスロットの**粉砕ボール**が出力と確率を増やし、"
                     "使うほど減ります。",
        },
        "scene": [(0, 0, 0, "sag_mill"), (1, 0, 0, "conduit_bundle"), (2, 0, 0, "basic_capacitor_bank")],
    },
    {
        "name": "conduits",
        "title": {"en_us": "Conduits", "de_de": "Leitungen", "ru_ru": "Проводники", "ja_jp": "コンジット"},
        "items": ["item_conduit", "fluid_conduit", "energy_conduit", "enhanced_energy_conduit", "ender_energy_conduit",
                  "redstone_conduit", "conduit_bundle"],
        "body": {
            "en_us": "One block holds conduits of different kinds side by side: items, fluids, energy and redstone. "
                     "A **Yeta Wrench** changes a side (in, out, both, off). Energy conduits carry 640, 5,120 or 20,480 µI/t, "
                     "and different tiers don't join.",
            "de_de": "Ein Block hält Leitungen verschiedener Art nebeneinander: Gegenstände, Flüssigkeiten, Energie, Redstone. "
                     "Ein **Yeta-Schraubenschlüssel** stellt eine Seite um (ein, aus, beides, keins). Energieleitungen tragen "
                     "640, 5.120 oder 20.480 µI/t; verschiedene Stufen verbinden sich nicht.",
            "ru_ru": "В одном блоке помещаются проводники разных видов: предметы, жидкости, энергия, редстоун. "
                     "**Ключ Йета** меняет сторону (вход, выход, оба, выкл.). Энергопроводы несут 640, 5 120 или 20 480 µI/т; "
                     "разные уровни не соединяются.",
            "ja_jp": "1つのブロックに種類の違うコンジットを並べられます：アイテム、液体、エネルギー、レッドストーン。"
                     "**イェータレンチ**で面を切り替えます（入力・出力・両方・無効）。エネルギーコンジットは640、5,120、20,480 µI/t、"
                     "段階が違うとつながりません。",
        },
        "scene": [(0, 0, 0, "minecraft:chest"), (1, 0, 0, "conduit_bundle"), (2, 0, 0, "conduit_bundle"),
                  (3, 0, 0, "alloy_smelter")],
    },
    {
        "name": "travel",
        "title": {"en_us": "Travel Anchors", "de_de": "Reiseanker", "ru_ru": "Якоря перемещения", "ja_jp": "トラベルアンカー"},
        "items": ["travel_anchor", "staff_of_travelling"],
        "body": {
            "en_us": "Stand on a **Travel Anchor**, look at another and sneak-jump to it. With the **Staff of Travelling** "
                     "you jump to any anchor you look at, or a short way ahead.",
            "de_de": "Auf einem **Reiseanker** stehen, einen anderen ansehen und schleichend springen. Mit dem "
                     "**Reisestab** springst du zu jedem angesehenen Anker oder ein Stück nach vorn.",
            "ru_ru": "Встаньте на **якорь перемещения**, посмотрите на другой и прыгните крадучись. С **посохом "
                     "путешествий** можно перенестись к любому видимому якорю или немного вперёд.",
            "ja_jp": "**トラベルアンカー**に乗り、別のアンカーを見てスニークジャンプします。**トラベルスタッフ**なら"
                     "見ているアンカーへ、または少し前へ移動できます。",
        },
        "scene": [(0, 0, 0, "travel_anchor"), (4, 0, 0, "travel_anchor")],
    },
    {
        "name": "glass",
        "title": {"en_us": "Fused Glass", "de_de": "Quarzglas", "ru_ru": "Плавленое стекло", "ja_jp": "溶融ガラス"},
        "items": ["fused_quartz", "fused_glass", "enlightened_fused_quartz"],
        "body": {
            "en_us": "**Fused Quartz** resists explosions like obsidian; **Enlightened** glass gives light. Glass of one kind "
                     "joins into a clear wall without seams.",
            "de_de": "**Quarzglas** widersteht Explosionen wie Obsidian; **erleuchtetes** Glas spendet Licht. Glas einer Art "
                     "verbindet sich zu einer nahtlosen Wand.",
            "ru_ru": "**Плавленый кварц** выдерживает взрывы, как обсидиан; **светящееся** стекло даёт свет. Стекло одного "
                     "вида сливается в стену без швов.",
            "ja_jp": "**溶融石英**は黒曜石のように爆発に耐え、**輝く**ガラスは光ります。同じ種類のガラスは継ぎ目なくつながります。",
        },
        "scene": [(x, y, 0, "fused_quartz") for x in range(3) for y in range(2)] + [(3, 0, 0, "enlightened_fused_quartz")],
    },
]


def add_pages(add, files, guide_index, parent="index.md"):
    """Adds the section and its pages through enderio_guide's `add(name, title_by, body_by, items, parent)`."""
    rows = {lang: "\n".join(f"- [{p['title'][lang]}]({p['name']}.md)" for p in PAGES) + "\n" for lang in LANGS}
    add("systems/index.md", SECTION, rows, [], parent)  # no item of its own: W on a machine opens its page
    for p in PAGES:
        blocks = [(x, y, z, b if ":" in b else f"{ID}:{b}") for x, y, z, b in p["scene"]]
        scene = guide_index.scene(files, ID, f"systems_{p['name']}", blocks)
        body = {lang: p["body"][lang] + "\n\n" + scene for lang in LANGS}
        add(f"systems/{p['name']}.md", p["title"], body, [f"{ID}:{i}" for i in p["items"]], "systems/index.md")
    return SECTION
