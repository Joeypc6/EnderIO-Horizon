# Ender IO for Horizon

An unofficial port of [Ender IO](https://github.com/SleepyTrousers/EnderIO) 5.3.72 (Minecraft 1.12.2, by the Ender IO
team; code in the public domain, see LICENSE and NOTICE) to [Horizon](https://github.com/horizon-rust/horizon) and
Minecraft 26.x, as one native mod. The Java mod was rewritten in Rust for Horizon's hooks, and its 1.12 ids were
flattened to 26.x ones. Textures, models and language files are Ender IO's.

- `mixins/enderio`: the mod, and `data/enderio-content.hmod` its blocks, items, recipes and textures.
- `tools/`: make the content from the original jar (`make_content.py`) and write the bundle (`make_bundle.py`).

## What's there

- The machines with their Java screens and slot layouts: alloy smelter, SAG mill, slice'n'splice, soul binder,
  vat, enchanter, painter, generators, capacitor banks, tanks and the rest.
- Conduits: item, fluid (normal and pressurized), energy and redstone, each face's settings screen, the probe's
  copy and paste, the Yeta wrench's modes, and facades (painted, transparent, hardened with 10x blast resistance).
- Filters: basic, advanced, limited, big, mod, power, existing, fluid, enchantment and soul, with ghost slots
  (tooltips and the mouse wheel) and the redstone signal filters. Every edit is checked by the server.
- The W guide from Ender IO's book with 3D scenes, JEI categories, and Jade.

## What differs from the Java mod

- Ore dictionary matching uses the common `c:` item tags (1.12's ore dictionary doesn't exist in 26.x).
- 1.12-only items and other mods' upgrades (Forestry, Thaumcraft, The One Probe) are left out.
- The mod filter's mouse wheel (clearing a slot set within 2.5 s) isn't there.
- Built for Horizon fdc87ae (Nightly 2026-11-08); see `horizon-native.json`.

## Tested

In game on Horizon `fdc87ae` builds this week (the last, `native-fdc87ae-83d10460ef68`, with the nine other ports): the machines' screens, all four
conduit types, every filter's screen and matching (enchantment and soul filters through item conduits, ore dictionary through `c:`
tags), the probe, the Yeta wrench, facades (a hardened one survives TNT, a plain one breaks), and the models against the 1.12 jar.
