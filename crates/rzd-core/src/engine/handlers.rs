use crate::entities::{
    AcceptanceIssue, AcceptanceIssueKind, CrewState, FailureKind, FailureSeverity, LocoState,
};
use crate::events::{EventKind, SimEvent, ViolationType};
use crate::network::SegmentId;
use crate::tu3::Tu3Row;
use crate::violations::Severity;

use super::Engine;
use super::EngineError;

impl Engine {
    // === SHIFT LIFECYCLE ===

    pub(crate) fn handle_appear(&mut self, event: SimEvent) -> Result<(), EngineError> {
        let crew_id = self.req_crew_id(&event)?;
        let crew = self
            .crews
            .get_mut(&crew_id)
            .ok_or(EngineError::CrewNotFound(crew_id.clone()))?;

        crew.set_state(CrewState::Appearing);
        crew.shift_start = Some(self.current_time);
        crew.shift_work_min = 0;
        crew.overtime_8h_flagged = false;
        crew.total_distance_km = 0.0;
        crew.in_voltage_sag = false;
        crew.violations.clear();
        crew.acceptance_issues.clear();

        self.emit_now(EventKind::MedCheckStart, Some(crew_id), None, None, 10);
        Ok(())
    }

    pub(crate) fn handle_med_check_start(&mut self, event: SimEvent) -> Result<(), EngineError> {
        let crew_id = self.req_crew_id(&event)?;
        if let Some(crew) = self.crews.get_mut(&crew_id) {
            crew.set_state(CrewState::MedCheck);
        }
        let duration = self.prng.range(10.0, 15.0) as u64;
        self.emit(
            self.current_time + duration,
            EventKind::MedCheckEnd,
            Some(crew_id),
            None,
            None,
            10,
        );
        Ok(())
    }

    pub(crate) fn handle_med_check_end(&mut self, event: SimEvent) -> Result<(), EngineError> {
        let crew_id = self.req_crew_id(&event)?;
        if let Some(crew) = self.crews.get_mut(&crew_id) {
            crew.set_state(CrewState::PreTripCheck);
        }
        let duration = self.prng.range(5.0, 10.0) as u64;
        self.emit(
            self.current_time + duration,
            EventKind::PreTripBriefingEnd,
            Some(crew_id),
            None,
            None,
            10,
        );
        Ok(())
    }

    pub(crate) fn handle_pretrip_briefing_end(
        &mut self,
        event: SimEvent,
    ) -> Result<(), EngineError> {
        let crew_id = self.req_crew_id(&event)?;
        self.emit_now(EventKind::AcceptanceStart, Some(crew_id), None, None, 10);
        Ok(())
    }

    // === ACCEPTANCE ===

    fn generate_acceptance_issues(
        &mut self,
        crew_id: &str,
        loco_id: &str,
        sections: u8,
    ) -> (Vec<AcceptanceIssue>, u32) {
        let base_min = sections as u32 * self.norms.acceptance_per_section_min;
        let mut total_min = base_min;
        let mut issues = Vec::new();

        if self.prng.chance(0.08) {
            let delay = self.prng.range(15.0, 60.0) as u32;
            total_min += delay;
            issues.push(AcceptanceIssue {
                kind: AcceptanceIssueKind::PreviousCrewDelayed,
                delay_min: delay,
                resolved: true,
            });
        }
        if self.prng.chance(0.04) {
            let delay = self.prng.range(20.0, 90.0) as u32;
            total_min += delay;
            let resolved = self.prng.chance(0.7);
            issues.push(AcceptanceIssue {
                kind: AcceptanceIssueKind::KlubUFault,
                delay_min: delay,
                resolved,
            });
            if !resolved {
                self.add_violation(
                    ViolationType::AcceptanceDelay,
                    Severity::Warning,
                    crew_id,
                    Some(loco_id.to_string()),
                    None,
                    &format!("КЛУБ-У неисправен, задержка {} мин", delay),
                    "Регламент CTL-16-2",
                );
            }
        }
        if self.prng.chance(0.03) {
            let delay = self.prng.range(10.0, 40.0) as u32;
            total_min += delay;
            issues.push(AcceptanceIssue {
                kind: AcceptanceIssueKind::RadioFault,
                delay_min: delay,
                resolved: self.prng.chance(0.85),
            });
        }
        if self.prng.chance(0.05) {
            let delay = self.prng.range(30.0, 120.0) as u32;
            total_min += delay;
            let resolved = self.prng.chance(0.6);
            issues.push(AcceptanceIssue {
                kind: AcceptanceIssueKind::MaintenanceNotDone,
                delay_min: delay,
                resolved,
            });
        }

        (issues, total_min)
    }

