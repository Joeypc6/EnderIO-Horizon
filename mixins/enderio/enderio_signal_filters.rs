// Ender IO 5.3.72's redstone signal filters (crazypants.enderio.base.filter.redstone, CC0 / Unlicense, the Ender IO
// team) for Horizon: the insulated redstone conduit's input filter (slot 0, the face's outputFilterUpgrades: timer,
// sensor) and output filter (slot 1, its inputFilterUpgrades: AND, OR, NAND, NOR, XOR, XNOR, NOT, toggle, counting),
// their NBT on the filter item (Java's names), what they do to a signal, their screens (RedstoneSignalLogicGui,
// IncrementingValueFilterGui) and the redstone panel's own buttons (RedstoneSettings: the signal colours and strong).
use super::Game;
use super::enderio::{gc, gi, js, Pos};
use super::enderio_conduits::Conduit;
use horizon_client::containers::ModBlockEntity;
use horizon_client::hud::HudBatch;
use horizon_client::items::ItemStack;
use horizon_world::nbt::{Compound, Tag};
use std::cell::RefCell;
use std::collections::HashMap;

pub(in crate::game) const ITEMS: [&str; 11] = [
    "enderio:redstone_and_filter",
    "enderio:redstone_or_filter",
    "enderio:redstone_nand_filter",
    "enderio:redstone_nor_filter",
    "enderio:redstone_xor_filter",
    "enderio:redstone_xnor_filter",
    "enderio:redstone_not_filter",
    "enderio:redstone_toggle_filter",
    "enderio:redstone_counting_filter",
    "enderio:redstone_timer_filter",
    "enderio:redstone_sensor_filter",
];

// EnumSignalFilterType's ordinal, by item.
fn logic_type(item: &str) -> Option<i32> {
    ITEMS[..6].iter().position(|i| *i == item).map(|p| [1, 0, 2, 3, 4, 5][p])
}

// hasGui: the logic filters, counting and timer.
pub(in crate::game) fn has_screen(item: &str) -> bool {
    logic_type(item).is_some() || item == ITEMS[8] || item == ITEMS[9]
}

fn tag(stack: &ItemStack) -> Compound {
    super::enderio_filters::filter_tag(stack).cloned().unwrap_or_default()
}

fn put(stack: &mut ItemStack, f: Compound) {
    let mut c = super::enderio::custom_data(stack).cloned().unwrap_or_default();
    c.insert(js("filter"), Tag::Compound(f));
    super::enderio::set_custom_data(stack, c);
}

fn sub_i(c: &Compound, outer: &str, inner: &str, d: i32) -> i32 {
    gc(c, outer).map_or(d, |t| gi(t, inner, d))
}

fn set_sub(c: &mut Compound, outer: &str, inner: &str, v: Tag) {
    let mut t = gc(c, outer).cloned().unwrap_or_default();
    t.insert(js(inner), v);
    c.insert(js(outer), Tag::Compound(t));
}

// LogicOutputSignalFilter: its type and two colours (DyeColor.fromIndex(i) by default: black, red).
fn logic_colors(stack: &ItemStack) -> [i32; 2] {
    let c = tag(stack);
    let mut out = [0, 1];
    if let Some(Tag::List(l)) = c.get(&js("signalColors")) {
        for (i, t) in l.iter().enumerate().take(2) {
            if let Tag::Compound(t) = t {
                out[i] = gi(t, "color", i as i32).clamp(0, 15);
            }
        }
    }
    out
}

fn write_logic(stack: &mut ItemStack, ty: i32, colors: [i32; 2]) {
    let mut c = Compound::new();
    set_sub(&mut c, "filterType", "type", Tag::Int(ty));
    let list = colors
        .iter()
        .enumerate()
        .map(|(i, col)| {
            let mut t = Compound::new();
            t.insert(js("index"), Tag::Int(i as i32));
            t.insert(js("color"), Tag::Int(*col));
            Tag::Compound(t)
        })
        .collect();
    c.insert(js("signalColors"), Tag::List(list));
    put(stack, c);
}

// IFilterIncrementingValue: the counting filter's maxCount (1), the timer's time (20).
fn incrementing(stack: &ItemStack) -> i32 {
    let c = tag(stack);
    if *stack.item == *ITEMS[8] { sub_i(&c, "maxCount", "max", 1) } else { sub_i(&c, "maxTime", "max", 20) }
}

