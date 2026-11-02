//! Crypto page: Real-time cryptocurrency prices, charts and 24h statistics.
//!
//! Laid out like the Stocks page: on the left the coin list (each row's code
//! and name, 24h line against the previous close, price and change pill);
//! on the right the picked coin's price, morphing chart over 1D, 1M, 6M or 1Y
//! with hover scrubbing, time/date axis and 24h statistics.

use crate::gfx::{TextStyle, Weight};
use crate::icons::Icon;
use crate::services::crypto::Watch;
use crate::state::AppState;
use crate::ui::{wid, wid_n, KeyInput, PillStyle, Ui, WidgetId, PILL_SMALL};
use poni_core::crypto::{self, CryptoQuote, Range, Show, Trend};
use poni_core::geom::{lerp, palette, Color, Point, Rect};
use poni_core::stocks;
use poni_core::svgpath::Seg;

const HEIGHT: f32 = 256.0;
const LIST_MIN: f32 = 232.0;
const LIST_MAX: f32 = 300.0;
const COL_GAP: f32 = 20.0;
const ROW_H: f32 = 48.0;
const ADD_H: f32 = 46.0;
const ROW_PAD: f32 = 8.0;
const PILL_W: f32 = 70.0;
const PILL_H: f32 = 20.0;
const SPARK_W: f32 = 48.0;
const EDIT_W: f32 = 26.0;
const GLIDE: f32 = 16.0;
const MORPH_S: f64 = 0.45;
const SAMPLES: usize = 160;
const FIELD_H: f32 = 34.0;
const RESULT_H: f32 = 36.0;
const PRICE_Y: f32 = 28.0;
const CHART_Y: f32 = 62.0;
const CHART_H: f32 = 106.0;
const AXIS_H: f32 = 16.0;
const STATS_Y: f32 = 192.0;
const STAT_H: f32 = 32.0;
const SEG_W: f32 = 32.0;
const SEG_H: f32 = 22.0;

pub fn height(_st: &AppState) -> f32 {
    HEIGHT
}

pub fn watch(st: &AppState, open: bool) -> Watch {
    let page_open = open && st.crypto_on();
    let pinned_code = st.store.ticker.as_ref().filter(|t| t.enabled && t.source == "crypto").map(|t| t.code.clone());
    let active = page_open || (open && pinned_code.is_some() && st.crypto_on());
    let mut codes = if page_open {
        st.store.crypto.codes.clone()
    } else if let Some(ref c) = pinned_code {
        vec![c.clone()]
    } else {
        Vec::new()
    };
    if let Some(ref c) = pinned_code {
        if !codes.iter().any(|x| x.eq_ignore_ascii_case(c)) {
            codes.push(c.clone());
        }
    }
    Watch {
        open: active,
        codes,
        chart: (page_open && st.store.crypto.range != Range::Day).then(|| st.store.crypto.pick.clone()),
        seq: st.crypto.refresh_seq,
    }
}

#[derive(Clone, Debug, Default)]
pub struct CryptoUi {
    pub adding: bool,
    pub query: String,
    hit: usize,
    focus: bool,
    srch_scroll: f32,
    srch_shown_scroll: f32,
    srch_list_drag: Option<(f32, f32, bool)>,
    srch_bar_drag: Option<(f32, f32)>,
    pub editing: bool,
    edit_k: f32,
    swap_k: f32,
    drag: Option<(String, f32)>,
    scroll: f32,
    shown_scroll: f32,
    list_drag: Option<(f32, f32, bool)>,
    bar_drag: Option<(f32, f32)>,
    rows: Vec<(String, f32)>,
    pick_y: Option<f32>,
    drawn: f64,
    chart: Morph,
}

#[derive(Clone, Debug, Default)]
struct Morph {
    key: String,
    from: Vec<(f32, f32)>,
    to: Vec<(f32, f32)>,
    base_from: Option<f32>,
    base_to: Option<f32>,
    color_from: Option<Color>,
    color_to: Option<Color>,
    t0: f64,
}

pub fn draw(ui: &mut Ui, st: &mut AppState, area: Rect) {
    let now = ui.now;
    let s = &mut st.ui.crypto;
    let dt = if now > s.drawn {
        (now - s.drawn).min(1.0 / 30.0) as f32
    } else {
        0.0
    };
    s.drawn = now;
    let mut moving = false;
    s.swap_k = glide(s.swap_k, 1.0, dt, 0.01, &mut moving);
    let list_w = (area.w * 0.44).clamp(LIST_MIN, LIST_MAX).round();
    let left = Rect::new(area.x, area.y, list_w, area.h.min(HEIGHT));
    let right = Rect::new(left.right() + COL_GAP, area.y, (area.w - list_w - COL_GAP).max(0.0), left.h);

    let base = ui.c.alpha();
    ui.c.set_alpha(base * ease_out(st.ui.crypto.swap_k));
    if st.ui.crypto.adding {
        search(ui, st, left, area, dt, &mut moving);
    } else {
        list(ui, st, left, area, dt, &mut moving);
    }
    ui.c.set_alpha(base);
    detail(ui, st, right);
    if moving {
        ui.request_anim();
    }
}

fn glide(pos: f32, target: f32, dt: f32, eps: f32, moving: &mut bool) -> f32 {
    let p = pos + (target - pos) * (1.0 - (-dt * GLIDE).exp());
    if (target - p).abs() <= eps {
        target
    } else {
        *moving = true;
        p
    }
}

fn ease_out(t: f32) -> f32 {
    let t = t.clamp(0.0, 1.0);
    1.0 - (1.0 - t).powi(3)
}

fn intersect(a: Rect, b: Rect) -> Option<Rect> {
    let x0 = a.x.max(b.x);
    let y0 = a.y.max(b.y);
    let x1 = a.right().min(b.right());
    let y1 = a.bottom().min(b.bottom());
    (x1 > x0 && y1 > y0).then(|| Rect::new(x0, y0, x1 - x0, y1 - y0))
}

fn code_n(code: &str) -> u64 {
    code.bytes().fold(0, |h, b| (h << 8) | u64::from(b))
}

fn handle_id(code: &str) -> WidgetId {
    wid_n("crypto.handle", code_n(code))
}

fn trend_color(t: Trend) -> Color {
    match t {
        Trend::Up => palette::GREEN,
        Trend::Down => palette::RED,
        Trend::Flat => Color::white(0.36),
    }
}

