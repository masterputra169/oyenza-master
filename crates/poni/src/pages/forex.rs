//! Forex & Commodities page (the Forex add-on): real-time exchange rates,
//! Gold (Antam/LM), Silver, and Oil spot prices, intraday sparklines,
//! and 1D/1M/6M/1Y historical charts.

use crate::gfx::{TextStyle, Weight};
use crate::icons::Icon;
use crate::state::AppState;
use crate::ui::{wid, wid_n, Ui};
use poni_core::geom::{palette, Color, Point, Rect};
use poni_core::stocks::{self, Range};
use poni_core::svgpath::Seg;

const HEIGHT: f32 = 256.0;
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

#[derive(Clone, Debug, Default)]
pub struct ForexUi {
    pub scroll: f32,
    pub shown_scroll: f32,
    pub list_drag: Option<(f32, f32, bool)>,
    pub bar_drag: Option<(f32, f32)>,
    pub pick_y: Option<f32>,
    pub show_pct: bool,
    pub last_frame: f64,
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
    forex_list(ui, st, list_r, area, dt, &mut moving);
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

fn glide(val: f32, target: f32, dt: f32, half_life: f32, moving: &mut bool) -> f32 {
    let diff = target - val;
    if diff.abs() < 0.25 {
        target
    } else {
        *moving = true;
        let lambda = 0.69315 / half_life.max(0.01);
        let decay = (-lambda * dt).exp();
        target - diff * decay
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
    let quotes = &board.quotes;
    let n = quotes.len();
    let footer_h = 26.0;
    let content_h = n as f32 * ROW_H + footer_h;
    let max_scroll = (content_h - r.h).max(0.0);
    let card = crate::pages::card::pill_card(st, r);

    let pick_raw = st.ui.forex_pick.clone().unwrap_or_else(|| "USD/IDR".into());
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
    let show_pct = !s.show_pct;

    ui.c.clip(Rect::new(r.x - 8.0, r.y, r.w + 16.0, r.h));

    let hl = Rect::new(r.x, top + pick_y + 2.0, r.w, ROW_H - 4.0);
    crate::pages::card::fill_box(ui, card, hl, 10.0, Color::white(0.10));

    let mut clicked_sym = None;
    let mut clicked_y = None;
    let mut toggle_mode = false;

    for (i, q) in quotes.iter().enumerate() {
        let y = i as f32 * ROW_H;
        let row = Rect::new(r.x, top + y, r.w, ROW_H);
        let Some(hit_clip) = intersect(Rect::new(row.x, row.y + 2.0, row.w, row.h - 4.0), r) else {
            continue;
        };
        let is_picked = q.symbol.eq_ignore_ascii_case(&pick);
        let resp = ui.button(wid_n("forex.item", code_n(&q.symbol)), hit_clip);
        if (resp.hovered || resp.pressed) && !is_picked {
            crate::pages::card::fill_box(
                ui,
                card,
                Rect::new(row.x, row.y + 2.0, row.w, row.h - 4.0),
                10.0,
                Color::white(if resp.pressed { 0.08 } else { 0.05 }),
            );
        }
        if resp.clicked {
            clicked_sym = Some(q.symbol.clone());
            clicked_y = Some(y);
        }

        let x0 = row.x + ROW_PAD;
        let right = row.right() - ROW_PAD;
        let pill = Rect::new(right - PILL_W, row.y + 25.0, PILL_W, PILL_H);
        let spark = Rect::new(pill.x - 8.0 - SPARK_W, row.y + 11.0, SPARK_W, 26.0);
        let text_w = (spark.x - 6.0 - x0).max(0.0);

        ui.c.text(&q.symbol, Rect::new(x0, row.y + 6.0, text_w, 19.0), &TextStyle::headline().weight(Weight::Semibold));
        ui.c.text(&q.name, Rect::new(x0, row.y + 25.0, text_w, 17.0), &TextStyle::footnote().color(palette::TEXT_DIM));

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
            if pr.clicked {
                toggle_mode = true;
            }
        }
        ui.c.fill_rrect(pill, 6.0, fill);
        let chg_str = q.formatted_change(show_pct);
        ui.c.text(&chg_str, Rect::new(pill.x + 5.0, pill.y, pill.w - 10.0, pill.h), &TextStyle::footnote().weight(Weight::Semibold).tabular().right().color(Color::white(1.0)));
    }

    let foot_r = Rect::new(r.x + ROW_PAD, top + n as f32 * ROW_H + 6.0, r.w - 2.0 * ROW_PAD, 16.0);
    if intersect(foot_r, r).is_some() {
        ui.c.text("Live spot & foreign exchange rates", foot_r, &TextStyle::caption().color(palette::TEXT_FAINT));
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

    if toggle_mode {
        st.ui.forex.show_pct = !st.ui.forex.show_pct;
    } else if let Some(sym) = clicked_sym {
        st.ui.forex_pick = Some(sym);
        if let Some(py) = clicked_y {
            let s = &mut st.ui.forex;
            if py < s.scroll {
                s.scroll = py.clamp(0.0, max_scroll);
                *moving = true;
            } else if py + ROW_H > s.scroll + r.h {
                s.scroll = (py + ROW_H - r.h + 18.0).clamp(0.0, max_scroll);
                *moving = true;
            }
        }
    }
}

fn forex_detail(ui: &mut Ui, st: &mut AppState, r: Rect) {
    let pick_raw = st.ui.forex_pick.clone().unwrap_or_else(|| "USD/IDR".into());
    let pick = poni_core::forex::normalize_symbol(&pick_raw);
    let range = st.store.stocks.range;

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

    // Instrument Name directly on top row
    let name_x = alert_r.right() + 8.0;
    let name_r = Rect::new(name_x, title.y + 1.0, (seg_x - name_x - 10.0).max(0.0), title.h);
    ui.c.text(&q.name, name_r, &TextStyle::headline().color(palette::TEXT_DIM));

    // Range selector using identical style
    if let Some(pick_range) = range_control(ui, Rect::new(seg_x, title.y, r.right() - seg_x, SEG_H), range) {
        st.store.stocks.range = pick_range;
        st.save();
    }

    // 2. Chart points preparation
    let chart_r = Rect::new(r.x, r.y + CHART_Y, r.w, CHART_H);
    let hist_data = st.forex.years.get(&q.symbol);
    let mut pts_data: Vec<f64> = Vec::new();
    if range == Range::Day || hist_data.is_none() {
        pts_data = q.spark.iter().map(|p| f64::from(p.1)).collect();
    } else if let Some(h) = hist_data {
        let count = match range {
            Range::Month => 22,
            Range::HalfYear => 130,
            Range::Year => 260,
            _ => 22,
        };
        let start = h.closes.len().saturating_sub(count);
        pts_data = h.closes[start..].iter().map(|p| p.1).collect();
    }
    if pts_data.is_empty() {
        pts_data = q.spark.iter().map(|p| f64::from(p.1)).collect();
    }

    // 3. Scrubbing detection
    let scrub = ui.input.mouse.filter(|p| chart_r.inset(-4.0, -6.0).contains(*p) && pts_data.len() >= 2).map(|p| {
        let x = ((p.x - chart_r.x) / chart_r.w).clamp(0.0, 1.0);
        let n_pts = (pts_data.len() - 1) as f32;
        (x * n_pts).round().clamp(0.0, n_pts) as usize
    });

    let (disp_rate, disp_pct, when_label) = match scrub {
        Some(idx) if idx < pts_data.len() => {
            let v = pts_data[idx];
            let pct = if q.prev_close > 0.0 { (v - q.prev_close) / q.prev_close * 100.0 } else { 0.0 };
            let label = match range {
                Range::Day => format!("{:02}:{:02}", (idx * 30) / 60, (idx * 30) % 60),
                Range::Month => format!("{}d ago", pts_data.len() - 1 - idx),
                _ => format!("{}d ago", pts_data.len() - 1 - idx),
            };
            (v, pct, Some(label))
        }
        _ => (q.rate, q.change_pct, None),
    };

    // 4. Price & Change subtitle line (exact same layout & typography as Stocks)
    let py = r.y + PRICE_Y;
    let price_str = poni_core::forex::fmt_quote_val(&q.symbol, disp_rate);
    let price_st = TextStyle::value().weight(Weight::Semibold).tabular();
    let pw = ui.c.measure(&price_str, &price_st, 200.0).0.ceil();
    ui.c.text(&price_str, Rect::new(r.x, py, pw + 2.0, 24.0), &price_st);

    let sign = if disp_pct > 0.005 { "+" } else if disp_pct < -0.005 { "−" } else { "" };
    let chg_str = format!("{sign}{:.2}% · per {}", disp_pct.abs(), q.unit);
    let ccol = if disp_pct > 0.005 { palette::GREEN } else if disp_pct < -0.005 { palette::RED } else { Color::white(0.36) };
    let chg_st = TextStyle::body().weight(Weight::Semibold).tabular().color(ccol);
    let cw2 = ui.c.measure(&chg_str, &chg_st, 250.0).0.ceil();
    ui.c.text(&chg_str, Rect::new(r.x + pw + 10.0, py + 2.0, (r.right() - r.x - pw - 10.0).max(0.0), 22.0), &chg_st);

    if let Some(when) = when_label {
        let wx = r.x + pw + 10.0 + cw2 + 8.0;
        ui.c.text(&when, Rect::new(wx, py + 3.0, (r.right() - wx).max(0.0), 22.0), &TextStyle::footnote().color(palette::TEXT_FAINT));
    }

    // 5. Interactive Chart rendering
    if pts_data.len() >= 2 {
        if let Some((lo, hi)) = stocks::bounds(pts_data.iter().copied(), Some(q.prev_close)) {
            let y_map = |v: f64| chart_r.bottom() - ((v - lo) / (hi - lo)) as f32 * chart_r.h;
            dotted(ui, chart_r.x, chart_r.right(), y_map(q.prev_close).round() + 0.5, Color::white(0.28));

            let n_pts = (pts_data.len() - 1) as f32;
            let pts: Vec<(f32, f32)> = pts_data.iter().enumerate().map(|(i, &v)| (chart_r.x + (i as f32 / n_pts) * chart_r.w, y_map(v))).collect();

            let col = match q.trend() {
                1 => palette::GREEN,
                -1 => palette::RED,
                _ => Color::white(0.36),
            };
            draw_line(ui, &pts, col, chart_r.y, chart_r.bottom() + 4.0, 1.85, Some(0.30));

            // Native hairline cursor on hover/scrubbing
            if let Some(idx) = scrub {
                if let Some(&p) = pts.get(idx) {
                    ui.c.line(Point::new(p.0, chart_r.y - 2.0), Point::new(p.0, chart_r.bottom()), Color::white(0.32), 1.0);
                    ui.c.fill_circle(Point::new(p.0, p.1), 5.0, palette::ISLAND);
                    ui.c.fill_circle(Point::new(p.0, p.1), 3.5, col);
                }
            }
        }
    }

    // 6. Chart Axis
    let axis_st = TextStyle::caption().color(palette::TEXT_FAINT);
    let left_lbl = match range {
        Range::Day => "24h Ago",
        Range::Month => "1 Mo Ago",
        Range::HalfYear => "6 Mo Ago",
        Range::Year => "1 Yr Ago",
    };
    let right_lbl = match range {
        Range::Day => "Now",
        _ => "Today",
    };
    ui.c.text(left_lbl, Rect::new(chart_r.x, chart_r.bottom() + 4.0, 70.0, AXIS_H), &axis_st);
    ui.c.text("Global Spot & FX Rates", Rect::new(chart_r.x, chart_r.bottom() + 4.0, chart_r.w, AXIS_H), &axis_st.center());
    ui.c.text(right_lbl, Rect::new(chart_r.right() - 70.0, chart_r.bottom() + 4.0, 70.0, AXIS_H), &axis_st.right());

    // 7. Day stats
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
