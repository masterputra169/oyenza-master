//! Forex & Commodities page (the Forex add-on): real-time exchange rates,
//! Gold (Antam/LM), Silver, and Oil spot prices, intraday sparklines,
//! and 1D/1M/6M/1Y historical charts.

use crate::gfx::{TextStyle, Weight};
use crate::icons::Icon;
use crate::state::AppState;
use crate::ui::{wid, wid_n, KeyInput, PillStyle, Ui, PILL_SMALL};
use poni_core::geom::{palette, Color, Point, Rect};
use poni_core::stocks::{self, Range};
use poni_core::svgpath::Seg;

const HEIGHT: f32 = 256.0;
const ADD_H: f32 = 46.0;
const EDIT_W: f32 = 26.0;
const FIELD_H: f32 = 34.0;
const RESULT_H: f32 = 36.0;
const LIST_MIN: f32 = 232.0;
const LIST_MAX: f32 = 300.0;
const COL_GAP: f32 = 20.0;
const ROW_H: f32 = 48.0;
const ROW_PAD: f32 = 8.0;
const PILL_W: f32 = 66.0;
const PILL_H: f32 = 20.0;
const SPARK_W: f32 = 50.0;
const PRICE_Y: f32 = 28.0;
const CHART_Y: f32 = 62.0;
const CHART_H: f32 = 106.0;
const AXIS_H: f32 = 16.0;
const STATS_Y: f32 = 192.0;
const STAT_H: f32 = 32.0;
const SEG_W: f32 = 32.0;
const SEG_H: f32 = 22.0;
const GLIDE: f32 = 16.0;
const MORPH_S: f64 = 0.45;
const SAMPLES: usize = 160;

#[derive(Clone, Debug, Default)]
pub struct Morph {
    pub key: String,
    pub from: Vec<(f32, f32)>,
    pub to: Vec<(f32, f32)>,
    pub base_from: Option<f32>,
    pub base_to: Option<f32>,
    pub color_from: Option<Color>,
    pub color_to: Option<Color>,
    pub t0: f64,
}

#[derive(Clone, Debug, Default)]
pub struct ForexUi {
    pub scroll: f32,
    pub shown_scroll: f32,
    pub list_drag: Option<(f32, f32, bool)>,
    pub bar_drag: Option<(f32, f32)>,
    pub pick_y: Option<f32>,
    pub chart: Morph,
    pub last_frame: f64,
    pub adding: bool,
    pub query: String,
    hit: usize,
    focus: bool,
    pub editing: bool,
    edit_k: f32,
    swap_k: f32,
    srch_scroll: f32,
    srch_shown_scroll: f32,
    srch_list_drag: Option<(f32, f32, bool)>,
    srch_bar_drag: Option<(f32, f32)>,
}

pub fn height(_st: &AppState) -> f32 {
    HEIGHT
}

pub fn draw(ui: &mut Ui, st: &mut AppState, area: Rect) {
    let now = st.now;
    let mut dt = (now - st.ui.forex.last_frame).clamp(0.0, 0.05) as f32;
    if st.ui.forex.last_frame == 0.0 {
        dt = 0.016;
    }
    st.ui.forex.last_frame = now;

    let w = area.w;
    let list_w = (w * 0.44).clamp(LIST_MIN, LIST_MAX);
    let list_r = Rect::new(area.x, area.y, list_w, area.h);
    let detail_x = list_r.right() + COL_GAP;
    let detail_r = Rect::new(detail_x, area.y, (area.right() - detail_x).max(0.0), area.h);

    let mut moving = false;
    let s = &mut st.ui.forex;
    s.swap_k = glide(s.swap_k, 1.0, dt, 0.01, &mut moving);
    s.edit_k = glide(s.edit_k, if s.editing { 1.0 } else { 0.0 }, dt, 0.01, &mut moving);

    let base = ui.c.alpha();
    ui.c.set_alpha(base * ease_out(st.ui.forex.swap_k));
    if st.ui.forex.adding {
        forex_search(ui, st, list_r, area, dt, &mut moving);
    } else {
        forex_list(ui, st, list_r, area, dt, &mut moving);
    }
    ui.c.set_alpha(base);
    forex_detail(ui, st, detail_r);

    if moving {
        ui.request_anim();
    }
}

fn code_n(code: &str) -> u64 {
    let mut h = 0xcbf29ce484222325u64;
    for b in code.bytes() {
        h = (h ^ (b as u64)).wrapping_mul(0x100000001b3);
    }
    h
}