// Actions: 0 / 1 the first colour next / back, 2 / 3 the second; 10 + 2n up, 11 + 2n down by 1, 10, 100, 1000.
pub(in crate::game) fn edit(stack: &ItemStack, action: i32) -> Option<ItemStack> {
    let mut s = stack.clone();
    if let Some(ty) = logic_type(&stack.item) {
        if !(0..4).contains(&action) {
            return None;
        }
        let mut cols = logic_colors(stack);
        let i = (action / 2) as usize;
        cols[i] = (cols[i] + if action % 2 == 0 { 1 } else { 15 }) % 16;
        write_logic(&mut s, ty, cols);
    } else if *stack.item == *ITEMS[8] || *stack.item == *ITEMS[9] {
        if !(10..18).contains(&action) {
            return None;
        }
        let by = [1, 10, 100, 1000][((action - 10) / 2) as usize] * if action % 2 == 0 { 1 } else { -1 };
        let v = incrementing(stack).saturating_add(by);
        let mut c = tag(stack);
        let outer = if *stack.item == *ITEMS[8] { "maxCount" } else { "maxTime" };
        set_sub(&mut c, outer, "max", Tag::Int(v));
        put(&mut s, c);
    } else {
        return None;
    }
    (s != *stack).then_some(s)
}

// ---- What the filters do (the network's update)

// The stateful filters' state by (dimension, bundle, face), read from the item when first seen: the timer's
// currentTime, the toggle's (active, deactivated), the counter's (count, deactivated).
#[derive(Clone, Copy, Debug, PartialEq)]
enum St {
    Timer(i32),
    Toggle(bool, bool),
    Count(i32, bool),
}

thread_local! {
    static STATE: RefCell<HashMap<(String, Pos, usize), St>> = RefCell::new(HashMap::new());
}

fn state(dim: &str, pos: Pos, d: usize, stack: &ItemStack) -> St {
    let key = (dim.to_owned(), pos, d);
    if let Some(s) = STATE.with_borrow(|m| m.get(&key).copied()) {
        if matches!((s, &*stack.item), (St::Timer(_), "enderio:redstone_timer_filter") | (St::Toggle(..), "enderio:redstone_toggle_filter") | (St::Count(..), "enderio:redstone_counting_filter")) {
            return s;
        }
    }
    let c = tag(stack);
    let deactivated = |c: &Compound| gc(c, "deactivated").map_or(true, |t| gi(t, "deactivated", 1) != 0);
    match &*stack.item {
        "enderio:redstone_toggle_filter" => St::Toggle(sub_i(&c, "toggleActive", "active", 0) != 0, deactivated(&c)),
        "enderio:redstone_counting_filter" => St::Count(sub_i(&c, "currentCount", "count", 0), deactivated(&c)),
        _ => St::Timer(sub_i(&c, "currentTime", "time", 0)),
    }
}

fn set_state(dim: &str, pos: Pos, d: usize, s: St) {
    STATE.with_borrow_mut(|m| m.insert((dim.to_owned(), pos, d), s));
}

// The state written back on the filter item (Java saves it with the conduit), as the conduit's new stack.
fn state_stack(stack: &ItemStack, s: St) -> ItemStack {
    let mut out = stack.clone();
    let mut c = tag(stack);
    match s {
        St::Timer(t) => set_sub(&mut c, "currentTime", "time", Tag::Int(t)),
        St::Toggle(a, de) => {
            set_sub(&mut c, "toggleActive", "active", Tag::Byte(i8::from(a)));
            set_sub(&mut c, "deactivated", "deactivated", Tag::Byte(i8::from(de)));
        }
        St::Count(n, de) => {
            set_sub(&mut c, "currentCount", "count", Tag::Int(n));
            set_sub(&mut c, "deactivated", "deactivated", Tag::Byte(i8::from(de)));
        }
    }
    put(&mut out, c);
    out
}

pub(in crate::game) fn input_filter(c: &Conduit, d: usize) -> Option<ItemStack> {
    c.stack("outputFilterUpgrades", d).filter(|s| *s.item == *ITEMS[9] || *s.item == *ITEMS[10])
}

