// Ender IO's screens (spec-machines.md 3), drawn from the jar's own GUI textures (gui/40/*, widgetsv2 at 2x, the
// power bar strip overlay.png) and EnderCore's button frames; their JEI categories and their Jade lines. A screen
// only asks: each press is a menu button the server checks and applies (enderio::menu_button).
use super::Game;
use super::enderio::*;
use horizon_client::hud::{HudBatch, rgba};
use std::cell::RefCell;

enderio_optional::if_mod_items!("jei", {
enderio_optional::kit_use!(layer as kit_layer);
enderio_optional::kit_use!(window as kit_window);
});

pub(in crate::game) const WIDGETS: &str = "enderio:gui/40/widgetsv2";
const EC_WIDGETS: &str = "enderio:gui/endercore_widgets";
const OVERLAY: &str = "enderio:gui/overlay";

// IconEIO (widgetsv2, 256-space UVs).
pub(in crate::game) const ICON_RS: [[f32; 2]; 4] = [[96.0, 224.0], [80.0, 224.0], [64.0, 224.0], [112.0, 224.0]];
pub(in crate::game) const ICON_IO_UP: [f32; 2] = [16.0, 224.0];
pub(in crate::game) const ICON_IO_DOWN: [f32; 2] = [32.0, 224.0];
pub(in crate::game) const ICON_CAPACITOR: [f32; 2] = [208.0, 160.0];
const ICON_ALLOY: [[f32; 2]; 3] = [[16.0, 144.0], [32.0, 144.0], [0.0, 144.0]];
const ICON_WHATSIT: [f32; 2] = [128.0, 208.0];
// EnderWidget (EnderCore widgets.png).
pub(in crate::game) const BUTTON: [f32; 2] = [0.0, 208.0];
pub(in crate::game) const BUTTON_HIGHLIGHT: [f32; 2] = [16.0, 208.0];
const BUTTON_DOWN: [f32; 2] = [48.0, 208.0];
const BUTTON_DOWN_HIGHLIGHT: [f32; 2] = [64.0, 208.0];

pub(in crate::game) fn commas(v: i64) -> String {
    let s = v.unsigned_abs().to_string();
    let mut out = String::new();
    for (i, c) in s.chars().enumerate() {
        if i > 0 && (s.len() - i) % 3 == 0 {
            out.push(',');
        }
        out.push(c);
    }
    if v < 0 { format!("-{out}") } else { out }
}

pub(in crate::game) fn lang(key: &str, english: &str) -> String {
    horizon_client::lang::get_or(key, english)
}

/// LangPower.RFt: "%s µI/t".
pub(in crate::game) fn rft(v: i64) -> String {
    format!("{} µI/t", commas(v))
}

pub(in crate::game) fn ticks() -> u64 {
    static START: std::sync::OnceLock<std::time::Instant> = std::sync::OnceLock::new();
    (START.get_or_init(std::time::Instant::now).elapsed().as_millis() / 50) as u64
}

pub(in crate::game) fn inside(r: [f32; 4], at: [f32; 2]) -> bool {
    at[0] >= r[0] && at[0] < r[0] + r[2] && at[1] >= r[1] && at[1] < r[1] + r[3]
}

pub(in crate::game) fn spr(game: &Game<'_>, batch: &mut HudBatch, tex: &str, o: [f32; 2], at: [f32; 2], uv: [f32; 4]) {
    game.native_sprite(batch, tex, uv, [o[0] + at[0], o[1] + at[1], uv[2], uv[3]]);
}

/// A widgetsv2 icon (its PNG is 512 for a 256 map), drawn `size` big.
pub(in crate::game) fn icon(game: &Game<'_>, batch: &mut HudBatch, o: [f32; 2], at: [f32; 2], uv: [f32; 2], wh: [f32; 2], size: [f32; 2]) {
    game.native_sprite(batch, WIDGETS, [uv[0] * 2.0, uv[1] * 2.0, wh[0] * 2.0, wh[1] * 2.0], [o[0] + at[0], o[1] + at[1], size[0], size[1]]);
}

pub(in crate::game) fn frame(game: &Game<'_>, batch: &mut HudBatch, o: [f32; 2], at: [f32; 2], uv: [f32; 2]) {
    ec(game, batch, o, at, [uv[0], uv[1], 16.0, 16.0]);
}

/// An EnderCore widget (EnderWidget's map is 256 square; its PNG is 512, so the texture's pixels are twice the map's).
pub(in crate::game) fn ec(game: &Game<'_>, batch: &mut HudBatch, o: [f32; 2], at: [f32; 2], uv: [f32; 4]) {
    game.native_sprite(batch, EC_WIDGETS, [uv[0] * 2.0, uv[1] * 2.0, uv[2] * 2.0, uv[3] * 2.0], [o[0] + at[0], o[1] + at[1], uv[2], uv[3]]);
}

// An EnderCore widget stretched into `dst` (IWidgetMap.render with a width and height, as ItemSettings draws the
// 32 x 16 priority field from the 16 x 16 BUTTON_DOWN).
pub(in crate::game) fn ec_stretch(game: &Game<'_>, batch: &mut HudBatch, o: [f32; 2], uv: [f32; 4], dst: [f32; 4]) {
    game.native_sprite(batch, EC_WIDGETS, [uv[0] * 2.0, uv[1] * 2.0, uv[2] * 2.0, uv[3] * 2.0], [o[0] + dst[0], o[1] + dst[1], dst[2], dst[3]]);
}

// Part of a GUI texture drawn at `alpha` (GhostSlotHandler.drawGhostSlotGrayout: the background over a ghost item
// at 0.75).
pub(in crate::game) fn spr_alpha(game: &Game<'_>, batch: &mut HudBatch, tex: &str, o: [f32; 2], at: [f32; 2], uv: [f32; 4], alpha: f32) {
    let Some(mut r) = game.gui.atlas.sprites.get(tex).copied() else { return };
    let (du, dv) = ((r.u1 - r.u0) / r.w as f32, (r.v1 - r.v0) / r.h as f32);
    r.u0 += uv[0] * du;
    r.v0 += uv[1] * dv;
    r.u1 = r.u0 + uv[2] * du;
    r.v1 = r.v0 + uv[3] * dv;
    batch.rect_uv(o[0] + at[0], o[1] + at[1], uv[2], uv[3], r, rgba(255, 255, 255, (alpha.clamp(0.0, 1.0) * 255.0).round() as u8));
}

pub(in crate::game) fn fill(game: &Game<'_>, batch: &mut HudBatch, o: [f32; 2], r: [f32; 4], argb: u32) {
    game.native_fill(batch, [o[0] + r[0], o[1] + r[1], r[2], r[3]], argb);
}

pub(in crate::game) fn text(game: &Game<'_>, batch: &mut HudBatch, s: &str, x: f32, y: f32, argb: u32, shadow: bool) {
    let [a, r, g, b] = argb.to_be_bytes();
    batch.text(&game.gui.atlas, s, x, y, rgba(r, g, b, a), shadow);
}

pub(in crate::game) fn width(game: &Game<'_>, s: &str) -> f32 {
    HudBatch::text_width(&game.gui.atlas, s)
}

// ---- The client's screen state (one screen open at a time)

#[derive(Default)]
struct Ui {
    pos: Option<Pos>,
    block: String,
    data: Vec<i32>,
    /// The redstone picker open: 0 the machine's, 1 the bank's input, 2 its output.
    picker: Option<usize>,
    io_open: bool,
    io_since: Option<std::time::Instant>,
    yaw: f32,
    pitch: f32,
    distance: f32,
    neighbours: bool,
    dragging: bool,
    hover_face: Option<usize>,
    field: Option<usize>,
    texts: [String; 2],
    settings_seen: i32,
}

thread_local! {
    static UI: RefCell<Ui> = RefCell::new(Ui::default());
}