fn list(ui: &mut Ui, st: &mut AppState, r: Rect, area: Rect, dt: f32, moving: &mut bool) {
    let codes = st.store.crypto.codes.clone();
    let n = codes.len();
    let content_h = n as f32 * ROW_H + ADD_H;
    let max_scroll = (content_h - r.h).max(0.0);
    let card = super::card::pill_card(st, r);
    let accent = st.accent;
    let board = st.crypto.board.as_ref();
    let (wl, s) = (&mut st.store.crypto, &mut st.ui.crypto);
    let mut save = false;

    // Mouse wheel over the whole Crypto area scrolls the list smoothly
    let notches = ui.wheel_in(area);
    if notches != 0.0 {
        *moving = true;
    }
    s.scroll = (s.scroll - notches * ROW_H).clamp(0.0, max_scroll);

    // Mouse scrollbar & click-and-drag scrolling when not in edit mode.
    let bar_hit = Rect::new(r.right() - 8.0, r.y, 8.0, r.h);
    if max_scroll > 0.0 && !s.editing {
        if ui.input.just_pressed {
            if let Some(m) = ui.input.mouse {
                if bar_hit.contains(m) {
                    let track_h = (r.h - 8.0).max(1.0);
                    let frac = ((m.y - (r.y + 4.0)) / track_h).clamp(0.0, 1.0);
                    s.scroll = (frac * max_scroll).clamp(0.0, max_scroll);
                    s.shown_scroll = s.scroll;
                    s.bar_drag = Some((m.y, s.scroll));
                } else if r.contains(m) {
                    s.list_drag = Some((m.y, s.scroll, false));
                }
            }
        }
        if ui.input.down {
            if let (Some((y0, sc0)), Some(m)) = (s.bar_drag, ui.input.mouse) {
                let track_h = (r.h - 8.0).max(1.0);
                let thumb_h = (track_h * (r.h / content_h)).clamp(24.0, track_h);
                let travel = (track_h - thumb_h).max(1.0);
                s.scroll = (sc0 + (m.y - y0) / travel * max_scroll).clamp(0.0, max_scroll);
                s.shown_scroll = s.scroll;
                ui.input.pressed_id = None;
            } else if let (Some((y0, sc0, moved)), Some(m)) = (&mut s.list_drag, ui.input.mouse) {
                let dy = m.y - *y0;
                if *moved || dy.abs() > 6.0 {
                    *moved = true;
                    s.scroll = (*sc0 - dy).clamp(0.0, max_scroll);
                    s.shown_scroll = s.scroll;
                    ui.input.pressed_id = None;
                    ui.input.clicked = None;
                }
            }
        } else {
            s.bar_drag = None;
            s.list_drag = None;
        }
    } else {
        s.bar_drag = None;
        s.list_drag = None;
    }

    s.shown_scroll = glide(s.shown_scroll.clamp(0.0, max_scroll), s.scroll, dt, 0.25, moving);
    s.edit_k = glide(s.edit_k, if s.editing { 1.0 } else { 0.0 }, dt, 0.005, moving);

    // Edit mode drag to reorder
    let drag_dy = s.drag.as_ref().and_then(|(c, _)| {
        let held = ui.input.down && ui.input.pressed_id == Some(handle_id(c));
        held.then(|| ui.input.mouse.zip(ui.input.press_pos).map_or(0.0, |(m, p)| m.y - p.y))
    });
    if s.drag.is_some() && drag_dy.is_none() {
        s.drag = None;
        save = true;
    }
    if let (Some((code, y0)), Some(dy), false) = (s.drag.clone(), drag_dy, codes.is_empty()) {
        let to = ((y0 + dy) / ROW_H).round().clamp(0.0, (codes.len() - 1) as f32) as usize;
        if wl.codes.get(to) != Some(&code) {
            wl.move_to(&code, to);
        }
    }

    let mut shown = Vec::with_capacity(wl.codes.len());
    for (i, code) in wl.codes.iter().enumerate() {
        let target = i as f32 * ROW_H;
        let y = match (&s.drag, drag_dy) {
            (Some((c, y0)), Some(dy)) if c == code => (y0 + dy).clamp(0.0, (n.saturating_sub(1)) as f32 * ROW_H),
            _ => match s.rows.iter().find(|(c, _)| c == code) {
                Some((_, y)) => glide(*y, target, dt, 0.25, moving),
                None => target,
            },
        };
        shown.push((code.clone(), y));
    }
    s.rows = shown;
    let top = r.y - s.shown_scroll;
    let pick_target = s.rows.iter().find(|(c, _)| *c == wl.pick).map_or(0.0, |p| p.1);
    let pick_y = match s.pick_y {
        Some(y) if s.drag.is_none() => glide(y, pick_target, dt, 0.25, moving),
        _ => pick_target,
    };
    s.pick_y = Some(pick_y);

    ui.c.clip(Rect::new(r.x - 8.0, r.y, r.w + 16.0, r.h));

    // Highlight for picked coin
    let hl = Rect::new(r.x, top + pick_y + 2.0, r.w, ROW_H - 4.0);
    if s.edit_k < 0.99 {
        let c = Color::white(0.10 * (1.0 - s.edit_k));
        super::card::fill_box(ui, card, hl, 10.0, c);
    }

    let mut picked = None;
    let mut picked_y = None;
    let mut removed = None;
    let mut cycle = false;

    let mut draw_order: Vec<usize> = (0..s.rows.len()).collect();
    if let Some((dc, _)) = &s.drag {
        if let Some(i) = s.rows.iter().position(|(c, _)| c == dc) {
            draw_order.retain(|j| *j != i);
            draw_order.push(i);
        }
    }

    for i in draw_order {
        let (code, y) = (&s.rows[i].0, s.rows[i].1);
        let row = Rect::new(r.x, top + y, r.w, ROW_H);
        if intersect(row, r).is_none() {
            continue;
        }
        let q = board.and_then(|b| b.quote(code));
        let name_str = q
            .map(|q| q.name.as_str())
            .filter(|n| !n.trim().is_empty())
            .map(|s| s.to_string())
            .unwrap_or_else(|| crypto::fallback_name(code));
        let name = name_str.as_str();
        let dragged = s.drag.as_ref().is_some_and(|(c, _)| c == code);
        let act = draw_row(ui, code, name, q, wl.show, s.edit_k, dragged, *code == wl.pick, row, r, card);
        match act {
            RowAct::Pick => {
                picked = Some(code.clone());
                picked_y = Some(y);
            }
            RowAct::Cycle => cycle = true,
            RowAct::Remove => removed = Some(code.clone()),
            RowAct::Drag => {
                if s.drag.is_none() {
                    s.drag = Some((code.clone(), y));
                }
            }
            RowAct::None => {}
        }
    }

    // Bottom controls: Add a coin, Edit/Done, Attribution
    let add = Rect::new(r.x, top + n as f32 * ROW_H, r.w, ADD_H);
    let line = Rect::new(add.x + ROW_PAD, add.y + 4.0, add.w - 2.0 * ROW_PAD, 22.0);
    let edit_label = if s.editing { "Done" } else { "Edit" };
    let edit_st = TextStyle::body().weight(Weight::Semibold).right();
    let ew = ui.c.measure(edit_label, &edit_st, 100.0).0.ceil();
    let edit_r = Rect::new(line.right() - ew - 8.0, line.y - 3.0, ew + 16.0, line.h + 6.0);
    let mut start_add = false;

    if intersect(add, r).is_some() {
        if wl.full() {
            ui.c.text(&format!("Your list is full: {} coins", crypto::MAX), line, &TextStyle::footnote().color(palette::TEXT_FAINT));
        } else if !s.editing {
            let add_r = Rect::new(add.x, line.y - 3.0, (edit_r.x - add.x - 8.0).max(0.0), line.h + 6.0);
            if let Some(add_hit) = intersect(add_r, r) {
                let resp = ui.button(wid("crypto.add"), add_hit);
                if resp.hovered || resp.pressed {
                    super::card::fill_box(ui, card, add_r, 9.0, Color::white(if resp.pressed { 0.10 } else { 0.06 }));
                }
                start_add = resp.clicked;
            }
            let cy = line.center().y;
            ui.c.fill_circle(Point::new(line.x + 9.0, cy), 9.0, accent.with_alpha(0.20));
            ui.c.icon(Icon::Plus, Rect::from_center(Point::new(line.x + 9.0, cy), 13.0, 13.0), accent);
            ui.c.text("Add a coin", Rect::new(line.x + 26.0, line.y, line.w - 26.0 - ew - 20.0, line.h), &TextStyle::body().weight(Weight::Semibold).color(accent));
        }

        if !wl.codes.is_empty() || s.editing {
            if let Some(edit_hit) = intersect(edit_r, r) {
                let resp = ui.button(wid("crypto.edit"), edit_hit);
                let c = if resp.hovered { palette::TEXT } else { palette::TEXT_DIM };
                ui.c.text(edit_label, Rect::new(edit_r.x, line.y, edit_r.w - 8.0, line.h), &edit_st.color(c));
                if resp.clicked {
                    s.editing = !s.editing;
                    s.drag = None;
                    save |= !s.editing;
                }
            }
        }
        let credit = "Live 24/7 prices via Binance market feed";
        ui.c.text(credit, Rect::new(line.x, add.y + 28.0, line.w, 14.0), &TextStyle::caption().color(palette::TEXT_FAINT));
    }

    if s.shown_scroll > 0.5 {
        fade(ui, Rect::new(r.x - 8.0, r.y, r.w + 16.0, 14.0), true);
    }
    if s.shown_scroll < max_scroll - 0.5 {
        fade(ui, Rect::new(r.x - 8.0, r.bottom() - 18.0, r.w + 16.0, 18.0), false);
    }
    // Interactive scrollbar thumb on hover/drag.
    if max_scroll > 0.5 && (ui.hovered(r) || s.bar_drag.is_some()) {
        let track_h = (r.h - 8.0).max(1.0);
        let thumb_h = (track_h * (r.h / content_h)).clamp(24.0, track_h);
        let thumb_y = r.y + 4.0 + (s.shown_scroll / max_scroll).clamp(0.0, 1.0) * (track_h - thumb_h);
        let active = ui.hovered(bar_hit) || s.bar_drag.is_some();
        let w = if active { 4.0 } else { 2.5 };
        ui.c.fill_rrect(Rect::new(r.right() - w - 1.0, thumb_y, w, thumb_h), w / 2.0, Color::white(if active { 0.42 } else { 0.22 }));
    }
    ui.c.unclip();

    if let Some(code) = picked {
        wl.pick = code;
        if let Some(py) = picked_y {
            if py < s.scroll {
                s.scroll = py.clamp(0.0, max_scroll);
                *moving = true;
            } else if py + ROW_H > s.scroll + r.h {
                s.scroll = (py + ROW_H - r.h + 18.0).clamp(0.0, max_scroll);
                *moving = true;
            }
        }
        save = true;
    }
    if cycle {
        wl.show = wl.show.next();
        save = true;
    }
    if let Some(code) = removed {
        wl.remove(&code);
        save = true;
        if wl.codes.is_empty() {
            s.editing = false;
        }
    }
    if start_add {
        s.adding = true;
        s.focus = true;
        s.query.clear();
        s.hit = 0;
        s.srch_scroll = 0.0;
        s.srch_shown_scroll = 0.0;
        s.swap_k = 0.0;
    }
    if save {
        st.save();
    }
}