pub(in crate::game) fn output_filter(c: &Conduit, d: usize) -> Option<ItemStack> {
    c.stack("inputFilterUpgrades", d).filter(|s| ITEMS[..9].contains(&&*s.item))
}

// IInputSignalFilter.apply on the level read from face `d`: the timer gives a pulse when its time comes round with
// no signal; the sensor reads the neighbour's comparator output.
pub(in crate::game) fn apply_input(game: &Game<'_>, c: &Conduit, pos: Pos, d: usize, level: i32) -> i32 {
    let Some(f) = input_filter(c, d) else { return level };
    if *f.item == *ITEMS[10] {
        return comparator(game, super::enderio::step(pos, d));
    }
    let dim = game.native_dimension();
    let St::Timer(cur) = state(dim, pos, d, &f) else { return 0 };
    if level == 0 && cur == incrementing(&f) {
        set_state(dim, pos, d, St::Timer(0));
        15
    } else {
        0
    }
}

// ComparatorInputSignalFilter: the block's comparator output. Ender IO's banks and tanks answer for themselves; a
// container by vanilla's AbstractContainerMenu.getRedstoneSignalFromContainer; anything else 0.
fn comparator(game: &Game<'_>, n: Pos) -> i32 {
    if let Some(horizon_client::containers::BlockEntity::Mod(m)) = game.world.block_entities.get(&(n[0], n[1], n[2])) {
        if let Some(v) = super::enderio::comparator(m).or_else(|| super::enderio_machines::tank_comparator(m)) {
            return v;
        }
    }
    let dim = game.native_dimension();
    let Some(slots) = game.native_container_slots(dim, n, horizon_core::geometry::Direction::Up) else { return 0 };
    if slots.is_empty() {
        return 0;
    }
    let mut any = false;
    let mut sum = 0.0f32;
    for s in slots.iter().filter_map(|s| s.stack.as_ref()) {
        any = true;
        sum += s.count as f32 / super::enderio::max_stack(game, &s.item) as f32;
    }
    if !any { 0 } else { (1.0 + sum / slots.len() as f32 * 14.0).floor() as i32 }
}

// IOutputSignalFilter.apply for the face's output colour over the bundled signal; the state's new stack when a
// stateful filter changed.
pub(in crate::game) fn apply_output(game: &Game<'_>, c: &Conduit, pos: Pos, d: usize, color: i32, bundled: &[i32; 16]) -> (i32, Option<ItemStack>) {
    let raw = bundled[color.clamp(0, 15) as usize];
    let Some(f) = output_filter(c, d) else { return (raw, None) };
    let on = |col: i32| bundled[col.clamp(0, 15) as usize] > 0;
    let max = |b: bool| if b { 15 } else { 0 };
    if let Some(ty) = logic_type(&f.item) {
        let cols = logic_colors(&f);
        let or = cols.iter().any(|c| on(*c));
        let and = cols.iter().all(|c| on(*c));
        let xor = cols.iter().filter(|c| on(**c)).count() % 2 == 1;
        return (max(match ty { 0 => or, 1 => and, 2 => !and, 3 => !or, 4 => xor, _ => !xor }), None);
    }
    if *f.item == *ITEMS[6] {
        return (max(raw == 0), None);
    }
    let dim = game.native_dimension();
    let before = state(dim, pos, d, &f);
    let (out, after) = match before {
        St::Toggle(mut active, mut de) => {
            if raw > 0 && de {
                active = !active;
                de = false;
            }
            if raw == 0 {
                de = true;
            }
            (max(active), St::Toggle(active, de))
        }
        St::Count(mut n, mut de) => {
            if raw > 0 && de {
                n += 1;
                de = false;
            }
            if raw == 0 {
                de = true;
            }
            let most = incrementing(&f);
            let v = if n > most {
                n = 1;
                0
            } else {
                max(n == most)
            };
            (v, St::Count(n, de))
        }
        St::Timer(_) => (raw, before),
    };
    if after != before {
        set_state(dim, pos, d, after);
        return (out, Some(state_stack(&f, after)));
    }
    (out, None)
}