/// The state for the open screen: fresh when another block's screen opened.
fn sync(game: &Game<'_>, block: &str) {
    let pos = game.native_screen_pos();
    UI.with_borrow_mut(|u| {
        if u.pos != pos || u.block != block {
            *u = Ui { pos, block: block.to_owned(), distance: 5.0, neighbours: false, settings_seen: -1, ..Ui::default() };
        }
    });
}

/// Sends a menu button press to the server (ContainerButtonClick), from a hook that can't return one.
pub(in crate::game) fn send_button(game: &Game<'_>, id: i32) {
    let Some(container) = game.remote.as_ref().and_then(|r| r.menu.as_ref()).map(|(w, _)| *w) else { return };
    game.send(horizon_protocol::packets::play::Serverbound::ContainerButtonClick(horizon_protocol::packets::play::ContainerButtonClick { container, button: id }));
}

fn ours(block: &str) -> bool {
    mach(block).is_some() || bank(block).is_some()
}

fn has_redstone_button(m: &Mach) -> bool {
    m.level > 0 && m.kind != Kind::Enchanter
}

// GuiMachineBase: the side buttons at xSize - 5 - 16 (the wired charger is 218 wide: 174); the enchanter has none.
fn side_x(block: &str) -> f32 {
    if mach(block).is_some_and(|m| m.kind == Kind::Charger) { 174.0 } else { 155.0 }
}
fn rs_button(block: &str) -> [f32; 4] {
    [side_x(block), 5.0, 16.0, 16.0]
}
fn io_button_of(block: &str) -> [f32; 4] {
    [side_x(block), 24.0, 16.0, 16.0]
}
fn has_io_button(block: &str) -> bool {
    !mach(block).is_some_and(|m| m.kind == Kind::Enchanter)
}
/// GuiOverlayIoConfig: (5 + left offset, ySize - 85, xSize - right offset - 10, 80); the charger's offsets are 21 and 42.
pub(in crate::game) fn overlay_rect(block: &str) -> [f32; 4] {
    if mach(block).is_some_and(|m| m.kind == Kind::Charger) { [26.0, 81.0, 166.0, 80.0] } else { OVERLAY_RECT }
}

const RS_BUTTON: [f32; 4] = [155.0, 5.0, 16.0, 16.0];
const IO_BUTTON: [f32; 4] = [155.0, 24.0, 16.0, 16.0];
const MODE_BUTTON: [f32; 4] = [155.0, 62.0, 16.0, 16.0];
const BANK_IN_RS: [f32; 4] = [152.0, 18.0, 16.0, 16.0];
const BANK_OUT_RS: [f32; 4] = [152.0, 36.0, 16.0, 16.0];
const BANK_IO: [f32; 4] = [152.0, 56.0, 16.0, 16.0];
const FIELDS: [[f32; 4]; 2] = [[80.0, 18.0, 68.0, 16.0], [80.0, 36.0, 68.0, 16.0]];
/// GuiOverlayIoConfig on a 176x166 screen: (5, ySize - 85, xSize - 10, 80).
const OVERLAY_RECT: [f32; 4] = [5.0, 81.0, 166.0, 80.0];

// ---- Layout

fn machine_slots(m: &Mach) -> Vec<Option<[f32; 2]>> {
    let all: Vec<[f32; 2]> = match m.kind {
        Kind::Alloy => vec![[54.0, 17.0], [79.0, 7.0], [103.0, 17.0], [79.0, 57.0], [12.0, 60.0]],
        Kind::Sag => vec![[80.0, 12.0], [122.0, 23.0], [49.0, 59.0], [70.0, 59.0], [91.0, 59.0], [112.0, 59.0], [12.0, 60.0]],
        Kind::Stirling => vec![[80.0, 34.0], [12.0, 60.0]],
        _ => return super::enderio_machines::slots(m),
    };
    (0..m.slots).map(|i| if m.kind == Kind::Sag && m.level == 0 && i == 1 { None } else { all.get(i).copied() }).collect()
}

pub fn screen(block: &str, out: &mut Option<super::native_hooks::NativeScreen>) {
    if block == super::enderio_conduits::BUNDLE {
        super::enderio_conduits::screen(block, out);
        return;
    }
    let slots = if let Some(m) = mach(block) {
        machine_slots(m)
    } else if bank(block).is_some() {
        vec![None]
    } else {
        return;
    };
    // GuiWiredCharger: 218 wide, the inventory at (29, 84).
    let wide = mach(block).is_some_and(|m| m.kind == Kind::Charger);
    *out = Some(super::native_hooks::NativeScreen { width: if wide { 218.0 } else { 176.0 }, height: 166.0, slots, inventory_y: 84.0, inventory_x: if wide { 29.0 } else { 8.0 }, own_labels: true, own_background: true });
}

fn texture(m: &Mach, mode: i32) -> &'static str {
    match (m.profile, m.level) {
        (Profile::SimpleAlloy, _) => "enderio:gui/40/simple_alloy_smelter",
        (Profile::SimpleFurnace, _) => "enderio:gui/40/simple_furnace",
        (Profile::Auto, _) => ["enderio:gui/40/alloy_smelter_auto", "enderio:gui/40/alloy_smelter_alloy", "enderio:gui/40/alloy_smelter_furnace"][mode.clamp(0, 2) as usize],
        (Profile::Sag, 0) => "enderio:gui/40/crusher_light",
        (Profile::Sag, _) => "enderio:gui/40/crusher",
        (Profile::Stirling, 0) => "enderio:gui/40/simple_stirling_generator",
        (Profile::Stirling, _) => "enderio:gui/40/stirling_generator",
        (Profile::Other, _) => super::enderio_machines::texture(m),
    }
}

pub(in crate::game) fn progress(data: &[i32]) -> Option<f32> {
    let p = data[D_PROGRESS];
    (p > 0).then(|| (p - 1) as f32 / 10000.0)
}

// ---- Drawing

/// PowerBar: bottom-up, the pulsing strip and two shimmer passes; no capacitor, the error strip and a capacitor.
pub(in crate::game) fn power_bar(game: &Game<'_>, batch: &mut HudBatch, o: [f32; 2], r: [f32; 4], stored: f64, max: f64, cap: bool) {
    let t = ticks();
    let [x, y, w, hh] = r;
    if !cap {
        let frame = ((t >> 2) % 5) as f32;
        let dy = y + 16.0 - 6.0;
        spr(game, batch, OVERLAY, o, [x, dy + hh - 24.0], [160.0 + frame * 9.0, 128.0, w, 24.0]);
        let bob = ((t as f32) / 10.0).sin() * 4.0;
        let st = horizon_client::items::ItemStack::new("enderio:basic_capacitor", 1);
        game.draw_item(batch, &st, o[0] + x + (w / 2.0).floor() - 8.0, o[1] + dy + hh - 24.0 - 13.0 + bob);
        return;
    }
    let h = if max <= 0.0 || stored <= 0.0 { 0.0 } else { (hh * (stored / max) as f32).round().clamp(1.0, hh) };
    if h <= 0.0 {
        return;
    }
    let mut frame = ((t / 3) % 20) as f32;
    if frame > 10.0 {
        frame = 20.0 - frame;
    }
    let top = y + hh - h;
    spr(game, batch, OVERLAY, o, [x, top], [frame * 10.0, 128.0, w, h]);
    let mut dy = top;
    let mut left = h;
    let shimmer = |batch: &mut HudBatch, dy: f32, bh: f32, speed: u64| {
        let mut u = 128.0;
        let mut v = 128.0 + ((t / speed) % 128) as f32;
        if v + bh > 255.0 {
            u += 10.0;
            v -= 64.0;
        }
        spr(game, batch, OVERLAY, o, [x, dy], [u, v, w, bh]);
    };
    while left > 64.0 {
        shimmer(batch, dy, 64.0, 1);
        shimmer(batch, dy, 64.0, 5);
        dy += 64.0;
        left -= 64.0;
    }
    shimmer(batch, dy, left, 1);
    shimmer(batch, dy, left, 5);
}

