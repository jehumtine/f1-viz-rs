use chrono::{DateTime, NaiveDate, NaiveDateTime, TimeZone, Utc};
use directories::ProjectDirs;
use rayon::prelude::*;
use reqwest::Client;
use std::time::Duration;
use std::{collections::HashMap, path::PathBuf};
use tokio::fs::*;

use crate::model::{CarDataRoot, PositionRoot};
use crate::{
    decode::decode_f1_z_payload,
    error::F1Error,
    model::{
        CarData, CarPosition, CarTelemetry, Driver, PositionSample, RaceControlMessage, RawDriver,
        RawLapCount, RawOffset, RawRaceControlBlock, RawRaceControlMessage, RawSessionInfo,
        RawWeather, SessionInfo, Weather,
    },
};

const BASE_URL: &str = "https://livetiming.formula1.com/static";

pub struct F1ArchiveClient {
    client: Client,
    session_path: String,
    cache_dir: PathBuf,
}

impl F1ArchiveClient {
    pub fn new(session_path: String) -> Self {
        let proj_dirs =
            ProjectDirs::from("", "", "f1-livetiming").expect("Failed to find cache directory");

        let safe_session_name = session_path.replace('/', "_").replace('\\', "_");
        let cache_dir = proj_dirs.cache_dir().join(safe_session_name);

        std::fs::create_dir_all(&cache_dir).expect("Failed to create cache dir");
        Self {
            client: Client::new(),
            session_path,
            cache_dir,
        }
    }

    async fn fetch_raw(&self, filename: &str) -> Result<String, F1Error> {
        let cache_file = self.cache_dir.join(filename);

        if cache_file.exists() {
            println!("[CACHE HIT] Loading {} from disk", filename);
            return Ok(read_to_string(&cache_file).await?);
        }

        println!("[CACHE MISS] Downloading {} from F1 API", filename);
        let url = format!("{}{}{}", BASE_URL, self.session_path, filename);
        let resp = self.client.get(&url).send().await?;
        let text = resp.text().await?;

        write(&cache_file, &text).await?;

        Ok(text)
    }