enum RowAct {
    None,
    Pick,
    Cycle,
    Remove,
    Drag,
}

#[allow(clippy::too_many_arguments)]
fn draw_row(
    ui: &mut Ui,
    code: &str,
    name: &str,
    q: Option<&CryptoQuote>,
    show: Show,
    edit_k: f32,
    dragged: bool,
    picked: bool,
    r: Rect,
    view: Rect,
    card: Option<(Rect, f32)>,
) -> RowAct {
    let n = code_n(code);
    let editing = edit_k > 0.01;
    let mut act = RowAct::None;
    let hit = Rect::new(r.x, r.y + 2.0, r.w, r.h - 4.0);
    let Some(hit_clip) = intersect(hit, view) else { return act };
    let resp = ui.button(wid_n("crypto.row", n), hit_clip);
    if dragged {
        super::card::fill_box(ui, card, hit, 10.0, Color::white(0.14));
    } else if (resp.hovered || resp.pressed) && !editing && !picked {
        super::card::fill_box(ui, card, hit, 10.0, Color::white(if resp.pressed { 0.08 } else { 0.05 }));
    }
    if resp.clicked && !editing {
        act = RowAct::Pick;
    }

    let shift = EDIT_W * ease_out(edit_k);
    let x0 = r.x + ROW_PAD + shift;
    let right = r.right() - ROW_PAD;
    let pill = Rect::new(right - PILL_W, r.y + 25.0, PILL_W, PILL_H);
    let spark = Rect::new(pill.x - 10.0 - SPARK_W, r.y + 11.0, SPARK_W, 26.0);
    let text_w = (spark.x - 8.0 - x0).max(0.0);

    ui.c.text(code, Rect::new(x0, r.y + 6.0, text_w, 19.0), &TextStyle::headline().weight(Weight::Semibold));
    ui.c.text(name, Rect::new(x0, r.y + 25.0, text_w, 17.0), &TextStyle::footnote().color(palette::TEXT_DIM));

    let base = ui.c.alpha();
    let k = 1.0 - edit_k;
    if k > 0.01 {
        ui.c.set_alpha(base * k);
        if let Some(q) = q {
            mini_chart(ui, q, spark);
            let price_str = crypto::fmt_price(q.price);
            ui.c.text(&price_str, Rect::new(right - 116.0, r.y + 5.0, 116.0, 19.0), &TextStyle::body().weight(Weight::Semibold).tabular().right());

            let mut fill = trend_color(q.trend());
            if let Some(pill_clip) = intersect(pill, view) {
                let pr = ui.button(wid_n("crypto.pill", n), pill_clip);
                if pr.pressed {
                    fill = fill.mul_alpha(0.75);
                }
                if pr.clicked && !editing {
                    act = RowAct::Cycle;
                }
            }
            ui.c.fill_rrect(pill, 6.0, fill);
            let chg_str = crypto::fmt_change(q, show);
            ui.c.text(&chg_str, Rect::new(pill.x + 5.0, pill.y, pill.w - 10.0, pill.h), &TextStyle::footnote().weight(Weight::Semibold).tabular().right().color(Color::white(1.0)));
        } else {
            ui.c.text("\u{2014}", Rect::new(right - 80.0, r.y + 5.0, 80.0, 19.0), &TextStyle::body().color(palette::TEXT_FAINT).right());
            ui.c.fill_rrect(pill, 6.0, Color::white(0.10));
        }
        ui.c.set_alpha(base);
    }

    if editing {
        ui.c.set_alpha(base * edit_k);
        let c = Point::new(r.x + ROW_PAD + 9.0, r.center().y);
        let rm = intersect(Rect::from_center(c, 26.0, 26.0), view).map(|rh| ui.button(wid_n("crypto.del", n), rh)).unwrap_or_default();
        ui.c.fill_circle(c, 9.0, if rm.pressed { palette::RED.mul_alpha(0.7) } else { palette::RED });
        ui.c.fill_rrect(Rect::from_center(c, 9.0, 2.0), 1.0, Color::white(1.0));
        if rm.clicked {
            act = RowAct::Remove;
        }

        let h = Rect::new(right - 36.0, r.y + 4.0, 36.0, r.h - 8.0);
        let hr = intersect(h, view).map(|hh| ui.button(handle_id(code), hh)).unwrap_or_default();
        let hc = if hr.hovered || hr.pressed || dragged { palette::TEXT } else { palette::TEXT_FAINT };
        for j in 0..3 {
            ui.c.fill_rrect(Rect::new(h.right() - 22.0, h.center().y - 5.0 + j as f32 * 5.0 - 0.75, 16.0, 1.5), 0.75, hc);
        }
        if hr.pressed {
            act = RowAct::Drag;
        }
        ui.c.set_alpha(base);
    }

    act
}

