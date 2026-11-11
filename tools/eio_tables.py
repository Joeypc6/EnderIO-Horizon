"""Static facts the Ender IO converter needs that the jar does not state in data form.

Every table here was read off the 1.12.2 sources (EnderIO 5.3.72, EnderCore, Forge 1.12.2's vanilla ore dictionary) or
off the 26.3 client's data; the comments say where.
"""

COLORS = ["white", "orange", "magenta", "light_blue", "yellow", "lime", "pink", "gray", "light_gray", "cyan", "purple",
          "blue", "brown", "green", "red", "black"]
# EnumDyeColor's MapColor (BlockFusedQuartz.getBlockTint with candyColors=false, the default): the glass tint.
MAP_COLOR = {"white": 0xFFFFFF, "orange": 0xD87F33, "magenta": 0xB24CD8, "light_blue": 0x6699D8, "yellow": 0xE5E533,
             "lime": 0x7FCC19, "pink": 0xF27FA5, "gray": 0x4C4C4C, "light_gray": 0x999999, "cyan": 0x4C7F99,
             "purple": 0x7F3FB2, "blue": 0x334CB2, "brown": 0x664C33, "green": 0x667F33, "red": 0x993333,
             "black": 0x191919}
# 1.12 dye colour names (EnumDyeColor.getName: "silver" is light gray) and Forge's oredict colour names.
DYE_112 = {"light_gray": "silver"}
ORE_COLOR = {"white": "White", "orange": "Orange", "magenta": "Magenta", "light_blue": "LightBlue", "yellow": "Yellow",
             "lime": "Lime", "pink": "Pink", "gray": "Gray", "light_gray": "LightGray", "cyan": "Cyan",
             "purple": "Purple", "blue": "Blue", "brown": "Brown", "green": "Green", "red": "Red", "black": "Black"}

# SoundType constants of 1.12 (field_185848_a...) as a 26.3 block that has that sound.
SOUND = {"wood": "minecraft:oak_planks", "ground": "minecraft:gravel", "plant": "minecraft:short_grass",
         "stone": "minecraft:stone", "metal": "minecraft:iron_block", "glass": "minecraft:glass",
         "cloth": "minecraft:white_wool", "sand": "minecraft:sand", "snow": "minecraft:snow_block",
         "ladder": "minecraft:ladder", "anvil": "minecraft:anvil", "slime": "minecraft:slime_block"}

ALLOY_HARDNESS = {  # Alloy / AlloyEndergy: (hardness); BlockAlloy.getExplosionResistance = hardness * 2
    "electrical_steel": 6.0, "energetic_alloy": 7.0, "vibrant_alloy": 4.0, "redstone_alloy": 1.0,
    "conductive_iron": 5.2, "pulsating_iron": 7.0, "dark_steel": 10.0, "soularium": 10.0, "end_steel": 10.0,
    "construction_alloy": 1.0, "crude_steel": 2.0, "crystalline_alloy": 6.0, "melodic_alloy": 5.0,
    "stellar_alloy": 10.0, "crystalline_pink_slime": 7.0, "energetic_silver": 7.0, "vivid_alloy": 4.0,
}
ALLOY_COLOR = {  # Alloy colour (the 1.12 `color` constructor argument), for the creative tab order only.
}

# 1.12 explosion resistance into 26.3's: setResistance(r) stores 3r and an explosion reads it / 5 (0.6 r); a block
# that only set its hardness h keeps 5h / 5 = h.
def res(hardness, set_resistance=None):
    return round(set_resistance * 0.6, 3) if set_resistance is not None else hardness