// TimerInputSignalFilter.shouldUpdate, each tick for the timers of a network (the sensor always): true when the
// network should read its inputs again.
pub(in crate::game) fn tick(game: &Game<'_>, c: &Conduit, pos: Pos, d: usize) -> bool {
    let Some(f) = input_filter(c, d) else { return false };
    if *f.item == *ITEMS[10] {
        return true;
    }
    let dim = game.native_dimension();
    let St::Timer(mut cur) = state(dim, pos, d, &f) else { return false };
    let time = incrementing(&f);
    cur += 1;
    if cur > time {
        cur = 1;
    }
    set_state(dim, pos, d, St::Timer(cur));
    cur == time || cur == 1
}

// ---- The redstone panel's own buttons (RedstoneSettings): input colour 22, 31; output colour 112, 31; strong 112, 51.

const IN_COLOR: [f32; 4] = [22.0, 31.0, 16.0, 16.0];
const OUT_COLOR: [f32; 4] = [112.0, 31.0, 16.0, 16.0];
const STRONG: [f32; 4] = [112.0, 51.0, 16.0, 16.0];
pub(in crate::game) const B_PANEL: i32 = 194; // 194 / 195 input colour next / back, 196 / 197 output, 198 strong

// Data 42..44: the input colour, the output colour, strong.
pub(in crate::game) fn fill_data(e: &mut ModBlockEntity, c: &Conduit, d: usize) {
    if e.data.len() < 64 {
        return;
    }
    e.data[42] = c.rs_input_color(d);
    e.data[43] = c.rs_output_color(d);
    e.data[44] = i32::from(c.rs_strong(d));
}

pub(in crate::game) fn panel_button(c: &mut Conduit, d: usize, id: i32) -> bool {
    match id {
        194 | 195 => {
            let v = (c.rs_input_color(d) + if id == 194 { 1 } else { 15 }) % 16;
            c.set_dir_i("inputSignalColor", d, v);
        }
        196 | 197 => {
            let v = (c.rs_output_color(d) + if id == 196 { 1 } else { 15 }) % 16;
            c.set_dir_i("outputSignalColor", d, v);
        }
        198 => {
            let v = !c.rs_strong(d);
            c.set_dir_i("signalStrong", d, i32::from(v));
        }
        _ => return false,
    }
    true
}

fn color_button(game: &Game<'_>, batch: &mut HudBatch, o: [f32; 2], r: [f32; 4], col: i32, mouse: [f32; 2]) {
    super::enderio_ui::button(game, batch, o, r, false, mouse);
    let c = super::enderio_conduits::DYE_COLORS[col.clamp(0, 15) as usize];
    super::enderio_ui::fill(game, batch, o, [r[0] + 2.0, r[1] + 2.0, r[2] - 4.0, r[3] - 4.0], 0xFF00_0000 | c);
}

pub(in crate::game) fn draw_panel(game: &Game<'_>, batch: &mut HudBatch, o: [f32; 2], data: &[i32], mouse: [f32; 2]) {
    // renderCustomOptions (top 29): "Signal Color" at left + 31 and left + 121, top + 6; "Strong Signal" at
    // left + 121, top + 26; dark grey.
    let colour = super::enderio_ui::lang("enderio.gui.signal_color", "Signal Color");
    let strong = super::enderio_ui::lang("enderio.gui.redstone_signal_strength", "Strong Signal");
    super::enderio_ui::text(game, batch, &colour, o[0] + 41.0, o[1] + 35.0, 0xFF40_4040, false);
    super::enderio_ui::text(game, batch, &colour, o[0] + 131.0, o[1] + 35.0, 0xFF40_4040, false);
    super::enderio_ui::text(game, batch, &strong, o[0] + 131.0, o[1] + 55.0, 0xFF40_4040, false);
    color_button(game, batch, o, IN_COLOR, data[42], mouse);
    color_button(game, batch, o, OUT_COLOR, data[43], mouse);
    // CheckBox: EnderWidget BUTTON, BUTTON_CHECKED when selected.
    super::enderio_ui::button(game, batch, o, STRONG, false, mouse);
    if data[44] != 0 {
        super::enderio_ui::ec(game, batch, o, [STRONG[0], STRONG[1]], [112.0, 208.0, 16.0, 16.0]);
    }
}