fn mini_chart(ui: &mut Ui, q: &CryptoQuote, r: Rect) {
    let Some((lo, hi)) = crypto::bounds(q.spark.iter().map(|p| f64::from(p.1)), Some(q.prev_close)) else { return };
    let y = |v: f64| r.bottom() - ((v - lo) / (hi - lo)) as f32 * r.h;
    dotted(ui, r.x, r.right(), y(q.prev_close).round() + 0.5, Color::white(0.30));
    if q.spark.len() < 2 {
        return;
    }
    let color = trend_color(q.trend());
    let n = (q.spark.len() - 1).max(1) as f32;
    let pts: Vec<(f32, f32)> = q.spark.iter().enumerate().map(|(i, p)| (r.x + (i as f32 / n) * r.w, y(f64::from(p.1)))).collect();
    draw_line(ui, &pts, color, r.y, r.bottom(), 1.25, None);
}

fn detail(ui: &mut Ui, st: &mut AppState, r: Rect) {
    let pick = st.store.crypto.pick.clone();
    let range = st.store.crypto.range;
    let q = st.crypto.board.as_ref().and_then(|b| b.quote(&pick)).cloned();

    // Title: Code and Full Name
    let code_st = TextStyle::value().weight(Weight::Bold);
    let cw = ui.c.measure(&pick, &code_st, 200.0).0.ceil();
    let title = Rect::new(r.x, r.y, r.w, 22.0);
    let seg_x = r.right() - SEG_W * Range::ALL.len() as f32;
    ui.c.text(&pick, Rect::new(title.x, title.y, cw + 2.0, title.h), &code_st);

    let is_pinned = st.store.ticker.as_ref().is_some_and(|t| t.enabled && t.source == "crypto" && t.code.eq_ignore_ascii_case(&pick));
    let pin_r = Rect::new(title.x + cw + 6.0, title.y - 1.0, 22.0, 22.0);
    let pin_resp = ui.button(wid("crypto.pin_notch"), pin_r);
    let pin_icon = if is_pinned { Icon::PinFill } else { Icon::Pin };
    let pin_col = if is_pinned { st.accent } else if pin_resp.hovered { palette::TEXT } else { palette::TEXT_FAINT };
    if pin_resp.hovered {
        ui.c.fill_circle(pin_r.center(), 10.0, Color::white(0.08));
    }
    ui.c.icon(pin_icon, Rect::from_center(pin_r.center(), 13.0, 13.0), pin_col);
    if pin_resp.clicked {
        if is_pinned {
            st.store.ticker = None;
        } else {
            st.store.ticker = Some(poni_core::store::TickerPin {
                enabled: true,
                source: "crypto".to_string(),
                code: pick.clone(),
            });
        }
        st.save();
    }

    // Bell / Alert target button
    let has_alert = st.store.alerts.iter().any(|a| a.source == "crypto" && a.code.eq_ignore_ascii_case(&pick) && !a.triggered);
    let bell_r = Rect::new(title.x + cw + 30.0, title.y - 1.0, 22.0, 22.0);
    let bell_resp = ui.button(wid("crypto.alert"), bell_r);
    let bell_col = if has_alert { st.accent } else if bell_resp.hovered { palette::TEXT } else { palette::TEXT_FAINT };
    if bell_resp.hovered {
        ui.c.fill_circle(bell_r.center(), 10.0, Color::white(0.08));
    }
    ui.c.icon(Icon::Bell, Rect::from_center(bell_r.center(), 13.0, 13.0), bell_col);
    if bell_resp.clicked {
        if has_alert {
            st.store.alerts.retain(|a| !(a.source == "crypto" && a.code.eq_ignore_ascii_case(&pick)));
            st.notify(Icon::Bell, palette::TEXT_DIM, "Alert Removed", &format!("Price alert for {} disabled", pick), 3.0, None);
        } else if let Some(ref cur_q) = q {
            let target = if cur_q.price > 0.0 { (cur_q.price * 1.05 * 100.0).round() / 100.0 } else { 1.0 };
            let now_id = std::time::SystemTime::now().duration_since(std::time::UNIX_EPOCH).map_or(1, |d| d.as_secs());
            st.store.alerts.push(poni_core::store::PriceAlert {
                id: now_id,
                source: "crypto".to_string(),
                code: pick.clone(),
                target_price: target,
                above: true,
                triggered: false,
                enabled: true,
            });
            st.notify(Icon::Bell, st.accent, "Price Alert Set!", &format!("{}: Alert when price reaches {}", pick, crypto::fmt_price(target)), 4.0, None);
        }
        st.save();
    }

    // Manual Refresh button
    let refresh_r = Rect::new(bell_r.right() + 4.0, title.y - 1.0, 22.0, 22.0);
    let refresh_resp = ui.button(wid("crypto.refresh_btn"), refresh_r);
    let refresh_col = if refresh_resp.hovered { palette::TEXT } else { palette::TEXT_FAINT };
    if refresh_resp.hovered {
        ui.c.fill_circle(refresh_r.center(), 10.0, Color::white(0.08));
    }
    ui.c.icon(Icon::Reset, Rect::from_center(refresh_r.center(), 13.0, 13.0), refresh_col);
    if refresh_resp.clicked {
        st.crypto.years.remove(&pick);
        st.crypto.refresh_seq = st.crypto.refresh_seq.wrapping_add(1);
        st.notify(Icon::Reset, st.accent, "Refreshing...", &format!("Updating {} chart & prices...", pick), 2.0, None);
    }

    let name_str = q
        .as_ref()
        .map(|q| q.name.as_str())
        .filter(|n| !n.trim().is_empty())
        .map(|s| s.to_string())
        .unwrap_or_else(|| crypto::fallback_name(&pick));
    let name = name_str.as_str();
    let name_x = refresh_r.right() + 8.0;
    let name_r = Rect::new(name_x, title.y + 1.0, (seg_x - name_x - 10.0).max(0.0), title.h);
    ui.c.text(name, name_r, &TextStyle::headline().color(palette::TEXT_DIM));

    // Timeframe selector (1D, 1M, 6M, 1Y)
    if let Some(pick_range) = range_control(ui, Rect::new(seg_x, title.y, r.right() - seg_x, SEG_H), range) {
        st.store.crypto.range = pick_range;
        st.save();
    }

    let Some(q) = q else {
        let msg = format!("Getting {pick}\u{2026}");
        ui.c.text(&msg, Rect::new(r.x, r.y + CHART_Y, r.w, CHART_H), &TextStyle::footnote().color(palette::TEXT_DIM).center());
        return;
    };

    let chart = Rect::new(r.x, r.y + CHART_Y, r.w, CHART_H);
    let line = chart_line(st, &q, range);
    let target = crypto::resample(&line.pts, SAMPLES);

    let now = ui.now;
    let m = &mut st.ui.crypto.chart;
    if line.key != m.key {
        let (from, base_from, color_from) = if m.to.is_empty() {
            let flat = line.base.unwrap_or_else(|| target.first().map_or(0.5, |p| p.1));
            (target.iter().map(|p| (p.0, flat)).collect(), line.base, Some(line.color))
        } else {
            let e = morph_k(m.t0, now);
            (lerp_pts(&m.from, &m.to, e), lerp_base(m.base_from, m.base_to, e), Some(lerp_color(m.color_from, m.color_to, e)))
        };
        *m = Morph {
            key: line.key.clone(),
            from,
            to: target.clone(),
            base_from,
            base_to: line.base,
            color_from,
            color_to: Some(line.color),
            t0: now,
        };
    }
    let e = morph_k(m.t0, now);
    if e < 1.0 {
        ui.request_anim();
    }
    let shown = lerp_pts(&m.from, &m.to, e);
    let color = lerp_color(m.color_from, m.color_to, e);
    let base = lerp_base(m.base_from, m.base_to, e);
    let to_px = |p: (f32, f32)| (chart.x + p.0 * chart.w, chart.bottom() - p.1 * chart.h);
    if let Some(b) = base {
        dotted(ui, chart.x, chart.right(), (chart.bottom() - b * chart.h).round() + 0.5, Color::white(0.30));
    }
    let px: Vec<(f32, f32)> = shown.iter().map(|p| to_px(*p)).collect();
    if line.pts.len() >= 2 {
        draw_line(ui, &px, color, chart.y, chart.bottom() + 6.0, 2.0, Some(0.34));
    }

    // Hover scrubbing on the chart
    let scrub = ui.input.mouse.filter(|p| chart.inset(-4.0, -6.0).contains(*p) && line.pts.len() >= 2).map(|p| {
        let x = ((p.x - chart.x) / chart.w).clamp(0.0, 1.0);
        line.pts.iter().enumerate().min_by(|a, b| (a.1 .0 - x).abs().total_cmp(&(b.1 .0 - x).abs())).map_or(0, |(i, _)| i)
    });
    if let Some(i) = scrub.filter(|_| e >= 1.0) {
        let (sx, sy) = to_px(line.pts[i]);
        ui.c.line(Point::new(sx, chart.y - 2.0), Point::new(sx, chart.bottom()), Color::white(0.32), 1.0);
        ui.c.fill_circle(Point::new(sx, sy), 5.0, palette::ISLAND);
        ui.c.fill_circle(Point::new(sx, sy), 3.5, color);
    }

    let (value, when) = match scrub.filter(|_| e >= 1.0) {
        Some(i) => (line.values[i], Some(line.label(i))),
        None => (q.price, None),
    };
    let from = if range == Range::Day { q.prev_close } else { line.values.first().copied().unwrap_or(q.prev_close) };
    let price_str = crypto::fmt_price(value);
    let price_st = TextStyle::value().weight(Weight::Semibold).tabular();
    let pw = ui.c.measure(&price_str, &price_st, 200.0).0.ceil();
    let py = r.y + PRICE_Y;
    ui.c.text(&price_str, Rect::new(r.x, py, pw + 2.0, 24.0), &price_st);

    let d = value - from;
    let pct = if from > 0.0 { d / from * 100.0 } else { 0.0 };
    let sign = if d < 0.0 { "\u{2212}" } else if d > 0.0 { "+" } else { "" };
    let change = format!("{sign}{} ({})", crypto::fmt_price(d.abs()), stocks::fmt_pct(pct));
    let ccol = if d.abs() < 1e-6 { Color::white(0.36) } else if d > 0.0 { palette::GREEN } else { palette::RED };
    let change_st = TextStyle::body().weight(Weight::Semibold).tabular().color(ccol);
    let cw2 = ui.c.measure(&change, &change_st, 200.0).0.ceil();
    ui.c.text(&change, Rect::new(r.x + pw + 10.0, py + 2.0, (r.right() - r.x - pw - 10.0).max(0.0), 22.0), &change_st);
    if let Some(when) = when {
        let wx = r.x + pw + 10.0 + cw2 + 8.0;
        ui.c.text(&when, Rect::new(wx, py + 3.0, (r.right() - wx).max(0.0), 22.0), &TextStyle::footnote().color(palette::TEXT_FAINT));
    }

    // Axis row: start/end labels and live status
    let axis = Rect::new(chart.x, chart.bottom() + 4.0, chart.w, AXIS_H);
    let (a, b) = line.ends();
    let axis_st = TextStyle::caption().color(palette::TEXT_FAINT);
    let aw = ui.c.measure(&a, &axis_st, 200.0).0.ceil();
    let bw = ui.c.measure(&b, &axis_st, 200.0).0.ceil();
    ui.c.text(&a, axis, &axis_st);
    ui.c.text(&b, axis, &axis_st.right());
    let status = if st.crypto.trouble.is_some() { "Offline preview" } else { "Live 24/7 \u{b7} USDT" };
    let room = Rect::new(axis.x + aw + 10.0, axis.y, (axis.w - aw - bw - 20.0).max(0.0), axis.h);
    ui.c.text(status, room, &TextStyle::caption().color(if st.crypto.trouble.is_some() { palette::ORANGE } else { palette::TEXT_FAINT }).center());

    // 2x4 Stats grid matching Stocks
    stats(ui, &q, Rect::new(r.x, r.y + STATS_Y, r.w, (r.h - STATS_Y).max(2.0 * STAT_H)));
}