# (hardness, explosion resistance in 26.3 terms, sound, requires a tool, light) per 1.12 block, from each class's
# constructor (BlockEnder's default is 0.5 / metal / no tool: a custom Material that needs none).
DEFAULT_STRENGTH = (0.5, 0.5, "metal", False, 0)
MACHINE_STRENGTH = (2.0, 2.0, "metal", False, 0)  # AbstractMachineBlock: setHardness(2), metal
STRENGTH = {
    "block_infinity": (50.0, res(50, 2000), "ground", False, 0),
    "block_infinity_fog": (0.0, 0.0, "cloth", False, 0),
    "block_holy_fog": (0.0, 0.0, "cloth", False, 15),
    "block_holier_fog": (0.0, 0.0, "cloth", False, 15),
    "block_dark_steel_anvil": (5.0, 1200.0, "anvil", True, 0),
    "block_dark_paper_anvil": (0.1, 0.1, "plant", False, 0),
    "block_broken_anvil": (5.0, 1200.0, "anvil", True, 0),
    "block_dark_steel_ladder": (0.4, 0.4, "metal", True, 0),
    "block_dark_iron_bars": (5.0, res(5, 2000), "metal", True, 0),
    "block_end_iron_bars": (5.0, res(5, 1000), "metal", True, 0),
    "block_dark_steel_door": (5.0, res(5, 2000), "metal", True, 0),
    "block_dark_steel_trapdoor": (5.0, res(5, 2000), "metal", True, 0),
    "block_reinforced_obsidian": (50.0, res(50, 2000), "stone", True, 0),
    "block_industrial_insulation": (0.5, 0.5, "cloth", False, 0),
    "block_confusion_charge": (0.0, 0.0, "plant", False, 0),
    "block_concussion_charge": (0.0, 0.0, "plant", False, 0),
    "block_ender_charge": (0.0, 0.0, "plant", False, 0),
    "block_exit_rail": (0.7, 0.7, "metal", False, 0),
    "block_decoration1": (0.5, 0.5, "metal", True, 0),
    "block_decoration2": (0.5, 0.5, "metal", True, 0),
    "block_decoration3": (0.5, 0.5, "metal", True, 0),
    "block_enderman_skull": (0.5, 0.5, "metal", False, 0),
    "block_death_pouch": (0.5, res(0.5, 6000000), "metal", False, 0),
    "block_cold_fire": (0.0, 0.0, "cloth", False, 15),
    "block_light_node": (0.0, 0.0, "glass", False, 15),  # BlockLightNode: lit while its electric light is on
    "block_conduit_bundle": (1.5, res(1.5, 10), "metal", False, 0),
    "block_cap_bank": (2.0, 2.0, "metal", False, 0),
    "block_creative_spawner": (2.0, res(2, 6000000), "metal", False, 0),
    "block_tank": (2.0, 2.0, "glass", False, 0),
    "block_killer_joe": (2.0, 2.0, "glass", False, 0),
    "block_reservoir": (0.5, 0.5, "glass", False, 0),
    "block_omni_reservoir": (0.5, 0.5, "glass", False, 0),
    # painted blocks (each BlockPainted*'s constructor)
    "block_painted_carpet": (0.1, 0.1, "cloth", False, 0),
    "block_painted_fence": (2.0, res(2, 5), "wood", False, 0),
    "block_painted_stone_fence": (2.0, res(2, 10), "stone", True, 0),
    "block_painted_fence_gate": (2.0, res(2, 5), "wood", False, 0),
    "block_painted_wall": (2.0, res(2, 10), "stone", True, 0),
    "block_painted_stair": (2.0, res(2, 5), "wood", False, 0),
    "block_painted_stone_stair": (2.0, res(2, 10), "stone", True, 0),
    "block_painted_slab": (2.0, res(2, 5), "wood", False, 0),
    "block_painted_stone_slab": (2.0, res(2, 10), "stone", True, 0),
    "block_painted_glowstone": (0.3, 0.3, "glass", False, 15),
    "block_painted_glowstone_solid": (0.3, 0.3, "glass", False, 15),
    "block_painted_redstone": (5.0, res(5, 10), "metal", True, 0),
    "block_painted_redstone_solid": (5.0, res(5, 10), "metal", True, 0),
    "block_painted_stone": (2.0, res(2, 10), "stone", True, 0),
    "block_painted_sand": (0.5, 0.5, "sand", False, 0),
    "block_painted_wooden_trapdoor": (3.0, 3.0, "wood", False, 0),
    "block_painted_iron_trapdoor": (5.0, 5.0, "metal", True, 0),
    "block_painted_dark_steel_trapdoor": (5.0, res(5, 2000), "metal", True, 0),
    "block_painted_wooden_door": (3.0, 3.0, "wood", False, 0),
    "block_painted_iron_door": (5.0, 5.0, "metal", True, 0),
    "block_painted_dark_steel_door": (5.0, res(5, 2000), "metal", True, 0),
    "block_painted_workbench": (2.5, 2.5, "wood", False, 0),
    "block_painted_reinforced_obsidian": (50.0, res(50, 2000), "stone", True, 0),
    "block_painted_reinforced_obsidian_nonsolid": (50.0, res(50, 2000), "stone", True, 0),
    "block_painted_packed_ice": (0.5, 0.5, "glass", False, 0),
    "block_painted_packed_ice_solid": (0.5, 0.5, "glass", False, 0),
    "block_painted_pressure_plate": (0.5, 0.5, "wood", False, 0),
    "block_detector_block": (0.5, 0.5, "metal", False, 0),
    "block_detector_block_silent": (0.5, 0.5, "metal", False, 0),
}
for _n in ("block_self_resetting_lever5", "block_self_resetting_lever5i", "block_self_resetting_lever10",
           "block_self_resetting_lever10i", "block_self_resetting_lever30", "block_self_resetting_lever30i",
           "block_self_resetting_lever60", "block_self_resetting_lever60i", "block_self_resetting_lever300",
           "block_self_resetting_lever300i"):
    STRENGTH[_n] = (0.5, 0.5, "wood", False, 0)

