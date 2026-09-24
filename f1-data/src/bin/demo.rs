use f1_data::engine::timeline::Timeline;
use f1_data::engine::track::CarTrack;
use f1_data::{F1ArchiveClient, F1Error, RawOffset, SessionClock, SessionPlayer};
use std::{
    collections::HashMap,
    time::{Duration, Instant},
};

macro_rules! profile {
    ($name:expr, $expr:expr) => {{
        let start = Instant::now();
        println!("⏳ {}...", $name);
        let result = $expr.await?;
        println!("✅ {} completed in {:?}", $name, start.elapsed());
        result
    }};
}

#[tokio::main]
async fn main() -> Result<(), F1Error> {
    let client =
        F1ArchiveClient::new("/2023/2023-05-07_Miami_Grand_Prix/2023-05-07_Race/".to_string());

    // 1. Driver List
    let drivers = profile!("Fetching drivers", client.get_driver_list());
    println!("   -> Found {} drivers\n", drivers.len());

    // 2. Position Data (Usually the largest file)
    let positions = profile!("Fetching positions", client.get_position_data());
    println!("   -> Received {} position samples\n", positions.len());
    // 3. Car Data (Usually the largest file)
    let car_data = profile!("Fetching car data", client.get_car_data());
    println!("   -> Received {} car data samples\n", car_data.len());

    // 4. Session Info
    let session_data = profile!("Fetching session data", client.get_session_data());
    println!("   -> Session: {}\n", session_data.meeting_name);

    // 5. Lap Count
    let lap_count = profile!("Fetching lap count", client.get_lap_count());
    println!("   -> Received {} lap count events\n", lap_count.len());

    // 6. Weather Data
    let weather_data = profile!("Fetching weather data", client.get_weather_data());
    println!("   -> Received {} weather samples\n", weather_data.len());

    // 7. Race Control Messages
    let race_control = profile!(
        "Fetching race control messages",
        client.get_race_control_messages()
    );
    println!("   -> Received {} messages\n", race_control.len());

    println!("🏁 All data fetched successfully!");

    let track_status = profile!("Fetching track status", client.get_track_status());
    println!(
        "   -> Received {} track status events\n",
        track_status.len()
    );

    let clock = SessionClock::from_positions(&positions);
    if let Some(first) = positions.first() {
        let wall = clock
            .to_absolute(first.offset)
            .map(|t| t.to_rfc3339())
            .unwrap_or_else(|| "uncalibrated".into());
        println!(
            "Clock calibrated: Offset {} == wall-clock {}",
            first.offset, wall
        );
    }

    let driver_num = 1;
    let track = CarTrack::build(driver_num, &positions, &car_data);

    println!("Car #{} Track built:", driver_num);
    println!("  -> {} position samples", track.positions.len());
    println!("  -> {} telemetry samples", track.telemetry.len());

    let mut tracks = HashMap::new();
    for &num in drivers.keys() {
        tracks.insert(num, CarTrack::build(num, &positions, &car_data));
    }

    let timeline = Timeline::from_feeds(
        positions,
        car_data,
        track_status,
        weather_data,
        race_control,
        lap_count,
    );

    let mut player = SessionPlayer::new(timeline, tracks);
    println!("Session duration: {}\n", player.duration());

    // --- Simulate a 25 FPS replay of the first 2 minutes and time it ---
    let start = Instant::now();
    let step = Duration::from_millis(40); // 25 fps
    let mut t = Duration::ZERO;
    let mut frames = 0;
    while t < Duration::from_secs(120) {
        let _frame = player.frame_at(RawOffset(t));
        frames += 1;
        t += step;
    }
    println!(
        "Rendered {} frames (25fps × 120s) in {:?}",
        frames,
        start.elapsed()
    );

    // --- Spot-check: discrete state + interpolated cars in one struct ---
    let frame = player.frame_at(RawOffset(Duration::from_secs(3600)));
    println!(
        "At {}: lap {}/{}, track status {}, {} cars in frame",
        frame.at,
        frame.state.lap.0,
        frame.state.lap.1,
        frame.state.track_status,
        frame.cars.len()
    );

    // --- Rewind demo: jump backwards, reducer must reset correctly ---
    let frame = player.frame_at(RawOffset(Duration::from_secs(4600)));
    println!(
        "Rewound to {}: lap {}/{}, track status {}",
        frame.at, frame.state.lap.0, frame.state.lap.1, frame.state.track_status
    );

    let f = player.frame_at(RawOffset(Duration::from_secs(4600)));
    println!("At {}: lap {}/{}", f.at, f.state.lap.0, f.state.lap.1); // expect 9/57 now

    let f = player.frame_at(RawOffset(Duration::from_secs(5600))); // genuine rewind
    println!(
        "Went forwards to {}: lap {}/{}",
        f.at, f.state.lap.0, f.state.lap.1
    );
    Ok(())
}