fn stats(ui: &mut Ui, q: &CryptoQuote, r: Rect) {
    let cells = [
        ("Open", crypto::fmt_price(q.open)),
        ("24h High", crypto::fmt_price(q.high)),
        ("24h Low", crypto::fmt_price(q.low)),
        ("Prev 24h", crypto::fmt_price(q.prev_close)),
        ("Mkt cap", crypto::fmt_cap(q.market_cap)),
        ("24h Value", crypto::fmt_volume(q.quote_volume)),
        ("24h Vol", format!("{} {}", stocks::fmt_big(q.volume), q.code)),
        ("24h Chg", stocks::fmt_pct(q.change_pct())),
    ];
    let gap = 8.0;
    let cw = (r.w - 3.0 * gap) / 4.0;
    let row_h = r.h / 2.0;
    let label_st = TextStyle::caption().color(palette::TEXT_FAINT);
    let value_st = TextStyle::footnote().weight(Weight::Semibold).tabular();
    for (i, (label, value)) in cells.iter().enumerate() {
        let (row, col) = (i / 4, i % 4);
        let cell = Rect::new(r.x + col as f32 * (cw + gap), r.y + row as f32 * row_h, cw, row_h);
        ui.c.line(Point::new(cell.x, cell.y + 0.5), Point::new(cell.right(), cell.y + 0.5), Color::white(0.08), 1.0);
        ui.c.text(label, Rect::new(cell.x, cell.y + 3.0, cell.w, 13.0), &label_st);
        ui.c.text(value, Rect::new(cell.x, cell.y + 15.0, cell.w, 15.0), &value_st);
    }
}