pub(in crate::game) fn local_cursor(game: &Game<'_>, o: [f32; 2]) -> [f32; 2] {
    let c = game.native_cursor();
    [c[0] - o[0], c[1] - o[1]]
}

pub(in crate::game) fn button(game: &Game<'_>, batch: &mut HudBatch, o: [f32; 2], r: [f32; 4], down: bool, mouse: [f32; 2]) {
    let hover = inside(r, mouse);
    let uv = match (down, hover) {
        (false, false) => BUTTON,
        (false, true) => BUTTON_HIGHLIGHT,
        (true, false) => BUTTON_DOWN,
        (true, true) => BUTTON_DOWN_HIGHLIGHT,
    };
    frame(game, batch, o, [r[0], r[1]], uv);
}

pub(in crate::game) fn screen_draw(game: &Game<'_>, block: &str, data: &[i32], batch: &mut HudBatch, origin: [f32; 2]) {
    if block == super::enderio_conduits::BUNDLE {
        super::enderio_conduits::screen_draw(game, data, batch, origin);
        return;
    }
    if !ours(block) || data.len() < DATA_SLOTS {
        return;
    }
    sync(game, block);
    UI.with_borrow_mut(|u| u.data = data.to_vec());
    let mouse = local_cursor(game, origin);
    let (io_open, picker) = UI.with_borrow(|u| (u.io_open, u.picker));
    if let Some(m) = mach(block) {
        if m.profile == Profile::Other {
            super::enderio_machines::draw(game, m, data, batch, origin, mouse);
        } else {
            draw_machine(game, m, data, batch, origin, mouse);
        }
        let rsb = rs_button(block);
        if has_redstone_button(m) {
            button(game, batch, origin, rsb, false, mouse);
            icon(game, batch, origin, [rsb[0], rsb[1]], ICON_RS[data[D_REDSTONE].clamp(0, 3) as usize], [16.0, 16.0], [16.0, 16.0]);
        }
        let iob = io_button_of(block);
        if has_io_button(block) {
            button(game, batch, origin, iob, io_open, mouse);
            icon(game, batch, origin, [iob[0], iob[1]], if io_open { ICON_IO_DOWN } else { ICON_IO_UP }, [16.0, 16.0], [16.0, 16.0]);
        }
        if m.profile == Profile::Auto {
            button(game, batch, origin, MODE_BUTTON, false, mouse);
            icon(game, batch, origin, [MODE_BUTTON[0] + 2.0, MODE_BUTTON[1] + 2.0], ICON_ALLOY[data[D_MODE].clamp(0, 2) as usize], [16.0, 16.0], [12.0, 12.0]);
        }
        if io_open {
            draw_io_overlay(game, block, data, batch, origin, mouse);
            draw_io_tints(game, m, data, batch, origin);
        }
        if picker == Some(0) {
            draw_picker(game, batch, origin, rsb, data[D_REDSTONE]);
        }
    } else {
        draw_bank(game, data, batch, origin, mouse);
        if io_open {
            draw_io_overlay(game, block, data, batch, origin, mouse);
        }
        match picker {
            Some(1) => draw_picker(game, batch, origin, BANK_IN_RS, data[D_IN_MODE]),
            Some(2) => draw_picker(game, batch, origin, BANK_OUT_RS, data[D_OUT_MODE]),
            _ => {}
        }
    }
}

fn draw_machine(game: &Game<'_>, m: &Mach, data: &[i32], batch: &mut HudBatch, o: [f32; 2], mouse: [f32; 2]) {
    let tex = texture(m, data[D_MODE]);
    spr(game, batch, tex, o, [0.0, 0.0], [0.0, 0.0, 176.0, 166.0]);
    let p = progress(data);
    match m.kind {
        Kind::Alloy => {
            if let Some(p) = p {
                // Two flames: s = progress(14) + 1, from the bottom.
                let s = ((p * 14.0) as i32 + 1) as f32;
                spr(game, batch, tex, o, [55.0, 49.0 - s], [176.0, 14.0 - s, 14.0, s]);
                spr(game, batch, tex, o, [103.0, 49.0 - s], [176.0, 14.0 - s, 14.0, s]);
            }
        }
        Kind::Sag => {
            if let Some(p) = p {
                let s = ((p * 24.0) as i32 + 1) as f32;
                spr(game, batch, tex, o, [79.0, 31.0], [200.0, 0.0, 18.0, s]);
            }
            if m.level > 0 && data[D_BALL] > 0 {
                let f = (data[D_BALL] - 1) as f32 / 1000.0;
                let h = ((16.0 * f) as i32).clamp(0, 16) as f32;
                if h > 0.0 {
                    spr(game, batch, tex, o, [142.0, 23.0 + (16.0 - h)], [186.0, 31.0, 4.0, h]);
                }
            }
        }
        Kind::Stirling => {
            if let Some(p) = p {
                let s = ((p * 12.0) as i32) as f32;
                spr(game, batch, tex, o, [80.0, 64.0 - s], [176.0, 12.0 - s, 14.0, s + 2.0]);
            }
            // "Generating: %s µI/t" and "Efficiency: %s%", centred, white with a shadow.
            let rate = if data[D_ACTIVE] != 0 { i64::from(get2(data, D_USE)) } else { 0 };
            let line = lang("enderio.gui.stirling_generator.output", "Generating: %s").replacen("%s", &rft(rate), 1);
            text(game, batch, &line, o[0] + 88.0 - (width(game, &line) / 2.0).floor(), o[1] + 7.0, 0xFFFF_FFFF, true);
            let eff = (data[D_EFFICIENCY] as f32 / 10.0).round() as i32;
            let line = lang("enderio.gui.stirling_generator.efficiency", "Efficiency: %s%%").replacen("%s", &eff.to_string(), 1).replace("%%", "%");
            text(game, batch, &line, o[0] + 88.0 - (width(game, &line) / 2.0).floor(), o[1] + 19.0, 0xFFFF_FFFF, true);
        }
        _ => {}
    }
    if let Some(cs) = m.cap {
        let _ = cs;
        if data[D_CAP] == 0 {
            icon(game, batch, o, [12.0, 60.0], ICON_CAPACITOR, [16.0, 16.0], [16.0, 16.0]);
        }
    }
    power_bar(game, batch, o, [16.0, 14.0, 9.0, 42.0], f64::from(get2(data, D_ENERGY)), f64::from(get2(data, D_MAX)), data[D_CAP] != 0);
    let _ = mouse;
}

/// GuiMachineBase.renderSlotHighlights: the hovered face's mode tints the inputs blue and the outputs orange.
fn draw_io_tints(game: &Game<'_>, m: &Mach, data: &[i32], batch: &mut HudBatch, o: [f32; 2]) {
    let Some(f) = UI.with_borrow(|u| u.hover_face) else { return };
    let mode = data[D_FACES + f];
    if m.profile == Profile::Other {
        // The machines' own highlight rectangles (tanks, big output slots).
        for (r, pull) in super::enderio_machines::tint_rects(m) {
            let on = if pull { mode == IO_PULL || mode == IO_PUSH_PULL } else { mode == IO_PUSH || mode == IO_PUSH_PULL };
            if on {
                fill(game, batch, o, r, if pull { 0x801A_66CC } else { 0x80CC_661A });
            }
        }
        return;
    }
    let slots = machine_slots(m);
    if mode == IO_PULL || mode == IO_PUSH_PULL {
        for &i in m.inputs {
            if let Some(Some(p)) = slots.get(i) {
                fill(game, batch, o, [p[0], p[1], 16.0, 16.0], 0x801A_66CC);
            }
        }
    }
    if mode == IO_PUSH || mode == IO_PUSH_PULL {
        for &i in m.outputs {
            if m.kind == Kind::Alloy {
                fill(game, batch, o, [75.0, 54.0, 24.0, 24.0], 0x80CC_661A);
            } else if let Some(Some(p)) = slots.get(i) {
                fill(game, batch, o, [p[0], p[1], 16.0, 16.0], 0x80CC_661A);
            }
        }
    }
}