    async fn fetch_stream(&self, filename: &str) -> Result<Vec<(String, String)>, F1Error> {
        let text = self.fetch_raw(filename).await?;

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
        let text = self.fetch_raw("DriverList.json").await?;

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
        let cache_file = self.cache_dir.join("positions.bin");

        if cache_file.exists() {
            println!("[BIN CACHE HIT] Loading parsed positions...");
            let bytes = tokio::fs::read(&cache_file).await?;
            return Ok(bincode::deserialize(&bytes).map_err(|e| F1Error::Bincode(e.to_string()))?);
        }

        println!("[BIN CACHE MISS] Parsing positions from JSON...");
        let lines = self.fetch_stream("Position.z.jsonStream").await?;

        let results: Result<Vec<Vec<PositionSample>>, F1Error> = lines
            .par_iter()
            .map(|(_, payload)| {
                let json_string = decode_f1_z_payload(payload)?;
                let root: PositionRoot = serde_json::from_str(&json_string)?;

                let mut block_samples = Vec::new();
                for pos_block in root.position {
                    let timestamp = pos_block.timestamp.parse::<DateTime<Utc>>()?;
                    let mut cars = HashMap::new();

                    for (num_str, entry) in pos_block.entries {
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
                    block_samples.push(PositionSample { timestamp, cars });
                }
                Ok(block_samples)
            })
            .collect();
        let samples = results?
            .into_iter()
            .flatten()
            .collect::<Vec<PositionSample>>();

        println!("[BIN CACHE] Saving parsed positions to binary cache...");
        let bytes = bincode::serialize(&samples).map_err(|e| F1Error::Bincode(e.to_string()))?;
        write(&cache_file, &bytes).await?;

        Ok(samples)
    }

    pub async fn get_car_data(&self) -> Result<Vec<CarData>, F1Error> {
        let cache_file = self.cache_dir.join("car_data.bin");

        if cache_file.exists() {
            println!("[BIN CACHE HIT] Loading parsed car data...");
            let bytes = tokio::fs::read(&cache_file).await?;
            return Ok(bincode::deserialize(&bytes).map_err(|e| F1Error::Bincode(e.to_string()))?);
        }

        println!("[BIN CACHE MISS] Parsing car data from JSON...");
        let lines = self.fetch_stream("CarData.z.jsonStream").await?;

        let results: Result<Vec<Vec<CarData>>, F1Error> = lines
            .par_iter()
            .map(|(_, payload)| {
                let json_string = decode_f1_z_payload(payload)?;
                let root: CarDataRoot = serde_json::from_str(&json_string)?;

                let mut block_samples = Vec::new();

                for entry in root.entries {
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
                    block_samples.push(CarData { timestamp, cars });
                }

                Ok(block_samples)
            })
            .collect();

        let samples = results?.into_iter().flatten().collect::<Vec<CarData>>();

        println!("[BIN CACHE] Saving parsed car data to binary cache...");
        let bytes = bincode::serialize(&samples).map_err(|e| F1Error::Bincode(e.to_string()))?;
        write(&cache_file, &bytes).await?;

        Ok(samples)
    }

    pub async fn get_session_data(&self) -> Result<SessionInfo, F1Error> {
        let text = self.fetch_raw("SessionInfo.json").await?;
        let raw: RawSessionInfo = serde_json::from_str(&text)?;
        let meeting_name = raw.meeting.as_ref().map(|m| m.name.clone()).unwrap();
        let country = raw.meeting.map(|m| m.country.name.clone()).unwrap();
        let session_name = raw.session_name;
        let start_date = parse_f1_date(&raw.start_date)?;

        Ok(SessionInfo {
            meeting_name,
            country,
            session_name,
            start_date,
        })
    }

    pub async fn get_lap_count(&self) -> Result<Vec<(RawOffset, u32, u32)>, F1Error> {
        let lines = self.fetch_stream("LapCount.jsonStream").await?;
        let mut events = Vec::new();

        for (ts_str, payload) in lines {
            let raw: RawLapCount = serde_json::from_str(&payload)?;
            let offset = parse_offset(&ts_str)?;
            let total = raw.total_laps.unwrap_or(raw.current_lap);
            events.push((RawOffset(offset), raw.current_lap, total));
        }
        Ok(events)
    }

    pub async fn get_weather_data(&self) -> Result<Vec<(RawOffset, Weather)>, F1Error> {
        let lines = self.fetch_stream("WeatherData.jsonStream").await?;
        let mut events = Vec::new();

        for (ts_str, payload) in lines {
            let raw: RawWeather = serde_json::from_str(&payload)?;
            let offset = parse_offset(&ts_str)?;

            let is_raining = raw.rainfall == "1" || raw.rainfall.eq_ignore_ascii_case("true");

            events.push((
                RawOffset(offset),
                Weather {
                    air_temp_c: raw.air_temp.parse().unwrap_or(0.0),
                    track_temp_c: raw.track_tmep.parse().unwrap_or(0.0),
                    humidity_pct: raw.humidity.parse().unwrap_or(0),
                    is_raining,
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
            let raw: RawRaceControlBlock = serde_json::from_str(&payload)?;
            let offset = parse_offset(&ts_str)?;

            let message_list = match raw.messages {
                serde_json::Value::Array(arr) => arr,
                serde_json::Value::Object(map) => map.into_values().collect(),
                _ => vec![],
            };

            for msg_value in message_list {
                let msg: RawRaceControlMessage = serde_json::from_value(msg_value)?;
                events.push((
                    RawOffset(offset),
                    RaceControlMessage {
                        category: msg.category,
                        message: msg.message,
                        flag: msg.flag,
                    },
                ));
            }
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

fn parse_f1_date(date_str: &str) -> Result<DateTime<Utc>, F1Error> {
    if let Ok(dt) = date_str.parse::<DateTime<Utc>>() {
        return Ok(dt);
    }

    if let Ok(ndt) = NaiveDateTime::parse_from_str(date_str, "%Y-%m-%dT%H:%M:%S") {
        return Ok(Utc.from_utc_datetime(&ndt));
    }

    if let Ok(naive_date) = NaiveDate::parse_from_str(date_str, "%Y-%m-%d") {
        let midnight = naive_date.and_hms_opt(0, 0, 0).unwrap();
        return Ok(Utc.from_utc_datetime(&midnight));
    }
    Err(F1Error::UnexpectedFormat(format!(
        "Could not parse date: '{}'",
        date_str
    )))
}
