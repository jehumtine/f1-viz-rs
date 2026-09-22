pub mod archive;
pub mod decode;
pub mod error;
pub mod model;

use crate::{archive::F1ArchiveClient, error::F1Error};
use std::time::Instant;

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

    Ok(())
}