/// CycleButton's picker: a 2x2 grid of 17-pixel cells, the current mode's under the button.
pub(in crate::game) fn picker_bounds(btn: [f32; 4], current: i32) -> [f32; 4] {
    let c = current.clamp(0, 3) as f32;
    let (cx, cy) = ((c % 2.0) * 17.0, 3.0 + (c / 2.0).floor() * 17.0);
    [btn[0] - cx, btn[1] - cy, 38.0, 38.0]
}

pub(in crate::game) fn draw_picker(game: &Game<'_>, batch: &mut HudBatch, o: [f32; 2], btn: [f32; 4], current: i32) {
    let b = picker_bounds(btn, current);
    fill(game, batch, o, [b[0] - 2.0, b[1] + 1.0, b[2] - 1.0, b[3] - 1.0], 0xFF40_4040);
    fill(game, batch, o, [b[0] - 1.0, b[1] + 2.0, b[2] - 3.0, b[3] - 3.0], 0xFF80_8080);
    for i in 0..4 {
        let at = [b[0] + (i % 2) as f32 * 17.0, b[1] + 3.0 + (i / 2) as f32 * 17.0];
        frame(game, batch, o, at, if i == current { BUTTON_DOWN_HIGHLIGHT } else { BUTTON });
        icon(game, batch, o, at, ICON_RS[i as usize], [16.0, 16.0], [16.0, 16.0]);
    }
}

pub(in crate::game) fn picker_cell(btn: [f32; 4], current: i32, at: [f32; 2]) -> Option<i32> {
    let b = picker_bounds(btn, current);
    (0..4).find(|i| inside([b[0] + (i % 2) as f32 * 17.0, b[1] + 3.0 + (i / 2) as f32 * 17.0, 17.0, 17.0], at))
}

fn draw_bank(game: &Game<'_>, data: &[i32], batch: &mut HudBatch, o: [f32; 2], mouse: [f32; 2]) {
    let tex = "enderio:gui/40/capacitor_bank";
    spr(game, batch, tex, o, [0.0, 0.0], [0.0, 0.0, 176.0, 166.0]);
    power_bar(game, batch, o, [8.0, 9.0, 9.0, 68.0], get4(data, D_NET_ENERGY) as f64, get4(data, D_NET_MAX) as f64, true);
    // "Max I/O <n> µI/t" centred at 5; the fields' labels right-aligned to 78; white, no shadow.
    let line = format!("{} {}", lang("enderio.gui.cap_bank.max_io", "Max I/O"), rft(i64::from(get2(data, D_MAX_IO))));
    text(game, batch, &line, o[0] + 88.0 - (width(game, &line) / 2.0).floor(), o[1] + 5.0, 0xFFFF_FFFF, false);
    for (i, key, english) in [(0usize, "enderio.gui.cap_bank.max_input", "Max Input"), (1, "enderio.gui.cap_bank.max_output", "Max Output")] {
        let label = format!("{}:", lang(key, english));
        text(game, batch, &label, o[0] + 78.0 - width(game, &label), o[1] + FIELDS[i][1] + 2.0, 0xFFFF_FFFF, false);
    }
    // The text fields (TextFieldEnder): filled with the network's values once they arrive, and after each change.
    let seen = data[D_SETTINGS_SEEN];
    UI.with_borrow_mut(|u| {
        if u.settings_seen != seen && u.field.is_none() {
            u.settings_seen = seen;
            u.texts = [get2(data, D_MAX_IN).to_string(), get2(data, D_MAX_OUT).to_string()];
        }
    });
    let (texts, focus) = UI.with_borrow(|u| (u.texts.clone(), u.field));
    for i in 0..2 {
        let r = FIELDS[i];
        fill(game, batch, o, [r[0] - 1.0, r[1] - 1.0, r[2] + 2.0, r[3] + 2.0], 0xFFA0_A0A0);
        fill(game, batch, o, r, 0xFF00_0000);
        let mut s = texts[i].clone();
        if focus == Some(i) && (ticks() / 6) % 2 == 0 {
            s.push('_');
        }
        text(game, batch, &s, o[0] + r[0] + 4.0, o[1] + r[1] + (r[3] - 8.0) / 2.0, 0xFFE0_E0E0, true);
    }
    let io_open = UI.with_borrow(|u| u.io_open);
    for (r, mode) in [(BANK_IN_RS, data[D_IN_MODE]), (BANK_OUT_RS, data[D_OUT_MODE])] {
        button(game, batch, o, r, false, mouse);
        icon(game, batch, o, [r[0], r[1]], ICON_RS[mode.clamp(0, 3) as usize], [16.0, 16.0], [16.0, 16.0]);
    }
    button(game, batch, o, BANK_IO, io_open, mouse);
    icon(game, batch, o, [BANK_IO[0], BANK_IO[1]], if io_open { ICON_IO_DOWN } else { ICON_IO_UP }, [16.0, 16.0], [16.0, 16.0]);
}

// ---- The IO overlay (GuiOverlayIoConfig, IoConfigRenderer): the block seen from where the player looks

fn overlay_view(yaw: f32, pitch: f32, distance: f32) -> horizon_client::mod_api::NativeSceneView {
    // Perspective FOV 30 at `distance` shows 2 * d * tan(15) blocks over the overlay's 80 pixels.
    let scale = OVERLAY_RECT[3] / (2.0 * distance * 15f32.to_radians().tan());
    horizon_client::mod_api::NativeSceneView { center: [0.5, 0.5, 0.5], yaw, pitch, scale }
}

/// The face of the block (0..1 cube) under `at` (overlay-local), the one nearest the viewer.
fn pick_face(view: &horizon_client::mod_api::NativeSceneView, at: [f32; 2]) -> Option<usize> {
    let r = glam::Mat3::from_rotation_x(-view.pitch.to_radians()) * glam::Mat3::from_rotation_y(view.yaw.to_radians());
    let rt = r.transpose();
    let mid = [OVERLAY_RECT[2] / 2.0, OVERLAY_RECT[3] / 2.0];
    let s = -view.scale;
    let base = glam::Vec3::from_array(view.center) + rt * glam::Vec3::new((at[0] - mid[0]) / s, (at[1] - mid[1]) / s, 0.0);
    let dir = rt * glam::Vec3::new(0.0, 0.0, 1.0 / s);
    let mut best: Option<(f32, usize)> = None;
    // (axis, plane, face)
    for (axis, plane, face) in [(0usize, 0.0f32, 4usize), (0, 1.0, 5), (1, 0.0, 0), (1, 1.0, 1), (2, 0.0, 2), (2, 1.0, 3)] {
        let d = dir[axis];
        if d.abs() < 1e-6 {
            continue;
        }
        let z = (plane - base[axis]) / d;
        let p = base + dir * z;
        let ok = (0..3).filter(|i| *i != axis).all(|i| (-0.0001..=1.0001).contains(&p[i]));
        if ok && best.is_none_or(|(bz, _)| z > bz) {
            best = Some((z, face));
        }
    }
    best.map(|(_, f)| f)
}

fn init_view(game: &Game<'_>) {
    let cam = game.native_camera();
    UI.with_borrow_mut(|u| {
        if let Some(c) = cam {
            u.yaw = c.yaw;
            u.pitch = c.pitch.clamp(-80.0, 80.0);
        }
        u.distance = 5.0;
        u.io_since = Some(std::time::Instant::now());
    });
}

fn mode_name(block: &str, mode: i32) -> String {
    let banked = bank(block).is_some();
    match mode {
        IO_PULL if banked => lang("enderio.gui.cap_bank.input_mode", "Input"),
        IO_PUSH if banked => lang("enderio.gui.cap_bank.output_mode", "Output"),
        IO_PULL => lang("enderio.gui.machine.ioMode.pull", "Pull"),
        IO_PUSH => lang("enderio.gui.machine.ioMode.push", "Push"),
        IO_PUSH_PULL => lang("enderio.gui.machine.ioMode.pullPush", "Push / Pull"),
        IO_DISABLED => lang("enderio.gui.machine.ioMode.disabled", "Disabled"),
        _ => lang("enderio.gui.machine.ioMode.none", "None"),
    }
}