    pub(crate) fn handle_acceptance_start(&mut self, event: SimEvent) -> Result<(), EngineError> {
        let crew_id = self.req_crew_id(&event)?;
        let crew_pos = self
            .crews
            .get(&crew_id)
            .map(|c| c.position.clone())
            .unwrap_or_default();

        let loco_id = self.find_available_loco(&crew_pos);

        if let Some(lid) = loco_id {
            self.acceptance_retries.remove(&crew_id);
            let sections = self.locos.get(&lid).map_or(2, |l| l.sections);
            self.set_loco_state(&lid, LocoState::InUse);

            if let Some(crew) = self.crews.get_mut(&crew_id) {
                crew.set_state(CrewState::Acceptance);
                crew.current_loco = Some(lid.clone());
            }

            let (issues, total_min) = self.generate_acceptance_issues(&crew_id, &lid, sections);

            if total_min > 90 {
                self.add_violation(
                    ViolationType::AcceptanceDelay,
                    Severity::Warning,
                    &crew_id,
                    Some(lid.clone()),
                    None,
                    &format!("Приёмка ТПС задержана >{} мин", total_min),
                    "Регламент CTL-16-2, п.3.1",
                );
            }

            if let Some(crew) = self.crews.get_mut(&crew_id) {
                crew.acceptance_issues.extend(issues);
            }

            if !self.in_warmup() {
                self.stats.record_acceptance(total_min);
            }
            self.emit(
                self.current_time + total_min as u64,
                EventKind::AcceptanceEnd,
                Some(crew_id),
                Some(lid),
                None,
                10,
            );
        } else {
            let retries = self.acceptance_retries.entry(crew_id.clone()).or_insert(0);
            *retries += 1;
            if *retries > 12 {
                self.acceptance_retries.remove(&crew_id);
                self.emit_now(EventKind::ShiftEnd, Some(crew_id), event.loco_id, None, 5);
            } else {
                let delay = (*retries as u64 * 10).min(60);
                self.emit(
                    self.current_time + delay,
                    EventKind::AcceptanceStart,
                    Some(crew_id),
                    None,
                    None,
                    20,
                );
            }
        }
        Ok(())
    }

    pub(crate) fn handle_acceptance_end(&mut self, event: SimEvent) -> Result<(), EngineError> {
        let crew_id = self.req_crew_id(&event)?;
        if let Some(crew) = self.crews.get_mut(&crew_id) {
            crew.set_state(CrewState::Waiting);
        }
        self.emit(
            self.current_time + 5,
            EventKind::DepartureCheckpoint,
            Some(crew_id),
            event.loco_id,
            None,
            10,
        );
        Ok(())
    }

    pub(crate) fn handle_departure_checkpoint(
        &mut self,
        event: SimEvent,
    ) -> Result<(), EngineError> {
        let crew_id = self.req_crew_id(&event)?;

        let wait_chance = self.prng.next_f32();
        if wait_chance < self.norms.waiting_probability {
            let wait_min = self.prng.range(
                self.norms.waiting_min_min as f32,
                self.norms.waiting_max_min as f32,
            ) as u64;
            if let Some(crew) = self.crews.get_mut(&crew_id) {
                crew.set_state(CrewState::Waiting);
            }
            self.emit_now(
                EventKind::WaitStart,
                Some(crew_id.clone()),
                event.loco_id.clone(),
                None,
                10,
            );
            self.emit(
                self.current_time + wait_min,
                EventKind::WaitEnd,
                Some(crew_id),
                event.loco_id,
                None,
                10,
            );
            if !self.in_warmup() {
                self.stats.record_wait(wait_min);
            }
        } else {
            self.start_movement(crew_id, event.loco_id)?;
        }
        Ok(())
    }