# The blocks of the jar that extend AbstractMachineBlock and draw from a `render` property, with the part of
# `block_machine_base` MachineRenderMapper puts under them (RenderMappers / each Block*.create*): None = FRONT_MAPPER.
# `mode` says what the mapper draws in the world: "rot" the front (front_on when active) turned to the facing;
# "fixed" render=front (front_on) as is; "layers" front and front_on both (KillerJoeRenderMapper's two layers); "farm"
# front, plus front_on when active; "soulbinder" the frame, plus the core when idle; "xp" front(_on) turned;
# "obelisk" ObeliskBakery's quads (drawn here as a model of its own).
MACHINES = {
    "block_simple_furnace": ("simple_body", "rot"),
    "block_simple_alloy_smelter": ("simple_body", "rot"),
    "block_alloy_smelter": ("body", "rot"),
    "block_enhanced_alloy_smelter": ("enhanced_body", "rot"),
    "block_buffer": (None, "rot"),
    "block_enchanter": (None, "rot"),
    "block_farm_station": (None, "farm"),
    "block_combustion_generator": (None, "rot"),
    "block_enhanced_combustion_generator": (None, "rot"),
    "block_simple_stirling_generator": ("simple_body", "rot"),
    "block_stirling_generator": ("body", "rot"),
    "block_zombie_generator": (None, "layers"),
    "block_franken_zombie_generator": (None, "layers"),
    "block_ender_generator": (None, "layers"),
    "block_lava_generator": ("body", "rot"),
    "block_killer_joe": (None, "layers"),
    "block_attractor_obelisk": (None, "obelisk"),
    "block_aversion_obelisk": (None, "obelisk"),
    "block_inhibitor_obelisk": (None, "obelisk"),
    "block_relocator_obelisk": (None, "obelisk"),
    "block_weather_obelisk": (None, "obelisk"),
    "block_experience_obelisk": (None, "obelisk"),
    "block_painter": ("body", "rot"),
    "block_simple_sag_mill": ("simple_body", "rot"),
    "block_sag_mill": ("body", "rot"),
    "block_enhanced_sag_mill": ("enhanced_body", "rot"),
    "block_slice_and_splice": ("soul", "rot"),
    "block_soul_binder": ("soul_frame", "soulbinder"),
    "block_powered_spawner": ("soul", "rot"),
    "block_vat": (None, "rot"),
    "block_enhanced_vat": (None, "rot"),
    "block_simple_wired_charger": ("simple_body", "rot"),
    "block_wired_charger": ("body", "rot"),
    "block_enhanced_wired_charger": ("enhanced_body", "rot"),
    "block_wireless_charger": (None, "wireless"),
    "block_transceiver": (None, "rot"),
    "block_vacuum_chest": (None, "fixed"),
    "block_xp_vacuum": (None, "xp"),
    "block_niard": (None, "rot"),
    "block_travel_anchor": (None, "fixed"),
    "block_tele_pad": (None, "telepad"),
    "block_impulse_hopper": (None, "fixed"),
    "block_simple_crafter": ("simple_body", "rot"),
    "block_crafter": (None, "rot"),
    "block_creative_spawner": (None, "rot"),
    "block_inventory_panel": (None, "invpanel"),
    "block_inventory_panel_sensor": ("body", "rot"),
    "block_inventory_chest_tiny": ("simple_body", "rot"),
    "block_inventory_chest_small": ("simple_body", "rot"),
    "block_inventory_chest_medium": ("simple_body", "rot"),
    "block_inventory_chest_big": ("body", "rot"),
    "block_inventory_chest_large": ("body", "rot"),
    "block_inventory_chest_huge": ("body", "rot"),
    "block_inventory_chest_enormous": ("enhanced_body", "rot"),
    "block_inventory_chest_warehouse": ("enhanced_body", "rot"),
    "block_inventory_chest_warehouse13": ("enhanced_body", "rot"),
    "block_power_monitor": ("body", "rot"),
    "block_advanced_power_monitor": ("body", "rot"),
    "block_tank": (None, "tank"),
}
# Machines whose 1.12 tile entity holds no facing worth a state (the mapper draws them the same every way).
NO_FACING = {"block_tank", "block_tele_pad", "block_travel_anchor", "block_vacuum_chest", "block_impulse_hopper",
             "block_wireless_charger", "block_killer_joe", "block_zombie_generator", "block_franken_zombie_generator",
             "block_ender_generator"}