fn draw_io_overlay(game: &Game<'_>, block: &str, data: &[i32], batch: &mut HudBatch, o: [f32; 2], mouse: [f32; 2]) {
    let r = overlay_rect(block);
    fill(game, batch, o, r, 0xFF00_0000);
    let Some(pos) = game.native_screen_pos() else { return };
    let (yaw, pitch, distance, neighbours) = UI.with_borrow(|u| (u.yaw, u.pitch, u.distance, u.neighbours));
    let view = overlay_view(yaw, pitch, distance);
    let mut blocks = vec![horizon_client::mod_api::NativeSceneBlock { pos: [0, 0, 0], state: game.world.block(pos[0], pos[1], pos[2]), entity: None }];
    if neighbours {
        for f in 0..6 {
            let p = step(pos, f);
            blocks.push(horizon_client::mod_api::NativeSceneBlock { pos: step([0, 0, 0], f), state: game.world.block(p[0], p[1], p[2]), entity: None });
        }
    }
    let rect = [o[0] + r[0], o[1] + r[1], o[0] + r[0] + r[2], o[1] + r[1] + r[3]];
    game.native_draw_scene(batch, rect, &blocks, view);
    let local = [mouse[0] - r[0], mouse[1] - r[1]];
    let face = if inside([0.0, 0.0, r[2], r[3]], local) { pick_face(&view, local) } else { None };
    UI.with_borrow_mut(|u| u.hover_face = face);
    // IO_WHATSIT in the bottom right corner, a hover quad over it.
    let w = [r[0] + r[2] - 17.0, r[1] + r[3] - 18.0];
    if inside([w[0], w[1], 16.0, 16.0], mouse) {
        fill(game, batch, o, [w[0], w[1], 16.0, 16.0], 0x9966_6666);
    }
    icon(game, batch, o, w, ICON_WHATSIT, [16.0, 16.0], [16.0, 16.0]);
    if let Some(f) = face {
        // The selected face's mode, at the bottom left, with its icon above it.
        let mode = data[D_FACES + f];
        let name = mode_name(block, mode);
        let y = r[1] + r[3] - 9.0 - 2.0;
        text(game, batch, &format!("{}: {}", lang(&format!("enderio.top.machine.side.{}", FACE_NAMES[f]), FACE_NAMES[f]), name), o[0] + r[0] + 4.0, o[1] + y, 0xFFFF_FFFF, false);
        let (uv, wh, raise) = match mode {
            IO_PULL => ([32.0, 176.0], [16.0, 8.0], 0.0),
            IO_PUSH => ([48.0, 176.0], [16.0, 8.0], 0.0),
            IO_PUSH_PULL => ([32.0, 176.0], [32.0, 8.0], 0.0),
            IO_DISABLED => ([64.0, 176.0], [16.0, 16.0], 5.0),
            _ => ([0.0, 0.0], [0.0, 0.0], 0.0),
        };
        if wh[0] > 0.0 {
            icon(game, batch, o, [r[0] + 4.0, y - wh[1] - 1.0 - raise], uv, wh, wh);
        }
    }
}

// ---- Tooltips

pub(in crate::game) fn screen_tooltip(game: &Game<'_>, block: &str, data: &[i32], at: [f32; 2], lines: &mut Vec<String>) {
    if block == super::enderio_conduits::BUNDLE {
        super::enderio_conduits::screen_tooltip(game, data, at, lines);
        return;
    }
    if !ours(block) || data.len() < DATA_SLOTS || UI.with_borrow(|u| u.picker.is_some()) {
        return;
    }
    let io_open = UI.with_borrow(|u| u.io_open);
    let rs_line = |mode: i32| -> Vec<String> {
        let key = ["ignore", "on", "off", "never"][mode.clamp(0, 3) as usize];
        let english = ["Always active", "Active with signal", "Active without signal", "Never active"][mode.clamp(0, 3) as usize];
        vec![lang("enderio.gui.tooltip.redstoneControlMode", "Redstone Mode"), lang(&format!("enderio.gui.tooltip.redstoneControlMode.{key}"), english)]
    };
    let io_lines = || -> Vec<String> {
        if io_open {
            vec![
                lang("enderio.gui.generic.ioMode.overlay.tooltip.visible.line1", "Configure IO"),
                lang("enderio.gui.generic.ioMode.overlay.tooltip.visible.line2", "Shift click to reset all sides"),
                lang("enderio.gui.generic.ioMode.overlay.tooltip.visible.line3", "to their default setting"),
            ]
        } else {
            vec![lang("enderio.gui.generic.ioMode.overlay.tooltip", "Configure IO")]
        }
    };
    if io_open && inside(overlay_rect(block), at) {
        return;
    }
    if let Some(m) = mach(block) {
        if m.profile == Profile::Other {
            if has_redstone_button(m) && inside(rs_button(block), at) {
                lines.extend(rs_line(data[D_REDSTONE]));
            } else if has_io_button(block) && inside(io_button_of(block), at) {
                lines.extend(io_lines());
            } else {
                super::enderio_machines::tooltip_lines(m, data, at, lines);
            }
            return;
        }
        let progress_line = |p: f32| lang("enderio.gui.generic.progress", "Progress %s%%").replacen("%s", &((p * 100.0) as i32).to_string(), 1).replace("%%", "%");
        if has_redstone_button(m) && inside(RS_BUTTON, at) {
            lines.extend(rs_line(data[D_REDSTONE]));
        } else if inside(IO_BUTTON, at) {
            lines.extend(io_lines());
        } else if m.profile == Profile::Auto && inside(MODE_BUTTON, at) {
            let key = ["all", "alloy", "furnace"][data[D_MODE].clamp(0, 2) as usize];
            let english = ["All Smelting", "Alloys Only", "Furnace Only"][data[D_MODE].clamp(0, 2) as usize];
            lines.push(lang("enderio.gui.alloy.mode.heading", "Furnace Mode"));
            lines.push(lang(&format!("enderio.gui.alloy.mode.{key}"), english));
        } else if inside([16.0, 14.0, 9.0, 42.0], at) {
            if data[D_CAP] == 0 {
                lines.push(lang("enderio.gui.generic.nocap.line1", "§3Capacitor missing"));
                lines.push(lang("enderio.gui.generic.nocap.line2", "Insert any capacitor so"));
                lines.push(lang("enderio.gui.generic.nocap.line3", "this machine can work!"));
            } else {
                lines.push(lang("enderio.gui.generic.max", "Max: %s").replacen("%s", &rft(i64::from(get2(data, D_USE))), 1));
                let loss = get2(data, D_LOSS) as f32 / 10000.0;
                if loss > 0.0 {
                    lines.push(lang("enderio.gui.generic.loss", "Leakage: %s").replacen("%s", &format!("{loss:.2} µI/t"), 1));
                }
                lines.push(format!("{} / {} µI", commas(i64::from(get2(data, D_ENERGY))), commas(i64::from(get2(data, D_MAX)))));
            }
        } else if let Some(p) = progress(data) {
            let rects: &[[f32; 4]] = match m.kind {
                Kind::Alloy => &[[55.0, 35.0, 14.0, 14.0], [103.0, 35.0, 14.0, 14.0]],
                Kind::Sag => &[[79.0, 31.0, 18.0, 24.0]],
                Kind::Stirling => &[[80.0, 52.0, 14.0, 14.0]],
                _ => &[],
            };
            if rects.iter().any(|r| inside(*r, at)) {
                if m.kind == Kind::Stirling {
                    // "Remaining {0,%}|{m}m {s}s|{RF} µI": secs = ticks / 20 + sign(ticks % 20).
                    let left = get2(data, D_BURN);
                    let secs = left / 20 + i32::from(left % 20 > 0);
                    lines.push(format!("Remaining {}%", (p * 100.0).round() as i32));
                    lines.push(if secs / 60 > 0 { format!("{}m {}s", secs / 60, secs % 60) } else { format!("{}s", secs % 60) });
                    lines.push(format!("{} µI", commas(i64::from(get2(data, D_USE)) * i64::from(left))));
                } else {
                    lines.push(progress_line(p));
                }
            }
        }
        if m.kind == Kind::Sag && m.level > 0 && data[D_BALL] > 0 && inside([142.0, 23.0, 5.0, 17.0], at) {
            let left = ((data[D_BALL] - 1) as f32 / 1000.0 * 100.0) as i32;
            lines.push(lang("enderio.grindingball.gui.durability", "Remaining: %s %%").replacen("%s", &left.to_string(), 1).replace("%%", "%"));
            lines.push(lang("enderio.grindingball.tooltip.line1", "%sSAG Mill Grinding Ball").replacen("%s", "§9", 1));
            for (i, k) in [(0usize, "line2"), (1, "line3"), (2, "line4")] {
                let english = ["%sMain Output %s%%", "%sBonus Output %s%%", "%sPower Use %s%%"][i];
                lines.push(lang(&format!("enderio.grindingball.tooltip.{k}"), english).replacen("%s", "§7", 1).replacen("%s", &data[D_BALL_STATS + i].to_string(), 1).replace("%%", "%"));
            }
        }
        if m.kind == Kind::Stirling && m.cap.is_some() && data[D_CAP] == 0 && inside([12.0, 60.0, 16.0, 16.0], at) {
            lines.push(lang("enderio.gui.stirling_generator.upgradeslot", "Upgrade slot"));
            // "{cap} gives §f{x.x}x§7 efficiency", x = the capacitor's burn efficiency over the default (0.8).
            for (i, item) in [(0usize, "double_layer_capacitor"), (1, "octadic_capacitor")] {
                let x = data[D_CAP_EFF + i] as f32 / 800.0;
                let name = lang(&format!("item.enderio.{item}"), item);
                lines.push(format!("{name} gives §f{x:.1}x§7 efficiency"));
            }
        }
        return;
    }
    if inside(BANK_IN_RS, at) {
        lines.extend(rs_line(data[D_IN_MODE]));
    } else if inside(BANK_OUT_RS, at) {
        lines.extend(rs_line(data[D_OUT_MODE]));
    } else if inside(BANK_IO, at) {
        lines.extend(io_lines());
    } else if inside([8.0, 9.0, 9.0, 68.0], at) {
        // GuiCapBank's power bar: "<stored> of", "§f<max> §7µI", the average change.
        lines.push(format!("{} of", commas(get4(data, D_NET_ENERGY))));
        lines.push(format!("§f{} §7µI", commas(get4(data, D_NET_MAX))));
        let avg = get2(data, D_AVG);
        let line = if avg > 0 { format!("§a+{} µI/t", commas(i64::from(avg))) } else if avg < 0 { format!("§c{} µI/t", commas(i64::from(avg))) } else { format!("§f{} µI/t", 0) };
        lines.push(line);
    }
}