    pub(crate) fn handle_wait_start(&mut self, event: SimEvent) -> Result<(), EngineError> {
        if let Some(crew_id) = &event.crew_id {
            if let Some(crew) = self.crews.get_mut(crew_id) {
                if crew.state != CrewState::Waiting {
                    crew.set_state(CrewState::Waiting);
                }
            }
        }
        Ok(())
    }

    pub(crate) fn handle_wait_end(&mut self, event: SimEvent) -> Result<(), EngineError> {
        let crew_id = self.req_crew_id(&event)?;
        if let Some(crew) = self.crews.get_mut(&crew_id) {
            crew.set_state(CrewState::Working);
            crew.shift_work_min += self.prng.range(20.0, 90.0) as u32;
        }
        self.start_movement(crew_id, event.loco_id)?;
        Ok(())
    }

    // === ARRIVAL ===

    pub(crate) fn handle_arrive(&mut self, event: SimEvent) -> Result<(), EngineError> {
        let crew_id = self.req_crew_id(&event)?;
        let seg_id = self.req_seg_id(&event)?;

        self.pending_arrivals.remove(&crew_id);

        let segment = self.network.get_segment(&seg_id).cloned();
        let (next_station, _travel_time_min, seg_distance) = {
            let crew = self
                .crews
                .get_mut(&crew_id)
                .ok_or(EngineError::CrewNotFound(crew_id.clone()))?;
            crew.route_index += 1;
            let tt = self
                .network
                .segment_travel_time_min(&seg_id)
                .unwrap_or(60.0) as u32;
            crew.shift_work_min += tt;
            let seg = segment
                .as_ref()
                .ok_or(EngineError::SegmentNotFound(seg_id.clone()))?;
            let dist = seg.distance_km;
            crew.total_distance_km += dist;
            (seg.to.clone(), tt, dist)
        };

        if let Some(crew) = self.crews.get_mut(&crew_id) {
            crew.position = next_station.clone();
        }

        if let Some(ref lid) = event.loco_id {
            self.move_loco(lid, &next_station);
            if let Some(loco) = self.locos.get_mut(lid) {
                loco.mileage_km += seg_distance;
            }
        }

        // Check overtime
        {
            let crew = self
                .crews
                .get(&crew_id)
                .ok_or(EngineError::CrewNotFound(crew_id.clone()))?;
            if crew.shift_work_min >= self.norms.max_shift_12h_min {
                self.add_violation(
                    ViolationType::Overtime12h,
                    Severity::Critical,
                    &crew_id,
                    event.loco_id.clone(),
                    event.segment_id.clone(),
                    &format!("Смена > {} мин (12ч)", crew.shift_work_min),
                    "Приказ Минтранса №339, п.4.2",
                );
                self.emit_now(
                    EventKind::LineChangeStart,
                    Some(crew_id.clone()),
                    event.loco_id.clone(),
                    event.segment_id.clone(),
                    5,
                );
                return Ok(());
            } else if crew.shift_work_min >= self.norms.max_shift_8h_min
                && !crew.overtime_8h_flagged
            {
                self.add_violation(
                    ViolationType::Overtime8h,
                    Severity::Warning,
                    &crew_id,
                    event.loco_id.clone(),
                    event.segment_id.clone(),
                    &format!("Смена > {} мин (8ч)", crew.shift_work_min),
                    "Приказ Минтранса №339, п.4.1",
                );
                if let Some(c) = self.crews.get_mut(&crew_id) {
                    c.overtime_8h_flagged = true;
                }
            }
        }

        // Check if route is finished
        let route_finished = {
            let crew = self
                .crews
                .get(&crew_id)
                .ok_or(EngineError::CrewNotFound(crew_id.clone()))?;
            crew.route_index >= crew.route_segments.len()
        };

        if route_finished {
            self.emit(
                self.current_time + 5,
                EventKind::ArrivalCheckpoint,
                Some(crew_id),
                event.loco_id,
                None,
                10,
            );
        } else {
            self.start_movement(crew_id, event.loco_id)?;
        }
        Ok(())
    }