# The obelisks' side textures (ObeliskRenderManager and each obelisk's own registration).
OBELISK_SIDE = {n: "block_attractor_side" for n in (  # ObeliskRenderManager: one texture set for all six
    "block_attractor_obelisk", "block_aversion_obelisk", "block_inhibitor_obelisk", "block_relocator_obelisk",
    "block_weather_obelisk", "block_experience_obelisk")}
# Code-built model kinds a native renderer must draw (spec 2, "Code-generated models"), by 1.12 block.
TESR = {"block_cap_bank": "cap bank fill/IO display", "block_conduit_bundle": "conduits, facades",
        "block_enchanter": "book", "block_enderman_skull": "skull rotation (22.5 degree steps)",
        "block_farm_station": "tools", "block_gauge": "energy level", "block_death_pouch": "urn model",
        "block_killer_joe": "zombie head, weapon", "block_niard": "fluid", "block_power_monitor": "graph",
        "block_advanced_power_monitor": "graph", "block_powered_spawner": "mob", "block_reservoir": "water level",
        "block_omni_reservoir": "water level", "block_soul_binder": "spinning skulls", "block_tank": "fluid level",
        "block_tele_pad": "blades", "block_transceiver": "spinning sphere", "block_travel_anchor": "ender sphere",
        "block_wired_charger": "item being charged", "block_simple_wired_charger": "item being charged",
        "block_enhanced_wired_charger": "item being charged", "block_zombie_generator": "zombie head",
        "block_franken_zombie_generator": "zombie head", "block_ender_generator": "ender head",
        "block_vacuum_chest": "range box", "block_xp_vacuum": "range box", "block_impulse_hopper": "ghost items",
        "block_wireless_charger": "range box", "block_solar_panel": "connected borders",
        "block_inventory_panel": "screen", "block_electric_light": "light node placement"}

# Painted blocks: the vanilla block each one copies the states, shapes and unpainted look of (the default paint
# source of each BlockPainted*: oak for wood, cobblestone/stone bricks for stone, the plain block otherwise).
PAINTED = {
    "block_painted_fence": "minecraft:oak_fence", "block_painted_stone_fence": "minecraft:nether_brick_fence",
    "block_painted_fence_gate": "minecraft:oak_fence_gate", "block_painted_wall": "minecraft:cobblestone_wall",
    "block_painted_stair": "minecraft:oak_stairs", "block_painted_stone_stair": "minecraft:stone_brick_stairs",
    "block_painted_slab": "minecraft:oak_slab", "block_painted_stone_slab": "minecraft:stone_slab",
    "block_painted_glowstone": "minecraft:glowstone", "block_painted_glowstone_solid": "minecraft:glowstone",
    "block_painted_carpet": "minecraft:white_carpet", "block_painted_redstone": "minecraft:redstone_block",
    "block_painted_redstone_solid": "minecraft:redstone_block", "block_painted_stone": "minecraft:cobblestone",
    "block_painted_sand": "minecraft:sand", "block_painted_wooden_trapdoor": "minecraft:oak_trapdoor",
    "block_painted_iron_trapdoor": "minecraft:iron_trapdoor", "block_painted_dark_steel_trapdoor": "minecraft:iron_trapdoor",
    "block_painted_wooden_door": "minecraft:oak_door", "block_painted_iron_door": "minecraft:iron_door",
    "block_painted_dark_steel_door": "minecraft:iron_door", "block_painted_workbench": "minecraft:crafting_table",
    "block_painted_reinforced_obsidian": "minecraft:obsidian",
    "block_painted_reinforced_obsidian_nonsolid": "minecraft:obsidian",
    "block_painted_packed_ice": "minecraft:packed_ice", "block_painted_packed_ice_solid": "minecraft:packed_ice",
}
# EnumPressurePlateType: the vanilla plate whose behaviour each copies (wood: any entity; stone: mobs and players;
# iron/gold: weighted; dark steel: players only; soularium: mobs only; tuned: one entity type, all as a stone plate).
PRESSURE_PLATES = {"wooden": "minecraft:oak_pressure_plate", "stone": "minecraft:stone_pressure_plate",
                   "heavy_weighted": "minecraft:heavy_weighted_pressure_plate",
                   "light_weighted": "minecraft:light_weighted_pressure_plate",
                   "dark_steel": "minecraft:stone_pressure_plate", "soularium": "minecraft:stone_pressure_plate",
                   "tuned_soularium": "minecraft:stone_pressure_plate"}
PLATE_TEXTURES = {"dark_steel": "block_dark_steel_pressure_plate", "soularium": "block_soularium_pressure_plate",
                  "tuned_soularium": "block_soularium_pressure_plate"}

