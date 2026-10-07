use std::collections::HashMap;

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

pub fn lap_at(crossings: &[f64], t: f64, race_start: f64) -> u32 {
    let a = crossings.partition_point(|&c| c <= race_start);
    let b = crossings.partition_point(|&c| c <= t);
    b.saturating_sub(a) as u32 + 1
}

#[derive(Debug, Clone)]
pub struct DriverRace {
    pub num: u8,
    /// Counted race-lap line crossings: anchored, artifacts removed.
    crossings: Vec<f64>,
    /// Session time at which this driver left the classification, if ever.
    retired_at: Option<f64>,
}

#[derive(Clone)]
pub struct RaceModel {
    drivers: Vec<DriverRace>,
    cl: CenterlineIndex,
    race_start: f64,
    lap_est: f64,
}

impl RaceModel {
    pub fn build(per_driver: &HashMap<u8, Vec<(f64, P2)>>, cl: CenterlineIndex) -> Self {
        let lap_est = cl.total as f64 / 60.0;
        let session_end = per_driver
            .values()
            .filter_map(|v| v.last())
            .map(|(t, _)| *t)
            .fold(0.0f64, f64::max);

        let mut form: Vec<f64> = Vec::new();
        for samples in per_driver.values() {
            let raw = raw_crossings(samples, &cl);
            let mut best_gap = 2.0 * lap_est;
            let mut best_end: Option<f64> = None;
            for w in raw.windows(2) {
                let g = w[1] - w[0];
                if g > best_gap && w[1] < 0.5 * session_end {
                    best_gap = g;
                    best_end = Some(w[1]);
                }
            }
            if let Some(f) = best_end {
                form.push(f);
            }
        }

        let anchor = if form.is_empty() {
            0.0
        } else {
            form.sort_by(|a, b| a.partial_cmp(b).unwrap());
            form[form.len() / 2] + 0.5 * lap_est
        };

        let mut drivers = Vec::new();
        let mut first_kept: Vec<f64> = Vec::new();
        for (&num, samples) in per_driver {
            let mut cs: Vec<f64> = raw_crossings(samples, &cl)
                .into_iter()
                .filter(|&c| c > anchor)
                .collect();
            cs.dedup_by(|b, a| *b - *a < 0.6 * lap_est);
            if let Some(&first) = cs.first() {
                first_kept.push(first);
            }

            let last_move = last_movement_time(samples);
            let last_cross = cs.last().copied().unwrap_or(0.0);
            let retired_at =
                if last_move < session_end - lap_est && last_cross < session_end - lap_est {
                    Some(last_move.max(last_cross))
                } else {
                    None
                };

            drivers.push(DriverRace {
                num,
                crossings: cs,
                retired_at,
            });
        }
        drivers.sort_by_key(|d| d.num);

        let race_start = first_kept.into_iter().fold(f64::INFINITY, f64::min);

        Self {
            drivers,
            cl,
            race_start,
            lap_est,
        }
    }

    /// The entire per-frame path. No heuristics, no thresholds, no new params ever.
    pub fn classify(&self, t: f64, cars: &HashMap<u8, UnifiedCarState>) -> Vec<LeaderRow> {
        let mut rows: Vec<LeaderRow> = self
            .drivers
            .iter()
            .filter_map(|d| {
                let state = cars.get(&d.num)?;
                Some(LeaderRow {
                    num: d.num,
                    lap: d.crossings.partition_point(|&c| c <= t) as u32 + 1,
                    progress: self
                        .cl
                        .progress(state.position.x_m as f32, state.position.y_m as f32),
                    on_track: state.position.on_track,
                    speed_kph: state.telemetry.speed_kph,
                    retired: d.retired_at.is_some_and(|r| t >= r),
                    gap: GapKind::Leader,
                })
            })
            .collect();

        rows.sort_by(|a, b| {
            a.retired
                .cmp(&b.retired)
                .then(b.lap.cmp(&a.lap))
                .then(b.progress.partial_cmp(&a.progress).unwrap())
        });

        let pre = t < self.race_start;
        if let Some(lead) = rows.first().cloned() {
            let v = (lead.speed_kph as f32 / 3.6).max(20.0);
            for (i, r) in rows.iter_mut().enumerate() {
                r.gap = if r.retired {
                    GapKind::Retired
                } else if i == 0 || pre {
                    GapKind::Leader
                } else if r.lap < lead.lap {
                    GapKind::Laps(lead.lap - r.lap)
                } else {
                    GapKind::Time((lead.progress - r.progress).rem_euclid(self.cl.total) / v)
                };
            }
        }
        rows
    }
}

#[derive(Debug, Clone, Copy)]
pub enum GapKind {
    Leader,
    Retired,
    Time(f32),
    Laps(u32),
}

#[derive(Debug, Clone)]
pub struct LeaderRow {
    pub num: u8,
    pub lap: u32,
    pub progress: f32,
    pub speed_kph: u32,
    pub retired: bool,
    pub gap: GapKind,
    pub on_track: bool,
}

fn raw_crossings(samples: &[(f64, P2)], cl: &CenterlineIndex) -> Vec<f64> {
    let mut out = Vec::new();
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
            out.push(t);
        }
        hint = idx;
        prev_s = s;
    }
    out
}

/// Last time the car's position changed meaningfully (>5 m from its anchor).
/// A parked car stops updating this — the retirement signal, computed once.
fn last_movement_time(samples: &[(f64, P2)]) -> f64 {
    let mut last = samples.first().map(|(t, _)| *t).unwrap_or(0.0);
    let mut anchor = samples[0].1;
    for &(t, p) in samples {
        if p.dist(anchor) > 5.0 {
            last = t;
            anchor = p;
        }
    }
    last
}
