use std::{collections::HashMap, str};

use chrono::{DateTime, Utc};
use derive_more::Display;
use serde::{Deserialize, Serialize};

use crate::model::raw::{RawCarDataEntry, RawOffset, RawPositionBlock};

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Driver {
    pub number: u8,
    pub code: String,
    pub name: String,
    pub team: String,
    pub color: String,
}

#[derive(Debug, Serialize, Deserialize)]
pub struct PositionRoot {
    #[serde(rename = "Position")]
    pub position: Vec<RawPositionBlock>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PositionSample {
    pub offset: RawOffset,
    pub absolute: Option<DateTime<Utc>>,
    pub cars: HashMap<u8, CarPosition>,
}

#[derive(Debug, Clone, Copy, Serialize, Deserialize)]
pub struct CarPosition {
    pub x_m: f64,
    pub y_m: f64,
    pub z_m: f64,
    pub on_track: bool,
}

#[derive(Debug, Deserialize, Serialize)]
pub struct CarDataRoot {
    #[serde(rename = "Entries")]
    pub entries: Vec<RawCarDataEntry>,
}

#[derive(Debug, Clone, Serialize, Display, Deserialize)]
#[display(
    "Offset: {offset}\n\
     Cars: \n{}",
    "fmt_cars(cars)"
)]
pub struct CarData {
    pub offset: RawOffset,
    pub cars: HashMap<u8, CarTelemetry>,
}

#[allow(dead_code)]
fn fmt_cars(cars: &HashMap<u8, CarTelemetry>) -> String {
    cars.iter()
        .map(|(id, telemetry)| format!("  [Car Id {}]\n{}", id, telemetry))
        .collect::<Vec<String>>()
        .join("\n")
}

#[derive(Debug, Clone, Copy, Serialize, Display, Deserialize)]
#[display(
    "RPM: {rpm}\n\
    Speed: {speed_kph}\n\
    Gear: {gear}\n\
    Throttle Pct: {throttle_pct}\n\
    Brake: {brake} \n\
    DRS: {drs}"
)]
pub struct CarTelemetry {
    pub rpm: u32,
    pub speed_kph: u32,
    pub gear: u8,
    pub throttle_pct: u8,
    pub brake: bool,
    pub drs: u8,
}

#[derive(Debug, Clone, Serialize, Display, Deserialize)]
#[display(
    "Meeting Name: {meeting_name}\n\
    Country: {country}\n\
    Session Name: {session_name}\n\
    Start Date : {start_date}"
)]
pub struct SessionInfo {
    pub meeting_name: String,
    pub country: String,
    pub session_name: String,
    pub start_date: DateTime<Utc>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TrackStatusEvent {
    pub status: u8,
    pub message: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Weather {
    pub air_temp_c: f32,
    pub track_temp_c: f32,
    pub humidity_pct: u8,
    pub is_raining: bool,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RaceControlMessage {
    pub category: String,
    pub message: String,
    pub flag: Option<String>,
}