// ---- Presses, drags, the wheel, keys: client-side state, and buttons sent to the server

fn current_data() -> Vec<i32> {
    UI.with_borrow(|u| u.data.clone())
}

pub(in crate::game) fn pressed(game: &Game<'_>, screen: &super::native_hooks::NativeScreenView<'_>, button: i32, modifiers: u32, canceled: &mut bool) {
    if screen.menu == Some(super::enderio_conduits::BUNDLE) && !*canceled && (button == 0 || button == 1) {
        let at = [screen.mouse[0] - screen.rect[0], screen.mouse[1] - screen.rect[1]];
        if super::enderio_conduits::pressed(game, at, button == 1) {
            *canceled = true;
        }
        return;
    }
    let Some(block) = screen.menu.filter(|m| ours(m)).map(str::to_owned) else { return };
    if *canceled || !(button == 0 || button == 1) {
        return;
    }
    sync(game, &block);
    let data = current_data();
    if data.len() < DATA_SLOTS {
        return;
    }
    let at = [screen.mouse[0] - screen.rect[0], screen.mouse[1] - screen.rect[1]];
    let shift = modifiers & 1 != 0;
    let right = button == 1;
    // An open picker takes the click; outside it, it closes.
    if let Some(p) = UI.with_borrow(|u| u.picker) {
        let (btn, cur, base) = match p {
            0 => (rs_button(&block), data[D_REDSTONE], B_REDSTONE),
            1 => (BANK_IN_RS, data[D_IN_MODE], B_BANK_IN_RS),
            _ => (BANK_OUT_RS, data[D_OUT_MODE], B_BANK_OUT_RS),
        };
        UI.with_borrow_mut(|u| u.picker = None);
        if inside(picker_bounds(btn, cur), at) {
            *canceled = true;
            if !right {
                if let Some(i) = picker_cell(btn, cur, at) {
                    send_button(game, base + i);
                }
            }
            return;
        }
    }
    let io_open = UI.with_borrow(|u| u.io_open);
    let orect = overlay_rect(&block);
    if io_open && inside(orect, at) {
        *canceled = true;
        let w = [orect[0] + orect[2] - 17.0, orect[1] + orect[3] - 18.0, 16.0, 16.0];
        if right {
            let (face, since) = UI.with_borrow(|u| (u.hover_face, u.io_since));
            if let Some(f) = face.filter(|_| since.is_some_and(|s| s.elapsed().as_millis() > 500)) {
                send_button(game, B_FACE + f as i32);
            }
        } else if inside(w, at) {
            UI.with_borrow_mut(|u| u.neighbours = !u.neighbours);
        } else {
            UI.with_borrow_mut(|u| u.dragging = true);
        }
        return;
    }
    let io_button = if mach(&block).is_some() { io_button_of(&block) } else { BANK_IO };
    if has_io_button(&block) && inside(io_button, at) {
        *canceled = true;
        if right {
            return;
        }
        if shift && io_open {
            send_button(game, B_RESET_IO);
        } else {
            if !io_open {
                init_view(game);
            }
            UI.with_borrow_mut(|u| u.io_open = !u.io_open);
        }
        return;
    }
    if let Some(m) = mach(&block) {
        if has_redstone_button(m) && inside(rs_button(&block), at) && !right {
            *canceled = true;
            UI.with_borrow_mut(|u| u.picker = Some(0));
        } else if m.profile == Profile::Auto && inside(MODE_BUTTON, at) {
            *canceled = true;
            send_button(game, if right { B_MODE_PREV } else { B_MODE_NEXT });
        } else if m.profile == Profile::Other && super::enderio_machines::pressed(game, m, &data, at, right, shift) {
            *canceled = true;
        }
        return;
    }
    if inside(BANK_IN_RS, at) && !right {
        *canceled = true;
        UI.with_borrow_mut(|u| u.picker = Some(1));
    } else if inside(BANK_OUT_RS, at) && !right {
        *canceled = true;
        UI.with_borrow_mut(|u| u.picker = Some(2));
    } else if let Some(i) = (0..2).find(|i| inside(FIELDS[*i], at)) {
        *canceled = true;
        UI.with_borrow_mut(|u| u.field = Some(i));
    } else {
        UI.with_borrow_mut(|u| u.field = None);
    }
}

pub(in crate::game) fn released(canceled: &mut bool) {
    UI.with_borrow_mut(|u| {
        if u.dragging {
            u.dragging = false;
            *canceled = true;
        }
    });
}

pub(in crate::game) fn dragged(game: &Game<'_>, delta: [f32; 2], canceled: &mut bool) {
    let size = game.native_screen_width().zip(game.native_screen_height()).unwrap_or((427.0, 240.0));
    UI.with_borrow_mut(|u| {
        if u.dragging {
            // yaw -= 4 dx 180, pitch += 2 dy 180, normalised to the window.
            u.yaw += 4.0 * delta[0] / size.0.max(1.0) * 180.0;
            u.pitch = (u.pitch + 2.0 * delta[1] / size.1.max(1.0) * 180.0).clamp(-80.0, 80.0);
            *canceled = true;
        }
    });
}

