use std::{collections::HashMap, time::Duration};

use crate::{
    engine::{
        timeline::{SessionState, Timeline},
        track::{CarTrack, UnifiedCarState},
    },
    model::raw::RawOffset,
};

#[derive(Debug, Clone)]
pub struct Frame {
    pub at: RawOffset,
    pub state: SessionState,
    pub cars: HashMap<u8, UnifiedCarState>,
}

pub struct SessionPlayer {
    timeline: Timeline,
    tracks: HashMap<u8, CarTrack>,
    cursor: usize,
    current_t: RawOffset,
    state: SessionState,
}

impl SessionPlayer {
    pub fn new(timeline: Timeline, tracks: HashMap<u8, CarTrack>) -> Self {
        Self {
            timeline,
            tracks,
            cursor: 0,
            current_t: RawOffset(Duration::ZERO),
            state: SessionState::default(),
        }
    }
    pub fn duration(&self) -> RawOffset {
        self.timeline
            .events()
            .last()
            .map(|e| e.at())
            .unwrap_or(RawOffset(Duration::ZERO))
    }

    fn advance_to(&mut self, t: RawOffset) {
        let events = self.timeline.events();
        while let Some(ev) = events.get(self.cursor) {
            if ev.at() > t {
                break;
            }
            self.state.apply(ev);
            self.cursor += 1;
        }
    }

    pub fn frame_at(&mut self, t: RawOffset) -> Frame {
        if t < self.current_t {
            self.cursor = 0;
            self.state = SessionState::default();
        }
        self.current_t = t;
        self.advance_to(t);

        let cars = self
            .tracks
            .iter()
            .filter_map(|(&drv, track)| track.interpolate_at(t).map(|s| (drv, s)))
            .collect();

        Frame {
            at: t,
            state: self.state.clone(),
            cars,
        }
    }
}