    pub(crate) fn handle_pass_waypoint(&mut self, event: SimEvent) -> Result<(), EngineError> {
        let crew_id = self.req_crew_id(&event)?;

        let route_finished = {
            let crew = self
                .crews
                .get_mut(&crew_id)
                .ok_or(EngineError::CrewNotFound(crew_id.clone()))?;
            crew.shift_work_min += 2;
            crew.route_index += 1;
            crew.route_index >= crew.route_segments.len()
        };

        if route_finished {
            self.emit(
                self.current_time + 5,
                EventKind::ArrivalCheckpoint,
                Some(crew_id),
                event.loco_id,
                None,
                10,
            );
        } else {
            self.start_movement(crew_id, event.loco_id)?;
        }
        Ok(())
    }

    // === FAILURES ===

    pub(crate) fn handle_failure(
        &mut self,
        event: SimEvent,
        kind: FailureKind,
    ) -> Result<(), EngineError> {
        let crew_id = self.req_crew_id(&event)?;

        // Cancel pending arrival for this crew
        if let Some(arrive_id) = self.pending_arrivals.remove(&crew_id) {
            self.queue.cancel(arrive_id);
        }

        if let Some(crew) = self.crews.get_mut(&crew_id) {
            crew.set_state(CrewState::Working);
        }

        if let Some(ref lid) = event.loco_id {
            self.set_loco_state(lid, LocoState::Failed);
            if let Some(loco) = self.locos.get_mut(lid) {
                loco.failures_total += 1;
            }
        }

        let severity = kind.severity();
        self.add_violation(
            ViolationType::LineChangeForced,
            if severity == FailureSeverity::Critical {
                Severity::Critical
            } else {
                Severity::Warning
            },
            &crew_id,
            event.loco_id.clone(),
            event.segment_id.clone(),
            &format!("Отказ {:?} на перегоне", kind),
            "Технический регламент",
        );

        let repair_time = match severity {
            FailureSeverity::Critical => self.prng.range(60.0, 180.0) as u64,
            FailureSeverity::Warning => self.prng.range(15.0, 60.0) as u64,
        };
        if !self.in_warmup() {
            self.stats.record_repair(repair_time);
        }

        self.emit(
            self.current_time + repair_time,
            EventKind::FailureResolved,
            Some(crew_id.clone()),
            event.loco_id.clone(),
            event.segment_id.clone(),
            5,
        );

        if severity == FailureSeverity::Critical
            && self.prng.chance(self.norms.line_change_on_critical as f64)
        {
            self.emit(
                self.current_time + 5,
                EventKind::LineChangeStart,
                Some(crew_id),
                event.loco_id,
                event.segment_id,
                5,
            );
        }
        Ok(())
    }

    pub(crate) fn handle_failure_resolved(&mut self, event: SimEvent) -> Result<(), EngineError> {
        if let Some(ref lid) = event.loco_id {
            let is_failed = self
                .locos
                .get(lid)
                .is_some_and(|l| l.state == LocoState::Failed);
            if is_failed {
                self.set_loco_state(lid, LocoState::Available);
            }
        }

        if let Some(ref crew_id) = event.crew_id {
            if let Some(crew) = self.crews.get_mut(crew_id) {
                crew.set_state(CrewState::Working);
            }
            self.start_movement(crew_id.clone(), event.loco_id)?;
        }
        Ok(())
    }

