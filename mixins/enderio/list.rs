fn enderio_list(mods: &mut Vec<crate::game::native_hooks::NativeMod>) {
    mods.push(crate::game::native_hooks::NativeMod {
        id: "enderio".into(),
        title: "Ender IO 5.3.72".into(),
        description: "Machines, power, conduits and travel, as in Ender IO.".into(),
        keys: Vec::new(),
    });
}
