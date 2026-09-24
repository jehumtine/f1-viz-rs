use crate::{CarData, CarPosition, CarTelemetry, PositionSample, model::raw::RawOffset};

#[derive(Debug, Clone, Copy)]
pub struct UnifiedCarState {
    pub position: CarPosition,
    pub telemetry: CarTelemetry,
}

#[derive(Debug, Clone, Default)]
pub struct CarTrack {
    pub positions: Vec<(RawOffset, CarPosition)>,
    pub telemetry: Vec<(RawOffset, CarTelemetry)>,
}

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
        telemetry.sort_by_key(|(t, _)| *t);

        Self {
            positions,
            telemetry,
        }
    }

    pub fn interpolate_at(&self, t: RawOffset) -> Option<UnifiedCarState> {
        let pos = self.interpolate_position(t)?;
        let telem = self.hold_telemetry(t)?;

        Some(UnifiedCarState {
            position: pos,
            telemetry: telem,
        })
    }

    fn interpolate_position(&self, t: RawOffset) -> Option<CarPosition> {
        let idx = self.positions.partition_point(|(time, _)| *time <= t);
        if idx == 0 {
            return self.positions.first().map(|(_, p)| *p);
        }
        if idx == self.positions.len() {
            return self.positions.last().map(|(_, p)| *p);
        }
        let before = &self.positions[idx - 1];
        let after = &self.positions[idx];

        let t_before = before.0.0.as_secs_f64();
        let t_after = after.0.0.as_secs_f64();
        let t_current = t.0.as_secs_f64();
        let ratio = ((t_current - t_before) / (t_after - t_before)).clamp(0.0, 1.0);
        Some(CarPosition {
            x_m: lerp(before.1.x_m, after.1.x_m, ratio),
            y_m: lerp(before.1.y_m, after.1.y_m, ratio),
            z_m: lerp(before.1.z_m, after.1.z_m, ratio),
            on_track: before.1.on_track,
        })
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