    pub(crate) fn handle_line_change_start(&mut self, event: SimEvent) -> Result<(), EngineError> {
        let crew_id = self.req_crew_id(&event)?;

        if let Some(ref lid) = event.loco_id {
            let was_in_use = self
                .locos
                .get(lid)
                .is_some_and(|l| l.state == LocoState::InUse);
            let crew_pos = self
                .crews
                .get(&crew_id)
                .map(|c| c.position.clone())
                .unwrap_or_default();
            if was_in_use {
                self.set_loco_state(lid, LocoState::Available);
            }
            if let Some(loco) = self.locos.get_mut(lid) {
                loco.position = crew_pos;
            }
        }

        let go_reserve = self
            .prng
            .chance(self.norms.reserve_after_line_change as f64);
        if go_reserve {
            if let Some(crew) = self.crews.get_mut(&crew_id) {
                crew.set_state(CrewState::Reserve);
                crew.current_loco = None;
            }
            self.emit_now(
                EventKind::ReserveStart,
                Some(crew_id.clone()),
                None,
                event.segment_id.clone(),
                10,
            );
            let travel_back = self.prng.range(60.0, 180.0) as u64;
            self.emit(
                self.current_time + travel_back,
                EventKind::ReserveEnd,
                Some(crew_id),
                None,
                None,
                10,
            );
        } else {
            self.emit_now(EventKind::ShiftEnd, Some(crew_id), event.loco_id, None, 10);
        }
        Ok(())
    }

    // === VOLTAGE SAG ===

    pub(crate) fn handle_voltage_sag_start(
        &mut self,
        event: SimEvent,
        voltage_v: u32,
        segment_id: SegmentId,
    ) -> Result<(), EngineError> {
        let crew_id = self.req_crew_id(&event)?;

        let segment = self.network.get_segment(&segment_id).cloned();
        if let Some(seg) = segment {
            let from_station = self.network.get_station(&seg.from);
            let is_critical = match from_station.map(|s| s.voltage_kv) {
                Some(crate::network::VoltageKind::DC3000) => {
                    voltage_v < self.norms.voltage_sag_threshold_dc as u32
                }
                Some(crate::network::VoltageKind::AC25000) => {
                    voltage_v < self.norms.voltage_sag_threshold_ac as u32
                }
                _ => false,
            };

            if is_critical {
                self.add_violation(
                    ViolationType::VoltageSagShift,
                    Severity::Warning,
                    &crew_id,
                    event.loco_id.clone(),
                    Some(segment_id.clone()),
                    &format!(
                        "Просадка напряжения {} В (норма {} В)",
                        voltage_v,
                        match from_station.map(|s| s.voltage_kv) {
                            Some(crate::network::VoltageKind::DC3000) => 3000,
                            _ => 25000,
                        }
                    ),
                    "Технические условия КС",
                );
            }

            if is_critical {
                if let Some(crew) = self.crews.get_mut(&crew_id) {
                    crew.in_voltage_sag = true;
                }
            }
        }

        let sag_duration = self.prng.range(10.0, 45.0) as u64;
        self.emit(
            self.current_time + sag_duration,
            EventKind::VoltageSagEnd,
            Some(crew_id),
            event.loco_id,
            Some(segment_id),
            8,
        );
        Ok(())
    }

    pub(crate) fn handle_voltage_sag_end(&mut self, event: SimEvent) -> Result<(), EngineError> {
        if let Some(ref crew_id) = event.crew_id {
            if let Some(crew) = self.crews.get_mut(crew_id) {
                crew.in_voltage_sag = false;
            }
        }
        Ok(())
    }

    // === RESERVE ===

    pub(crate) fn handle_reserve_start(&mut self, event: SimEvent) -> Result<(), EngineError> {
        if let Some(crew_id) = &event.crew_id {
            if let Some(crew) = self.crews.get_mut(crew_id) {
                crew.set_state(CrewState::Reserve);
            }
        }
        Ok(())
    }