pub(in crate::game) fn scrolled(game: &Game<'_>, screen: &super::native_hooks::NativeScreenView<'_>, scroll: [f64; 2], canceled: &mut bool) {
    let at = [screen.mouse[0] - screen.rect[0], screen.mouse[1] - screen.rect[1]];
    // The bundle's filter screens: the wheel on a ghost slot.
    if screen.menu == Some(super::enderio_conduits::BUNDLE) {
        if !*canceled && super::enderio_conduits::scrolled(game, at, scroll[1]) {
            *canceled = true;
        }
        return;
    }
    if !screen.menu.is_some_and(ours) {
        return;
    }
    UI.with_borrow_mut(|u| {
        if u.io_open && inside(screen.menu.map_or(OVERLAY_RECT, overlay_rect), at) {
            // distance -= wheel * 0.01, a notch being 120.
            u.distance = (u.distance - scroll[1] as f32 * 1.2).clamp(1.0, 200.0);
            *canceled = true;
        }
    });
}

/// The bank's max input / output: valid when empty, or not led by 0 and no more than the network's max I/O.
fn commit_field(game: &Game<'_>, i: usize) {
    let (s, data) = UI.with_borrow(|u| (u.texts[i].clone(), u.data.clone()));
    if data.len() < DATA_SLOTS {
        return;
    }
    let v: i64 = if s.is_empty() { 0 } else { s.parse().unwrap_or(i64::MAX) };
    if v > i64::from(get2(&data, D_MAX_IO)) {
        return;
    }
    let v = v as i32;
    send_button(game, B_LOW | (v & 0xFFFF));
    send_button(game, if i == 0 { B_SET_IN } else { B_SET_OUT } | ((v >> 16) & 0xFFFF));
}

pub(in crate::game) fn key(game: &Game<'_>, screen: &super::native_hooks::NativeScreenView<'_>, key: &str, canceled: &mut bool) {
    if !screen.menu.is_some_and(|m| bank(m).is_some()) {
        return;
    }
    let Some(i) = UI.with_borrow(|u| u.field) else { return };
    match key {
        "key.keyboard.backspace" => {
            UI.with_borrow_mut(|u| {
                u.texts[i].pop();
            });
            commit_field(game, i);
            *canceled = true;
        }
        "key.keyboard.escape" => {}
        _ => *canceled = true,
    }
}

pub(in crate::game) fn char_typed(game: &Game<'_>, screen: &super::native_hooks::NativeScreenView<'_>, c: char, canceled: &mut bool) {
    if !screen.menu.is_some_and(|m| bank(m).is_some()) {
        return;
    }
    let Some(i) = UI.with_borrow(|u| u.field) else { return };
    *canceled = true;
    if !c.is_ascii_digit() {
        return;
    }
    let data = current_data();
    let next = UI.with_borrow(|u| format!("{}{c}", u.texts[i]));
    let ok = !next.starts_with('0') && next.parse::<i64>().is_ok_and(|v| data.len() >= DATA_SLOTS && v <= i64::from(get2(&data, D_MAX_IO)));
    if ok {
        UI.with_borrow_mut(|u| u.texts[i] = next);
        commit_field(game, i);
    }
}

// ---- Registration: JEI and Jade

pub(in crate::game) fn register(game: &mut Game<'_>) {
    enderio_optional::if_mod!("jei", {
        let mut p = kit_layer::Provider::new(NS);
        p.is_catalyst = |item| ALLOY_CATALYSTS.contains(&item) || SAG_CATALYSTS.contains(&item) || super::enderio_machines::is_catalyst(item);
        p.station_catalysts = |icon| if icon == "minecraft:furnace" { &FURNACE_CATALYSTS[..] } else { &[] };
        p.tabs = jei_tabs;
        kit_layer::register(p);
    } else { () });
    register_jade(game);
}

pub(in crate::game) fn register_jade(game: &mut Game<'_>) {
    enderio_optional::if_mod!("jade", {
        if game.native_on(NS) {
            jade_register();
        }
    } else { let _ = game; () });
}

enderio_optional::if_mod_items!("jei", {
use crate::game::jei::Stack;

const ALLOY_CATALYSTS: [&str; 3] = ["enderio:simple_alloy_smelter", "enderio:alloy_smelter", "enderio:enhanced_alloy_smelter"];
const SAG_CATALYSTS: [&str; 3] = ["enderio:simple_sag_mill", "enderio:sag_mill", "enderio:enhanced_sag_mill"];
// AlloyRecipeCategory.setModes: the furnace-capable smelters do minecraft.smelting too.
const FURNACE_CATALYSTS: [&str; 3] = ["enderio:simple_powered_furnace", "enderio:alloy_smelter", "enderio:enhanced_alloy_smelter"];


pub(in crate::game) fn stacks(i: &horizon_client::crafting::Ingredient, count: u32) -> Vec<Stack> {
    i.order.iter().map(|id| Stack { item: id.clone(), count, components: String::new() }).collect()
}

pub(in crate::game) fn energy_text(at: [f32; 2], energy: i32) -> kit_window::PictureText {
    // EnergyIngredientRenderer: the recipe's energy.
    let t = format!("{} µI", commas(i64::from(energy)));
    kit_window::PictureText { at, text: t, color: 0xFF80_8080, hover: 0xFF80_8080, right: false, tip: Vec::new() }
}

pub(in crate::game) fn picture(sprite: &str, src: [f32; 4], covers: Vec<([f32; 4], [f32; 2])>, slots: Vec<([f32; 2], Vec<Stack>, Vec<String>)>, texts: Vec<kit_window::PictureText>) -> kit_window::Card {
    kit_window::Card::Picture { sprite: sprite.to_owned(), src, covers, slots, items: Vec::new(), texts, hovers: Vec::new(), fluids: Vec::new() }
}

fn jei_tabs(game: &Game<'_>, item: &Stack, uses: bool) -> Vec<kit_window::Tab> {
    let id = item.item.as_str();
    let c = caches(game);
    let mut tabs = Vec::new();
    // AlloyRecipeCategory: alloy_smelter_auto (45,3) 82x78, slots at (8,13) (33,3) (57,13), the output at (33,54),
    // two flames at (10,32) and (58,32), the energy at (63,52); synthetic recipes are left out.
    let all = ALLOY_CATALYSTS.contains(&id);
    let mut cards = Vec::new();
    for r in c.alloy.iter().filter(|r| !r.synthetic) {
        let shows = all || if uses { r.inputs.iter().any(|(i, _)| i.items.contains(id)) } else { *r.output.item == *id };
        if !shows {
            continue;
        }
        let places = [[9.0, 14.0], [34.0, 4.0], [58.0, 14.0]];
        let mut slots: Vec<([f32; 2], Vec<Stack>, Vec<String>)> = r.inputs.iter().enumerate().map(|(i, (ing, n))| (places[i], stacks(ing, *n), Vec::new())).collect();
        slots.push(([34.0, 55.0], vec![Stack { item: r.output.item.to_string(), count: r.output.count, components: String::new() }], Vec::new()));
        let flame = [176.0, 0.0, 14.0, 14.0];
        cards.push(picture("enderio:gui/40/alloy_smelter_auto", [45.0, 3.0, 82.0, 78.0], vec![(flame, [10.0, 32.0]), (flame, [58.0, 32.0])], slots, vec![energy_text([63.0, 53.0], r.energy)]));
    }
    if !cards.is_empty() {
        tabs.push(kit_window::Tab { icon: "enderio:alloy_smelter".to_owned(), title: lang("block.enderio.alloy_smelter", "Alloy Smelter"), catalysts: ALLOY_CATALYSTS.iter().map(|s| (*s).to_owned()).collect(), cards });
    }
    // SagMillRecipeCategory: crusher (44,3) 109x78 with 10 to the left (here the card starts at the texture), the
    // input at (45,8), outputs at (14..77,55), the ball at (87,19), the arrow (201,1,16,22) at (46,29).
    let all = SAG_CATALYSTS.contains(&id);
    let balls: Vec<Stack> = c.balls.iter().map(|b| Stack::item(&b.item)).collect();
    let mut cards = Vec::new();
    for r in &c.sag {
        let is_ball = c.ball(id).is_some();
        let shows = all || if uses { r.input.items.contains(id) || (is_ball && r.bonus != Bonus::None) } else { r.outputs.iter().any(|(s, _)| *s.item == *id) };
        if !shows {
            continue;
        }
        let mut slots: Vec<([f32; 2], Vec<Stack>, Vec<String>)> = vec![([36.0, 9.0], stacks(&r.input, r.count), Vec::new())];
        for (i, (s, chance)) in r.outputs.iter().take(4).enumerate() {
            let tip = if *chance > 0.0 && *chance < 1.0 {
                vec![format!("§7{}", lang("enderio.jei.sagmill.chance", "Chance: {0}%").replace("{0}", &((chance * 100.0) as i32).to_string()))]
            } else {
                Vec::new()
            };
            slots.push(([5.0 + 21.0 * i as f32, 56.0], vec![Stack { item: s.item.to_string(), count: s.count, components: String::new() }], tip));
        }
        if r.bonus != Bonus::None {
            slots.push(([78.0, 20.0], balls.clone(), Vec::new()));
        }
        cards.push(picture("enderio:gui/40/crusher", [44.0, 3.0, 109.0, 78.0], vec![([201.0, 1.0, 16.0, 22.0], [36.0, 29.0])], slots, vec![energy_text([90.0, 56.0], r.energy)]));
    }
    if !cards.is_empty() {
        tabs.push(kit_window::Tab { icon: "enderio:sag_mill".to_owned(), title: lang("block.enderio.sag_mill", "SAG Mill"), catalysts: SAG_CATALYSTS.iter().map(|s| (*s).to_owned()).collect(), cards });
    }
    tabs.extend(super::enderio_machines::jei_tabs(game, item, uses));
    tabs
}
});