# 1.12 vanilla texture names Ender IO's models use, as 26.3's.
VANILLA_TEXTURES = {
    "blocks/lever": "minecraft:block/lever", "blocks/planks_oak": "minecraft:block/oak_planks",
    "blocks/cobblestone": "minecraft:block/cobblestone", "blocks/redstone_block": "minecraft:block/redstone_block",
    "blocks/water_still": "minecraft:block/water_still", "blocks/glowstone": "minecraft:block/glowstone",
    "blocks/trapdoor": "minecraft:block/oak_trapdoor", "blocks/sand": "minecraft:block/sand",
    "blocks/portal": "minecraft:block/nether_portal", "blocks/nether_brick": "minecraft:block/nether_bricks",
    "blocks/iron_trapdoor": "minecraft:block/iron_trapdoor", "blocks/bedrock": "minecraft:block/bedrock",
    "items/bucket_empty": "minecraft:item/bucket",
}
VANILLA_PARENTS = {"block/block": "minecraft:block/block", "item/generated": "minecraft:item/generated",
                   "item/handheld": "minecraft:item/handheld", "item/bow": "minecraft:item/bow",
                   "block/cube_all": "minecraft:block/cube_all", "block/cube": "minecraft:block/cube",
                   "block/thin_block": "minecraft:block/thin_block", "minecraft:cube_all": "minecraft:block/cube_all",
                   "anvil": "minecraft:block/template_anvil"}

# ---------------------------------------------------------------------------------------------------------------
# Recipes: 1.12 vanilla ids (with a meta, or None for "any meta") as 26.3 ids. An id the 1.12 game never had stays out
# (it can't resolve there either). A list is an "any of these" ingredient; a "#tag" a tag.
WOODS = ["oak", "spruce", "birch", "jungle", "acacia", "dark_oak"]
_FLOWERS = ["poppy", "blue_orchid", "allium", "azure_bluet", "red_tulip", "orange_tulip", "white_tulip", "pink_tulip",
            "oxeye_daisy"]
_DOUBLE = ["sunflower", "lilac", "tall_grass", "large_fern", "rose_bush", "peony"]
_DYES = ["ink_sac", "red_dye", "green_dye", "cocoa_beans", "lapis_lazuli", "purple_dye", "cyan_dye", "light_gray_dye",
         "gray_dye", "pink_dye", "lime_dye", "yellow_dye", "light_blue_dye", "magenta_dye", "orange_dye", "bone_meal"]
_STONES = ["stone", "granite", "polished_granite", "diorite", "polished_diorite", "andesite", "polished_andesite"]
_SKULLS = ["skeleton_skull", "wither_skeleton_skull", "zombie_head", "player_head", "creeper_head", "dragon_head"]
V112 = {
    ("noteblock", None): "minecraft:note_block", ("waterlily", None): "minecraft:lily_pad",
    ("web", None): "minecraft:cobweb", ("snow", None): "minecraft:snow_block", ("melon", None): "minecraft:melon_slice",
    ("melon_block", None): "minecraft:melon", ("deadbush", None): "minecraft:dead_bush",
    ("yellow_flower", None): "minecraft:dandelion", ("quartz_ore", None): "minecraft:nether_quartz_ore",
    ("wooden_pressure_plate", None): "minecraft:oak_pressure_plate", ("reeds", None): "minecraft:sugar_cane",
    ("fish", None): ["minecraft:cod", "minecraft:salmon", "minecraft:tropical_fish", "minecraft:pufferfish"],
    ("fish", 0): "minecraft:cod", ("fish", 1): "minecraft:salmon", ("fish", 2): "minecraft:tropical_fish",
    ("fish", 3): "minecraft:pufferfish",
    ("coal", None): ["minecraft:coal", "minecraft:charcoal"], ("coal", 0): "minecraft:coal", ("coal", 1): "minecraft:charcoal",
    ("sand", None): ["minecraft:sand", "minecraft:red_sand"], ("sand", 0): "minecraft:sand", ("sand", 1): "minecraft:red_sand",
    ("stone", None): [f"minecraft:{s}" for s in _STONES],
    ("stonebrick", None): ["minecraft:stone_bricks", "minecraft:mossy_stone_bricks", "minecraft:cracked_stone_bricks",
                           "minecraft:chiseled_stone_bricks"],
    ("stonebrick", 0): "minecraft:stone_bricks", ("stonebrick", 1): "minecraft:mossy_stone_bricks",
    ("stonebrick", 2): "minecraft:cracked_stone_bricks", ("stonebrick", 3): "minecraft:chiseled_stone_bricks",
    ("stone_slab", 7): "minecraft:quartz_slab", ("prismarine", 0): "minecraft:prismarine",
    ("prismarine", 1): "minecraft:prismarine_bricks", ("prismarine", 2): "minecraft:dark_prismarine",
    ("sponge", 0): "minecraft:sponge", ("sponge", 1): "minecraft:wet_sponge",
    ("tallgrass", 0): "minecraft:dead_bush", ("tallgrass", 1): "minecraft:short_grass", ("tallgrass", 2): "minecraft:fern",
    ("wool", None): "#minecraft:wool", ("carpet", None): "#minecraft:wool_carpets",
    ("skull", None): [f"minecraft:{s}" for s in _SKULLS],
    ("red_flower", None): [f"minecraft:{f}" for f in _FLOWERS],
    ("double_plant", None): [f"minecraft:{f}" for f in _DOUBLE],
    ("dye", None): [f"minecraft:{d}" for d in _DYES],
    ("concrete", None): "#minecraft:concrete", ("concrete_powder", None): "#minecraft:concrete_powders",
    ("planks", None): "#minecraft:planks", ("log", None): "#minecraft:logs", ("sapling", None): "#minecraft:saplings",
    ("leaves", None): "#minecraft:leaves",
    ("golden_apple", 0): "minecraft:golden_apple", ("golden_apple", 1): "minecraft:enchanted_golden_apple",
}
for _i, _f in enumerate(_FLOWERS):
    V112[("red_flower", _i)] = f"minecraft:{_f}"