    pub(crate) fn handle_reserve_end(&mut self, event: SimEvent) -> Result<(), EngineError> {
        let crew_id = self.req_crew_id(&event)?;
        if let Some(crew) = self.crews.get_mut(&crew_id) {
            crew.position = crew.depot_id.clone();
        }
        self.emit_now(EventKind::ShiftEnd, Some(crew_id), None, None, 10);
        Ok(())
    }

    // === TURNAROUND ===

    pub(crate) fn handle_turnaround_rest_start(
        &mut self,
        event: SimEvent,
    ) -> Result<(), EngineError> {
        if let Some(crew_id) = &event.crew_id {
            if let Some(crew) = self.crews.get_mut(crew_id) {
                crew.set_state(CrewState::RestingAway);
            }
        }
        Ok(())
    }

    pub(crate) fn handle_turnaround_rest_end(
        &mut self,
        event: SimEvent,
    ) -> Result<(), EngineError> {
        let crew_id = self.req_crew_id(&event)?;
        self.emit_now(
            EventKind::HandoverStart,
            Some(crew_id),
            event.loco_id,
            None,
            10,
        );
        Ok(())
    }

    pub(crate) fn handle_handover_start(&mut self, event: SimEvent) -> Result<(), EngineError> {
        let crew_id = self.req_crew_id(&event)?;
        let handover_min = self.prng.range(
            self.norms.handover_min_min as f32,
            self.norms.handover_max_min as f32,
        ) as u64;
        if let Some(crew) = self.crews.get_mut(&crew_id) {
            crew.shift_work_min += handover_min as u32;
        }
        self.emit(
            self.current_time + handover_min,
            EventKind::HandoverEnd,
            Some(crew_id),
            event.loco_id,
            None,
            10,
        );
        Ok(())
    }

    pub(crate) fn handle_handover_end(&mut self, event: SimEvent) -> Result<(), EngineError> {
        self.handle_shift_end(event)
    }

    pub(crate) fn handle_arrival_checkpoint(&mut self, event: SimEvent) -> Result<(), EngineError> {
        let crew_id = self.req_crew_id(&event)?;
        let handover_min = self.prng.range(15.0, 30.0) as u64;
        if let Some(crew) = self.crews.get_mut(&crew_id) {
            crew.shift_work_min += handover_min as u32;
        }

        let position = self
            .crews
            .get(&crew_id)
            .map(|c| c.position.clone())
            .unwrap_or_default();
        let at_turnaround = self.is_turnaround(&position);

        if at_turnaround {
            let rest_min = self.prng.range(180.0, 360.0) as u64;

            if (rest_min as u32) < self.norms.min_turnaround_rest_min {
                self.add_violation(
                    ViolationType::ShortRest,
                    Severity::Warning,
                    &crew_id,
                    event.loco_id.clone(),
                    None,
                    &format!(
                        "Отдых в обороте {} мин < минимума {} мин",
                        rest_min, self.norms.min_turnaround_rest_min
                    ),
                    "Приказ Минтранса №339, п.5.3",
                );
            }

            let crew_work = self.crews.get(&crew_id).map_or(0, |c| c.shift_work_min);
            if (rest_min as f32) < (crew_work as f32 * self.norms.turnaround_rest_ratio) {
                self.add_violation(
                    ViolationType::ShortRest,
                    Severity::Warning,
                    &crew_id,
                    event.loco_id.clone(),
                    None,
                    &format!(
                        "Отдых в обороте {} мин < 50% работы ({})",
                        rest_min, crew_work
                    ),
                    "Приказ Минтранса №339, п.5.3",
                );
            }

            self.emit_now(
                EventKind::TurnaroundRestStart,
                Some(crew_id.clone()),
                event.loco_id.clone(),
                None,
                10,
            );
            self.emit(
                self.current_time + rest_min,
                EventKind::TurnaroundRestEnd,
                Some(crew_id),
                event.loco_id,
                None,
                10,
            );
        } else {
            self.emit_now(EventKind::ShiftEnd, Some(crew_id), event.loco_id, None, 10);
        }
        Ok(())
    }

