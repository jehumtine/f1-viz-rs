use std::time::Duration;
use std::{collections::HashMap, str};

use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};

#[derive(Debug, Deserialize)]
pub struct RawDriver {
    #[serde(rename = "RacingNumber")]
    pub racing_number: String,
    #[serde(rename = "BroadcastName")]
    pub broadcast_name: String,
    #[serde(rename = "FullName")]
    pub full_name: String,
    #[serde(rename = "Tla")] // Three Letter Acronym
    pub tla: String,
    #[serde(rename = "TeamName")]
    pub team_name: String,
    #[serde(rename = "TeamColour")]
    pub team_colour: String, //Hex code
}

#[derive(Debug, Clone, Serialize)]
pub struct Driver {
    pub number: u8,
    pub code: String,
    pub name: String,
    pub team: String,
    pub color: String,
}

#[derive(Debug, Deserialize)]
pub struct RawPositionBlock {
    #[serde(rename = "Timestamp")]
    pub timestamp: String,
    #[serde(rename = "Entries")]
    pub entries: HashMap<String, RawPositionEntry>,
}

#[derive(Debug, Deserialize)]
pub struct RawPositionEntry {
    #[serde(rename = "Status")]
    pub status: String, // "OnTrack", "OffTrack", "PitLane"
    #[serde(rename = "X")]
    pub x: i32,
    #[serde(rename = "Y")]
    pub y: i32,
    #[serde(rename = "Z")]
    pub z: i32,
}

#[derive(Debug, Clone, Serialize)]
pub struct PositionSample {
    pub timestamp: DateTime<Utc>,
    pub cars: HashMap<u8, CarPosition>,
}

#[derive(Debug, Clone, Copy, Serialize)]
pub struct CarPosition {
    pub x_m: f64,
    pub y_m: f64,
    pub z_m: f64,
    pub on_track: bool,
}

#[derive(Debug, Deserialize)]
pub struct RawCarDataBlock {
    pub entries: Vec<RawCarDataEntry>,
}

#[derive(Debug, Deserialize)]
pub struct RawCarDataEntry {
    pub utc: String,
    pub cars: HashMap<String, RawCarChannels>,
}

#[derive(Debug, Deserialize)]
pub struct RawCarChannels {
    pub channels: HashMap<String, i32>,
}

#[derive(Debug, Clone, Serialize)]
pub struct CarData {
    pub timestamp: DateTime<Utc>,
    pub cars: HashMap<u8, CarTelemetry>,
}

#[derive(Debug, Clone, Copy, Serialize)]
pub struct CarTelemetry {
    pub rpm: u32,
    pub speed_kph: u32,
    pub gear: u8,
    pub throttle_pct: u8,
    pub brake: bool,
    pub drs: u8,
}

#[derive(Debug, Clone, Deserialize)]
pub struct RawSessionInfo {
    #[serde(rename = "Meeting")]
    pub meeting: Option<RawMeeting>,
    #[serde(rename = "Name")]
    pub session_name: String,
    #[serde(rename = "StartDate")]
    pub start_date: String,
    #[serde(rename = "EndDate")]
    pub end_date: String,
    #[serde(rename = "GmtOffset")]
    pub gmt_offset: String,
}

#[derive(Debug, Clone, Serialize)]
pub struct SessionInfo {
    pub meeting_name: String,
    pub country: String,
    pub session_name: String,
    pub start_date: DateTime<Utc>,
}

#[derive(Debug, Clone, Deserialize)]
pub struct RawMeeting {
    #[serde(rename = "Name")]
    pub name: String,
    #[serde(rename = "Country")]
    pub country: String,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
pub struct RawOffset(pub Duration);

#[derive(Debug, Deserialize)]
pub struct RawTrackStatus {
    #[serde(rename = "Status")]
    pub status: u8,
    #[serde(rename = "Message")]
    pub message: String,
}

#[derive(Debug, Clone, Serialize)]
pub struct TrackStatusEvent {
    pub status: u8,
    pub message: String,
}

#[derive(Debug, Deserialize)]
pub struct RawLapCount {
    #[serde(rename = "CurrentLap")]
    pub current_lap: u32,
    #[serde(rename = "TotalLaps")]
    pub total_laps: Option<u32>,
}

#[derive(Debug, Deserialize)]
pub struct RawWeather {
    #[serde(rename = "AirTemp")]
    pub air_temp: String,
    #[serde(rename = "TrackTemp")]
    pub track_tmep: String,
    #[serde(rename = "Humidity")]
    pub humidity: String,
    #[serde(rename = "Rainfall")]
    pub rainfall: bool,
}

#[derive(Debug, Clone, Serialize)]
pub struct WeatherSample {
    pub air_temp_c: f32,
    pub track_temp_c: f32,
    pub humidity_pct: u8,
    pub is_raining: bool,
}

#[derive(Debug, Deserialize)]
pub struct RawRaceControl {
    #[serde(rename = "Category")]
    pub category: String,
    #[serde(rename = "Message")]
    pub message: String,
    #[serde(rename = "Flag")]
    pub flag: Option<String>,
}

#[derive(Debug, Clone, Serialize)]
pub struct RaceControlMessage {
    pub category: String,
    pub message: String,
    pub flag: Option<String>,
}