struct Line {
    key: String,
    pts: Vec<(f32, f32)>,
    values: Vec<f64>,
    minutes: Vec<u16>,
    days: Vec<i64>,
    base: Option<f32>,
    color: Color,
}

impl Line {
    fn label(&self, i: usize) -> String {
        match (self.minutes.get(i), self.days.get(i)) {
            (Some(m), _) => {
                let hrs_ago = (1440u16.saturating_sub(*m)) / 60;
                let mins_ago = (1440u16.saturating_sub(*m)) % 60;
                if hrs_ago == 0 && mins_ago == 0 {
                    "Now".to_string()
                } else if mins_ago == 0 {
                    format!("{hrs_ago}h ago")
                } else {
                    format!("{hrs_ago}h {mins_ago}m ago")
                }
            }
            (_, Some(d)) => stocks::date_label(*d),
            _ => String::new(),
        }
    }

    fn ends(&self) -> (String, String) {
        match (self.days.first(), self.days.last()) {
            (Some(a), Some(b)) => (stocks::date_label(*a), stocks::date_label(*b)),
            _ => ("24h ago".to_string(), "Now".to_string()),
        }
    }
}

fn chart_line(st: &AppState, q: &CryptoQuote, range: Range) -> Line {
    let board_at = st.crypto.board.as_ref().map_or(0, |b| b.at);
    let mut line = Line {
        key: String::new(),
        pts: Vec::new(),
        values: Vec::new(),
        minutes: Vec::new(),
        days: Vec::new(),
        base: None,
        color: trend_color(q.trend()),
    };
    if range == Range::Day {
        let first_s = q.spark.first().map_or(0, |p| p.1.to_bits());
        let last_s = q.spark.last().map_or(0, |p| p.1.to_bits());
        line.key = format!("{}/1d/{board_at}/{first_s}/{last_s}", q.code);
        line.values = q.spark.iter().map(|p| f64::from(p.1)).collect();
        line.minutes = q.spark.iter().map(|p| p.0).collect();
        if let Some((lo, hi)) = crypto::bounds(line.values.iter().copied(), Some(q.prev_close)) {
            let y = |v: f64| ((v - lo) / (hi - lo)) as f32;
            let n = (q.spark.len().saturating_sub(1)).max(1) as f32;
            line.pts = q.spark.iter().enumerate().map(|(i, p)| (i as f32 / n, y(f64::from(p.1)))).collect();
            line.base = Some(y(q.prev_close));
        }
        return line;
    }

    let today = crypto::today_num();
    let days_back = match range {
        Range::Month => 30,
        Range::HalfYear => 180,
        Range::Year => 365,
        Range::Day => 1,
    };
    let hist = st.crypto.years.get(&q.code);
    let has_real = hist.is_some();
    let raw_pts: Vec<(i32, f64)> = if let Some(h) = hist {
        h.closes.iter().copied().filter(|(d, _)| *d >= today - days_back).collect()
    } else {
        let sample = crypto::sample_history(&q.code, today, q.price);
        sample.closes.into_iter().filter(|(d, _)| *d >= today - days_back).collect()
    };

    line.key = format!("{}/{}/{}/{}/{}", q.code, range.label(), has_real, raw_pts.len(), q.price.to_bits());
    line.days = raw_pts.iter().map(|(d, _)| i64::from(*d)).collect();
    line.values = raw_pts.iter().map(|(_, p)| *p).collect();
    if let Some(last) = line.values.last_mut() {
        *last = q.price;
    }
    let vals = &line.values;
    if let (Some((lo, hi)), Some(&first), Some(&last)) = (crypto::bounds(vals.iter().copied(), None), vals.first(), vals.last()) {
        let n = (vals.len().saturating_sub(1)).max(1) as f32;
        line.pts = vals.iter().enumerate().map(|(i, &c)| (i as f32 / n, ((c - lo) / (hi - lo)) as f32)).collect();
        line.color = if (last - first).abs() < 1e-6 {
            Color::white(0.36)
        } else if last > first {
            palette::GREEN
        } else {
            palette::RED
        };
    }
    line
}