    pub(crate) fn handle_shift_end(&mut self, event: SimEvent) -> Result<(), EngineError> {
        let crew_id = self.req_crew_id(&event)?;

        if let Some(arrive_id) = self.pending_arrivals.remove(&crew_id) {
            self.queue.cancel(arrive_id);
        }

        self.update_crew_utilization(&crew_id);

        let (
            shift_start,
            shift_work_min,
            loco_id_str,
            distance,
            depot_id,
            current_leg_id,
            route_segments,
        ) = {
            let crew = self
                .crews
                .get(&crew_id)
                .ok_or(EngineError::CrewNotFound(crew_id.clone()))?;
            (
                crew.shift_start.unwrap_or(0),
                crew.shift_work_min,
                crew.current_loco.clone(),
                crew.total_distance_km,
                crew.depot_id.clone(),
                crew.current_leg.clone(),
                crew.route_segments.clone(),
            )
        };

        let loco_id = event.loco_id.or(loco_id_str).unwrap_or_default();

        if !loco_id.is_empty() {
            self.set_loco_state(&loco_id, LocoState::Available);
            let crew_pos = self
                .crews
                .get(&crew_id)
                .map(|c| c.position.clone())
                .unwrap_or_default();
            self.move_loco(&loco_id, &crew_pos);
        }

        if !self.in_warmup() {
            let vcount = self.violations.count_by_crew(&crew_id) as u32;
            let leg = current_leg_id
                .as_ref()
                .and_then(|leg_id| self.network.legs.get(leg_id));
            let segment_ids = if route_segments.is_empty() {
                leg.map(|leg| leg.segments.clone()).unwrap_or_default()
            } else {
                route_segments
            };
            let first_segment = segment_ids
                .first()
                .and_then(|segment_id| self.network.segments.get(segment_id));
            let last_segment = segment_ids
                .last()
                .and_then(|segment_id| self.network.segments.get(segment_id));
            let from_station = first_segment
                .and_then(|segment| self.network.stations.get(&segment.from))
                .map(|station| station.name.clone())
                .unwrap_or_default();
            let to_station = last_segment
                .and_then(|segment| self.network.stations.get(&segment.to))
                .map(|station| station.name.clone())
                .unwrap_or_default();
            let road_name = leg
                .and_then(|leg| self.network.roads.get(&leg.road_id))
                .map(|road| road.name.clone())
                .or_else(|| {
                    self.network
                        .roads
                        .values()
                        .find(|road| road.depots.contains(&depot_id))
                        .map(|road| road.name.clone())
                })
                .unwrap_or_default();
            let depot_name = self
                .network
                .stations
                .values()
                .find(|station| station.depot_id.as_ref() == Some(&depot_id))
                .map(|station| station.name.clone())
                .unwrap_or_else(|| depot_id.clone());
            let avg_speed_kmh = if shift_work_min > 0 {
                distance / (shift_work_min as f32 / 60.0)
            } else {
                0.0
            };
            let max_speed_kmh = segment_ids
                .iter()
                .filter_map(|segment_id| self.network.segments.get(segment_id))
                .map(|segment| segment.base_speed_kmh)
                .fold(0.0_f32, f32::max);

            self.tu3_rows.push(Tu3Row {
                road_name,
                depot_name,
                crew_id: crew_id.clone(),
                loco_id,
                leg_id: current_leg_id.unwrap_or_default(),
                leg_name: leg.map(|leg| leg.name.clone()).unwrap_or_default(),
                from_station,
                to_station,
                route_segments: segment_ids.join(","),
                shift_start,
                shift_end: Some(self.current_time),
                work_duration_min: shift_work_min,
                total_distance_km: distance,
                avg_speed_kmh,
                max_speed_kmh,
                segment_count: segment_ids.len() as u32,
                mileage_km: distance,
                violations_count: vcount,
                ..Tu3Row::empty()
            });
            self.stats.record_shift(shift_work_min, distance);
        }

        if let Some(crew) = self.crews.get_mut(&crew_id) {
            crew.set_state(CrewState::Finished);
            crew.total_shifts += 1;
            crew.cumulative_work_min += shift_work_min;

            let is_night = crew.is_night_shift();
            if is_night {
                crew.night_shifts_in_row += 1;
            } else {
                crew.night_shifts_in_row = 0;
            }
        }

        let night_shifts = self
            .crews
            .get(&crew_id)
            .map_or(0, |c| c.night_shifts_in_row);
        let is_night = self.crews.get(&crew_id).is_some_and(|c| c.is_night_shift());
        if is_night && night_shifts > self.norms.max_night_shifts_in_row {
            self.add_violation(
                ViolationType::NightOverload,
                Severity::Critical,
                &crew_id,
                None,
                None,
                &format!("> {} ночных смен подряд", night_shifts),
                "Приказ Минтранса №339, п.4.5",
            );
        }

        self.emit_now(EventKind::HomeRestStart, Some(crew_id), None, None, 10);
        Ok(())
    }

