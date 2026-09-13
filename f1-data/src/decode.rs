use std::io::Read;

use base64::{Engine, engine::general_purpose::STANDARD};
use flate2::read::DeflateDecoder;
use serde_json::Value;

use crate::error::F1Error;

pub fn decode_f1_z_payload(payload: &str) -> Result<Value, F1Error> {
    let trimmed = payload.trim().trim_matches('"');
    let compressed = STANDARD.decode(trimmed)?;

    let mut decoder = DeflateDecoder::new(&compressed[..]);
    let mut decompressed_string = String::new();
    decoder.read_to_string(&mut decompressed_string)?;
    let clean_json = decompressed_string.trim_start_matches('\u{feff}');

    Ok(serde_json::from_str(clean_json)?)
}
