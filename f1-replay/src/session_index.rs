use std::collections::{HashMap, HashSet};
use std::sync::mpsc;

use bevy::prelude::{Res, ResMut, Resource};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SessionKind {
    Practice1,
    Practice2,
    Practice3,
    Sprint,
    Qualifying,
    Race,
}

impl SessionKind {
    pub fn label(self) -> &'static str {
        match self {
            SessionKind::Practice1 => "FP1",
            SessionKind::Practice2 => "FP2",
            SessionKind::Practice3 => "FP3",
            SessionKind::Sprint => "Sprint",
            SessionKind::Qualifying => "Quali",
            SessionKind::Race => "Race",
        }
    }
    pub fn from_name(name: &str) -> Option<Self> {
        match name {
            "Practice 1" => Some(Self::Practice1),
            "Practice 2" => Some(Self::Practice2),
            "Practice 3" => Some(Self::Practice3),
            "Sprint" => Some(Self::Sprint),
            "Qualifying" => Some(Self::Qualifying),
            "Race" => Some(Self::Race),
            _ => None,
        }
    }
}

#[derive(Debug, Clone)]
pub struct SessionEntry {
    pub path: String,
    pub meeting: String,
    pub year: u32,
    pub round: u32,
    pub kind: SessionKind,
}

#[derive(Default)]
pub enum IndexStatus {
    #[default]
    Idle,
    Fetching,
    Ready,
    Failed(String),
}

#[derive(Resource, Default)]
pub struct SessionIndex {
    pub years: Vec<u32>,
    pub years_status: IndexStatus,
    pub entries: Vec<SessionEntry>,
    pub loaded_years: HashSet<u32>,
    pub fetching_years: HashSet<u32>,
    pub failed_years: HashMap<u32, String>,
}

pub enum IndexMsg {
    Years(Result<Vec<u32>, String>),
    YearLoaded(u32, Result<Vec<SessionEntry>, String>),
}

fn current_year() -> u32 {
    let secs = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map(|d| d.as_secs())
        .unwrap_or(0);
    let days = secs / 86400;
    1970 + (days as f64 / 365.2425) as u32
}

#[derive(Resource)]
pub struct IndexChannel {
    pub sender: mpsc::Sender<IndexMsg>,
    pub receiver: std::sync::Mutex<mpsc::Receiver<IndexMsg>>,
}

impl Default for IndexChannel {
    fn default() -> Self {
        let (tx, rx) = mpsc::channel();
        Self {
            sender: tx,
            receiver: std::sync::Mutex::new(rx),
        }
    }
}

const F1_API: &str = "https://livetiming.formula1.com/static";

fn build_client() -> Result<reqwest::Client, String> {
    reqwest::Client::builder()
        .user_agent("Mozilla/5.0 (Windows NT 10.0; Win64; x64) AppleWebKit/537.36")
        .build()
        .map_err(|e| e.to_string())
}

pub async fn fetch_years() -> Result<Vec<u32>, String> {
    let client = build_client()?;
    let top = current_year() + 1; // never hardcode the ceiling again
    let mut years = Vec::new();
    let mut misses = 0u32;
    for year in (2016..=top).rev() {
        let url = format!("{F1_API}/{year}/Index.json");
        let ok = match client.get(&url).send().await {
            Ok(r) => r.status().is_success(), // 404/403 now count as misses
            Err(_) => false,
        };
        if ok {
            years.push(year);
            misses = 0;
        } else {
            misses += 1;
            if misses >= 4 {
                break;
            } // reached the archive floor
        }
    }
    if years.is_empty() {
        return Err("no seasons responded".into());
    }
    Ok(years)
}

pub async fn fetch_year(year: u32) -> Result<Vec<SessionEntry>, String> {
    let client = build_client()?;
    let url = format!("{F1_API}/{year}/Index.json");
    let resp = client
        .get(&url)
        .header(reqwest::header::ACCEPT, "application/json, text/plain, */*")
        .header(reqwest::header::ACCEPT_ENCODING, "identity") // no gzip → no decode feature needed
        .send()
        .await
        .map_err(|e| format!("send: {e}"))?;
    let status = resp.status();
    let body = resp.text().await.map_err(|e| format!("body: {e}"))?;
    if !status.is_success() {
        return Err(format!("HTTP {status}"));
    }
    let v: serde_json::Value = serde_json::from_str(&body).map_err(|e| {
        let head: String = body.chars().take(80).collect();
        format!("parse: {e} | body starts: {head:?}")
    })?;

    let mut out = Vec::new();
    let meetings = v["Meetings"]
        .as_array()
        .map(|a| a.as_slice())
        .unwrap_or(&[]);
    for (i, meeting) in meetings.iter().enumerate() {
        let round = (i + 1) as u32;
        let meeting_display = meeting["Name"].as_str().unwrap_or("Unknown").to_string();
        for session in meeting["Sessions"]
            .as_array()
            .map(|a| a.as_slice())
            .unwrap_or(&[])
        {
            let Some(kind) = SessionKind::from_name(session["Name"].as_str().unwrap_or("")) else {
                continue;
            };
            let path = session["Path"].as_str().unwrap_or("");
            if path.is_empty() {
                continue;
            }
            out.push(SessionEntry {
                path: format!("/{}", path.trim_start_matches('/')),
                meeting: meeting_display.clone(),
                year,
                round,
                kind,
            });
        }
    }
    Ok(out)
}

pub fn request_year(index: &mut ResMut<SessionIndex>, tx: &mpsc::Sender<IndexMsg>, year: u32) {
    if index.loaded_years.contains(&year) || index.fetching_years.contains(&year) {
        return;
    }
    index.fetching_years.insert(year);
    let tx = tx.clone();
    std::thread::spawn(move || {
        let rt = tokio::runtime::Runtime::new().expect("tokio runtime");
        let result = rt.block_on(fetch_year(year));
        let _ = tx.send(IndexMsg::YearLoaded(year, result));
    });
}

pub fn request_years(index: &mut ResMut<SessionIndex>, tx: &mpsc::Sender<IndexMsg>) {
    if !matches!(index.years_status, IndexStatus::Idle) {
        return;
    }
    index.years_status = IndexStatus::Fetching;
    let tx = tx.clone();
    std::thread::spawn(move || {
        let rt = tokio::runtime::Runtime::new().expect("tokio runtime");
        let _ = tx.send(IndexMsg::Years(rt.block_on(fetch_years())));
    });
}

pub fn poll_index(channel: Res<IndexChannel>, mut index: ResMut<SessionIndex>) {
    while let Ok(msg) = channel.receiver.lock().unwrap().try_recv() {
        match msg {
            IndexMsg::Years(Ok(y)) => {
                index.years = y;
                index.years_status = IndexStatus::Ready;
            }
            IndexMsg::Years(Err(e)) => {
                index.years_status = IndexStatus::Failed(e);
            }
            IndexMsg::YearLoaded(y, Ok(mut entries)) => {
                index.fetching_years.remove(&y);
                index.failed_years.remove(&y);
                index.loaded_years.insert(y);
                index.entries.append(&mut entries);
                index
                    .entries
                    .sort_by_key(|e| (std::cmp::Reverse(e.year), e.round));
            }
            IndexMsg::YearLoaded(y, Err(reason)) => {
                index.fetching_years.remove(&y);
                eprintln!("[index] year {y} failed: {reason}");
                index.failed_years.insert(y, reason);
            }
        }
    }
}

pub fn request_years_on_enter(mut index: ResMut<SessionIndex>, channel: Res<IndexChannel>) {
    request_years(&mut index, &channel.sender);
}
