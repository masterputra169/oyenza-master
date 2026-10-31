//! Real-time Weather models and parser from Open-Meteo API.
//!
//! Complies with the repository rule:
//! - Free, zero-API-key, lightweight endpoint.
//! - Parsed with standard serde_json.

use serde::{Deserialize, Serialize};

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct WeatherCurrent {
    #[serde(default)]
    pub time: String,
    pub temperature_2m: f32,
    pub relative_humidity_2m: u8,
    pub apparent_temperature: f32,
    #[serde(default)]
    pub is_day: u8,
    pub weather_code: u16,
    pub wind_speed_10m: f32,
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct OpenMeteoResponse {
    #[serde(default)]
    pub latitude: f64,
    #[serde(default)]
    pub longitude: f64,
    pub current: WeatherCurrent,
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct WeatherReport {
    pub city: String,
    pub temp: f32,
    pub feels_like: f32,
    pub humidity: u8,
    pub code: u16,
    pub is_day: bool,
    pub wind_speed: f32,
    pub updated_at: u64,
}

impl WeatherReport {
    pub fn condition(&self) -> &'static str {
        match self.code {
            0 => "Clear Sky",
            1 => "Mainly Clear",
            2 => "Partly Cloudy",
            3 => "Overcast",
            45 | 48 => "Foggy",
            51 | 53 | 55 => "Drizzle",
            56 | 57 => "Freezing Drizzle",
            61 | 63 => "Rain",
            65 => "Heavy Rain",
            66 | 67 => "Freezing Rain",
            71 | 73 | 75 | 77 => "Snow",
            80 | 81 => "Rain Showers",
            82 => "Violent Rain Showers",
            85 | 86 => "Snow Showers",
            95 => "Thunderstorm",
            96 | 99 => "Thunderstorm with Hail",
            _ => "Cloudy",
        }
    }

    pub fn icon_emoji(&self) -> &'static str {
        match self.code {
            0 => if self.is_day { "☀️" } else { "🌙" },
            1 => if self.is_day { "🌤️" } else { "🌙" },
            2 => if self.is_day { "⛅" } else { "☁️" },
            3 => "☁️",
            45 | 48 => "🌫️",
            51..=57 => "🌦️",
            61..=67 => "🌧️",
            71..=77 => "🌨️",
            80..=82 => "🌧️",
            85..=86 => "🌨️",
            95..=99 => "⛈️",
            _ => "⛅",
        }
    }

    pub fn idle_text(&self) -> String {
        format!("{:.0}°C {}", self.temp, self.condition())
    }

    pub fn from_json(city: &str, json_str: &str, now_unix: u64) -> Result<Self, String> {
        let resp: OpenMeteoResponse = serde_json::from_str(json_str).map_err(|e| format!("weather json: {e}"))?;
        Ok(Self {
            city: city.to_string(),
            temp: resp.current.temperature_2m,
            feels_like: resp.current.apparent_temperature,
            humidity: resp.current.relative_humidity_2m,
            code: resp.current.weather_code,
            is_day: resp.current.is_day == 1,
            wind_speed: resp.current.wind_speed_10m,
            updated_at: now_unix,
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parses_open_meteo_response() {
        let sample = r#"{
            "latitude": -6.22,
            "longitude": 106.85,
            "current": {
                "time": "2026-10-31T09:15",
                "interval": 900,
                "temperature_2m": 30.6,
                "relative_humidity_2m": 64,
                "apparent_temperature": 35.4,
                "is_day": 1,
                "weather_code": 55,
                "wind_speed_10m": 4.6
            }
        }"#;

        let report = WeatherReport::from_json("Jakarta", sample, 1730366100).expect("should parse");
        assert_eq!(report.city, "Jakarta");
        assert!((report.temp - 30.6).abs() < 0.01);
        assert_eq!(report.humidity, 64);
        assert_eq!(report.condition(), "Drizzle");
        assert_eq!(report.icon_emoji(), "🌦️");
        assert_eq!(report.idle_text(), "31°C Drizzle");
    }
}