fn morph_k(t0: f64, now: f64) -> f32 {
    ease_out(((now - t0) / MORPH_S) as f32)
}

fn lerp_pts(a: &[(f32, f32)], b: &[(f32, f32)], t: f32) -> Vec<(f32, f32)> {
    if a.len() != b.len() {
        return b.to_vec();
    }
    a.iter().zip(b).map(|(p, q)| (lerp(p.0, q.0, t), lerp(p.1, q.1, t))).collect()
}

fn lerp_base(a: Option<f32>, b: Option<f32>, t: f32) -> Option<f32> {
    match (a, b) {
        (Some(a), Some(b)) => Some(lerp(a, b, t)),
        (_, b) => b.filter(|_| t >= 0.5),
    }
}

fn lerp_color(a: Option<Color>, b: Option<Color>, t: f32) -> Color {
    match (a, b) {
        (Some(a), Some(b)) => Color::lerp(a, b, t),
        (_, Some(b)) => b,
        (Some(a), None) => a,
        _ => Color::white(0.36),
    }
}

fn range_control(ui: &mut Ui, r: Rect, now: Range) -> Option<Range> {
    let mut out = None;
    for (i, range) in Range::ALL.into_iter().enumerate() {
        let seg = Rect::new(r.x + i as f32 * SEG_W, r.y, SEG_W, r.h);
        let resp = ui.button(wid_n("crypto.range", i as u64), seg);
        let on = range == now;
        if on {
            ui.c.fill_rrect(seg.inset(1.0, 0.0), r.h / 2.0, Color::white(0.16));
        } else if resp.hovered {
            ui.c.fill_rrect(seg.inset(1.0, 0.0), r.h / 2.0, Color::white(0.07));
        }
        let c = if on || resp.hovered { palette::TEXT } else { palette::TEXT_DIM };
        ui.c.text(range.label(), seg, &TextStyle::footnote().weight(Weight::Semibold).center().color(c));
        if resp.clicked && !on {
            out = Some(range);
        }
    }
    out
}

fn search(ui: &mut Ui, st: &mut AppState, r: Rect, area: Rect, dt: f32, moving: &mut bool) {
    let accent = st.accent;
    let id = wid("crypto.query");
    let cancel_w = ui.pill_width("Cancel", false, true);
    let cancel = Rect::new(r.right() - cancel_w, r.y + (FIELD_H - PILL_SMALL) / 2.0, cancel_w, PILL_SMALL);
    let field = Rect::new(r.x, r.y, (cancel.x - 8.0 - r.x).max(0.0), FIELD_H);
    let card = super::card::pill_card(st, r);

    let s = &mut st.ui.crypto;
    if std::mem::take(&mut s.focus) {
        ui.focus_text(id, s.query.chars().count());
    }
    let mut moves = 0i32;
    if ui.input.focused == Some(id) {
        ui.input.keys.retain(|k| match k {
            KeyInput::Up => {
                moves -= 1;
                false
            }
            KeyInput::Down => {
                moves += 1;
                false
            }
            _ => true,
        });
    }
    let resp = ui.text_input_row(id, field, &mut s.query, "Top 100 CMC / Binance code or name", 24, accent, Some(Icon::Search));
    if resp.changed {
        s.hit = 0;
        s.srch_scroll = 0.0;
        s.srch_shown_scroll = 0.0;
    }
    let matches = crypto::search_coins(&s.query, &st.store.crypto.codes);
    if !matches.is_empty() {
        s.hit = (s.hit as i32 + moves).clamp(0, matches.len() as i32 - 1) as usize;
    }

    let typed_code = crypto::code_of(&s.query);
    let mut chosen = None;
    if resp.submitted {
        chosen = matches.get(s.hit).map(|&(c, _)| c.to_string()).or_else(|| typed_code.clone());
    }
    let cancelled = ui.pill(wid("crypto.cancel"), cancel, "Cancel", None, PillStyle::Utility).clicked || resp.cancelled;

    let top = r.y + FIELD_H + 8.0;
    let list_r = Rect::new(r.x, top, r.w, (r.bottom() - top).max(0.0));
    let content_h = matches.len() as f32 * RESULT_H;
    let max_scroll = (content_h - list_r.h).max(0.0);

    // Auto-scroll when moving selection via Up/Down keys
    if moves != 0 && !matches.is_empty() {
        let hit_y = s.hit as f32 * RESULT_H;
        if hit_y < s.srch_scroll {
            s.srch_scroll = hit_y.clamp(0.0, max_scroll);
            *moving = true;
        } else if hit_y + RESULT_H > s.srch_scroll + list_r.h {
            s.srch_scroll = (hit_y + RESULT_H - list_r.h).clamp(0.0, max_scroll);
            *moving = true;
        }
    }

    // Mouse wheel scrolling inside the search list
    let notches = ui.wheel_in(area);
    if notches != 0.0 {
        *moving = true;
    }
    s.srch_scroll = (s.srch_scroll - notches * RESULT_H * 1.5).clamp(0.0, max_scroll);

    // Scrollbar & click-and-drag scrolling for the search list
    let bar_hit = Rect::new(list_r.right() - 8.0, list_r.y, 8.0, list_r.h);
    if max_scroll > 0.0 {
        if ui.input.just_pressed {
            if let Some(m) = ui.input.mouse {
                if bar_hit.contains(m) {
                    let track_h = (list_r.h - 8.0).max(1.0);
                    let frac = ((m.y - (list_r.y + 4.0)) / track_h).clamp(0.0, 1.0);
                    s.srch_scroll = (frac * max_scroll).clamp(0.0, max_scroll);
                    s.srch_shown_scroll = s.srch_scroll;
                    s.srch_bar_drag = Some((m.y, s.srch_scroll));
                } else if list_r.contains(m) {
                    s.srch_list_drag = Some((m.y, s.srch_scroll, false));
                }
            }
        }
        if ui.input.down {
            if let (Some((y0, sc0)), Some(m)) = (s.srch_bar_drag, ui.input.mouse) {
                let track_h = (list_r.h - 8.0).max(1.0);
                let thumb_h = (track_h * (list_r.h / content_h)).clamp(24.0, track_h);
                let travel = (track_h - thumb_h).max(1.0);
                s.srch_scroll = (sc0 + (m.y - y0) / travel * max_scroll).clamp(0.0, max_scroll);
                s.srch_shown_scroll = s.srch_scroll;
                ui.input.pressed_id = None;
            } else if let (Some((y0, sc0, moved)), Some(m)) = (&mut s.srch_list_drag, ui.input.mouse) {
                let dy = m.y - *y0;
                if *moved || dy.abs() > 6.0 {
                    *moved = true;
                    s.srch_scroll = (*sc0 - dy).clamp(0.0, max_scroll);
                    s.srch_shown_scroll = s.srch_scroll;
                    ui.input.pressed_id = None;
                    ui.input.clicked = None;
                }
            }
        } else {
            s.srch_bar_drag = None;
            s.srch_list_drag = None;
        }
    } else {
        s.srch_bar_drag = None;
        s.srch_list_drag = None;
    }

    s.srch_shown_scroll = glide(s.srch_shown_scroll.clamp(0.0, max_scroll), s.srch_scroll, dt, 0.25, moving);

    if matches.is_empty() {
        let hint_r = Rect::new(r.x + 4.0, top + 2.0, r.w - 8.0, 44.0);
        let hint = TextStyle::footnote().color(palette::TEXT_DIM).wrap().top();
        let msg = match &typed_code {
            Some(c) if st.store.crypto.codes.iter().any(|x| x.eq_ignore_ascii_case(c)) => format!("{c} is on your list already."),
            Some(c) => format!("Press Enter to add {c}USDT from Binance."),
            None => {
                if s.query.trim().is_empty() {
                    "Type a coin code or name, like BTC or HYPE.".to_string()
                } else {
                    "No matching CoinMarketCap / Binance coin.".to_string()
                }
            }
        };
        ui.c.text(&msg, hint_r, &hint);
    } else {
        ui.c.clip(Rect::new(list_r.x - 8.0, list_r.y, list_r.w + 16.0, list_r.h));
        let y_base = list_r.y - s.srch_shown_scroll;
        for (i, &(code, name)) in matches.iter().enumerate() {
            let row = Rect::new(r.x, y_base + i as f32 * RESULT_H, r.w, RESULT_H);
            let Some(row_hit) = intersect(row, list_r) else {
                continue;
            };
            let b = ui.button(wid_n("crypto.srch_hit", code_n(code)), row_hit);
            if b.clicked {
                chosen = Some(code.to_string());
            }
            let lit = i == s.hit || b.hovered;
            if lit {
                super::card::fill_box(ui, card, row.inset(0.0, 1.0), 9.0, Color::white(if b.pressed { 0.12 } else if i == s.hit { 0.09 } else { 0.06 }));
            }
            let tx = row.x + ROW_PAD;
            let code_st = TextStyle::body().weight(Weight::Semibold);
            let cw = ui.c.measure(code, &code_st, 100.0).0.ceil().max(48.0);
            ui.c.text(code, Rect::new(tx, row.y, cw + 2.0, row.h), &code_st);
            ui.c.text(name, Rect::new(tx + cw + 12.0, row.y, row.right() - tx - cw - 20.0, row.h), &TextStyle::footnote().color(palette::TEXT_DIM));
        }

        if s.srch_shown_scroll > 0.5 {
            fade(ui, Rect::new(list_r.x - 8.0, list_r.y, list_r.w + 16.0, 12.0), true);
        }
        if s.srch_shown_scroll < max_scroll - 0.5 {
            fade(ui, Rect::new(list_r.x - 8.0, list_r.bottom() - 14.0, list_r.w + 16.0, 14.0), false);
        }
        if max_scroll > 0.5 && (ui.hovered(list_r) || s.srch_bar_drag.is_some()) {
            let track_h = (list_r.h - 8.0).max(1.0);
            let thumb_h = (track_h * (list_r.h / content_h)).clamp(24.0, track_h);
            let thumb_y = list_r.y + 4.0 + (s.srch_shown_scroll / max_scroll).clamp(0.0, 1.0) * (track_h - thumb_h);
            let active = ui.hovered(bar_hit) || s.srch_bar_drag.is_some();
            let w = if active { 4.0 } else { 2.5 };
            ui.c.fill_rrect(Rect::new(list_r.right() - w - 1.0, thumb_y, w, thumb_h), w / 2.0, Color::white(if active { 0.42 } else { 0.22 }));
        }
        ui.c.unclip();
    }

    if let Some(code) = chosen {
        if st.store.crypto.add(&code) {
            st.store.crypto.pick = code;
            st.ui.crypto.scroll = f32::MAX;
            st.save();
        }
        close_search(ui, &mut st.ui.crypto);
    } else if cancelled {
        close_search(ui, &mut st.ui.crypto);
    }
}