enderio_optional::if_mod_items!("jade", {
// TOPCompatibility: Ender IO's probe lines (1.12 has The One Probe, not WAILA), from the server's state. Jade draws
// the energy itself from the energy buffer; the redstone and side lines are TOP's "extended" ones, options here.
fn jade_register() {
    crate::game::jade::register_option("plugin_enderio.redstone", "Ender IO Redstone", false);
    crate::game::jade::register_option("plugin_enderio.side_config", "Ender IO Side Config", false);
    crate::game::jade::register_server(jade_lines);
    crate::game::jade::register_progress(jade_progress);
}

/// mkProgressLine with progress: the bar, "<n> %".
fn jade_progress(game: &mut Game<'_>, pos: [i32; 3], block: &str) -> Vec<(f32, Option<String>)> {
    let Some(m) = mach(block) else { return Vec::new() };
    let Some(e) = game.native_mod_block_entity(pos) else { return Vec::new() };
    let p = match m.kind {
        Kind::Stirling => {
            let total = gi(&e.nbt, "totalBurnTime", 0);
            let burn = gi(&e.nbt, "burnTime", 0);
            if burn > 0 && total > 0 { burn as f32 / total as f32 } else { 0.0 }
        }
        _ => task(&e).map_or(0.0, |t| t.progress()),
    };
    if p > 0.0 && energy(&e) > 0 {
        vec![(p, Some(format!("{} %", (p * 100.0) as i32)))]
    } else {
        Vec::new()
    }
}

fn hit_face(pos: [i32; 3]) -> Option<usize> {
    let hit = crate::game::jade::request_hit()?;
    let local = [hit[0] - f64::from(pos[0]), hit[1] - f64::from(pos[1]), hit[2] - f64::from(pos[2])];
    let mut best = (f64::MAX, 0usize);
    for (axis, plane, face) in [(0usize, 0.0f64, 4usize), (0, 1.0, 5), (1, 0.0, 0), (1, 1.0, 1), (2, 0.0, 2), (2, 1.0, 3)] {
        let d = (local[axis] - plane).abs();
        if d < best.0 {
            best = (d, face);
        }
    }
    Some(best.1)
}

fn jade_lines(game: &mut Game<'_>, pos: [i32; 3], block: &str, lines: &mut Vec<String>) {
    if !ours(block) {
        return;
    }
    let Some(e) = game.native_mod_block_entity(pos) else { return };
    if let Some(m) = mach(block) {
        let c = caches(game);
        let stored = energy(&e);
        let max = if m.kind.powered() { max_energy(&c, &e, m) } else { 0 };
        let p = match m.kind {
            Kind::Stirling => if gi(&e.nbt, "burnTime", 0) > 0 { 1.0 } else { 0.0 },
            Kind::Combustion | Kind::Zombie => if gi(&e.nbt, "active", 0) != 0 { 1.0 } else { 0.0 },
            Kind::Enchanter | Kind::Tank => 1.0,
            _ => task(&e).map_or(0.0, |t| t.progress()),
        };
        let active = is_active(&e, m);
        // mkProgressLine: out of power, or active / idle when there is no progress.
        if p > 0.0 {
            if max > 0 && stored == 0 {
                lines.push(lang("enderio.top.progress.outofpower", "§4Out of Power"));
            }
        } else {
            lines.push(if active { lang("enderio.top.machine.active", "active") } else { lang("enderio.top.machine.idle", "idle") });
        }
        // mkEnergyLine: without energy, "No Power" in place of the bar.
        if max > 0 && stored == 0 {
            lines.push(lang("enderio.top.machine.outofpower", "§4No Power"));
        }
        super::enderio_machines::jade_lines(&e, m, lines);
        if m.level > 0 && m.kind != Kind::Enchanter && crate::game::jade::plugin_on(game, "plugin_enderio.redstone") {
            let mode = gi(&e.nbt, "redstoneControlMode", 0).clamp(0, 3);
            let key = ["ignore", "on", "off", "never"][mode as usize];
            lines.push(lang(&format!("enderio.gui.tooltip.redstoneControlMode.{key}"), key));
            let on = gi(&e.nbt, "redstoneCheckPassed", 1) != 0;
            lines.push(format!("§eCurrently: §f{}", if on { lang("enderio.top.redstone.true", "On") } else { lang("enderio.top.redstone.false", "Off") }));
        }
    } else {
        // mkEnergyLine with I/O: the network's average change and limits.
        let Some((avg, max_in, max_out)) = with_net(game, pos, |n| ((n.avg_in.average() - n.avg_out.average()).round() as i64, n.max_input(), n.max_output())) else { return };
        let sign = if avg == 0 { "§f".to_owned() } else if avg > 0 { "§a+".to_owned() } else { "§c".to_owned() };
        lines.push(format!("§eI/O: §f{sign}{} µI/t", commas(avg)));
        lines.push(format!("§eMax Input: §f{} µI/t", commas(i64::from(max_in))));
        lines.push(format!("§eMax Output: §f{} µI/t", commas(i64::from(max_out))));
    }
    if crate::game::jade::plugin_on(game, "plugin_enderio.side_config") {
        if let Some(f) = hit_face(pos) {
            let mode = face_modes(&e.nbt)[f];
            let colored = ["§7None", "§bPull", "§6Push", "§6Push §f/ §bPull", "§cDisabled"][mode as usize];
            let key = ["none", "pull", "push", "pullPush", "disabled"][mode as usize];
            lines.push(format!("§eSide: §f{}", lang(&format!("enderio.top.machine.side.{}", FACE_NAMES[f]), FACE_NAMES[f])));
            lines.push(format!("§eMode: §f{}", lang(&format!("enderio.gui.machine.ioMode.{key}.colored"), colored)));
        }
    }
}
});
