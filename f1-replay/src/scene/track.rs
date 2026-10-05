use std::collections::HashMap;

use anyhow::{Result, bail};
use f1_data::{RawOffset, model::domain::PositionSample};

pub const CENTERLINE_POINTS: usize = 600;
const GAP_MAX_S: f64 = 1.5;
const MIN_SAMPLES: usize = 100;
const CLOSE_MAX_M: f32 = 120.0;
const MARGIN_M: f32 = 15.0;

#[derive(Debug, Clone, Copy, PartialEq, Default)]
pub struct P2 {
    pub x: f32,
    pub y: f32,
}

impl P2 {
    pub fn new(x: f32, y: f32) -> Self {
        Self { x, y }
    }
    pub fn dist(self, o: Self) -> f32 {
        ((self.x - o.x).powi(2) + (self.y - o.y).powi(2)).sqrt()
    }
    pub fn lerp(self, o: Self, t: f32) -> Self {
        Self {
            x: self.x + (o.x - self.x) * t,
            y: self.y + (o.y - self.y) * t,
        }
    }
}

#[derive(Debug, Clone)]
pub struct TrackFrame {
    pub centerline: Vec<P2>,
    pub bounds_min: P2,
    pub bounds_max: P2,
    pub start_idx: usize,
    pub lap_len_m: f32,
}

impl TrackFrame {
    pub fn center(&self) -> P2 {
        P2::new(
            (self.bounds_min.x + self.bounds_max.x) * 0.5,
            (self.bounds_min.y + self.bounds_max.y) * 0.5,
        )
    }
    pub fn size(&self) -> P2 {
        P2::new(
            self.bounds_max.x - self.bounds_min.x,
            self.bounds_max.y - self.bounds_min.y,
        )
    }

    pub fn to_svg(&self) -> String {
        let mut s = format!(
            "<svg xmlns='http://www.w3.org/2000/svg' viewBox='{} {} {} {}'>",
            self.bounds_min.x,
            self.bounds_min.y,
            self.size().x,
            self.size().y
        );
        s.push_str("<polyline fill='none' stroke='#838B99' stroke-width='10' points='");
        for p in &self.centerline {
            s.push_str(&format!("{},{} ", p.x, p.y));
        }
        let st = self.centerline[self.start_idx];
        s.push_str(&format!(
            "'/><circle cx='{}' cy='{}' r='20' fill='#FFB100'/></svg>",
            st.x, st.y
        ));
        s
    }
}

pub fn derive_track_frame(
    positions: &[PositionSample],
    lap_events: &[(RawOffset, u32, Option<u32>)],
) -> Result<TrackFrame> {
    let mut per_driver: HashMap<u8, Vec<(f64, P2)>> = HashMap::new();
    for s in positions {
        let t = s.offset.0.as_secs_f64();
        for (&num, p) in &s.cars {
            if p.on_track {
                per_driver
                    .entry(num)
                    .or_default()
                    .push((t, P2::new(p.x_m as f32, p.y_m as f32)));
            }
        }
    }
    println!(
        "[track] on-track samples: {} across {} drivers",
        per_driver.values().map(|v| v.len()).sum::<usize>(),
        per_driver.len()
    );

    let bounds = lap_boundaries(lap_events);
    println!("[track] {} lap boundaries from LapCount", bounds.len());
    if bounds.len() < 2 {
        bail!("LapCount feed gave <2 usable boundaries")
    }

    let t_line = bounds[1];
    let mut line_pt = P2::new(0.0, 0.0);
    let mut best_time_diff = f64::MAX;
    for samples in per_driver.values() {
        let idx = samples
            .partition_point(|s| s.0 < t_line)
            .min(samples.len().saturating_sub(1));
        let (t, p) = samples[idx];
        let diff = (t - t_line).abs();
        if diff < best_time_diff {
            best_time_diff = diff;
            line_pt = p;
        }
    }

    let mut candidates: Vec<Vec<P2>> = Vec::new();
    for w in bounds.windows(2) {
        for samples in per_driver.values() {
            let lo = samples.partition_point(|s| s.0 < w[0]);
            let hi = samples.partition_point(|s| s.0 < w[1]);
            let slice = &samples[lo..hi];
            if slice.len() < MIN_SAMPLES {
                continue;
            }
            if slice.windows(2).any(|p| p[1].0 - p[0].0 > GAP_MAX_S) {
                continue;
            }
            let pts: Vec<P2> = slice.iter().map(|s| s.1).collect();
            if pts[0].dist(*pts.last().unwrap()) > CLOSE_MAX_M {
                continue;
            }
            candidates.push(pts);
        }
    }
    println!(
        "[track] {} candidate laps passed completeness+closure",
        candidates.len()
    );
    if candidates.is_empty() {
        bail!("no candidate lap survived completeness/closure gates")
    }

    let lens: Vec<f32> = candidates.iter().map(|c| poly_len(c)).collect();
    let med = median_of(&lens);
    let survivors: Vec<Vec<P2>> = candidates
        .into_iter()
        .zip(&lens)
        .filter(|(_, l)| (*l - med).abs() <= 0.08 * med)
        .map(|(c, _)| c)
        .collect();
    println!(
        "[track] {} laps survived length gate (median {:.0}m)",
        survivors.len(),
        med
    );
    if survivors.is_empty() {
        bail!("length gate filtered everything")
    }

    let resampled: Vec<Vec<P2>> = survivors
        .iter()
        .map(|c| resample_closed(c, CENTERLINE_POINTS))
        .collect();

    let mut aligned: Vec<Vec<P2>> = Vec::with_capacity(resampled.len());
    for curve in resampled {
        let mut best_i = 0;
        let mut best_d = f32::MAX;
        for (i, p) in curve.iter().enumerate() {
            let d = p.dist(line_pt);
            if d < best_d {
                best_d = d;
                best_i = i;
            }
        }
        let mut rotated = Vec::with_capacity(CENTERLINE_POINTS);
        rotated.extend_from_slice(&curve[best_i..]);
        rotated.extend_from_slice(&curve[..best_i]);
        aligned.push(rotated);
    }
    println!(
        "[track] phase-aligned {} curves to start/finish line",
        aligned.len()
    );

    let med_curve = pointwise_median(&aligned);
    let kept: Vec<&Vec<P2>> = aligned
        .iter()
        .filter(|c| c.iter().zip(&med_curve).all(|(p, m)| p.dist(*m) < 20.0))
        .collect();
    let centerline = if kept.is_empty() {
        med_curve
    } else {
        pointwise_mean(&kept)
    };

    let lap_len_m = poly_len(&centerline) + centerline.last().unwrap().dist(centerline[0]);
    let (mut min, mut max) = (P2::new(f32::MAX, f32::MAX), P2::new(f32::MIN, f32::MIN));
    for p in &centerline {
        min.x = min.x.min(p.x);
        max.x = max.x.max(p.x);
        min.y = min.y.min(p.y);
        max.y = max.y.max(p.y);
    }
    let bounds_min = P2::new(min.x - MARGIN_M, min.y - MARGIN_M);
    let bounds_max = P2::new(max.x + MARGIN_M, max.y + MARGIN_M);

    let start_idx = centerline
        .iter()
        .enumerate()
        .min_by(|(_, a), (_, b)| a.dist(line_pt).partial_cmp(&b.dist(line_pt)).unwrap())
        .map(|(i, _)| i)
        .unwrap_or(0);
    println!(
        "[track] centerline {} pts, lap {:.0} m",
        centerline.len(),
        lap_len_m
    );

    Ok(TrackFrame {
        centerline,
        bounds_min,
        bounds_max,
        start_idx,
        lap_len_m,
    })
}

