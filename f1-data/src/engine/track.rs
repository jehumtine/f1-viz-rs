use crate::{CarData, CarPosition, CarTelemetry, PositionSample, model::raw::RawOffset};

#[derive(Debug, Clone, Copy)]
pub struct UnifiedCarState {
    pub position: CarPosition,
    pub telemetry: CarTelemetry,
    pub live: bool,
}

#[derive(Debug, Clone)]
pub struct CarTrack {
    pub driver_num: u8,
    dense: Vec<(f32, f32, f32, bool)>,
    holes: Vec<(f64, f64, f32, f32, f32)>, // (start_s, end_s, heading_dx, heading_dy, speed_ms)
    pub telemetry: Vec<(RawOffset, CarTelemetry)>,
}

const DENSE_HZ: f64 = 10.0;
const HOLE_S: f64 = 2.0;

impl CarTrack {
    pub fn build(
        driver_num: u8,
        all_positions: &[PositionSample],
        all_car_data: &[CarData],
    ) -> Self {
        let mut positions = Vec::new();
        let mut telemetry = Vec::new();
        for sample in all_positions {
            if let Some(pos) = sample.cars.get(&driver_num) {
                positions.push((sample.offset, *pos));
            }
        }
        for sample in all_car_data {
            if let Some(telem) = sample.cars.get(&driver_num) {
                telemetry.push((sample.offset, *telem));
            }
        }
        positions.sort_by_key(|(t, _)| *t);
        positions.dedup_by(|b, a| {
            let dt = (b.0.0.as_secs_f64() - a.0.0.as_secs_f64()).abs();
            if dt < 0.001 {
                return true;
            }
            let dx = b.1.x_m - a.1.x_m;
            let dy = b.1.y_m - a.1.y_m;
            (dx * dx + dy * dy).sqrt() / dt > 115.0
        });
        telemetry.sort_by_key(|(t, _)| *t);

        let mut dense: Vec<(f32, f32, f32, bool)> = Vec::with_capacity(positions.len() * 8);
        let mut holes: Vec<(f64, f64, f32, f32, f32)> = Vec::new();
        let n = positions.len();

        for i in 0..n {
            let (t1, p1) = positions[i];
            let s1 = t1.0.as_secs_f64();
            dense.push((s1 as f32, p1.x_m as f32, p1.y_m as f32, p1.on_track));

            let Some((t2, p2)) = positions.get(i + 1).copied() else {
                break;
            };
            let s2 = t2.0.as_secs_f64();

            if s2 - s1 > HOLE_S {
                // Record the dropout with heading and speed for extrapolation
                let dx = p2.x_m - p1.x_m;
                let dy = p2.y_m - p1.y_m;
                let dist = (dx * dx + dy * dy).sqrt();
                let heading_x = if dist > 0.01 { (dx / dist) as f32 } else { 0.0 };
                let heading_y = if dist > 0.01 { (dy / dist) as f32 } else { 0.0 };

                // Get the average telemetry speed during this dropout
                let avg_speed = avg_speed_during(&telemetry, s1, s2);

                holes.push((s1, s2, heading_x, heading_y, avg_speed));
                continue;
            }

            let p0 = if i >= 1 { positions[i - 1].1 } else { p1 };
            let p3 = positions.get(i + 2).map(|q| q.1).unwrap_or(p2);
            let span = s2 - s1;
            let mut k = 1;
            loop {
                let tk = s1 + k as f64 / DENSE_HZ;
                if tk >= s2 - 1e-6 {
                    break;
                }
                let u = (tk - s1) / span;
                dense.push((
                    tk as f32,
                    catmull_rom(p0.x_m, p1.x_m, p2.x_m, p3.x_m, u) as f32,
                    catmull_rom(p0.y_m, p1.y_m, p2.y_m, p3.y_m, u) as f32,
                    p1.on_track,
                ));
                k += 1;
            }
        }
        dense.shrink_to_fit();

        Self {
            driver_num,
            dense,
            holes,
            telemetry,
        }
    }