    // === HOME REST ===

    pub(crate) fn handle_home_rest_start(&mut self, event: SimEvent) -> Result<(), EngineError> {
        let crew_id = self.req_crew_id(&event)?;

        let cumulative = self
            .crews
            .get(&crew_id)
            .map_or(0, |c| c.cumulative_work_min);
        if let Some(crew) = self.crews.get_mut(&crew_id) {
            crew.set_state(CrewState::RestingHome);
        }

        if cumulative > self.norms.max_monthly_work_min {
            self.add_violation(
                ViolationType::MonthlyOvertime,
                Severity::Critical,
                &crew_id,
                None,
                None,
                &format!("Месячная норма {} мин превышена", cumulative),
                "Приказ Минтранса №339, п.4.3",
            );
        }

        let rest_min = self.prng.range(
            self.norms.min_home_rest_min as f32,
            (self.norms.min_home_rest_min + 120) as f32,
        ) as u64;

        if (rest_min as u32) < self.norms.min_home_rest_min {
            self.add_violation(
                ViolationType::ShortHomeRest,
                Severity::Warning,
                &crew_id,
                None,
                None,
                &format!(
                    "Домашний отдых {} мин < минимума {} мин",
                    rest_min, self.norms.min_home_rest_min
                ),
                "Приказ Минтранса №339, п.4.4",
            );
        }

        self.emit(
            self.current_time + rest_min,
            EventKind::HomeRestEnd,
            Some(crew_id),
            None,
            None,
            10,
        );
        Ok(())
    }

    pub(crate) fn handle_home_rest_end(&mut self, event: SimEvent) -> Result<(), EngineError> {
        let crew_id = self.req_crew_id(&event)?;
        if let Some(crew) = self.crews.get_mut(&crew_id) {
            crew.set_state(CrewState::AtHome);
            crew.shift_start = None;
            crew.shift_work_min = 0;
            crew.overtime_8h_flagged = false;
            crew.total_distance_km = 0.0;
            crew.in_voltage_sag = false;
            crew.last_home_rest_end = Some(self.current_time);
            crew.current_loco = None;
            crew.route_segments.clear();
            crew.route_index = 0;
        }
        Ok(())
    }

    // === VIOLATIONS ===

    pub(crate) fn handle_violation_detected(
        &mut self,
        _event: SimEvent,
        _vtype: &crate::events::ViolationType,
    ) -> Result<(), EngineError> {
        Ok(())
    }

    pub(crate) fn handle_injected_violation(
        &mut self,
        event: SimEvent,
        violation_type: crate::events::ViolationType,
        severity: Severity,
        description: String,
        norm_reference: String,
    ) -> Result<(), EngineError> {
        let crew_id = self.req_crew_id(&event)?;
        self.add_violation(
            violation_type,
            severity,
            &crew_id,
            event.loco_id.clone(),
            event.segment_id.clone(),
            &description,
            &norm_reference,
        );
        Ok(())
    }
}