fn lap_boundaries(lap_events: &[(RawOffset, u32, Option<u32>)]) -> Vec<f64> {
    let mut bounds = Vec::new();
    let mut last: Option<u32> = None;
    for (at, cur, _) in lap_events {
        let t = at.0.as_secs_f64();
        match last {
            None => bounds.push(t),
            Some(p) if *cur == p + 1 => bounds.push(t),
            _ => {} // duplicate or jumped counter: not a boundary
        }
        last = Some(*cur);
    }
    bounds
}

fn resample_closed(pts: &[P2], n: usize) -> Vec<P2> {
    let mut cum = vec![0.0f32; pts.len() + 1];
    for i in 0..pts.len() {
        let nxt = if i + 1 == pts.len() {
            pts[0]
        } else {
            pts[i + 1]
        };
        cum[i + 1] = cum[i] + pts[i].dist(nxt);
    }
    let total = cum[pts.len()];
    let mut out = Vec::with_capacity(n);
    let mut seg = 0usize;
    for k in 0..n {
        let target = total * k as f32 / n as f32;
        while seg + 1 < pts.len() && cum[seg + 1] < target {
            seg += 1;
        }
        let b = if seg + 1 == pts.len() {
            pts[0]
        } else {
            pts[seg + 1]
        };
        let span = cum[seg + 1] - cum[seg];
        let t = if span > 1e-6 {
            (target - cum[seg]) / span
        } else {
            0.0
        };
        out.push(pts[seg].lerp(b, t));
    }
    out
}

fn poly_len(pts: &[P2]) -> f32 {
    pts.windows(2).map(|w| w[0].dist(w[1])).sum()
}

fn median_of(v: &[f32]) -> f32 {
    if v.is_empty() {
        return 0.0;
    }
    let mut s = v.to_vec();
    s.sort_by(|a, b| a.partial_cmp(b).unwrap());
    s[s.len() / 2]
}

fn pointwise_median(cands: &[Vec<P2>]) -> Vec<P2> {
    if cands.is_empty() {
        return Vec::new();
    }
    (0..cands[0].len())
        .map(|i| {
            let mut xs: Vec<f32> = cands.iter().map(|c| c[i].x).collect();
            let mut ys: Vec<f32> = cands.iter().map(|c| c[i].y).collect();
            xs.sort_by(|a, b| a.partial_cmp(b).unwrap());
            ys.sort_by(|a, b| b.partial_cmp(a).unwrap());
            ys.sort_by(|a, b| a.partial_cmp(b).unwrap());
            P2::new(xs[xs.len() / 2], ys[ys.len() / 2])
        })
        .collect()
}

fn pointwise_mean(kept: &[&Vec<P2>]) -> Vec<P2> {
    if kept.is_empty() {
        return Vec::new();
    }
    (0..kept[0].len())
        .map(|i| {
            let n = kept.len() as f32;
            P2::new(
                kept.iter().map(|c| c[i].x).sum::<f32>() / n,
                kept.iter().map(|c| c[i].y).sum::<f32>() / n,
            )
        })
        .collect()
}
