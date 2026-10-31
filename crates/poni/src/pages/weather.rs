//! Real-time Weather page powered by Open-Meteo.
//!
//! Complies with the repository rule:
//! - Nothing runs at rest: updates every 15 minutes only while watched.
//! - Completely free, zero API key needed.

use crate::gfx::{TextStyle, Weight};
use crate::state::AppState;
use crate::ui::{wid, wid_n, PillStyle, Ui};
use poni_core::geom::{palette, Rect};
use poni_core::store::TickerPin;

pub const PRESET_CITIES: &[(&str, f64, f64)] = &[
    ("Jakarta", -6.2088, 106.8456),
    ("Surabaya", -7.2575, 112.7521),
    ("Bandung", -6.9175, 107.6191),
    ("Medan", 3.5952, 98.6722),
    ("Bali", -8.6705, 115.2126),
    ("Yogya", -7.7956, 110.3695),
    ("Tokyo", 35.6762, 139.6503),
    ("Singapore", 1.3521, 103.8198),
    ("London", 51.5074, -0.1278),
];

pub fn height(_st: &AppState) -> f32 {
    256.0
}

pub fn draw(ui: &mut Ui, st: &mut AppState, area: Rect) {
    let card = super::card::pill_card(st, area);
    let is_pinned = st.store.ticker.as_ref().is_some_and(|t| t.enabled && t.source == "weather");
    let is_auto_device = st.store.weather_city.is_none();

    let current_city = st.store.weather_city.clone().unwrap_or_else(|| {
        if let Some(ref w) = st.weather.report {
            w.city.clone()
        } else {
            "Device Location".to_string()
        }
    });

    let top_y = area.y;

    // Header row
    let city_title = if is_auto_device {
        format!("📍 Device Location · {current_city}")
    } else {
        format!("📍 Destination · {current_city}")
    };
    ui.c.text(
        &city_title,
        Rect::new(area.x, top_y, 230.0, 24.0),
        &TextStyle::body().weight(Weight::Bold).color(st.accent),
    );

    // Right header buttons: [Pin to Island] and [Refresh]
    let mut bx = area.right();
    let ref_w = 64.0;
    bx -= ref_w;
    if ui.pill(wid("weather.refresh"), Rect::new(bx, top_y, ref_w, 22.0), "Refresh", None, PillStyle::Utility).clicked {
        st.weather.last_fetched = 0.0;
        st.save();
    }

    bx -= 6.0;
    let pin_w = if is_pinned { 72.0 } else { 92.0 };
    bx -= pin_w;
    let pin_label = if is_pinned { "📌 Pinned" } else { "Pin to Island" };
    let pin_style = if is_pinned { PillStyle::Primary(st.accent) } else { PillStyle::Utility };
    if ui.pill(wid("weather.pin"), Rect::new(bx, top_y, pin_w, 22.0), pin_label, None, pin_style).clicked {
        if is_pinned {
            st.store.ticker = None;
        } else {
            st.store.ticker = Some(TickerPin {
                enabled: true,
                source: "weather".into(),
                code: current_city.clone(),
            });
        }
        st.save();
    }

    // City Selector Pills (Horizontal): First is "📍 Auto (Device)", followed by destinations
    let city_y = top_y + 28.0;
    let chip_h = 22.0;
    let mut cx = area.x;

    // Auto Device Chip
    let auto_w = 96.0;
    let auto_style = if is_auto_device { PillStyle::Primary(st.accent) } else { PillStyle::Utility };
    let auto_resp = ui.pill(wid("weather.city_auto"), Rect::new(cx, city_y, auto_w, chip_h), "Auto (Device)", None, auto_style);
    if auto_resp.clicked && !is_auto_device {
        st.store.weather_city = None;
        st.weather.last_fetched = 0.0;
        st.save();
    }
    cx += auto_w + 6.0;

    for (i, (name, _, _)) in PRESET_CITIES.iter().enumerate() {
        let is_sel = !is_auto_device && current_city.eq_ignore_ascii_case(name);
        let chip_w = (name.len() as f32 * 7.5 + 16.0).max(44.0);
        if cx + chip_w > area.right() {
            break;
        }
        let style = if is_sel {
            PillStyle::Primary(st.accent)
        } else {
            PillStyle::Utility
        };
        let resp = ui.pill(wid_n("weather.city", i as u64), Rect::new(cx, city_y, chip_w, chip_h), name, None, style);
        if resp.clicked && !is_sel {
            st.store.weather_city = Some(name.to_string());
            st.weather.last_fetched = 0.0;
            st.save();
        }
        cx += chip_w + 6.0;
    }

    // Main Hero Weather Card
    let hero_y = city_y + chip_h + 8.0;
    let hero_h = 88.0;
    let hero_r = Rect::new(area.x, hero_y, area.w, hero_h);
    super::card::fill_box(ui, card, hero_r, 12.0, palette::SURFACE);

    if let Some(ref w) = st.weather.report {
        let temp_str = format!("{:.0}°C", w.temp);
        let emoji = w.icon_emoji();
        let display_txt = format!("{temp_str} {emoji}");
        ui.c.text(
            &display_txt,
            Rect::new(hero_r.x + 16.0, hero_r.y + 12.0, 200.0, 36.0),
            &TextStyle::display(),
        );

        let sub_txt = format!("{} · Feels like {:.0}°C", w.condition(), w.feels_like);
        ui.c.text(
            &sub_txt,
            Rect::new(hero_r.x + 16.0, hero_r.y + 52.0, 240.0, 20.0),
            &TextStyle::footnote().color(palette::TEXT_DIM),
        );

        // Right summary badges
        let rw = 150.0;
        let rx = hero_r.right() - rw - 16.0;
        let hum_str = format!("💧 Humidity: {}%", w.humidity);
        ui.c.text(&hum_str, Rect::new(rx, hero_r.y + 16.0, rw, 18.0), &TextStyle::footnote().weight(Weight::Semibold));
        let wind_str = format!("💨 Wind: {:.1} km/h", w.wind_speed);
        ui.c.text(&wind_str, Rect::new(rx, hero_r.y + 36.0, rw, 18.0), &TextStyle::footnote().weight(Weight::Semibold));
        let time_str = if w.is_day { "☀️ Daytime" } else { "🌙 Nighttime" };
        ui.c.text(time_str, Rect::new(rx, hero_r.y + 56.0, rw, 18.0), &TextStyle::caption().color(palette::TEXT_DIM));
    } else {
        ui.c.text(
            "Fetching real-time weather from Open-Meteo...",
            Rect::new(hero_r.x + 16.0, hero_r.y + 24.0, hero_r.w - 32.0, 24.0),
            &TextStyle::body().color(palette::TEXT_DIM),
        );
        ui.c.text(
            "Select a destination city or Auto (Device) to update weather.",
            Rect::new(hero_r.x + 16.0, hero_r.y + 50.0, hero_r.w - 32.0, 20.0),
            &TextStyle::footnote().color(palette::TEXT_DIM),
        );
    }

    // Bottom Grid: 4 Metric Cards
    let bot_y = hero_y + hero_h + 8.0;
    let bot_h = (area.bottom() - bot_y).max(72.0);
    let cols = Rect::new(area.x, bot_y, area.w, bot_h).columns(4, 6.0);

    let (hum_val, wind_val, feels_val, cond_val) = if let Some(ref w) = st.weather.report {
        (
            format!("{}%", w.humidity),
            format!("{:.1} km/h", w.wind_speed),
            format!("{:.0}°C", w.feels_like),
            w.condition().to_string(),
        )
    } else {
        ("-".into(), "-".into(), "-".into(), "Loading".into())
    };

    // Metric 1: Humidity
    super::card::fill_box(ui, card, cols[0], 10.0, palette::SURFACE_HOVER);
    ui.c.text("Humidity", Rect::new(cols[0].x + 8.0, cols[0].y + 8.0, cols[0].w - 16.0, 14.0), &TextStyle::caption().color(palette::TEXT_DIM));
    ui.c.text(&hum_val, Rect::new(cols[0].x + 8.0, cols[0].y + 24.0, cols[0].w - 16.0, 22.0), &TextStyle::body().weight(Weight::Bold));
    ui.c.text("Relative air", Rect::new(cols[0].x + 8.0, cols[0].y + 48.0, cols[0].w - 16.0, 14.0), &TextStyle::caption().color(palette::TEXT_DIM));

    // Metric 2: Wind
    super::card::fill_box(ui, card, cols[1], 10.0, palette::SURFACE_HOVER);
    ui.c.text("Wind Speed", Rect::new(cols[1].x + 8.0, cols[1].y + 8.0, cols[1].w - 16.0, 14.0), &TextStyle::caption().color(palette::TEXT_DIM));
    ui.c.text(&wind_val, Rect::new(cols[1].x + 8.0, cols[1].y + 24.0, cols[1].w - 16.0, 22.0), &TextStyle::body().weight(Weight::Bold));
    ui.c.text("10m altitude", Rect::new(cols[1].x + 8.0, cols[1].y + 48.0, cols[1].w - 16.0, 14.0), &TextStyle::caption().color(palette::TEXT_DIM));

    // Metric 3: Feels like
    super::card::fill_box(ui, card, cols[2], 10.0, palette::SURFACE_HOVER);
    ui.c.text("Feels Like", Rect::new(cols[2].x + 8.0, cols[2].y + 8.0, cols[2].w - 16.0, 14.0), &TextStyle::caption().color(palette::TEXT_DIM));
    ui.c.text(&feels_val, Rect::new(cols[2].x + 8.0, cols[2].y + 24.0, cols[2].w - 16.0, 22.0), &TextStyle::body().weight(Weight::Bold));
    ui.c.text("Heat index", Rect::new(cols[2].x + 8.0, cols[2].y + 48.0, cols[2].w - 16.0, 14.0), &TextStyle::caption().color(palette::TEXT_DIM));

    // Metric 4: Condition / API
    super::card::fill_box(ui, card, cols[3], 10.0, palette::SURFACE_HOVER);
    ui.c.text("Condition", Rect::new(cols[3].x + 8.0, cols[3].y + 8.0, cols[3].w - 16.0, 14.0), &TextStyle::caption().color(palette::TEXT_DIM));
    ui.c.text(&cond_val, Rect::new(cols[3].x + 8.0, cols[3].y + 24.0, cols[3].w - 16.0, 22.0), &TextStyle::footnote().weight(Weight::Bold).color(palette::GREEN));
    ui.c.text("Open-Meteo", Rect::new(cols[3].x + 8.0, cols[3].y + 48.0, cols[3].w - 16.0, 14.0), &TextStyle::caption().color(palette::TEXT_DIM));
}
