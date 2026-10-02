use std::time::{Duration, Instant};

use bevy::prelude::*;
use f1_data::RawOffset;

#[derive(Resource)]
pub struct PlaybackClock {
    pub t: Duration,
    pub speed: f32,
    pub playing: bool,
    last_wall: Option<Instant>,
}

impl Default for PlaybackClock {
    fn default() -> Self {
        Self {
            t: Duration::ZERO,
            speed: 1.0,
            playing: true,
            last_wall: None,
        }
    }
}

impl PlaybackClock {
    pub fn tick(&mut self, now: Instant) -> RawOffset {
        if let Some(prev) = self.last_wall {
            if self.playing {
                self.t += (now - prev).mul_f32(self.speed);
            }
        }
        self.last_wall = Some(now);
        RawOffset(self.t)
    }
    pub fn starting_at(secs: u64) -> Self {
        Self {
            t: Duration::from_secs(secs),
            ..Default::default()
        }
    }
}
