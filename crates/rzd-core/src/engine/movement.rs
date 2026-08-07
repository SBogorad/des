use crate::entities::{CrewState, FailureKind, LocoState};
use crate::events::EventKind;
use crate::network::VoltageKind;

use super::Engine;
use super::EngineError;

impl Engine {
    pub fn region_multiplier_for_segment(&self, seg: &crate::network::Segment) -> f32 {
        self.network
            .region_for_segment(&seg.id)
            .map(|region_id| {
                self.network
                    .region_norms
                    .get_default(region_id)
                    .failure_rate_multiplier
            })
            .unwrap_or(1.0)
    }

    pub(crate) fn find_available_loco(&self, station: &str) -> Option<String> {
        self.loco_by_station.get(station).and_then(|ids| {
            ids.iter()
                .find(|id| {
                    self.locos
                        .get(*id)
                        .is_some_and(|l| l.state == LocoState::Available)
                })
                .cloned()
        })
    }

    pub(crate) fn move_loco(&mut self, loco_id: &str, new_station: &str) {
        if let Some(loco) = self.locos.get_mut(loco_id) {
            let old_station = loco.position.clone();
            loco.position = new_station.to_string();

            if let Some(stations) = self.loco_by_station.get_mut(&old_station) {
                stations.retain(|id| id != loco_id);
            }
            self.loco_by_station
                .entry(new_station.to_string())
                .or_default()
                .push(loco_id.to_string());
        }
    }

    pub(crate) fn set_loco_state(&mut self, loco_id: &str, state: LocoState) {
        if let Some(loco) = self.locos.get_mut(loco_id) {
            if !loco.state.can_transition_to(&state) {
                return;
            }
            loco.state = state;
        }
        let utilization = match state {
            LocoState::InUse => 1.0,
            LocoState::Available | LocoState::Failed | LocoState::Maintenance { .. } => 0.0,
        };
        self.stats
            .loco_utilization
            .update(self.current_time as f64, utilization);
    }

    pub(crate) fn update_crew_utilization(&mut self, crew_id: &str) {
        let utilization = self.crews.get(crew_id).map_or(0.0, |c| match c.state {
            CrewState::Working | CrewState::Waiting | CrewState::Reserve => 1.0,
            _ => 0.0,
        });
        self.stats
            .crew_utilization
            .update(self.current_time as f64, utilization);
    }

    pub(crate) fn is_turnaround(&self, station_id: &str) -> bool {
        self.turnaround_stations.contains(station_id)
    }

    pub(crate) fn start_movement(
        &mut self,
        crew_id: String,
        loco_id: Option<String>,
    ) -> Result<(), EngineError> {
        let (current_segment, _segment_index) = {
            let crew = self
                .crews
                .get(&crew_id)
                .ok_or(EngineError::CrewNotFound(crew_id.clone()))?;
            if crew.route_segments.is_empty() || crew.route_index >= crew.route_segments.len() {
                self.emit(
                    self.current_time + 10,
                    EventKind::ShiftEnd,
                    Some(crew_id),
                    loco_id,
                    None,
                    10,
                );
                return Ok(());
            }
            let seg_id = crew.route_segments[crew.route_index].clone();
            let idx = crew.route_index;
            (seg_id, idx)
        };

        let base_travel_time = self
            .network
            .segment_travel_time_min(&current_segment)
            .unwrap_or(60.0);

        let physics_time = if let Some(seg) = self.network.get_segment(&current_segment) {
            let loco_kind = loco_id
                .as_ref()
                .and_then(|lid| self.locos.get(lid))
                .map(|l| l.loco_kind)
                .unwrap_or(crate::entities::LocoKind::Electric);
            let loco_sections = loco_id
                .as_ref()
                .and_then(|lid| self.locos.get(lid))
                .map(|l| l.sections)
                .unwrap_or(2);
            let region_mult = if let Some(seg_ref) = self.network.get_segment(&current_segment) {
                self.region_multiplier_for_segment(seg_ref)
            } else {
                1.0
            };
            crate::physics::TrainPhysics::for_loco_kind(loco_kind, loco_sections)
                .compute_travel_time_min(seg.distance_km, seg.base_speed_kmh, seg.gradient_factor)
                * region_mult
        } else {
            base_travel_time
        };

        let sag_factor = if self.crews.get(&crew_id).is_some_and(|c| c.in_voltage_sag) {
            self.norms.voltage_sag_speed_factor
        } else {
            1.0
        };
        let jitter = self.prng.range(0.85, 1.15);
        let actual_time = (physics_time * sag_factor * jitter) as u64;

        if let Some(crew) = self.crews.get_mut(&crew_id) {
            crew.set_state(CrewState::Working);
        }

        // Check for failure during travel
        let seg = self.network.get_segment(&current_segment).cloned();
        if let Some(seg) = seg {
            let fail_prob = seg.failure_base_rate * self.region_multiplier_for_segment(&seg);
            if self.prng.chance(fail_prob as f64) {
                let failure_time = self.prng.range(0.0, actual_time as f32) as u64;
                let all_failures = FailureKind::all();
                let fail_kind =
                    all_failures[self.prng.range(0.0, all_failures.len() as f32) as usize];
                self.emit(
                    self.current_time + failure_time.max(1),
                    EventKind::FailureOccurred(fail_kind),
                    Some(crew_id.clone()),
                    loco_id.clone(),
                    Some(current_segment.clone()),
                    5,
                );
            }

            // Voltage sag
            if seg.voltage_sag_prob > 0.0 && self.prng.chance(seg.voltage_sag_prob as f64) {
                let sag_time = self.prng.range(0.0, actual_time as f32) as u64;
                let from_station = self.network.get_station(&seg.from);
                let voltage_v = match from_station.map(|s| s.voltage_kv) {
                    Some(VoltageKind::DC3000) => self.prng.range(2200.0, 2700.0) as u32,
                    Some(VoltageKind::AC25000) => self.prng.range(18000.0, 22000.0) as u32,
                    _ => 0,
                };
                if voltage_v > 0 {
                    self.emit(
                        self.current_time + sag_time.max(1),
                        EventKind::VoltageSagStart {
                            voltage_v,
                            segment_id: current_segment.clone(),
                        },
                        Some(crew_id.clone()),
                        loco_id.clone(),
                        Some(current_segment.clone()),
                        5,
                    );
                }
            }
        }

        // Schedule arrival at next station
        let travel_seg = current_segment.clone();
        let cid = crew_id.clone();
        let arrive_id = self.emit(
            self.current_time + actual_time,
            EventKind::ArriveStation,
            Some(crew_id),
            loco_id,
            Some(travel_seg),
            10,
        );
        self.pending_arrivals.insert(cid, arrive_id);
        Ok(())
    }
}