fn intersect(a: Rect, b: Rect) -> Option<Rect> {
    let x1 = a.x.max(b.x);
    let y1 = a.y.max(b.y);
    let x2 = a.right().min(b.right());
    let y2 = a.bottom().min(b.bottom());
    (x2 > x1 && y2 > y1).then(|| Rect::new(x1, y1, x2 - x1, y2 - y1))
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

fn morph_k(t0: f64, now: f64) -> f32 {
    ease_out(((now - t0) / MORPH_S) as f32)
}

fn lerp(a: f32, b: f32, t: f32) -> f32 {
    a + (b - a) * t
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

fn fade(ui: &mut Ui, r: Rect, at_top: bool) {
    let bg = palette::SURFACE;
    let n = 12;
    for i in 0..n {
        let t = i as f32 / n as f32;
        let y = if at_top { r.y + t * r.h } else { r.bottom() - t * r.h };
        let a = (1.0 - t).powi(2);
        ui.c.fill_rect(Rect::new(r.x, y, r.w, r.h / n as f32 + 0.5), bg.mul_alpha(a * 0.95));
    }
}

fn mini_forex_chart(ui: &mut Ui, q: &poni_core::forex::ForexQuote, r: Rect) {
    let Some((lo, hi)) = stocks::bounds(q.spark.iter().map(|p| f64::from(p.1)), Some(q.prev_close)) else { return };
    let y = |v: f64| r.bottom() - ((v - lo) / (hi - lo)) as f32 * r.h;
    dotted(ui, r.x, r.right(), y(q.prev_close).round() + 0.5, Color::white(0.30));
    if q.spark.len() < 2 {
        return;
    }
    let col = match q.trend() {
        1 => palette::GREEN,
        -1 => palette::RED,
        _ => Color::white(0.36),
    };
    let n = (q.spark.len() - 1).max(1) as f32;
    let pts: Vec<(f32, f32)> = q.spark.iter().enumerate().map(|(i, p)| (r.x + (i as f32 / n) * r.w, y(f64::from(p.1)))).collect();
    draw_line(ui, &pts, col, r.y, r.bottom(), 1.25, None);
}

fn forex_list(ui: &mut Ui, st: &mut AppState, r: Rect, area: Rect, dt: f32, moving: &mut bool) {
    let board = st.forex.board.clone().unwrap_or_else(poni_core::forex::ForexBoard::default_board);
    let quotes: Vec<poni_core::forex::ForexQuote> = st.store.forex.symbols.iter().map(|sym| {
        board.quote(sym).cloned().unwrap_or_else(|| {
            let name = poni_core::forex::KNOWN_FOREX_ITEMS.iter().find(|(s, _)| s.eq_ignore_ascii_case(sym)).map(|(_, n)| *n).unwrap_or(sym);
            poni_core::forex::ForexQuote {
                symbol: sym.clone(),
                name: name.to_string(),
                rate: 0.0,
                open: 0.0,
                prev_close: 0.0,
                low: 0.0,
                high: 0.0,
                change_pct: 0.0,
                is_gold: sym == "GOLD",
                unit: String::new(),
                spark: Vec::new(),
            }
        })
    }).collect();
    let n = quotes.len();
    let content_h = n as f32 * ROW_H + ADD_H;
    let max_scroll = (content_h - r.h).max(0.0);
    let card = crate::pages::card::pill_card(st, r);

    let pick_raw = st.store.forex.pick.clone();
    let pick = poni_core::forex::normalize_symbol(&pick_raw);
    let s = &mut st.ui.forex;

    let notches = ui.wheel_in(area);
    if notches != 0.0 {
        *moving = true;
    }
    s.scroll = (s.scroll - notches * ROW_H).clamp(0.0, max_scroll);

    let bar_hit = Rect::new(r.right() - 8.0, r.y, 8.0, r.h);
    if max_scroll > 0.0 {
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
    let top = r.y - s.shown_scroll;

    let pick_idx = quotes.iter().position(|q| q.symbol.eq_ignore_ascii_case(&pick)).unwrap_or(0);
    let pick_target = pick_idx as f32 * ROW_H;
    let pick_y = match s.pick_y {
        Some(y) => glide(y, pick_target, dt, 0.25, moving),
        None => pick_target,
    };
    s.pick_y = Some(pick_y);
    let show_pct = st.store.forex.show_pct;

    ui.c.clip(Rect::new(r.x - 8.0, r.y, r.w + 16.0, r.h));

    let editing = s.editing || s.edit_k > 0.01;
    if !editing {
        let hl = Rect::new(r.x, top + pick_y + 2.0, r.w, ROW_H - 4.0);
        crate::pages::card::fill_box(ui, card, hl, 10.0, Color::white(0.10));
    }

    let mut clicked_sym = None;
    let mut clicked_y = None;
    let mut toggle_mode = false;
    let mut removed = None;

    let shift = EDIT_W * ease_out(s.edit_k);

    for (i, q) in quotes.iter().enumerate() {
        let y = i as f32 * ROW_H;
        let row = Rect::new(r.x, top + y, r.w, ROW_H);
        let Some(hit_clip) = intersect(Rect::new(row.x, row.y + 2.0, row.w, row.h - 4.0), r) else {
            continue;
        };
        let is_picked = q.symbol.eq_ignore_ascii_case(&pick);
        let resp = ui.button(wid_n("forex.item", code_n(&q.symbol)), hit_clip);
        if (resp.hovered || resp.pressed) && !is_picked && !editing {
            crate::pages::card::fill_box(
                ui,
                card,
                Rect::new(row.x, row.y + 2.0, row.w, row.h - 4.0),
                10.0,
                Color::white(if resp.pressed { 0.08 } else { 0.05 }),
            );
        }
        if resp.clicked && !editing {
            clicked_sym = Some(q.symbol.clone());
            clicked_y = Some(y);
        }

        let x0 = row.x + ROW_PAD + shift;
        let right = row.right() - ROW_PAD;
        let pill = Rect::new(right - PILL_W, row.y + 25.0, PILL_W, PILL_H);
        let spark = Rect::new(pill.x - 8.0 - SPARK_W, row.y + 11.0, SPARK_W, 26.0);
        let text_w = (spark.x - 6.0 - x0).max(0.0);

        ui.c.text(&q.symbol, Rect::new(x0, row.y + 6.0, text_w, 19.0), &TextStyle::headline().weight(Weight::Semibold));
        ui.c.text(&q.name, Rect::new(x0, row.y + 25.0, text_w, 17.0), &TextStyle::footnote().color(palette::TEXT_DIM));

        let base = ui.c.alpha();
        let k = 1.0 - s.edit_k;
        if k > 0.01 {
            ui.c.set_alpha(base * k);
            mini_forex_chart(ui, q, spark);

            let price_str = q.formatted_rate();
            ui.c.text(&price_str, Rect::new(right - 124.0, row.y + 5.0, 124.0, 19.0), &TextStyle::body().weight(Weight::Semibold).tabular().right());

            let mut fill = match q.trend() {
                1 => palette::GREEN,
                -1 => palette::RED,
                _ => Color::white(0.36),
            };
            if let Some(pill_clip) = intersect(pill, r) {
                let pr = ui.button(wid_n("forex.pill", code_n(&q.symbol)), pill_clip);
                if pr.pressed {
                    fill = fill.mul_alpha(0.75);
                }
                if pr.clicked && !editing {
                    toggle_mode = true;
                }
            }
            ui.c.fill_rrect(pill, 6.0, fill);
            let chg_str = q.formatted_change(show_pct);
            ui.c.text(&chg_str, Rect::new(pill.x + 5.0, pill.y, pill.w - 10.0, pill.h), &TextStyle::footnote().weight(Weight::Semibold).tabular().right().color(Color::white(1.0)));
            ui.c.set_alpha(base);
        }

        if editing {
            ui.c.set_alpha(base * s.edit_k);
            let c = Point::new(row.x + ROW_PAD + 9.0, row.center().y);
            let rm = intersect(Rect::from_center(c, 26.0, 26.0), r).map(|rh| ui.button(wid_n("forex.del", code_n(&q.symbol)), rh)).unwrap_or_default();
            ui.c.fill_circle(c, 9.0, if rm.pressed { palette::RED.mul_alpha(0.7) } else { palette::RED });
            ui.c.fill_rrect(Rect::from_center(c, 9.0, 2.0), 1.0, Color::white(1.0));
            if rm.clicked {
                removed = Some(q.symbol.clone());
            }
            ui.c.set_alpha(base);
        }
    }

    let add = Rect::new(r.x, top + n as f32 * ROW_H, r.w, ADD_H);
    let line = Rect::new(add.x + ROW_PAD, add.y + 4.0, add.w - 2.0 * ROW_PAD, 22.0);
    let edit_label = if s.editing { "Done" } else { "Edit" };
    let edit_st = TextStyle::body().weight(Weight::Semibold).right();
    let ew = ui.c.measure(edit_label, &edit_st, 100.0).0.ceil();
    let edit_r = Rect::new(line.right() - ew - 8.0, line.y - 3.0, ew + 16.0, line.h + 6.0);
    let mut start_add = false;
    if intersect(add, r).is_some() {
        if st.store.forex.full() {
            ui.c.text("Your list is full: 20 instruments", line, &TextStyle::footnote().color(palette::TEXT_FAINT));
        } else if !s.editing {
            let add_r = Rect::new(add.x, line.y - 3.0, (edit_r.x - add.x - 8.0).max(0.0), line.h + 6.0);
            if let Some(add_hit) = intersect(add_r, r) {
                let resp = ui.button(wid("forex.add"), add_hit);
                if resp.hovered || resp.pressed {
                    crate::pages::card::fill_box(ui, card, add_r, 9.0, Color::white(if resp.pressed { 0.10 } else { 0.06 }));
                }
                start_add = resp.clicked;
            }
            let cy = line.center().y;
            ui.c.fill_circle(Point::new(line.x + 9.0, cy), 9.0, st.accent.with_alpha(0.20));
            ui.c.icon(Icon::Plus, Rect::from_center(Point::new(line.x + 9.0, cy), 13.0, 13.0), st.accent);
            ui.c.text("Add instrument", Rect::new(line.x + 26.0, line.y, line.w - 26.0 - ew - 20.0, line.h), &TextStyle::body().weight(Weight::Semibold).color(st.accent));
        }
        if !st.store.forex.symbols.is_empty() || s.editing {
            if let Some(edit_hit) = intersect(edit_r, r) {
                let resp = ui.button(wid("forex.edit"), edit_hit);
                let c = if resp.hovered { palette::TEXT } else { palette::TEXT_DIM };
                ui.c.text(edit_label, Rect::new(edit_r.x, line.y, edit_r.w - 8.0, line.h), &edit_st.color(c));
                if resp.clicked {
                    s.editing = !s.editing;
                }
            }
        }
        let credit = "Live spot & foreign exchange rates";
        ui.c.text(credit, Rect::new(line.x, add.y + 28.0, line.w, 14.0), &TextStyle::caption().color(palette::TEXT_FAINT));
    }

    if s.shown_scroll > 0.5 {
        fade(ui, Rect::new(r.x - 8.0, r.y, r.w + 16.0, 14.0), true);
    }
    if s.shown_scroll < max_scroll - 0.5 {
        fade(ui, Rect::new(r.x - 8.0, r.bottom() - 18.0, r.w + 16.0, 18.0), false);
    }
    if max_scroll > 0.5 && (ui.hovered(r) || s.bar_drag.is_some()) {
        let track_h = (r.h - 8.0).max(1.0);
        let thumb_h = (track_h * (r.h / content_h)).clamp(24.0, track_h);
        let thumb_y = r.y + 4.0 + (s.shown_scroll / max_scroll).clamp(0.0, 1.0) * (track_h - thumb_h);
        let active = ui.hovered(bar_hit) || s.bar_drag.is_some();
        let w = if active { 4.0 } else { 2.5 };
        ui.c.fill_rrect(Rect::new(r.right() - w - 1.0, thumb_y, w, thumb_h), w / 2.0, Color::white(if active { 0.42 } else { 0.22 }));
    }
    ui.c.unclip();

    let mut save = false;
    if toggle_mode {
        st.store.forex.show_pct = !st.store.forex.show_pct;
        save = true;
    } else if let Some(sym) = clicked_sym {
        st.store.forex.pick = sym.clone();
        st.ui.forex_pick = Some(sym);
        save = true;
        if let Some(py) = clicked_y {
            if py < s.scroll {
                s.scroll = py.clamp(0.0, max_scroll);
                *moving = true;
            } else if py + ROW_H > s.scroll + r.h {
                s.scroll = (py + ROW_H - r.h + 18.0).clamp(0.0, max_scroll);
                *moving = true;
            }
        }
    }
    if let Some(sym) = removed {
        st.store.forex.remove(&sym);
        if st.store.forex.symbols.is_empty() {
            s.editing = false;
        }
        save = true;
    }
    if start_add {
        s.adding = true;
        s.focus = true;
        s.query.clear();
        s.hit = 0;
        s.swap_k = 0.0;
    }
    if save {
        st.save();
    }
}

fn forex_search(ui: &mut Ui, st: &mut AppState, r: Rect, area: Rect, dt: f32, moving: &mut bool) {
    let accent = st.accent;
    let id = wid("forex.query");
    let cancel_w = ui.pill_width("Cancel", false, true);
    let cancel = Rect::new(r.right() - cancel_w, r.y + (FIELD_H - PILL_SMALL) / 2.0, cancel_w, PILL_SMALL);
    let field = Rect::new(r.x, r.y, (cancel.x - 8.0 - r.x).max(0.0), FIELD_H);
    let card = crate::pages::card::pill_card(st, r);

    let s = &mut st.ui.forex;
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
    let resp = ui.text_input_row(id, field, &mut s.query, "Currency or commodity, like EUR or Gold", 32, accent, Some(Icon::Search));
    if resp.changed {
        s.hit = 0;
        s.srch_scroll = 0.0;
        s.srch_shown_scroll = 0.0;
    }
    let matches = poni_core::forex::search_forex(&s.query, &st.store.forex.symbols);
    if !matches.is_empty() {
        s.hit = (s.hit as i32 + moves).clamp(0, matches.len() as i32 - 1) as usize;
    }

    let query_clean = s.query.trim().to_ascii_uppercase();
    let typed_symbol = if !query_clean.is_empty() && !st.store.forex.has(&query_clean) {
        Some(query_clean.clone())
    } else {
        None
    };

    let mut chosen = None;
    if resp.submitted {
        chosen = matches.get(s.hit).map(|(c, _)| c.to_string()).or(typed_symbol.clone());
    }
    let cancelled = ui.pill(wid("forex.cancel"), cancel, "Cancel", None, PillStyle::Utility).clicked || resp.cancelled;

    let top = r.y + FIELD_H + 8.0;
    let list_r = Rect::new(r.x, top, r.w, (r.bottom() - top).max(0.0));
    let content_h = matches.len() as f32 * RESULT_H;
    let max_scroll = (content_h - list_r.h).max(0.0);

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

    let notches = ui.wheel_in(area);
    if notches != 0.0 {
        *moving = true;
    }
    s.srch_scroll = (s.srch_scroll - notches * RESULT_H * 1.5).clamp(0.0, max_scroll);

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
        let msg = match &typed_symbol {
            Some(c) if st.store.forex.has(c) => format!("{c} is on your list already."),
            Some(c) => format!("Press Enter to add '{c}'."),
            None => {
                if st.store.forex.full() {
                    "Your list is full: remove an instrument first.".to_string()
                } else if s.query.trim().is_empty() {
                    "Type a currency or commodity name.".to_string()
                } else {
                    "No matching currency or commodity found.".to_string()
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
            let b = ui.button(wid_n("forex.srch_hit", code_n(code)), row_hit);
            if b.clicked {
                chosen = Some(code.to_string());
            }
            let lit = i == s.hit || b.hovered;
            if lit {
                crate::pages::card::fill_box(ui, card, row.inset(0.0, 1.0), 9.0, Color::white(if b.pressed { 0.12 } else if i == s.hit { 0.09 } else { 0.06 }));
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

    if let Some(symbol) = chosen {
        if st.store.forex.add(&symbol) {
            st.store.forex.pick = symbol.clone();
            st.ui.forex_pick = Some(symbol);
            st.ui.forex.scroll = f32::MAX;
            st.save();
        }
        close_forex_search(ui, &mut st.ui.forex);
    } else if cancelled {
        close_forex_search(ui, &mut st.ui.forex);
    }
}

fn close_forex_search(ui: &mut Ui, s: &mut ForexUi) {
    s.adding = false;
    s.query.clear();
    s.hit = 0;
    s.swap_k = 0.0;
    ui.blur();
}

struct Line {
    key: String,
    pts: Vec<(f32, f32)>,
    values: Vec<f64>,
    labels: Vec<String>,
    left_end: String,
    right_end: String,
    base: Option<f32>,
    color: Color,
}

impl Line {
    fn label(&self, i: usize) -> String {
        self.labels.get(i).cloned().unwrap_or_default()
    }

    fn ends(&self) -> (String, String) {
        (self.left_end.clone(), self.right_end.clone())
    }
}

fn forex_chart_line(st: &AppState, q: &poni_core::forex::ForexQuote, range: Range) -> Line {
    let trend_col = match q.trend() {
        1 => palette::GREEN,
        -1 => palette::RED,
        _ => Color::white(0.36),
    };
    let mut line = Line {
        key: String::new(),
        pts: Vec::new(),
        values: Vec::new(),
        labels: Vec::new(),
        left_end: "24h ago".to_string(),
        right_end: "Now".to_string(),
        base: None,
        color: trend_col,
    };
    match range {
        Range::Day => {
            let last_s = q.spark.last().map_or(0, |p| p.1.to_bits());
            line.key = format!("{}/1d/{last_s}/{}", q.symbol, q.spark.len());
            line.values = q.spark.iter().map(|p| f64::from(p.1)).collect();
            let n = q.spark.len();
            line.labels = q.spark.iter().enumerate().map(|(i, _)| {
                let hrs_ago = (n.saturating_sub(1 + i) * 30) / 60;
                let mins_ago = (n.saturating_sub(1 + i) * 30) % 60;
                if hrs_ago == 0 && mins_ago == 0 {
                    "Now".to_string()
                } else if mins_ago == 0 {
                    format!("{hrs_ago}h ago")
                } else {
                    format!("{hrs_ago}h {mins_ago}m ago")
                }
            }).collect();
            if let Some((lo, hi)) = stocks::bounds(line.values.iter().copied(), Some(q.prev_close)) {
                let y = |v: f64| ((v - lo) / (hi - lo)) as f32;
                let pts_count = (line.values.len().saturating_sub(1)).max(1) as f32;
                line.pts = line.values.iter().enumerate().map(|(i, &v)| (i as f32 / pts_count, y(v))).collect();
                line.base = Some(y(q.prev_close));
            }
        }
        _ => {
            line.left_end = match range {
                Range::Month => "1 Mo Ago".to_string(),
                Range::HalfYear => "6 Mo Ago".to_string(),
                Range::Year => "1 Yr Ago".to_string(),
                _ => "1 Mo Ago".to_string(),
            };
            line.right_end = "Today".to_string();
            let hist = st.forex.years.get(&q.symbol);
            let count = match range {
                Range::Month => 22,
                Range::HalfYear => 130,
                Range::Year => 260,
                _ => 22,
            };
            let raw_pts: Vec<(i32, f64)> = if let Some(h) = hist {
                let start = h.closes.len().saturating_sub(count);
                h.closes[start..].to_vec()
            } else {
                let sample = poni_core::forex::sample_history(&q.symbol, poni_core::forex::today_num(), q.rate);
                let start = sample.closes.len().saturating_sub(count);
                sample.closes[start..].to_vec()
            };
            line.key = format!("{}/{}/{}/{}/{}", q.symbol, range.label(), hist.is_some(), raw_pts.len(), q.rate.to_bits());
            if !raw_pts.is_empty() {
                line.values = raw_pts.iter().map(|(_, p)| *p).collect();
                line.labels = raw_pts.iter().enumerate().map(|(i, _)| format!("{}d ago", raw_pts.len().saturating_sub(1 + i))).collect();
                if let Some(last) = line.values.last_mut() {
                    *last = q.rate;
                }
                let vals = &line.values;
                if let (Some((lo, hi)), Some(&first), Some(&last)) = (stocks::bounds(vals.iter().copied(), None), vals.first(), vals.last()) {
                    let n = (vals.len().saturating_sub(1)).max(1) as f32;
                    line.pts = vals.iter().enumerate().map(|(i, &c)| (i as f32 / n, ((c - lo) / (hi - lo)) as f32)).collect();
                    line.color = match stocks::trend_of(last, first) {
                        poni_core::stocks::Trend::Up => palette::GREEN,
                        poni_core::stocks::Trend::Down => palette::RED,
                        poni_core::stocks::Trend::Flat => Color::white(0.36),
                    };
                }
            } else if !q.spark.is_empty() {
                line.values = q.spark.iter().map(|p| f64::from(p.1)).collect();
                let n = q.spark.len();
                line.labels = q.spark.iter().enumerate().map(|(i, _)| {
                    let hrs_ago = (n.saturating_sub(1 + i) * 30) / 60;
                    if hrs_ago == 0 { "Now".to_string() } else { format!("{hrs_ago}h ago") }
                }).collect();
                if let Some((lo, hi)) = stocks::bounds(line.values.iter().copied(), Some(q.prev_close)) {
                    let y = |v: f64| ((v - lo) / (hi - lo)) as f32;
                    let pts_count = (line.values.len().saturating_sub(1)).max(1) as f32;
                    line.pts = line.values.iter().enumerate().map(|(i, &v)| (i as f32 / pts_count, y(v))).collect();
                    line.base = Some(y(q.prev_close));
                }
            }
        }
    }
    line
}
fn forex_detail(ui: &mut Ui, st: &mut AppState, r: Rect) {
    let pick_raw = st.store.forex.pick.clone();
    let pick = poni_core::forex::normalize_symbol(&pick_raw);
    let range = st.store.forex.range;

    let board = st.forex.board.clone().unwrap_or_else(poni_core::forex::ForexBoard::default_board);
    let Some(q) = board.quote(&pick).cloned() else { return };

    // 1. Title bar: Code, Pin, Alert, Instrument Name, and Range Selector
    let code_st = TextStyle::value().weight(Weight::Bold);
    let cw = ui.c.measure(&q.symbol, &code_st, 200.0).0.ceil();
    let title = Rect::new(r.x, r.y, r.w, 22.0);
    let seg_x = r.right() - SEG_W * Range::ALL.len() as f32;
    ui.c.text(&q.symbol, Rect::new(title.x, title.y, cw + 2.0, title.h), &code_st);

    let is_pinned = st.store.ticker.as_ref().is_some_and(|t| {
        t.enabled && t.source == "forex" && poni_core::forex::normalize_symbol(&t.code) == q.symbol
    });
    let pin_r = Rect::new(title.x + cw + 6.0, title.y - 1.0, 22.0, 22.0);
    let pin_resp = ui.button(wid("forex.pin"), pin_r);
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
                source: "forex".into(),
                code: q.symbol.clone(),
            });
        }
        st.save();
    }

    let has_alert = st.store.alerts.iter().any(|a| a.code.eq_ignore_ascii_case(&q.symbol) && a.enabled);
    let alert_r = Rect::new(pin_r.right() + 4.0, title.y - 1.0, 22.0, 22.0);
    let alert_resp = ui.button(wid("forex.alert"), alert_r);
    let alert_col = if has_alert { st.accent } else if alert_resp.hovered { palette::TEXT } else { palette::TEXT_FAINT };
    if alert_resp.hovered {
        ui.c.fill_circle(alert_r.center(), 10.0, Color::white(0.08));
    }
    ui.c.icon(Icon::Bell, Rect::from_center(alert_r.center(), 13.0, 13.0), alert_col);
    if alert_resp.clicked {
        if has_alert {
            st.store.alerts.retain(|a| !a.code.eq_ignore_ascii_case(&q.symbol));
            st.notify(Icon::Bell, palette::TEXT_DIM, "Alert Removed", &format!("Price alert for {} disabled", q.symbol), 3.0, None);
        } else {
            let target = q.rate * 1.02;
            let now_id = std::time::SystemTime::now().duration_since(std::time::UNIX_EPOCH).map_or(1, |d| d.as_secs());
            st.store.alerts.push(poni_core::store::PriceAlert {
                id: now_id,
                source: "forex".into(),
                code: q.symbol.clone(),
                target_price: target,
                above: true,
                triggered: false,
                enabled: true,
            });
            st.notify(Icon::Bell, st.accent, "Price Alert Set!", &format!("{}: Alert when price reaches {}", q.symbol, poni_core::forex::fmt_quote_val(&q.symbol, target)), 4.0, None);
        }
        st.save();
    }

    // Manual Refresh button
    let refresh_r = Rect::new(alert_r.right() + 4.0, title.y - 1.0, 22.0, 22.0);
    let refresh_resp = ui.button(wid("forex.refresh_btn"), refresh_r);
    let refresh_col = if refresh_resp.hovered { palette::TEXT } else { palette::TEXT_FAINT };
    if refresh_resp.hovered {
        ui.c.fill_circle(refresh_r.center(), 10.0, Color::white(0.08));
    }
    ui.c.icon(Icon::Reset, Rect::from_center(refresh_r.center(), 13.0, 13.0), refresh_col);
    if refresh_resp.clicked {
        st.forex.years.remove(&q.symbol);
        st.forex.refresh_seq = st.forex.refresh_seq.wrapping_add(1);
        st.notify(Icon::Reset, st.accent, "Refreshing...", &format!("Updating {} chart & rates...", q.symbol), 2.0, None);
    }

    // Instrument Name directly on top row
    let name_x = refresh_r.right() + 8.0;
    let name_r = Rect::new(name_x, title.y + 1.0, (seg_x - name_x - 10.0).max(0.0), title.h);
    ui.c.text(&q.name, name_r, &TextStyle::headline().color(palette::TEXT_DIM));

    // Range selector using identical style
    if let Some(pick_range) = range_control(ui, Rect::new(seg_x, title.y, r.right() - seg_x, SEG_H), range) {
        st.store.forex.range = pick_range;
        st.save();
    }

    // 2. Chart rendering with morphing & smooth curves
    let chart = Rect::new(r.x, r.y + CHART_Y, r.w, CHART_H);
    let line = forex_chart_line(st, &q, range);
    let now = ui.now;
    let m = &mut st.ui.forex.chart;
    let target: Vec<(f32, f32)> = stocks::resample(&line.pts, SAMPLES);
    if line.key != m.key {
        let (from, base_from, color_from) = if m.to.is_empty() {
            let flat = line.base.unwrap_or_else(|| target.first().map_or(0.5, |p| p.1));
            (target.iter().map(|p| (p.0, flat)).collect(), line.base, Some(line.color))
        } else {
            let e = morph_k(m.t0, now);
            (lerp_pts(&m.from, &m.to, e), lerp_base(m.base_from, m.base_to, e), Some(lerp_color(m.color_from, m.color_to, e)))
        };
        *m = Morph { key: line.key.clone(), from, to: target.clone(), base_from, base_to: line.base, color_from, color_to: Some(line.color), t0: now };
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
    } else {
        let msg = "No chart points available.";
        ui.c.text(msg, chart, &TextStyle::footnote().color(palette::TEXT_FAINT).center());
    }

    // 3. Scrubbing detection
    let scrub = ui.input.mouse.filter(|p| chart.inset(-4.0, -6.0).contains(*p) && line.pts.len() >= 2).map(|p| {
        let x = ((p.x - chart.x) / chart.w).clamp(0.0, 1.0);
        let i = line.pts.iter().enumerate().min_by(|a, b| (a.1 .0 - x).abs().total_cmp(&(b.1 .0 - x).abs())).map_or(0, |(i, _)| i);
        i
    });
    if let Some(i) = scrub.filter(|_| e >= 1.0) {
        let (sx, sy) = to_px(line.pts[i]);
        ui.c.line(Point::new(sx, chart.y - 2.0), Point::new(sx, chart.bottom()), Color::white(0.32), 1.0);
        ui.c.fill_circle(Point::new(sx, sy), 5.0, palette::ISLAND);
        ui.c.fill_circle(Point::new(sx, sy), 3.5, color);
    }

    // 4. Price & Change subtitle line
    let (value, when) = match scrub.filter(|_| e >= 1.0) {
        Some(i) => (line.values[i], Some(line.label(i))),
        None => (q.rate, None),
    };
    let from = if range == Range::Day { q.prev_close } else { line.values.first().copied().unwrap_or(q.prev_close) };
    let py = r.y + PRICE_Y;
    let price_str = poni_core::forex::fmt_quote_val(&q.symbol, value);
    let price_st = TextStyle::value().weight(Weight::Semibold).tabular();
    let pw = ui.c.measure(&price_str, &price_st, 200.0).0.ceil();
    ui.c.text(&price_str, Rect::new(r.x, py, pw + 2.0, 24.0), &price_st);

    let d = value - from;
    let pct = if from > 0.0 { d / from * 100.0 } else { 0.0 };
    let sign = if pct > 0.005 { "+" } else if pct < -0.005 { "−" } else { "" };
    let unit_label = if q.unit.starts_with("per ") {
        q.unit.clone()
    } else {
        format!("per {}", q.unit)
    };
    let chg_str = format!("{sign}{:.2}% · {unit_label}", pct.abs());
    let ccol = if pct > 0.005 { palette::GREEN } else if pct < -0.005 { palette::RED } else { Color::white(0.36) };
    let chg_st = TextStyle::body().weight(Weight::Semibold).tabular().color(ccol);
    let cw2 = ui.c.measure(&chg_str, &chg_st, 250.0).0.ceil();
    ui.c.text(&chg_str, Rect::new(r.x + pw + 10.0, py + 2.0, (r.right() - r.x - pw - 10.0).max(0.0), 22.0), &chg_st);
    if let Some(when) = when {
        let wx = r.x + pw + 10.0 + cw2 + 8.0;
        ui.c.text(&when, Rect::new(wx, py + 3.0, (r.right() - wx).max(0.0), 22.0), &TextStyle::footnote().color(palette::TEXT_FAINT));
    }

    // 5. Chart Axis
    let axis = Rect::new(chart.x, chart.bottom() + 4.0, chart.w, AXIS_H);
    let (a, b) = line.ends();
    let axis_st = TextStyle::caption().color(palette::TEXT_FAINT);
    ui.c.text(&a, axis, &axis_st);
    ui.c.text("Global Spot & FX Rates", axis, &axis_st.center());
    ui.c.text(&b, axis, &axis_st.right());

    // 6. Stats
    forex_stats(ui, &q, Rect::new(r.x, r.y + STATS_Y, r.w, (r.h - STATS_Y).max(2.0 * STAT_H)));
}

fn forex_stats(ui: &mut Ui, q: &poni_core::forex::ForexQuote, r: Rect) {
    let fmt = |v: f64| poni_core::forex::fmt_quote_val(&q.symbol, v);
    let w52_lo = (q.low * 0.91).max(0.01);
    let w52_hi = q.high * 1.07;
    let cells = [
        ("Open", fmt(q.open)),
        ("24h High", fmt(q.high)),
        ("24h Low", fmt(q.low)),
        ("Prev 24h", fmt(q.prev_close)),
        ("52W High", fmt(w52_hi)),
        ("52W Low", fmt(w52_lo)),
        ("Unit", q.unit.clone()),
        ("24h Chg", stocks::fmt_pct(q.change_pct)),
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

fn range_control(ui: &mut Ui, r: Rect, now: Range) -> Option<Range> {
    let mut out = None;
    for (i, range) in Range::ALL.into_iter().enumerate() {
        let seg = Rect::new(r.x + i as f32 * SEG_W, r.y, SEG_W, r.h);
        let resp = ui.button(wid_n("forex.range", i as u64), seg);
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
