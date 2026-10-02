use std::collections::HashMap;

use f1_data::RawOffset;
use f1_data::engine::track::UnifiedCarState;

use crate::scene::track::P2;

/// Arc-length index over the derived centerline. index 0 = start/finish.
#[derive(Clone)]
pub struct CenterlineIndex {
    points: Vec<P2>,
    cum: Vec<f32>,
    pub total: f32,
}

impl CenterlineIndex {
    pub fn build(centerline: &[P2]) -> Self {
        let n = centerline.len();
        let mut cum = vec![0.0f32; n];
        for i in 1..n {
            cum[i] = cum[i - 1] + centerline[i - 1].dist(centerline[i]);
        }
        let total = cum[n - 1] + centerline[n - 1].dist(centerline[0]);
        Self {
            points: centerline.to_vec(),
            cum,
            total,
        }
    }

    pub fn nearest_index(&self, x: f32, y: f32) -> usize {
        let mut best_i = 0usize;
        let mut best_d = f32::MAX;
        for (i, p) in self.points.iter().enumerate() {
            let d = (p.x - x).powi(2) + (p.y - y).powi(2);
            if d < best_d {
                best_d = d;
                best_i = i;
            }
        }
        best_i
    }

    pub fn nearest_index_near(&self, x: f32, y: f32, hint: usize, window: usize) -> usize {
        let n = self.points.len();
        let mut best_i = hint;
        let mut best_d = f32::MAX;
        for off in -(window as i64)..=(window as i64) {
            let i = (hint as i64 + off).rem_euclid(n as i64) as usize;
            let p = self.points[i];
            let d = (p.x - x).powi(2) + (p.y - y).powi(2);
            if d < best_d {
                best_d = d;
                best_i = i;
            }
        }
        best_i
    }

    pub fn progress(&self, x: f32, y: f32) -> f32 {
        self.cum[self.nearest_index(x, y)]
    }
}

pub fn build_crossings(
    per_driver: &HashMap<u8, Vec<(f64, P2)>>,
    cl: &CenterlineIndex,
) -> HashMap<u8, Vec<f64>> {
    let mut out = HashMap::new();
    for (&num, samples) in per_driver {
        let mut crossings = Vec::new();
        let mut hint = 0usize;
        let mut prev_s = 0.0f32;
        for (i, &(t, p)) in samples.iter().enumerate() {
            let idx = if i == 0 {
                cl.nearest_index(p.x, p.y)
            } else {
                cl.nearest_index_near(p.x, p.y, hint, 20)
            };
            let s = cl.cum[idx];
            if i > 0 && prev_s > 0.85 * cl.total && s < 0.15 * cl.total {
                crossings.push(t);
            }
            hint = idx;
            prev_s = s;
        }
        out.insert(num, crossings);
    }
    out
}

pub fn race_start(lap_events: &[(RawOffset, u32, Option<u32>)]) -> f64 {
    lap_events
        .iter()
        .find(|(_, cur, _)| *cur == 1)
        .map(|(at, _, _)| at.0.as_secs_f64())
        .or_else(|| lap_events.first().map(|(at, _, _)| at.0.as_secs_f64()))
        .unwrap_or(0.0)
}

pub fn lap_at(crossings: &[f64], t: f64, race_start: f64) -> u32 {
    let a = crossings.partition_point(|&c| c <= race_start);
    let b = crossings.partition_point(|&c| c <= t);
    b.saturating_sub(a) as u32 + 1
}

#[derive(Debug, Clone, Copy)]
pub enum GapKind {
    Leader,
    Time(f32),
    Laps(u32),
}

#[derive(Debug, Clone)]
pub struct LeaderRow {
    pub num: u8,
    pub lap: u32,
    pub progress: f32,
    pub speed_kph: u32,
    pub gap: GapKind,
}

pub fn compute_leaderboard(
    cars: &HashMap<u8, UnifiedCarState>,
    cl: &CenterlineIndex,
    crossings: &HashMap<u8, Vec<f64>>,
    t: f64,
    race_start: f64,
) -> Vec<LeaderRow> {
    let mut rows: Vec<LeaderRow> = cars
        .iter()
        .map(|(&num, state)| LeaderRow {
            num,
            lap: crossings
                .get(&num)
                .map(|c| lap_at(c, t, race_start)) // forward it
                .unwrap_or(1),
            progress: cl.progress(state.position.x_m as f32, state.position.y_m as f32),
            speed_kph: state.telemetry.speed_kph,
            gap: GapKind::Leader,
        })
        .collect();

    // Race order: higher lap first, then further along the lap.
    rows.sort_by(|a, b| {
        b.lap
            .cmp(&a.lap)
            .then(b.progress.partial_cmp(&a.progress).unwrap())
    });

    if let Some(lead) = rows.first().cloned() {
        // Floor leader speed so gaps don't explode under SC/pit stops.
        let lead_speed = (lead.speed_kph as f32 / 3.6).max(20.0);
        for (i, row) in rows.iter_mut().enumerate() {
            if i == 0 {
                row.gap = GapKind::Leader;
            } else if row.lap < lead.lap {
                row.gap = GapKind::Laps(lead.lap - row.lap);
            } else {
                let dist = (lead.progress - row.progress).max(0.0);
                row.gap = GapKind::Time(dist / lead_speed);
            }
        }
    }
    rows
}
