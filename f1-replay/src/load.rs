use std::collections::HashMap;

use anyhow::Context;
use bevy::ecs::resource::Resource;
use bevy::prelude::info;
use f1_data::api::client::F1ArchiveClient;
use f1_data::engine::clock::SessionClock;
use f1_data::engine::player::SessionPlayer;
use f1_data::engine::timeline::Timeline;
use f1_data::engine::track::CarTrack;
use f1_data::model::domain::{Driver, SessionInfo};

use crate::race;
use crate::scene::track::{P2, TrackFrame, derive_track_frame};
use crate::theme::TeamPalette;

#[derive(Resource)]
pub struct SessionBundle {
    pub player: SessionPlayer,
    pub clock: SessionClock,
    pub drivers: HashMap<u8, Driver>,
    pub palette: TeamPalette,
    pub info: SessionInfo,
    pub track: TrackFrame,
    pub race: race::RaceModel,
}

pub async fn load_bundle(session_path: &str) -> anyhow::Result<SessionBundle> {
    let client = F1ArchiveClient::new(session_path.to_string());
    println!("we are here! this is the session_path {:?}", session_path);

    let drivers = client.get_driver_list().await.context("driver list")?;
    let info = client.get_session_data().await.context("session info")?;
    let positions = client.get_position_data().await.context("positions")?;
    let laps = client.get_lap_count().await.context("lap count")?; // moved up

    let track = derive_track_frame(&positions, &laps).context("track derivation")?;
    let xs: Vec<f32> = track.centerline.iter().map(|p| p.x).collect();
    let ys: Vec<f32> = track.centerline.iter().map(|p| p.y).collect();
    info!(
        "[track-diag] bbox x[{:.0},{:.0}] y[{:.0},{:.0}] lap {:.0} m pts {}",
        xs.iter().fold(f32::INFINITY, |a, &b| a.min(b)),
        xs.iter().fold(f32::NEG_INFINITY, |a, &b| a.max(b)),
        ys.iter().fold(f32::INFINITY, |a, &b| a.min(b)),
        ys.iter().fold(f32::NEG_INFINITY, |a, &b| a.max(b)),
        track.lap_len_m,
        track.centerline.len(),
    );
    if track.lap_len_m < 100.0 {
        anyhow::bail!(
            "degenerate track (lap {:.0} m): position feed for this session didn't parse — \
         likely a schema change, check raw Position stream",
            track.lap_len_m
        );
    }
    let cl = race::CenterlineIndex::build(&track.centerline);
    let mut per_driver: HashMap<u8, Vec<(f64, P2)>> = HashMap::new();
    let mut last: HashMap<u8, (f64, P2)> = HashMap::new();
    for s in &positions {
        let t = s.offset.0.as_secs_f64();
        for (&num, p) in &s.cars {
            if !p.on_track {
                continue;
            }
            let pt = P2::new(p.x_m as f32, p.y_m as f32);
            if let Some((lt, lp)) = last.get(&num) {
                let dt = t - *lt;
                if dt > 0.01 && lp.dist(pt) / dt as f32 > 120.0 {
                    continue;
                }
            }
            last.insert(num, (t, pt));
            per_driver.entry(num).or_default().push((t, pt));
        }
    }
    let race = race::RaceModel::build(&per_driver, cl);
    #[cfg(debug_assertions)]
    let _ = std::fs::write("track_debug.svg", track.to_svg());
    let car_data = client.get_car_data().await.context("car data")?;
    let status = client.get_track_status().await.context("track status")?;
    let weather = client.get_weather_data().await.context("weather")?;
    let messages = client
        .get_race_control_messages()
        .await
        .context("race control")?;
    let laps = client.get_lap_count().await.context("lap count")?;

    let clock = SessionClock::from_positions(&positions);
    let tracks = drivers
        .keys()
        .map(|&n| (n, CarTrack::build(n, &positions, &car_data)))
        .collect();

    //TODO: make this efficient
    let timeline = Timeline::from_feeds(
        positions.to_vec(),
        car_data.to_vec(),
        status,
        weather,
        messages,
        laps.to_vec(),
    );
    let player = SessionPlayer::new(timeline, tracks);

    let palette = TeamPalette::from_drivers(&drivers);

    Ok(SessionBundle {
        player,
        clock,
        drivers,
        palette,
        info,
        track,
        race,
    })
}
