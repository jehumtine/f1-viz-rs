use std::collections::HashMap;
use std::time::Duration;

use chrono::{DateTime, Utc};
use reqwest::Client;

use crate::{
    decode::decode_f1_z_payload,
    error::F1Error,
    model::{
        CarData, CarPosition, CarTelemetry, Driver, PositionSample, RaceControlMessage,
        RawCarDataBlock, RawDriver, RawLapCount, RawOffset, RawPositionBlock, RawRaceControl,
        RawSessionInfo, RawWeather, SessionInfo, WeatherSample,
    },
};

const BASE_URL: &str = "https://livetiming.formula1.com/static";

pub struct F1ArchiveClient {
    client: Client,
    session_path: String,
}

impl F1ArchiveClient {
    pub fn new(session_path: String) -> Self {
        Self {
            client: Client::new(),
            session_path,
        }
    }

    async fn fetch_stream(&self, filename: &str) -> Result<Vec<(String, String)>, F1Error> {
        let url = format!("{}{}{}", BASE_URL, self.session_path, filename);
        let resp = self.client.get(&url).send().await?;
        let text = resp.text().await?;

        let mut lines = Vec::new();
        for line in text.split("\r\n") {
            if line.len() > 12 {
                let timestamp = line[..12].to_string();
                let payload = line[12..].to_string();
                lines.push((timestamp, payload));
            }
        }
        Ok(lines)
    }

    pub async fn get_driver_list(&self) -> Result<HashMap<u8, Driver>, F1Error> {
        let url = format!("{}{}DriverList.json", BASE_URL, self.session_path);
        let resp = self.client.get(&url).send().await?;
        let text = resp.text().await?;

        let raw_map: HashMap<String, RawDriver> = serde_json::from_str(&text)?;

        let mut drivers = HashMap::new();
        for (num_str, raw) in raw_map {
            if let Ok(num) = num_str.parse::<u8>() {
                drivers.insert(
                    num,
                    Driver {
                        number: num,
                        code: raw.tla,
                        name: raw.broadcast_name,
                        team: raw.team_name,
                        color: raw.team_colour,
                    },
                );
            }
        }
        Ok(drivers)
    }

    pub async fn get_position_data(&self) -> Result<Vec<PositionSample>, F1Error> {
        let lines = self.fetch_stream("Position.z.jsonStream").await?;
        let mut samples = Vec::new();

        for (_, payload) in lines {
            let json: serde_json::Value = decode_f1_z_payload(&payload)?;

            if let Some(positions) = json.get("Position").and_then(|v| v.as_array()) {
                for pos_block in positions {
                    let block: RawPositionBlock = serde_json::from_value(pos_block.clone())?;
                    let timestamp = block.timestamp.parse::<DateTime<Utc>>()?;

                    let mut cars = HashMap::new();
                    for (num_str, entry) in block.entries {
                        if let Ok(num) = num_str.parse::<u8>() {
                            cars.insert(
                                num,
                                CarPosition {
                                    x_m: entry.x as f64 / 10.0,
                                    y_m: entry.y as f64 / 10.0,
                                    z_m: entry.z as f64 / 10.0,
                                    on_track: entry.status == "OnTrack",
                                },
                            );
                        }
                    }
                    samples.push(PositionSample { timestamp, cars });
                }
            }
        }
        Ok(samples)
    }

    pub async fn get_car_data(&self) -> Result<Vec<CarData>, F1Error> {
        let lines = self.fetch_stream("CarData.z.jsonStream").await?;
        let mut samples = Vec::new();

        for (_, payload) in lines {
            let json: serde_json::Value = decode_f1_z_payload(&payload)?;
            let block: RawCarDataBlock = serde_json::from_value(json)?;

            for entry in block.entries {
                let timestamp = entry.utc.parse::<DateTime<Utc>>()?;
                let mut cars = HashMap::new();

                for (num_str, raw_car) in entry.cars {
                    if let Ok(num) = num_str.parse::<u8>() {
                        let ch = &raw_car.channels;

                        cars.insert(
                            num,
                            CarTelemetry {
                                rpm: ch.get("0").copied().unwrap_or(0) as u32,
                                speed_kph: ch.get("2").copied().unwrap_or(0) as u32,
                                gear: ch.get("3").copied().unwrap_or(0) as u8,
                                throttle_pct: ch.get("4").copied().unwrap_or(0) as u8,
                                brake: ch.get("5").copied().unwrap_or(0) == 1,
                                drs: ch.get("45").copied().unwrap_or(0) as u8,
                            },
                        );
                    }
                }
                samples.push(CarData { timestamp, cars });
            }
        }
        Ok(samples)
    }

