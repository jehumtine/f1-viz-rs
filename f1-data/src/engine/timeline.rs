use crate::{
    CarData, PositionSample, RaceControlMessage, TrackStatusEvent,
    model::{domain::Weather, raw::RawOffset},
};

#[derive(Debug, Clone)]
pub enum TimelineEvent {
    Position {
        at: RawOffset,
        sample: PositionSample,
    },
    CarData {
        at: RawOffset,
        sample: CarData,
    },
    TrackStatus {
        at: RawOffset,
        event: TrackStatusEvent,
    },
    Weather {
        at: RawOffset,
        sample: Weather,
    },
    RaceControl {
        at: RawOffset,
        message: RaceControlMessage,
    },
    LapCount {
        at: RawOffset,
        current: u32,
        total: Option<u32>,
    },
}

impl TimelineEvent {
    pub fn at(&self) -> RawOffset {
        match self {
            Self::Position { at, .. }
            | Self::CarData { at, .. }
            | Self::TrackStatus { at, .. }
            | Self::Weather { at, .. }
            | Self::RaceControl { at, .. }
            | Self::LapCount { at, .. } => *at,
        }
    }
}

pub struct Timeline(Vec<TimelineEvent>);

impl Timeline {
    /// Fold every fetched feed into one chronologically sorted timeline.
    /// Consumes the feeds — the Timeline becomes the single source of truth.
    pub fn from_feeds(
        positions: Vec<PositionSample>,
        car_data: Vec<CarData>,
        track_status: Vec<(RawOffset, TrackStatusEvent)>,
        weather: Vec<(RawOffset, Weather)>,
        race_control: Vec<(RawOffset, RaceControlMessage)>,
        lap_count: Vec<(RawOffset, u32, Option<u32>)>,
    ) -> Self {
        let mut events = Vec::new();

        for sample in positions {
            let at = sample.offset;
            events.push(TimelineEvent::Position { at, sample });
        }
        for sample in car_data {
            let at = sample.offset;
            events.push(TimelineEvent::CarData { at, sample });
        }
        for (at, event) in track_status {
            events.push(TimelineEvent::TrackStatus { at, event });
        }
        for (at, sample) in weather {
            events.push(TimelineEvent::Weather { at, sample });
        }
        for (at, message) in race_control {
            events.push(TimelineEvent::RaceControl { at, message });
        }
        for (at, current, total) in lap_count {
            events.push(TimelineEvent::LapCount { at, current, total });
        }
        events.sort_by_key(TimelineEvent::at);
        Self(events)
    }

    pub fn events(&self) -> &[TimelineEvent] {
        &self.0
    }

    pub fn len(&self) -> usize {
        self.0.len()
    }
}

#[derive(Debug, Clone, Default)]
pub struct SessionState {
    pub track_status: u8,
    pub track_status_message: String,
    pub lap: (u32, u32),
    pub weather: Option<Weather>,
    pub last_race_control: Option<RaceControlMessage>,
}

impl SessionState {
    pub fn apply(&mut self, event: &TimelineEvent) {
        match event {
            TimelineEvent::TrackStatus { event, .. } => {
                self.track_status = event.status;
                self.track_status_message = event.message.clone()
            }
            TimelineEvent::LapCount { current, total, .. } => {
                self.lap.0 = *current;
                if let Some(t) = total {
                    self.lap.1 = *t;
                }
            }
            TimelineEvent::Weather { sample, .. } => self.weather = Some(sample.clone()),
            TimelineEvent::RaceControl { message, .. } => {
                self.last_race_control = Some(message.clone())
            }
            _ => {}
        }
    }
}