pub(in crate::game) fn panel_press(game: &Game<'_>, data: &[i32], at: [f32; 2], right: bool) -> bool {
    let _ = data;
    let send = |id: i32| super::enderio_ui::send_button(game, id);
    if super::enderio_ui::inside(IN_COLOR, at) {
        send(if right { 195 } else { 194 });
    } else if super::enderio_ui::inside(OUT_COLOR, at) {
        send(if right { 197 } else { 196 });
    } else if !right && super::enderio_ui::inside(STRONG, at) {
        send(198);
    } else {
        return false;
    }
    true
}

fn color_name(c: i32) -> String {
    let names = ["black", "red", "green", "brown", "blue", "purple", "cyan", "silver", "gray", "pink", "lime", "yellow", "lightBlue", "magenta", "orange", "white"];
    let n = names[c.clamp(0, 15) as usize];
    super::enderio_ui::lang(&format!("enderio.color.{n}"), n)
}

pub(in crate::game) fn panel_tooltip(data: &[i32], at: [f32; 2], lines: &mut Vec<String>) {
    let l = super::enderio_ui::lang;
    if super::enderio_ui::inside(IN_COLOR, at) {
        lines.push(l("enderio.gui.signal_color", "Signal Color"));
        lines.push(color_name(data[42]));
    } else if super::enderio_ui::inside(OUT_COLOR, at) {
        lines.push(l("enderio.gui.signal_color", "Signal Color"));
        lines.push(color_name(data[43]));
    } else if super::enderio_ui::inside(STRONG, at) {
        lines.push(l("enderio.gui.redstone_signal_strength", "Strong Signal"));
    }
}

// ---- The filter screens (AbstractFilterGui on item_filter)

fn heading(item: &str) -> String {
    let (key, english) = match item {
        "enderio:redstone_and_filter" => ("and", "Redstone AND Filter"),
        "enderio:redstone_or_filter" => ("or", "Redstone OR Filter"),
        "enderio:redstone_nand_filter" => ("nand", "Redstone NAND Filter"),
        "enderio:redstone_nor_filter" => ("nor", "Redstone NOR Filter"),
        "enderio:redstone_xor_filter" => ("xor", "Redstone XOR Filter"),
        "enderio:redstone_xnor_filter" => ("xnor", "Redstone XNOR Filter"),
        "enderio:redstone_counting_filter" => ("counting", "Redstone Counting Filter"),
        _ => ("timer", "Redstone Timer Filter"),
    };
    super::enderio_ui::lang(&format!("enderio.gui.redstone_filter.{key}"), english)
}

fn value_name(item: &str) -> String {
    if item == ITEMS[8] { super::enderio_ui::lang("enderio.gui.redstone_filter.count", "Count") } else { super::enderio_ui::lang("enderio.gui.redstone_filter.time", "Time") }
}

const CLOSE: [f32; 4] = [3.0, 3.0, 16.0, 16.0];

// RedstoneSignalLogicGui: a colour button each at (13, 34 + 20 i), "Input Signal n" beside it.
fn color_rect(i: usize) -> [f32; 4] {
    [13.0, 34.0 + 20.0 * i as f32, 16.0, 16.0]
}

// IncrementingValueFilterGui: the + / - after the label (xOffset + width + 24, yOffset - 4 / + 4).
fn plus_minus(game: &Game<'_>, item: &str) -> ([f32; 4], [f32; 4]) {
    let x = 13.0 + super::enderio_ui::width(game, &value_name(item)).round() + 24.0;
    ([x, 30.0, 8.0, 8.0], [x, 38.0, 8.0, 8.0])
}

thread_local! {
    // The text width the last frame measured, for the presses (which have no font).
    static PM: std::cell::Cell<([f32; 4], [f32; 4])> = const { std::cell::Cell::new(([0.0; 4], [0.0; 4])) };
}

