use std::{collections::HashMap, fmt, time::Duration};

use serde::{Deserialize, Serialize};

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

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Serialize, Deserialize)]
pub struct RawOffset(pub Duration);

impl fmt::Display for RawOffset {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        let total_secs = self.0.as_secs();
        let hours = total_secs / 3600;
        let mins = (total_secs % 3600) / 60;
        let secs = total_secs % 60;
        let millis = self.0.subsec_millis();
        write!(f, "{:02}:{:02}:{:02}.{:03}", hours, mins, secs, millis)
    }
}

#[derive(Debug, Deserialize, Serialize)]
pub struct RawTrackStatus {
    #[serde(rename = "Status")]
    pub status: String,
    #[serde(rename = "Message")]
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
