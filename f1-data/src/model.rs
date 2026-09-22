use std::time::Duration;
use std::{collections::HashMap, str};

use chrono::{DateTime, Utc};
use derive_more::Display;
use serde::{Deserialize, Serialize};

#[derive(Debug, Serialize, Deserialize)]
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

#[derive(Debug, Deserialize, Serialize)]
pub struct RawPositionBlock {
    #[serde(rename = "Timestamp")]
    pub timestamp: String,
    #[serde(rename = "Entries")]
    pub entries: HashMap<String, RawPositionEntry>,
}

#[derive(Debug, Deserialize, Serialize)]
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

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PositionSample {
    pub timestamp: DateTime<Utc>,
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

#[derive(Debug, Deserialize, Serialize)]
pub struct RawCarDataBlock {
    #[serde(rename = "Entries")]
    pub entries: Vec<RawCarDataEntry>,
}

#[derive(Debug, Deserialize, Serialize)]
pub struct RawCarDataEntry {
    #[serde(rename = "Utc")]
    pub utc: String,
    #[serde(rename = "Cars")]
    pub cars: HashMap<String, RawCarChannels>,
}

#[derive(Debug, Deserialize, Serialize)]
pub struct RawCarChannels {
    #[serde(rename = "Channels")]
    pub channels: HashMap<String, i32>,
}

#[derive(Debug, Clone, Serialize, Display, Deserialize)]
#[display(
    "Timestamp: {timestamp}\n\
     Cars: \n{}",
    "fmt_cars(cars)"
)]
pub struct CarData {
    pub timestamp: DateTime<Utc>,
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

#[derive(Debug, Clone, Deserialize, Serialize)]
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

#[derive(Debug, Clone, Deserialize, Serialize)]
pub struct RawMeeting {
    #[serde(rename = "Name")]
    pub name: String,
    #[serde(rename = "Country")]
    pub country: RawCountry,
}

#[derive(Debug, Clone, Deserialize, Serialize)]
pub struct RawCountry {
    #[serde(rename = "Name")]
    pub name: String,
    #[serde(rename = "Code")]
    pub code: String,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
pub struct RawOffset(pub Duration);

#[derive(Debug, Deserialize, Serialize)]
pub struct RawTrackStatus {
    #[serde(rename = "Status")]
    pub status: u8,
    #[serde(rename = "Message")]
    pub message: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TrackStatusEvent {
    pub status: u8,
    pub message: String,
}

#[derive(Debug, Deserialize, Serialize)]
pub struct RawLapCount {
    #[serde(rename = "CurrentLap")]
    pub current_lap: u32,
    #[serde(rename = "TotalLaps")]
    pub total_laps: Option<u32>,
}

#[derive(Debug, Deserialize, Serialize)]
pub struct RawWeather {
    #[serde(rename = "AirTemp")]
    pub air_temp: String,
    #[serde(rename = "TrackTemp")]
    pub track_tmep: String,
    #[serde(rename = "Humidity")]
    pub humidity: String,
    #[serde(rename = "Rainfall")]
    pub rainfall: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Weather {
    pub air_temp_c: f32,
    pub track_temp_c: f32,
    pub humidity_pct: u8,
    pub is_raining: bool,
}

#[derive(Debug, Deserialize, Serialize)]
pub struct RawRaceControl {
    #[serde(rename = "Category")]
    pub category: String,
    #[serde(rename = "Message")]
    pub message: String,
    #[serde(rename = "Flag")]
    pub flag: Option<String>,
}

#[derive(Debug, Deserialize, Serialize)]
pub struct RawRaceControlBlock {
    #[serde(rename = "Messages")]
    pub messages: serde_json::Value,
}

#[derive(Debug, Deserialize, Serialize)]
pub struct RawRaceControlMessage {
    #[serde(rename = "Category")]
    pub category: String,
    #[serde(rename = "Message")]
    pub message: String,
    #[serde(rename = "Flag")]
    pub flag: Option<String>,
    #[serde(rename = "RacingNumber")]
    pub racing_number: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RaceControlMessage {
    pub category: String,
    pub message: String,
    pub flag: Option<String>,
}