for _i, _f in enumerate(_DOUBLE):
    V112[("double_plant", _i)] = f"minecraft:{_f}"
for _i, _d in enumerate(_DYES):
    V112[("dye", _i)] = f"minecraft:{_d}"
for _i, _s in enumerate(_STONES):
    V112[("stone", _i)] = f"minecraft:{_s}"
for _i, _s in enumerate(_SKULLS):
    V112[("skull", _i)] = f"minecraft:{_s}"
for _i, _c in enumerate(COLORS):
    for _k, _f in (("wool", "wool"), ("carpet", "carpet"), ("concrete", "concrete"), ("concrete_powder", "concrete_powder"),
                   ("stained_glass", "stained_glass"), ("stained_glass_pane", "stained_glass_pane"),
                   ("stained_hardened_clay", "terracotta")):
        V112[(_k, _i)] = f"minecraft:{_c}_{_f}"
for _i, _w in enumerate(WOODS):
    V112[("planks", _i)] = f"minecraft:{_w}_planks"
    V112[("sapling", _i)] = f"minecraft:{_w}_sapling"
for _i, _w in enumerate(WOODS[:4]):
    V112[("log", _i)] = f"minecraft:{_w}_log"
    V112[("leaves", _i)] = f"minecraft:{_w}_leaves"
# Metas that pick nothing new: a meta of 0 on an item without subtypes is the item itself (handled in code).
V112_SUBTYPED = {"wool", "carpet", "concrete", "concrete_powder", "stained_glass", "stained_glass_pane",
                 "stained_hardened_clay", "dye", "skull", "red_flower", "double_plant", "tallgrass", "stone",
                 "stonebrick", "sand", "coal", "fish", "sponge", "prismarine", "planks", "log", "sapling", "leaves",
                 "stone_slab", "golden_apple"}

