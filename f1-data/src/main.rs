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

    if let Some(first_sample) = positions.get(0) {
        if let Some(ver_pos) = first_sample.cars.get(&1) {
            println!("Verstappen start pos: X={}, Y={}", ver_pos.x_m, ver_pos.y_m);
        }
    }

    Ok(())
}