pub(in crate::game) fn draw(game: &Game<'_>, batch: &mut HudBatch, o: [f32; 2], stack: &ItemStack, _data: &[i32]) {
    read_modifiers(game);
    let mouse = super::enderio_ui::local_cursor(game, o);
    super::enderio_ui::spr(game, batch, "enderio:gui/40/item_filter", o, [0.0, 0.0], [0.0, 0.0, super::enderio_filters::W, super::enderio_filters::H]);
    super::enderio_ui::text(game, batch, &heading(&stack.item), o[0] + 28.0, o[1] + 13.0, 0xFF40_4040, false);
    super::enderio_ui::button(game, batch, o, CLOSE, false, mouse);
    super::enderio_ui::icon(game, batch, o, [3.0, 3.0], [0.0, 208.0], [16.0, 16.0], [16.0, 16.0]);
    if logic_type(&stack.item).is_some() {
        let cols = logic_colors(stack);
        let label = super::enderio_ui::lang("enderio.gui.redstone_filter.input_signal", "Input Signal");
        for i in 0..2 {
            color_button(game, batch, o, color_rect(i), cols[i], mouse);
            super::enderio_ui::text(game, batch, &format!("{label} {}", i + 1), o[0] + 33.0, o[1] + 38.0 + 20.0 * i as f32, 0xFF40_4040, false);
        }
    } else {
        let name = value_name(&stack.item);
        let w = super::enderio_ui::width(game, &name);
        super::enderio_ui::text(game, batch, &name, o[0] + 13.0, o[1] + 34.0, 0xFF40_4040, false);
        super::enderio_ui::text(game, batch, &format!(" {}", incrementing(stack)), o[0] + 13.0 + w, o[1] + 34.0, 0xFF40_4040, false);
        let (up, down) = plus_minus(game, &stack.item);
        PM.with(|p| p.set((up, down)));
        for (r, uv) in [(up, [208.0, 32.0]), (down, [208.0, 40.0])] {
            let hover = super::enderio_ui::inside(r, mouse);
            super::enderio_ui::ec(game, batch, o, [r[0], r[1]], [uv[0], uv[1] + if hover { 16.0 } else { 0.0 }, 8.0, 8.0]);
        }
    }
}

// The action for a click (A_CLOSE for the arrow), Some(-1) for a used click, None for none.
pub(in crate::game) fn press(stack: &ItemStack, _data: &[i32], at: [f32; 2], right: bool) -> Option<i32> {
    if super::enderio_ui::inside(CLOSE, at) {
        return Some(if right { -1 } else { super::enderio_filters::A_CLOSE });
    }
    if logic_type(&stack.item).is_some() {
        for i in 0..2 {
            if super::enderio_ui::inside(color_rect(i), at) {
                return Some(i as i32 * 2 + i32::from(right));
            }
        }
        return None;
    }
    let (up, down) = PM.with(std::cell::Cell::get);
    let hit = if super::enderio_ui::inside(up, at) { 0 } else if super::enderio_ui::inside(down, at) { 1 } else { return None };
    if right {
        return Some(-1);
    }
    // Shift x 10, Control x 100, Alt x 1000: the strongest held wins (Java multiplies; 10 x 100 x 1000 is past any use).
    let step = MODS.with(std::cell::Cell::get);
    Some(10 + 2 * step + hit)
}

thread_local! {
    // The modifier step (0 none, 1 Shift, 2 Control, 3 Alt), read each frame.
    static MODS: std::cell::Cell<i32> = const { std::cell::Cell::new(0) };
}

pub(in crate::game) fn read_modifiers(game: &Game<'_>) {
    let s = if game.native_has_alt_down() { 3 } else if game.native_has_control_down() { 2 } else if game.native_has_shift_down() { 1 } else { 0 };
    MODS.with(|m| m.set(s));
}

pub(in crate::game) fn tooltip(stack: &ItemStack, _data: &[i32], at: [f32; 2], lines: &mut Vec<String>) {
    if super::enderio_ui::inside(CLOSE, at) {
        lines.push(super::enderio_ui::lang("enderio.gui.item_filter.close", "Close Filter"));
        lines.push(super::enderio_ui::lang("enderio.gui.item_filter.close2", "Go back to previous inventory"));
    } else if logic_type(&stack.item).is_some() {
        let cols = logic_colors(stack);
        for i in 0..2 {
            if super::enderio_ui::inside(color_rect(i), at) {
                lines.push(super::enderio_ui::lang("enderio.gui.redstone_filter.signal_color", "Redstone Signal Color"));
                lines.push(color_name(cols[i]));
            }
        }
    }
}