# ---------------------------------------------------------------------------------------------------------------
# The ore dictionary as it stands in this game: Forge 1.12.2's OreDictionary.initVanillaEntries and EnderCore's
# OreDict.registerVanilla with their items in 26.3's names, plus the copper 26.3 adds (Ender IO's copper powder and the
# recipes that wait for ingotCopper see it). Ender IO's own registrations (MaterialOredicts) are added from the
# flattening table at run time. name -> (26.3 tag or None, members). A tag of None means the recipes name the item
# itself (a vanilla-only name with no convention tag).
VANILLA_ORE = {
    "logWood": ("minecraft:logs", None), "plankWood": ("minecraft:planks", None),
    "slabWood": ("minecraft:wooden_slabs", None), "stairWood": ("minecraft:wooden_stairs", None),
    "fenceWood": ("minecraft:wooden_fences", None), "fenceGateWood": ("minecraft:fence_gates", None),
    "doorWood": ("minecraft:wooden_doors", None), "stickWood": ("c:rods/wooden", ["minecraft:stick"]),
    "treeSapling": ("minecraft:saplings", None), "treeLeaves": ("minecraft:leaves", None),
    "vine": ("c:vines", ["minecraft:vine"]),
    "oreGold": ("c:ores/gold", ["minecraft:gold_ore", "minecraft:deepslate_gold_ore", "minecraft:nether_gold_ore"]),
    "oreIron": ("c:ores/iron", ["minecraft:iron_ore", "minecraft:deepslate_iron_ore"]),
    "oreLapis": ("c:ores/lapis", ["minecraft:lapis_ore", "minecraft:deepslate_lapis_ore"]),
    "oreDiamond": ("c:ores/diamond", ["minecraft:diamond_ore", "minecraft:deepslate_diamond_ore"]),
    "oreRedstone": ("c:ores/redstone", ["minecraft:redstone_ore", "minecraft:deepslate_redstone_ore"]),
    "oreEmerald": ("c:ores/emerald", ["minecraft:emerald_ore", "minecraft:deepslate_emerald_ore"]),
    "oreQuartz": ("c:ores/quartz", ["minecraft:nether_quartz_ore"]),
    "oreCoal": ("c:ores/coal", ["minecraft:coal_ore", "minecraft:deepslate_coal_ore"]),
    "oreCopper": ("c:ores/copper", ["minecraft:copper_ore", "minecraft:deepslate_copper_ore"]),
    "ingotIron": ("c:ingots/iron", ["minecraft:iron_ingot"]), "ingotGold": ("c:ingots/gold", ["minecraft:gold_ingot"]),
    "ingotCopper": ("c:ingots/copper", ["minecraft:copper_ingot"]),
    "ingotBrick": ("c:bricks/normal", ["minecraft:brick"]),
    "ingotBrickNether": ("c:bricks/nether", ["minecraft:nether_brick"]),
    "nuggetGold": ("c:nuggets/gold", ["minecraft:gold_nugget"]), "nuggetIron": ("c:nuggets/iron", ["minecraft:iron_nugget"]),
    "nuggetCopper": ("c:nuggets/copper", ["minecraft:copper_nugget"]),
    "gemDiamond": ("c:gems/diamond", ["minecraft:diamond"]), "gemEmerald": ("c:gems/emerald", ["minecraft:emerald"]),
    "gemQuartz": ("c:gems/quartz", ["minecraft:quartz"]), "gemPrismarine": ("c:gems/prismarine", ["minecraft:prismarine_shard"]),
    "dustPrismarine": ("c:dusts/prismarine", ["minecraft:prismarine_crystals"]),
    "dustRedstone": ("c:dusts/redstone", ["minecraft:redstone"]),
    "dustGlowstone": ("c:dusts/glowstone", ["minecraft:glowstone_dust"]), "gemLapis": ("c:gems/lapis", ["minecraft:lapis_lazuli"]),
    "blockGold": ("c:storage_blocks/gold", ["minecraft:gold_block"]), "blockIron": ("c:storage_blocks/iron", ["minecraft:iron_block"]),
    "blockLapis": ("c:storage_blocks/lapis", ["minecraft:lapis_block"]),
    "blockDiamond": ("c:storage_blocks/diamond", ["minecraft:diamond_block"]),
    "blockRedstone": ("c:storage_blocks/redstone", ["minecraft:redstone_block"]),
    "blockEmerald": ("c:storage_blocks/emerald", ["minecraft:emerald_block"]),
    "blockQuartz": ("c:storage_blocks/quartz", ["minecraft:quartz_block"]),
    "blockCoal": ("c:storage_blocks/coal", ["minecraft:coal_block"]),
    "blockCopper": ("c:storage_blocks/copper", ["minecraft:copper_block"]),
    "cropWheat": ("c:crops/wheat", ["minecraft:wheat"]), "cropPotato": ("c:crops/potato", ["minecraft:potato"]),
    "cropCarrot": ("c:crops/carrot", ["minecraft:carrot"]), "cropNetherWart": ("c:crops/nether_wart", ["minecraft:nether_wart"]),
    "sugarcane": ("c:crops/sugar_cane", ["minecraft:sugar_cane"]), "blockCactus": ("c:crops/cactus", ["minecraft:cactus"]),
    "dye": ("c:dyes", None), "paper": ("c:paper", ["minecraft:paper"]),
    "slimeball": ("c:slime_balls", ["minecraft:slime_ball"]), "enderpearl": ("c:ender_pearls", ["minecraft:ender_pearl"]),
    "bone": ("c:bones", ["minecraft:bone"]), "gunpowder": ("c:gunpowders", ["minecraft:gunpowder"]),
    "string": ("c:strings", ["minecraft:string"]), "netherStar": ("c:nether_stars", ["minecraft:nether_star"]),
    "leather": ("c:leathers", ["minecraft:leather"]), "feather": ("c:feathers", ["minecraft:feather"]),
    "egg": ("c:eggs", ["minecraft:egg"]), "dirt": ("minecraft:dirt", None), "grass": (None, ["minecraft:grass_block"]),
    "stone": ("c:stones", ["minecraft:stone"]), "cobblestone": ("c:cobblestones", ["minecraft:cobblestone"]),
    "gravel": ("c:gravels", ["minecraft:gravel"]), "sand": ("c:sands", ["minecraft:sand", "minecraft:red_sand"]),
    "sandstone": ("c:sandstone/blocks", ["minecraft:sandstone", "minecraft:chiseled_sandstone", "minecraft:cut_sandstone",
                                         "minecraft:red_sandstone", "minecraft:chiseled_red_sandstone", "minecraft:cut_red_sandstone"]),
    "netherrack": ("c:netherracks", ["minecraft:netherrack"]), "obsidian": ("c:obsidians", ["minecraft:obsidian"]),
    "glowstone": ("c:glowstone", ["minecraft:glowstone"]), "endstone": ("c:end_stones", ["minecraft:end_stone"]),
    "torch": (None, ["minecraft:torch"]), "workbench": ("c:player_workstations/crafting_tables", ["minecraft:crafting_table"]),
    "blockSlime": ("c:storage_blocks/slime", ["minecraft:slime_block"]),
    "blockPrismarine": (None, ["minecraft:prismarine"]), "blockPrismarineBrick": (None, ["minecraft:prismarine_bricks"]),
    "blockPrismarineDark": (None, ["minecraft:dark_prismarine"]),
    "stoneGranite": (None, ["minecraft:granite"]), "stoneGranitePolished": (None, ["minecraft:polished_granite"]),
    "stoneDiorite": (None, ["minecraft:diorite"]), "stoneDioritePolished": (None, ["minecraft:polished_diorite"]),
    "stoneAndesite": (None, ["minecraft:andesite"]), "stoneAndesitePolished": (None, ["minecraft:polished_andesite"]),
    "blockGlassColorless": ("c:glass_blocks/colorless", ["minecraft:glass"]),
    "blockGlass": ("c:glass_blocks", ["minecraft:glass"] + [f"minecraft:{c}_stained_glass" for c in COLORS]),
    "paneGlassColorless": ("c:glass_panes/colorless", ["minecraft:glass_pane"]),
    "paneGlass": ("c:glass_panes", ["minecraft:glass_pane"] + [f"minecraft:{c}_stained_glass_pane" for c in COLORS]),
    "wool": ("minecraft:wool", None), "chest": ("c:chests", ["minecraft:chest", "minecraft:trapped_chest", "minecraft:ender_chest"]),
    "chestWood": ("c:chests/wooden", ["minecraft:chest", "minecraft:trapped_chest"]),
    "chestEnder": ("c:chests/ender", ["minecraft:ender_chest"]), "chestTrapped": ("c:chests/trapped", ["minecraft:trapped_chest"]),
    # EnderCore OreDict.registerVanilla
    "barsIron": (None, ["minecraft:iron_bars"]), "blockHopper": (None, ["minecraft:hopper"]),
    "itemCoal": (None, ["minecraft:coal"]), "itemCharcoal": (None, ["minecraft:charcoal"]),
    "pearlEnderEye": (None, ["minecraft:ender_eye"]), "itemBlazeRod": (None, ["minecraft:blaze_rod"]),
    "itemBlazePowder": (None, ["minecraft:blaze_powder"]), "itemClay": (None, ["minecraft:clay_ball"]),
    "itemFlint": (None, ["minecraft:flint"]), "itemGhastTear": (None, ["minecraft:ghast_tear"]),
    "itemLeather": (None, ["minecraft:leather"]),
}
for _c in COLORS:
    _dye = {"white": "white_dye", "black": "black_dye", "brown": "brown_dye", "blue": "blue_dye"}.get(_c, f"{_c}_dye")
    # Forge 1.12 put the old dye metas in; 26.3's dyes are their own items (bone meal, ink sac, cocoa and lapis aren't).
    VANILLA_ORE["dye" + ORE_COLOR[_c]] = (f"c:dyes/{_c}", [f"minecraft:{_dye}"])
    VANILLA_ORE["blockGlass" + ORE_COLOR[_c]] = (None, [f"minecraft:{_c}_stained_glass"])
    VANILLA_ORE["paneGlass" + ORE_COLOR[_c]] = (None, [f"minecraft:{_c}_stained_glass_pane"])

# The oredict prefixes, as the c: convention's folders (spec 4).
ORE_PREFIX = [("ingot", "ingots"), ("nugget", "nuggets"), ("dust", "dusts"), ("block", "storage_blocks"),
              ("ore", "ores"), ("gem", "gems"), ("gear", "gears"), ("plate", "plates"), ("rod", "rods"),
              ("crop", "crops"), ("seed", "seeds"), ("dye", "dyes"), ("ball", "grinding_balls")]
