use crate::entities::LocoKind;
use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TrainPhysics {
    pub acceleration_kmh_per_s: f32,
    pub braking_kmh_per_s: f32,
    pub max_speed_kmh: f32,
    pub resistance_factor: f32,
}

impl Default for TrainPhysics {
    fn default() -> Self {
        Self {
            acceleration_kmh_per_s: 0.5,
            braking_kmh_per_s: 0.8,
            max_speed_kmh: 120.0,
            resistance_factor: 1.0,
        }
    }
}

impl TrainPhysics {
    pub fn for_loco_kind(kind: LocoKind, sections: u8) -> Self {
        let base_accel = match kind {
            LocoKind::Electric => 0.6,
            LocoKind::Diesel => 0.4,
            LocoKind::Dual => 0.5,
        };
        let section_boost = 1.0 + (sections as f32 - 1.0) * 0.05;

        Self {
            acceleration_kmh_per_s: base_accel * section_boost,
            braking_kmh_per_s: 0.8,
            max_speed_kmh: match kind {
                LocoKind::Electric => 140.0,
                LocoKind::Diesel => 100.0,
                LocoKind::Dual => 120.0,
            },
            resistance_factor: 1.0,
        }
    }

    pub fn compute_travel_time_min(
        &self,
        distance_km: f32,
        segment_max_speed_kmh: f32,
        gradient_factor: f32,
    ) -> f32 {
        let effective_max = self.max_speed_kmh.min(segment_max_speed_kmh);

        let accel_time_s = effective_max / self.acceleration_kmh_per_s;
        let accel_dist_km = 0.5 * effective_max * accel_time_s / 3600.0;

        let braking_time_s = effective_max / self.braking_kmh_per_s;
        let braking_dist_km = 0.5 * effective_max * braking_time_s / 3600.0;

        let total_min = if distance_km <= accel_dist_km + braking_dist_km {
            let v_peak = (2.0 * distance_km * self.acceleration_kmh_per_s * 3600.0).sqrt();
            let t_accel = v_peak / self.acceleration_kmh_per_s;
            let t_decel = v_peak / self.braking_kmh_per_s;
            (t_accel + t_decel) / 60.0
        } else {
            let cruise_dist_km = distance_km - accel_dist_km - braking_dist_km;
            let cruise_time_s = cruise_dist_km / effective_max * 3600.0;
            (accel_time_s + cruise_time_s + braking_time_s) / 60.0
        };

        total_min * gradient_factor * self.resistance_factor
    }
}

pub fn compute_travel_time(
    distance_km: f32,
    base_speed_kmh: f32,
    gradient_factor: f32,
    loco_kind: LocoKind,
    loco_sections: u8,
) -> f32 {
    let physics = TrainPhysics::for_loco_kind(loco_kind, loco_sections);
    physics.compute_travel_time_min(distance_km, base_speed_kmh, gradient_factor)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_travel_time_basic() {
        let physics = TrainPhysics::default();
        let time = physics.compute_travel_time_min(50.0, 120.0, 1.0);
        assert!(
            time > 20.0 && time < 40.0,
            "Expected 20-40 min, got {}",
            time
        );
    }

    #[test]
    fn test_travel_time_gradient() {
        let physics = TrainPhysics::default();
        let flat = physics.compute_travel_time_min(50.0, 120.0, 1.0);
        let uphill = physics.compute_travel_time_min(50.0, 120.0, 1.2);
        assert!(uphill > flat, "Uphill {} should be > flat {}", uphill, flat);
    }

    #[test]
    fn test_travel_time_short_distance() {
        let physics = TrainPhysics::default();
        let time = physics.compute_travel_time_min(2.0, 120.0, 1.0);
        assert!(
            time > 0.5,
            "Short distance should still take > 0.5 min, got {}",
            time
        );
    }

    #[test]
    fn test_loco_kind_differences() {
        let electric = TrainPhysics::for_loco_kind(LocoKind::Electric, 2);
        let diesel = TrainPhysics::for_loco_kind(LocoKind::Diesel, 1);
        let t_e = electric.compute_travel_time_min(100.0, 120.0, 1.0);
        let t_d = diesel.compute_travel_time_min(100.0, 120.0, 1.0);
        assert!(
            t_e < t_d,
            "Electric {} should be faster than diesel {}",
            t_e,
            t_d
        );
    }

    #[test]
    fn test_sections_boost() {
        let single = TrainPhysics::for_loco_kind(LocoKind::Electric, 1);
        let double = TrainPhysics::for_loco_kind(LocoKind::Electric, 2);
        let t_s = single.compute_travel_time_min(100.0, 120.0, 1.0);
        let t_d = double.compute_travel_time_min(100.0, 120.0, 1.0);
        assert!(
            t_d < t_s,
            "Double section {} should be faster than single {}",
            t_d,
            t_s
        );
    }
}
