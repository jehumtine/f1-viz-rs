use std::time::Duration;

use crate::{error::F1Error, model::raw::RawOffset};

pub fn parse_f1_offset(ts_str: &str) -> Result<RawOffset, F1Error> {
    let parts: Vec<&str> = ts_str.split(':').collect();
    if parts.len() != 3 {
        return Err(F1Error::UnexpectedFormat(format!(
            "Invalid offset format: {}",
            ts_str
        )));
    }

    let hours: u64 = parts[0]
        .parse()
        .map_err(|_| F1Error::UnexpectedFormat("Hours parse error".into()))?;
    let mins: u64 = parts[1]
        .parse()
        .map_err(|_| F1Error::UnexpectedFormat("Mins parse error".into()))?;
    let secs: f64 = parts[2]
        .parse()
        .map_err(|_| F1Error::UnexpectedFormat("Secs parse error".into()))?;

    let total_secs = (hours * 3600) + (mins * 60) + (secs.trunc() as u64);
    // Convert fractional seconds to nanoseconds, capping at 999_999_999
    let nanos = ((secs.fract() * 1_000_000_000.0) as u32).min(999_999_999);

    Ok(RawOffset(Duration::new(total_secs, nanos)))
}