    pub async fn get_session_data(&self) -> Result<SessionInfo, F1Error> {
        let url = format!("{}{}SessionInfo.json", BASE_URL, self.session_path);
        let resp = self.client.get(&url).send().await?;
        let text = resp.text().await?;

        let raw: RawSessionInfo = serde_json::from_str(&text)?;
        let meeting_name = raw.clone().meeting.map(|m| m.name).unwrap();
        let country = raw.meeting.map(|m| m.country).unwrap();
        let session_name = raw.session_name;
        let start_date = raw.start_date.parse::<DateTime<Utc>>()?;

        Ok(SessionInfo {
            meeting_name,
            country,
            session_name,
            start_date,
        })
    }

    pub async fn get_lap_count(&self) -> Result<Vec<(RawOffset, u32, u32)>, F1Error> {
        let lines = self.fetch_stream("TrackStatus.jsonStream").await?;
        let mut events = Vec::new();

        for (ts_str, payload) in lines {
            let raw: RawLapCount = serde_json::from_str(&payload)?;
            let offset = parse_offset(&ts_str)?;
            let total = raw.total_laps.unwrap_or(raw.current_lap);
            events.push((RawOffset(offset), raw.current_lap, total));
        }
        Ok(events)
    }

    pub async fn get_weather_data(&self) -> Result<Vec<(RawOffset, WeatherSample)>, F1Error> {
        let lines = self.fetch_stream("WeatherData.jsonStream").await?;
        let mut events = Vec::new();

        for (ts_str, payload) in lines {
            let raw: RawWeather = serde_json::from_str(&payload)?;
            let offset = parse_offset(&ts_str)?;
            events.push((
                RawOffset(offset),
                WeatherSample {
                    air_temp_c: raw.air_temp.parse().unwrap_or(0.0),
                    track_temp_c: raw.track_tmep.parse().unwrap_or(0.0),
                    humidity_pct: raw.humidity.parse().unwrap_or(0),
                    is_raining: raw.rainfall,
                },
            ));
        }
        Ok(events)
    }

    pub async fn get_race_control_messages(
        &self,
    ) -> Result<Vec<(RawOffset, RaceControlMessage)>, F1Error> {
        let lines = self.fetch_stream("RaceControlMessages.jsonStream").await?;
        let mut events = Vec::new();

        for (ts_str, payload) in lines {
            let raw: RawRaceControl = serde_json::from_str(&payload)?;
            let offset = parse_offset(&ts_str)?;
            events.push((
                RawOffset(offset),
                RaceControlMessage {
                    category: raw.category,
                    message: raw.message,
                    flag: raw.flag,
                },
            ));
        }
        Ok(events)
    }
}

fn parse_offset(ts_str: &str) -> Result<Duration, F1Error> {
    let parts: Vec<&str> = ts_str.split(':').collect();
    if parts.len() != 3 {
        return Err(F1Error::UnexpectedFormat(
            "Invalid timestamp format".to_string(),
        ));
    }
    let hours: u64 = parts[0]
        .parse()
        .map_err(|_| F1Error::UnexpectedFormat("Hours parse error".to_string()))?;
    let mins: u64 = parts[1]
        .parse()
        .map_err(|_| F1Error::UnexpectedFormat("Mins parse error".to_string()))?;
    let secs: f64 = parts[2]
        .parse()
        .map_err(|_| F1Error::UnexpectedFormat("Secs parse error".to_string()))?;

    let total_secs = (hours * 3600) + (mins * 60) + (secs.trunc() as u64);
    let nanos = ((secs.fract() * 1_000_000_000.0) as u32).min(999_999_999);

    Ok(Duration::new(total_secs, nanos))
}
