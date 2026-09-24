use chrono::{DateTime, Utc};

use crate::{PositionSample, model::raw::RawOffset};

#[derive(Debug, Clone, Default)]
pub struct SessionClock {
    t0: Option<DateTime<Utc>>,
}

impl SessionClock {
    pub fn learn_from(&mut self, raw_offset: RawOffset, absolute: DateTime<Utc>) {
        if self.t0.is_none() {
            if let Ok(d) = chrono::Duration::from_std(raw_offset.0) {
                self.t0 = Some(absolute - d);
            }
        }
    }

    pub fn to_absolute(&self, raw_offset: RawOffset) -> Option<DateTime<Utc>> {
        let d = chrono::Duration::from_std(raw_offset.0).ok()?;
        self.t0.map(|t0| t0 + d)
    }

    pub fn from_positions(samples: &[PositionSample]) -> Self {
        let mut clock = Self::default();
        for sample in samples {
            if let Some(abs) = sample.absolute {
                clock.learn_from(sample.offset, abs);
                break;
            }
        }
        clock
    }
}