fn close_search(ui: &mut Ui, s: &mut CryptoUi) {
    s.adding = false;
    s.query.clear();
    s.hit = 0;
    s.srch_scroll = 0.0;
    s.srch_shown_scroll = 0.0;
    s.srch_list_drag = None;
    s.srch_bar_drag = None;
    s.swap_k = 0.0;
    ui.blur();
}

fn dotted(ui: &mut Ui, x0: f32, x1: f32, y: f32, c: Color) {
    let mut segs = Vec::with_capacity(((x1 - x0) / 2.0).max(0.0) as usize + 2);
    let mut x = x0 + 0.5;
    while x + 0.5 < x1 {
        segs.push(Seg::MoveTo(x, y));
        segs.push(Seg::LineTo(x + 0.5, y));
        x += 4.0;
    }
    ui.c.path(&segs, None, Some((c, 1.2)));
}

fn draw_line(ui: &mut Ui, pts: &[(f32, f32)], color: Color, top: f32, bottom: f32, width: f32, fade: Option<f32>) {
    if pts.len() < 2 {
        return;
    }
    let line: Vec<Seg> = pts.iter().enumerate().map(|(i, &(x, y))| if i == 0 { Seg::MoveTo(x, y) } else { Seg::LineTo(x, y) }).collect();
    let mut area = line.clone();
    area.push(Seg::LineTo(pts[pts.len() - 1].0, bottom));
    area.push(Seg::LineTo(pts[0].0, bottom));
    area.push(Seg::Close);
    match fade {
        Some(alpha) => ui.c.fill_path_fade(&area, color, top, bottom, alpha, 0.0),
        None => ui.c.path(&area, Some(color.mul_alpha(0.16)), None),
    }
    ui.c.path(&line, None, Some((color, width)));
}

fn fade(ui: &mut Ui, r: Rect, top: bool) {
    if ui.c.look().material == poni_core::look::Material::Glass {
        return;
    }
    const STEPS: usize = 8;
    let step = r.h / STEPS as f32;
    for k in 0..STEPS {
        let a = 0.92 * (1.0 - (k as f32 + 0.5) / STEPS as f32).powf(1.4);
        let y = if top { r.y + k as f32 * step } else { r.bottom() - (k as f32 + 1.0) * step };
        ui.c.fill_rect(Rect::new(r.x, y, r.w, step), palette::ISLAND.with_alpha(a));
    }
}
