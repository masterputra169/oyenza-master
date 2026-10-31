//! Real-time Weather models and multi-provider parsers (MET Norway & Open-Meteo).
//!
//! Complies with the repository rule:
//! - Free, zero-API-key, lightweight endpoints.
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
            82 => "Heavy Showers",
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

    /// Parses an Open-Meteo `/v1/forecast` JSON response.
    pub fn from_json(city: &str, json_str: &str, now_unix: u64) -> Result<Self, String> {
        let resp: OpenMeteoResponse = serde_json::from_str(json_str).map_err(|e| format!("open-meteo json: {e}"))?;
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

    /// Parses a MET Norway (`api.met.no/weatherapi/locationforecast/2.0/compact`) JSON response.
    pub fn from_met_no_json(city: &str, json_str: &str, now_unix: u64) -> Result<Self, String> {
        let v: serde_json::Value = serde_json::from_str(json_str).map_err(|e| format!("met.no json: {e}"))?;
        let first = v
            .get("properties")
            .and_then(|p| p.get("timeseries"))
            .and_then(|t| t.as_array())
            .and_then(|a| a.first())
            .ok_or_else(|| "met.no: missing timeseries[0]".to_string())?;
        let data = first.get("data").ok_or_else(|| "met.no: missing data".to_string())?;
        let details = data
            .get("instant")
            .and_then(|i| i.get("details"))
            .ok_or_else(|| "met.no: missing instant.details".to_string())?;

        let temp = details.get("air_temperature").and_then(|x| x.as_f64()).unwrap_or(28.0) as f32;
        let humidity_f = details.get("relative_humidity").and_then(|x| x.as_f64()).unwrap_or(70.0) as f32;
        let humidity = humidity_f.round().clamp(0.0, 100.0) as u8;
        // MET Norway reports wind_speed in m/s; convert to km/h (* 3.6)
        let wind_ms = details.get("wind_speed").and_then(|x| x.as_f64()).unwrap_or(2.0) as f32;
        let wind_speed = wind_ms * 3.6;

        // Australian Bureau of Meteorology apparent temperature formula (Steadman)
        let rh = f64::from(humidity_f);
        let t = f64::from(temp);
        let e = (rh / 100.0) * 6.105 * ((17.27 * t) / (237.7 + t)).exp();
        let feels_like = (t + 0.33 * e - 0.70 * f64::from(wind_ms) - 4.00) as f32;

        let symbol = data
            .get("next_1_hours")
            .or_else(|| data.get("next_6_hours"))
            .or_else(|| data.get("next_12_hours"))
            .and_then(|n| n.get("summary"))
            .and_then(|s| s.get("symbol_code"))
            .and_then(|c| c.as_str())
            .unwrap_or("partlycloudy_day");

        let is_day = !symbol.ends_with("_night");
        let base_sym = symbol.trim_end_matches("_day").trim_end_matches("_night").trim_end_matches("_polartwilight");
        let code = match base_sym {
            "clearsky" => 0,
            "fair" => 1,
            "partlycloudy" => 2,
            "cloudy" => 3,
            "fog" => 45,
            "lightrainshowers" | "rainshowers" => 80,
            "heavyrainshowers" => 82,
            "lightrain" => 51,
            "rain" => 61,
            "heavyrain" => 65,
            "lightsleet" | "sleet" | "heavysleet" | "lightsnow" | "snow" | "heavysnow" => 71,
            "snowshowers" | "lightsnowshowers" | "heavysnowshowers" => 85,
            s if s.contains("thunder") => 95,
            _ => 2,
        };

        Ok(Self {
            city: city.to_string(),
            temp,
            feels_like,
            humidity,
            code,
            is_day,
            wind_speed,
            updated_at: now_unix,
        })
    }

    /// Fallback report when completely offline so the UI never hangs on Loading.
    pub fn sample(city: &str, lat: f64, now_unix: u64) -> Self {
        let is_tropical = lat.abs() < 23.5;
        let temp = if is_tropical { 29.0 } else { 18.0 };
        let feels_like = if is_tropical { 32.0 } else { 17.5 };
        Self {
            city: city.to_string(),
            temp,
            feels_like,
            humidity: if is_tropical { 74 } else { 62 },
            code: 2,
            is_day: true,
            wind_speed: 9.4,
            updated_at: now_unix,
        }
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

    #[test]
    fn parses_met_no_response() {
        let sample = r#"{
            "properties": {
                "timeseries": [
                    {
                        "time": "2026-10-31T13:00:00Z",
                        "data": {
                            "instant": {
                                "details": {
                                    "air_temperature": 26.0,
                                    "relative_humidity": 89.4,
                                    "wind_speed": 3.1
                                }
                            },
                            "next_1_hours": {
                                "summary": {
                                    "symbol_code": "rain"
                                }
                            }
                        }
                    }
                ]
            }
        }"#;
        let report = WeatherReport::from_met_no_json("Jakarta", sample, 1730366100).expect("should parse met.no");
        assert_eq!(report.city, "Jakarta");
        assert!((report.temp - 26.0).abs() < 0.01);
        assert_eq!(report.humidity, 89);
        assert_eq!(report.condition(), "Rain");
        assert!((report.wind_speed - 11.16).abs() < 0.1);
    }
}
