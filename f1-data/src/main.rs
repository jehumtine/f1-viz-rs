use crate::{archive::F1ArchiveClient, error::F1Error};

pub mod archive;
pub mod decode;
pub mod error;
pub mod model;

#[tokio::main]
async fn main() -> Result<(), F1Error> {
    let client =
        F1ArchiveClient::new("/2023/2023-05-07_Miami_Grand_Prix/2023-05-07_Race/".to_string());
    println!("Fetching drivers..");
    let drivers = client.get_driver_list().await?;
    println!("Found {} drivers", drivers.len());
    println!("Fetching positions...");
    let positions = client.get_position_data().await?;
    println!("Received {} position samples", positions.len());

    println!("Getting car list");
    let car_data = client.get_car_data().await?;
    println!("Getting Session data");
    let session_data = client.get_session_data().await?;
    println!("Received session_data {}", session_data);
    let lap_count = client.get_lap_count().await?;
    let weather_data = client.get_weather_data().await?;
    println!("Received weather data {:?}", weather_data);
    let race_control_messages = client.get_race_control_messages().await?;
    println!("Race control messages {:?}", race_control_messages);

    if let Some(first_sample) = positions.get(0) {
        if let Some(ver_pos) = first_sample.cars.get(&1) {
            println!("Verstappen start pos: X={}, Y={}", ver_pos.x_m, ver_pos.y_m);
        }
    }

    Ok(())
}