    pub fn interpolate_at(&self, t: RawOffset) -> Option<UnifiedCarState> {
        let (pos, live) = self.interpolate_position(t)?;
        let telem = self.hold_telemetry(t)?;
        Some(UnifiedCarState {
            position: pos,
            telemetry: telem,
            live,
        })
    }

    fn interpolate_position(&self, t: RawOffset) -> Option<(CarPosition, bool)> {
        let ts = t.0.as_secs_f64();

        // Check if we're inside a dropout — if so, extrapolate forward
        for (start, end, hx, hy, speed) in &self.holes {
            if ts >= *start && ts <= *end {
                // Find the last dense point before the dropout
                let idx = self.dense.partition_point(|q| (q.0 as f64) <= *start);
                let base = if idx > 0 {
                    self.dense[idx - 1]
                } else {
                    self.dense[0]
                };

                // Extrapolate forward from the base point
                let elapsed = ts - *start;
                let distance = (*speed as f64) * elapsed;
                let x = base.1 as f64 + (*hx as f64) * distance;
                let y = base.2 as f64 + (*hy as f64) * distance;

                return Some((
                    CarPosition {
                        x_m: x,
                        y_m: y,
                        z_m: 0.0,
                        on_track: base.3,
                    },
                    true, // Still live — smooth motion
                ));
            }
        }

        let n = self.dense.len();
        if n == 0 {
            return None;
        }
        let idx = self.dense.partition_point(|q| (q.0 as f64) <= ts);
        if idx == 0 {
            let q = self.dense[0];
            return Some((
                CarPosition {
                    x_m: q.1 as f64,
                    y_m: q.2 as f64,
                    z_m: 0.0,
                    on_track: q.3,
                },
                true,
            ));
        }
        if idx >= n {
            let q = self.dense[n - 1];
            return Some((
                CarPosition {
                    x_m: q.1 as f64,
                    y_m: q.2 as f64,
                    z_m: 0.0,
                    on_track: q.3,
                },
                true,
            ));
        }
        let (ta, xa, ya, oa) = self.dense[idx - 1];
        let (tb, xb, yb, _ob) = self.dense[idx];
        let u = ((ts - ta as f64) / (tb - ta).max(1e-6) as f64).clamp(0.0, 1.0);
        Some((
            CarPosition {
                x_m: lerp(xa as f64, xb as f64, u),
                y_m: lerp(ya as f64, yb as f64, u),
                z_m: 0.0,
                on_track: oa,
            },
            true,
        ))
    }

    fn hold_telemetry(&self, t: RawOffset) -> Option<CarTelemetry> {
        let idx = self.telemetry.partition_point(|(time, _)| *time <= t);
        if idx == 0 {
            return self.telemetry.first().map(|(_, t)| *t);
        }
        let (_, telem) = self.telemetry.get(idx - 1)?;
        Some(*telem)
    }
}

fn lerp(a: f64, b: f64, t: f64) -> f64 {
    a + (b - a) * t
}
fn catmull_rom(p0: f64, p1: f64, p2: f64, p3: f64, u: f64) -> f64 {
    0.5 * ((2.0 * p1)
        + (-p0 + p2) * u
        + (2.0 * p0 - 5.0 * p1 + 4.0 * p2 - p3) * u * u
        + (-p0 + 3.0 * p1 - 3.0 * p2 + p3) * u * u * u)
}

fn avg_speed_during(telemetry: &[(RawOffset, CarTelemetry)], start_s: f64, end_s: f64) -> f32 {
    let mut sum = 0.0;
    let mut count = 0;
    for (t, telem) in telemetry {
        let s = t.0.as_secs_f64();
        if s >= start_s && s <= end_s {
            sum += telem.speed_kph as f64 / 3.6; // km/h -> m/s
            count += 1;
        }
    }
    if count > 0 {
        (sum / count as f64) as f32
    } else {
        0.0
    }
}
