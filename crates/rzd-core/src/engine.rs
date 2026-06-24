use crate::events::{EventQueue, SimEvent, EventKind, ViolationType};
use crate::network::{Network, SegmentId, VoltageKind, StationKind};
use crate::entities::{
    Crew, Locomotive, SimTime, CrewState, LocoState,
    FailureKind, FailureSeverity, AcceptanceIssue, AcceptanceIssueKind,
};
use crate::norms::RzdNorms;
use crate::prng::Mulberry32;
use crate::violations::{Violation, ViolationAnalytics, Severity};
use crate::tu3::Tu3Row;
use crate::export::{CedExport, build_ced_export};
use crate::statistics::SimulationStats;
use std::collections::{HashMap, HashSet};

pub struct Engine {
    pub network: Network,
    pub norms: RzdNorms,
    pub prng: Mulberry32,

    pub queue: EventQueue,
    pub crews: HashMap<String, Crew>,
    pub locos: HashMap<String, Locomotive>,

    pub violations: ViolationAnalytics,
    pub tu3_rows: Vec<Tu3Row>,
    pub stats: SimulationStats,

    pub current_time: SimTime,
    max_time: SimTime,
    warmup_minutes: SimTime,

    // Spatial indexes
    loco_by_station: HashMap<String, Vec<String>>,
    turnaround_stations: HashSet<String>,

    // Event tracking for cancellation
    pending_arrivals: HashMap<String, u64>,
}

impl Engine {
    pub fn new(network: Network, seed: u32) -> Self {
        let turnaround_stations: HashSet<String> = network.stations.values()
            .filter(|s| s.kind == StationKind::Turnaround)
            .map(|s| s.id.clone())
            .collect();

        Self {
            network,
            norms: RzdNorms::default(),
            prng: Mulberry32::new(seed),
            queue: EventQueue::new(),
            crews: HashMap::new(),
            locos: HashMap::new(),
            violations: ViolationAnalytics::new(),
            tu3_rows: Vec::new(),
            stats: SimulationStats::new(),
            current_time: 0,
            max_time: u64::MAX,
            warmup_minutes: 0,
            loco_by_station: HashMap::new(),
            turnaround_stations,
            pending_arrivals: HashMap::new(),
        }
    }

    pub fn set_warmup(&mut self, minutes: SimTime) {
        self.warmup_minutes = minutes;
    }

    pub fn add_crew(&mut self, crew: Crew) {
        self.crews.insert(crew.id.clone(), crew);
    }

    pub fn schedule_all_shifts(&mut self, start_offset: SimTime, interval: SimTime) {
        let crew_ids: Vec<String> = self.crews.keys().cloned().collect();
        for (idx, crew_id) in crew_ids.into_iter().enumerate() {
            self.emit(start_offset + (idx as u64) * interval, EventKind::ShiftAppear, Some(crew_id), None, None, 10);
        }
    }

    pub fn add_loco(&mut self, loco: Locomotive) {
        self.loco_by_station.entry(loco.position.clone()).or_default().push(loco.id.clone());
        self.locos.insert(loco.id.clone(), loco);
    }

    pub fn run(&mut self, until: SimTime) {
        self.max_time = until;
        while let Some(event) = self.queue.pop() {
            if event.time > until {
                break;
            }
            self.current_time = event.time;
            self.process_event(event);
        }
    }

    pub fn crew_count(&self) -> usize {
        self.crews.len()
    }

    pub fn loco_count(&self) -> usize {
        self.locos.len()
    }

    pub fn stats(&self) -> &SimulationStats {
        &self.stats
    }

    pub fn violations_summary(&self) -> crate::violations::ViolationSummary {
        self.violations.summary()
    }

    pub fn export_ced(&self) -> CedExport {
        build_ced_export(&self.tu3_rows, &self.violations.violations)
    }

    pub fn region_multiplier_for_segment(&self, seg: &crate::network::Segment) -> f32 {
        self.network.region_for_segment(&seg.id)
            .map(|region_id| self.network.region_norms.get_default(region_id).failure_rate_multiplier)
            .unwrap_or(1.0)
    }

    // === EVENT BUILDER HELPER ===

    fn emit(&mut self, time: SimTime, kind: EventKind, crew_id: Option<String>, loco_id: Option<String>, segment_id: Option<String>, priority: u8) -> u64 {
        self.queue.push(SimEvent { id: 0, time, kind, crew_id, loco_id, segment_id, priority })
    }

    fn emit_now(&mut self, kind: EventKind, crew_id: Option<String>, loco_id: Option<String>, segment_id: Option<String>, priority: u8) -> u64 {
        self.emit(self.current_time, kind, crew_id, loco_id, segment_id, priority)
    }

    // === LOCO INDEX HELPERS ===

    fn find_available_loco(&self, station: &str) -> Option<String> {
        self.loco_by_station.get(station)
            .and_then(|ids| {
                ids.iter().find(|id| {
                    self.locos.get(*id).is_some_and(|l| l.state == LocoState::Available)
                })
                .cloned()
            })
    }

    fn move_loco(&mut self, loco_id: &str, new_station: &str) {
        if let Some(loco) = self.locos.get_mut(loco_id) {
            let old_station = loco.position.clone();
            loco.position = new_station.to_string();

            if let Some(stations) = self.loco_by_station.get_mut(&old_station) {
                stations.retain(|id| id != loco_id);
            }
            self.loco_by_station.entry(new_station.to_string()).or_default().push(loco_id.to_string());
        }
    }

    fn set_loco_state(&mut self, loco_id: &str, state: LocoState) {
        if let Some(loco) = self.locos.get_mut(loco_id) {
            loco.state = state;
        }
        let utilization = match state {
            LocoState::InUse => 1.0,
            LocoState::Available | LocoState::Failed | LocoState::Maintenance { .. } => 0.0,
        };
        self.stats.loco_utilization.update(self.current_time as f64, utilization);
    }

    fn update_crew_utilization(&mut self, crew_id: &str) {
        let utilization = self.crews.get(crew_id).map_or(0.0, |c| match c.state {
            CrewState::Working | CrewState::Waiting | CrewState::Reserve => 1.0,
            _ => 0.0,
        });
        self.stats.crew_utilization.update(self.current_time as f64, utilization);
    }

    fn is_turnaround(&self, station_id: &str) -> bool {
        self.turnaround_stations.contains(station_id)
    }

    // === VIOLATION HELPER ===

    #[allow(clippy::too_many_arguments)]
    fn add_violation(&mut self, vtype: ViolationType, severity: Severity, crew_id: &str, loco_id: Option<String>, segment_id: Option<String>, description: &str, norm_reference: &str) -> String {
        if self.in_warmup() {
            return String::new();
        }
        let vid = self.violations.next_id();
        self.violations.add_violation(Violation {
            id: vid.clone(),
            violation_type: vtype,
            severity,
            crew_id: crew_id.to_string(),
            loco_id,
            segment_id,
            detected_at: self.current_time,
            description: description.to_string(),
            norm_reference: norm_reference.to_string(),
        });
        self.stats.record_violation();
        vid
    }

    fn in_warmup(&self) -> bool {
        self.current_time < self.warmup_minutes
    }

    // === MAIN EVENT DISPATCH ===

    fn process_event(&mut self, event: SimEvent) {
        self.stats.record_event();
        match event.kind {
            EventKind::ShiftAppear => self.handle_appear(event),
            EventKind::MedCheckStart => self.handle_med_check_start(event),
            EventKind::MedCheckEnd => self.handle_med_check_end(event),
            EventKind::PreTripBriefingEnd => self.handle_pretrip_briefing_end(event),
            EventKind::AcceptanceStart => self.handle_acceptance_start(event),
            EventKind::AcceptanceEnd => self.handle_acceptance_end(event),
            EventKind::DepartureCheckpoint => self.handle_departure_checkpoint(event),
            EventKind::WaitStart => self.handle_wait_start(event),
            EventKind::WaitEnd => self.handle_wait_end(event),
            EventKind::ArriveStation => self.handle_arrive(event),
            EventKind::PassWaypoint => self.handle_pass_waypoint(event),
            EventKind::FailureOccurred(kind) => self.handle_failure(event, kind),
            EventKind::FailureResolved => self.handle_failure_resolved(event),
            EventKind::LineChangeStart => self.handle_line_change_start(event),
            EventKind::VoltageSagStart { voltage_v, segment_id: ref seg_id } => {
                let seg_id = seg_id.clone();
                self.handle_voltage_sag_start(event, voltage_v, seg_id);
            }
            EventKind::VoltageSagEnd => self.handle_voltage_sag_end(event),
            EventKind::ReserveStart => self.handle_reserve_start(event),
            EventKind::ReserveEnd => self.handle_reserve_end(event),
            EventKind::TurnaroundRestStart => self.handle_turnaround_rest_start(event),
            EventKind::TurnaroundRestEnd => self.handle_turnaround_rest_end(event),
            EventKind::HandoverStart => self.handle_handover_start(event),
            EventKind::HandoverEnd => self.handle_handover_end(event),
            EventKind::ArrivalCheckpoint => self.handle_arrival_checkpoint(event),
            EventKind::ShiftEnd => self.handle_shift_end(event),
            EventKind::HomeRestStart => self.handle_home_rest_start(event),
            EventKind::HomeRestEnd => self.handle_home_rest_end(event),
            EventKind::ViolationDetected(vtype) => self.handle_violation_detected(event, &vtype),
        }
    }

    // === SHIFT LIFECYCLE ===

    fn handle_appear(&mut self, event: SimEvent) {
        let crew_id = match &event.crew_id { Some(id) => id.clone(), None => return };
        let crew = match self.crews.get_mut(&crew_id) { Some(c) => c, None => return };

        crew.set_state(CrewState::Appearing);
        crew.shift_start = Some(self.current_time);
        crew.shift_work_min = 0;
        crew.overtime_8h_flagged = false;
        crew.total_distance_km = 0.0;
        crew.in_voltage_sag = false;
        crew.violations.clear();
        crew.acceptance_issues.clear();

        self.emit_now(EventKind::MedCheckStart, Some(crew_id), None, None, 10);
    }

    fn handle_med_check_start(&mut self, event: SimEvent) {
        let crew_id = match &event.crew_id { Some(id) => id.clone(), None => return };
        if let Some(crew) = self.crews.get_mut(&crew_id) {
            crew.set_state(CrewState::MedCheck);
        }
        let duration = self.prng.range(10.0, 15.0) as u64;
        self.emit(self.current_time + duration, EventKind::MedCheckEnd, Some(crew_id), None, None, 10);
    }

    fn handle_med_check_end(&mut self, event: SimEvent) {
        let crew_id = match &event.crew_id { Some(id) => id.clone(), None => return };
        if let Some(crew) = self.crews.get_mut(&crew_id) {
            crew.set_state(CrewState::PreTripCheck);
        }
        let duration = self.prng.range(5.0, 10.0) as u64;
        self.emit(self.current_time + duration, EventKind::PreTripBriefingEnd, Some(crew_id), None, None, 10);
    }

    fn handle_pretrip_briefing_end(&mut self, event: SimEvent) {
        let crew_id = match &event.crew_id { Some(id) => id.clone(), None => return };
        self.emit_now(EventKind::AcceptanceStart, Some(crew_id), None, None, 10);
    }

    // === ACCEPTANCE (decomposed) ===

    fn generate_acceptance_issues(&mut self, crew_id: &str, loco_id: &str, sections: u8) -> (Vec<AcceptanceIssue>, u32) {
        let base_min = sections as u32 * self.norms.acceptance_per_section_min;
        let mut total_min = base_min;
        let mut issues = Vec::new();

        if self.prng.chance(0.08) {
            let delay = self.prng.range(15.0, 60.0) as u32;
            total_min += delay;
            issues.push(AcceptanceIssue { kind: AcceptanceIssueKind::PreviousCrewDelayed, delay_min: delay, resolved: true });
        }
        if self.prng.chance(0.04) {
            let delay = self.prng.range(20.0, 90.0) as u32;
            total_min += delay;
            let resolved = self.prng.chance(0.7);
            issues.push(AcceptanceIssue { kind: AcceptanceIssueKind::KlubUFault, delay_min: delay, resolved });
            if !resolved {
                self.add_violation(ViolationType::AcceptanceDelay, Severity::Warning, crew_id,
                    Some(loco_id.to_string()), None,
                    &format!("КЛУБ-У неисправен, задержка {} мин", delay),
                    "Регламент CTL-16-2");
            }
        }
        if self.prng.chance(0.03) {
            let delay = self.prng.range(10.0, 40.0) as u32;
            total_min += delay;
            issues.push(AcceptanceIssue { kind: AcceptanceIssueKind::RadioFault, delay_min: delay, resolved: self.prng.chance(0.85) });
        }
        if self.prng.chance(0.05) {
            let delay = self.prng.range(30.0, 120.0) as u32;
            total_min += delay;
            let resolved = self.prng.chance(0.6);
            issues.push(AcceptanceIssue { kind: AcceptanceIssueKind::MaintenanceNotDone, delay_min: delay, resolved });
        }

        (issues, total_min)
    }

    fn handle_acceptance_start(&mut self, event: SimEvent) {
        let crew_id = match &event.crew_id { Some(id) => id.clone(), None => return };
        let crew_pos = match self.crews.get(&crew_id) { Some(c) => c.position.clone(), None => return };

        let loco_id = self.find_available_loco(&crew_pos);

        if let Some(lid) = loco_id {
            let sections = self.locos.get(&lid).map_or(2, |l| l.sections);
            self.set_loco_state(&lid, LocoState::InUse);

            if let Some(crew) = self.crews.get_mut(&crew_id) {
                crew.set_state(CrewState::Acceptance);
                crew.current_loco = Some(lid.clone());
            }

            let (issues, total_min) = self.generate_acceptance_issues(&crew_id, &lid, sections);

            if total_min > 90 {
                self.add_violation(ViolationType::AcceptanceDelay, Severity::Warning, &crew_id,
                    Some(lid.clone()), None,
                    &format!("Приёмка ТПС задержана >{} мин", total_min),
                    "Регламент CTL-16-2, п.3.1");
            }

            if let Some(crew) = self.crews.get_mut(&crew_id) {
                crew.acceptance_issues.extend(issues);
            }

            if !self.in_warmup() {
                self.stats.record_acceptance(total_min);
            }
            self.emit(self.current_time + total_min as u64, EventKind::AcceptanceEnd, Some(crew_id), Some(lid), None, 10);
        } else {
            self.emit(self.current_time + 10, EventKind::AcceptanceStart, Some(crew_id), None, None, 20);
        }
    }

    fn handle_acceptance_end(&mut self, event: SimEvent) {
        let crew_id = match &event.crew_id { Some(id) => id.clone(), None => return };
        if let Some(crew) = self.crews.get_mut(&crew_id) {
            crew.set_state(CrewState::Waiting);
        }
        self.emit(self.current_time + 5, EventKind::DepartureCheckpoint, Some(crew_id), event.loco_id, None, 10);
    }

    fn handle_departure_checkpoint(&mut self, event: SimEvent) {
        let crew_id = match &event.crew_id { Some(id) => id.clone(), None => return };

        let wait_chance = self.prng.next_f32();
        if wait_chance < self.norms.waiting_probability {
            let wait_min = self.prng.range(self.norms.waiting_min_min as f32, self.norms.waiting_max_min as f32) as u64;
            if let Some(crew) = self.crews.get_mut(&crew_id) {
                crew.set_state(CrewState::Waiting);
            }
            self.emit_now(EventKind::WaitStart, Some(crew_id.clone()), event.loco_id.clone(), None, 10);
            self.emit(self.current_time + wait_min, EventKind::WaitEnd, Some(crew_id), event.loco_id, None, 10);
            if !self.in_warmup() {
                self.stats.record_wait(wait_min);
            }
        } else {
            self.start_movement(crew_id, event.loco_id);
        }
    }

    fn handle_wait_start(&mut self, event: SimEvent) {
        if let Some(crew_id) = &event.crew_id {
            if let Some(crew) = self.crews.get_mut(crew_id) {
                crew.set_state(CrewState::Waiting);
            }
        }
    }

    fn handle_wait_end(&mut self, event: SimEvent) {
        let crew_id = match &event.crew_id { Some(id) => id.clone(), None => return };
        if let Some(crew) = self.crews.get_mut(&crew_id) {
            crew.set_state(CrewState::Working);
            crew.shift_work_min += self.prng.range(20.0, 90.0) as u32;
        }
        self.start_movement(crew_id, event.loco_id);
    }

    // === MOVEMENT ===

    fn start_movement(&mut self, crew_id: String, loco_id: Option<String>) {
        let (current_segment, _segment_index) = {
            let crew = match self.crews.get(&crew_id) { Some(c) => c, None => return };
            if crew.route_segments.is_empty() || crew.route_index >= crew.route_segments.len() {
                self.emit(self.current_time + 10, EventKind::ShiftEnd, Some(crew_id), loco_id, None, 10);
                return;
            }
            let seg_id = crew.route_segments[crew.route_index].clone();
            let idx = crew.route_index;
            (seg_id, idx)
        };

        let base_travel_time = self.network.segment_travel_time_min(&current_segment).unwrap_or(60.0);
        let sag_factor = if self.crews.get(&crew_id).is_some_and(|c| c.in_voltage_sag) {
            self.norms.voltage_sag_speed_factor
        } else {
            1.0
        };
        let jitter = self.prng.range(0.85, 1.15);
        let actual_time = (base_travel_time * sag_factor * jitter) as u64;

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
                let fail_kind = all_failures[self.prng.range(0.0, all_failures.len() as f32) as usize];
                self.emit(self.current_time + failure_time.max(1), EventKind::FailureOccurred(fail_kind),
                    Some(crew_id.clone()), loco_id.clone(), Some(current_segment.clone()), 5);
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
                    self.emit(self.current_time + sag_time.max(1), EventKind::VoltageSagStart { voltage_v, segment_id: current_segment.clone() },
                        Some(crew_id.clone()), loco_id.clone(), Some(current_segment.clone()), 5);
                }
            }
        }

        // Schedule arrival at next station
        let travel_seg = current_segment.clone();
        let cid = crew_id.clone();
        let arrive_id = self.emit(self.current_time + actual_time, EventKind::ArriveStation,
            Some(crew_id), loco_id, Some(travel_seg), 10);
        self.pending_arrivals.insert(cid, arrive_id);
    }

    fn handle_arrive(&mut self, event: SimEvent) {
        let crew_id = match &event.crew_id { Some(id) => id.clone(), None => return };
        let seg_id = match &event.segment_id { Some(s) => s.clone(), None => return };

        self.pending_arrivals.remove(&crew_id);

        let segment = self.network.get_segment(&seg_id).cloned();
        let (next_station, _travel_time_min, seg_distance) = {
            let crew = match self.crews.get_mut(&crew_id) { Some(c) => c, None => return };
            crew.route_index += 1;
            let tt = self.network.segment_travel_time_min(&seg_id).unwrap_or(60.0) as u32;
            crew.shift_work_min += tt;
            let seg = match segment.as_ref() { Some(s) => s, None => return };
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

        // Check overtime (with guard flag to avoid duplicates)
        {
            let crew = match self.crews.get(&crew_id) { Some(c) => c, None => return };
            if crew.shift_work_min >= self.norms.max_shift_12h_min {
                self.add_violation(ViolationType::Overtime12h, Severity::Critical, &crew_id,
                    event.loco_id.clone(), event.segment_id.clone(),
                    &format!("Смена > {} мин (12ч)", crew.shift_work_min),
                    "Приказ Минтранса №339, п.4.2");
                self.emit_now(EventKind::LineChangeStart, Some(crew_id.clone()), event.loco_id.clone(), event.segment_id.clone(), 5);
                return;
            } else if crew.shift_work_min >= self.norms.max_shift_8h_min && !crew.overtime_8h_flagged {
                self.add_violation(ViolationType::Overtime8h, Severity::Warning, &crew_id,
                    event.loco_id.clone(), event.segment_id.clone(),
                    &format!("Смена > {} мин (8ч)", crew.shift_work_min),
                    "Приказ Минтранса №339, п.4.1");
                if let Some(c) = self.crews.get_mut(&crew_id) {
                    c.overtime_8h_flagged = true;
                }
            }
        }

        // Check if route is finished
        let route_finished = {
            let crew = match self.crews.get(&crew_id) { Some(c) => c, None => return };
            crew.route_index >= crew.route_segments.len()
        };

        if route_finished {
            self.emit(self.current_time + 5, EventKind::ArrivalCheckpoint, Some(crew_id), event.loco_id, None, 10);
        } else {
            self.start_movement(crew_id, event.loco_id);
        }
    }

    fn handle_pass_waypoint(&mut self, event: SimEvent) {
        if let Some(crew_id) = &event.crew_id {
            if let Some(crew) = self.crews.get_mut(crew_id) {
                crew.shift_work_min += 2;
            }
        }
    }

    // === FAILURES ===

    fn handle_failure(&mut self, event: SimEvent, kind: FailureKind) {
        let crew_id = match &event.crew_id { Some(id) => id.clone(), None => return };

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
        self.add_violation(ViolationType::LineChangeForced,
            if severity == FailureSeverity::Critical { Severity::Critical } else { Severity::Warning },
            &crew_id, event.loco_id.clone(), event.segment_id.clone(),
            &format!("Отказ {:?} на перегоне", kind),
            "Технический регламент");

        let repair_time = match severity {
            FailureSeverity::Critical => self.prng.range(60.0, 180.0) as u64,
            FailureSeverity::Warning => self.prng.range(15.0, 60.0) as u64,
        };
        if !self.in_warmup() {
            self.stats.record_repair(repair_time);
        }

        self.emit(self.current_time + repair_time, EventKind::FailureResolved,
            Some(crew_id.clone()), event.loco_id.clone(), event.segment_id.clone(), 5);

        if severity == FailureSeverity::Critical && self.prng.chance(self.norms.line_change_on_critical as f64) {
            self.emit(self.current_time + 5, EventKind::LineChangeStart,
                Some(crew_id), event.loco_id, event.segment_id, 5);
        }
    }

    fn handle_failure_resolved(&mut self, event: SimEvent) {
        // Only set Available if loco is actually Failed (not already handled by line change)
        if let Some(ref lid) = event.loco_id {
            if let Some(loco) = self.locos.get_mut(lid) {
                if loco.state == LocoState::Failed {
                    loco.state = LocoState::Available;
                }
            }
        }

        if let Some(ref crew_id) = event.crew_id {
            if let Some(crew) = self.crews.get_mut(crew_id) {
                crew.set_state(CrewState::Working);
            }
            self.start_movement(crew_id.clone(), event.loco_id);
        }
    }

    fn handle_line_change_start(&mut self, event: SimEvent) {
        let crew_id = match &event.crew_id { Some(id) => id.clone(), None => return };

        // Return loco to station as Available only if it's currently InUse (not Failed)
        if let Some(ref lid) = event.loco_id {
            if let Some(loco) = self.locos.get_mut(lid) {
                if loco.state == LocoState::InUse {
                    loco.state = LocoState::Available;
                }
                let crew_pos = self.crews.get(&crew_id).map(|c| c.position.clone()).unwrap_or_default();
                loco.position = crew_pos;
            }
        }

        let go_reserve = self.prng.chance(self.norms.reserve_after_line_change as f64);
        if go_reserve {
            if let Some(crew) = self.crews.get_mut(&crew_id) {
                crew.set_state(CrewState::Reserve);
                crew.current_loco = None;
            }
            self.emit_now(EventKind::ReserveStart, Some(crew_id.clone()), None, event.segment_id.clone(), 10);
            let travel_back = self.prng.range(60.0, 180.0) as u64;
            self.emit(self.current_time + travel_back, EventKind::ReserveEnd, Some(crew_id), None, None, 10);
        } else {
            self.emit_now(EventKind::ShiftEnd, Some(crew_id), event.loco_id, None, 10);
        }
    }

    // === VOLTAGE SAG ===

    fn handle_voltage_sag_start(&mut self, event: SimEvent, voltage_v: u32, segment_id: SegmentId) {
        let crew_id = match &event.crew_id { Some(id) => id.clone(), None => return };

        let segment = self.network.get_segment(&segment_id).cloned();
        if let Some(seg) = segment {
            let from_station = self.network.get_station(&seg.from);
            let is_critical = match from_station.map(|s| s.voltage_kv) {
                Some(VoltageKind::DC3000) => voltage_v < self.norms.voltage_sag_threshold_dc as u32,
                Some(VoltageKind::AC25000) => voltage_v < self.norms.voltage_sag_threshold_ac as u32,
                _ => false,
            };

            if is_critical {
                self.add_violation(ViolationType::VoltageSagShift, Severity::Warning, &crew_id,
                    event.loco_id.clone(), Some(segment_id.clone()),
                    &format!("Просадка напряжения {} В (норма {} В)", voltage_v,
                        match from_station.map(|s| s.voltage_kv) {
                            Some(VoltageKind::DC3000) => 3000,
                            _ => 25000,
                        }),
                    "Технические условия КС");
            }

            // Apply speed factor during sag
            if is_critical {
                if let Some(crew) = self.crews.get_mut(&crew_id) {
                    crew.in_voltage_sag = true;
                }
            }
        }

        let sag_duration = self.prng.range(10.0, 45.0) as u64;
        self.emit(self.current_time + sag_duration, EventKind::VoltageSagEnd,
            Some(crew_id), event.loco_id, Some(segment_id), 8);
    }

    fn handle_voltage_sag_end(&mut self, event: SimEvent) {
        if let Some(ref crew_id) = event.crew_id {
            if let Some(crew) = self.crews.get_mut(crew_id) {
                crew.in_voltage_sag = false;
            }
        }
    }

    // === RESERVE ===

    fn handle_reserve_start(&mut self, event: SimEvent) {
        if let Some(crew_id) = &event.crew_id {
            if let Some(crew) = self.crews.get_mut(crew_id) {
                crew.set_state(CrewState::Reserve);
            }
        }
    }

    fn handle_reserve_end(&mut self, event: SimEvent) {
        let crew_id = match &event.crew_id { Some(id) => id.clone(), None => return };
        if let Some(crew) = self.crews.get_mut(&crew_id) {
            crew.position = crew.depot_id.clone();
        }
        self.emit_now(EventKind::ShiftEnd, Some(crew_id), None, None, 10);
    }

    // === TURNAROUND ===

    fn handle_turnaround_rest_start(&mut self, event: SimEvent) {
        if let Some(crew_id) = &event.crew_id {
            if let Some(crew) = self.crews.get_mut(crew_id) {
                crew.set_state(CrewState::RestingAway);
            }
        }
    }

    fn handle_turnaround_rest_end(&mut self, event: SimEvent) {
        let crew_id = match &event.crew_id { Some(id) => id.clone(), None => return };
        self.emit_now(EventKind::HandoverStart, Some(crew_id), event.loco_id, None, 10);
    }

    fn handle_handover_start(&mut self, event: SimEvent) {
        let crew_id = match &event.crew_id { Some(id) => id.clone(), None => return };
        let handover_min = self.prng.range(self.norms.handover_min_min as f32, self.norms.handover_max_min as f32) as u64;
        if let Some(crew) = self.crews.get_mut(&crew_id) {
            crew.shift_work_min += handover_min as u32;
        }
        self.emit(self.current_time + handover_min, EventKind::HandoverEnd, Some(crew_id), event.loco_id, None, 10);
    }

    fn handle_handover_end(&mut self, event: SimEvent) {
        self.handle_shift_end(event);
    }

    fn handle_arrival_checkpoint(&mut self, event: SimEvent) {
        let crew_id = match &event.crew_id { Some(id) => id.clone(), None => return };
        let handover_min = self.prng.range(15.0, 30.0) as u64;
        if let Some(crew) = self.crews.get_mut(&crew_id) {
            crew.shift_work_min += handover_min as u32;
        }

        let position = self.crews.get(&crew_id).map(|c| c.position.clone()).unwrap_or_default();
        let at_turnaround = self.is_turnaround(&position);

        if at_turnaround {
            let rest_min = self.prng.range(180.0, 360.0) as u64;

            if (rest_min as u32) < self.norms.min_turnaround_rest_min {
                self.add_violation(ViolationType::ShortRest, Severity::Warning, &crew_id,
                    event.loco_id.clone(), None,
                    &format!("Отдых в обороте {} мин < минимума {} мин", rest_min, self.norms.min_turnaround_rest_min),
                    "Приказ Минтранса №339, п.5.3");
            }

            let crew_work = self.crews.get(&crew_id).map_or(0, |c| c.shift_work_min);
            if (rest_min as f32) < (crew_work as f32 * self.norms.turnaround_rest_ratio) {
                self.add_violation(ViolationType::ShortRest, Severity::Warning, &crew_id,
                    event.loco_id.clone(), None,
                    &format!("Отдых в обороте {} мин < 50% работы ({})", rest_min, crew_work),
                    "Приказ Минтранса №339, п.5.3");
            }

            self.emit_now(EventKind::TurnaroundRestStart, Some(crew_id.clone()), event.loco_id.clone(), None, 10);
            self.emit(self.current_time + rest_min, EventKind::TurnaroundRestEnd, Some(crew_id), event.loco_id, None, 10);
        } else {
            self.emit_now(EventKind::ShiftEnd, Some(crew_id), event.loco_id, None, 10);
        }
    }

    fn handle_shift_end(&mut self, event: SimEvent) {
        let crew_id = match &event.crew_id { Some(id) => id.clone(), None => return };

        if let Some(arrive_id) = self.pending_arrivals.remove(&crew_id) {
            self.queue.cancel(arrive_id);
        }

        self.update_crew_utilization(&crew_id);

        let (shift_start, shift_work_min, loco_id_str, distance) = {
            let crew = match self.crews.get(&crew_id) { Some(c) => c, None => return };
            (crew.shift_start.unwrap_or(0), crew.shift_work_min, crew.current_loco.clone(), crew.total_distance_km)
        };

        let loco_id = event.loco_id.or(loco_id_str).unwrap_or_default();

        if !loco_id.is_empty() {
            self.set_loco_state(&loco_id, LocoState::Available);
            let crew_pos = self.crews.get(&crew_id).map(|c| c.position.clone()).unwrap_or_default();
            self.move_loco(&loco_id, &crew_pos);
        }

        if !self.in_warmup() {
            let vcount = self.violations.count_by_crew(&crew_id) as u32;
            self.tu3_rows.push(Tu3Row {
                crew_id: crew_id.clone(),
                loco_id,
                shift_start,
                shift_end: Some(self.current_time),
                work_duration_min: shift_work_min,
                total_distance_km: distance,
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
            (is_night, crew.night_shifts_in_row)
        } else {
            (false, 0)
        };

        let night_shifts = self.crews.get(&crew_id).map_or(0, |c| c.night_shifts_in_row);
        let is_night = self.crews.get(&crew_id).is_some_and(|c| c.is_night_shift());
        if is_night && night_shifts > self.norms.max_night_shifts_in_row {
            self.add_violation(ViolationType::NightOverload, Severity::Critical, &crew_id,
                None, None,
                &format!("> {} ночных смен подряд", night_shifts),
                "Приказ Минтранса №339, п.4.5");
        }

        self.emit_now(EventKind::HomeRestStart, Some(crew_id), None, None, 10);
    }

    // === HOME REST ===

    fn handle_home_rest_start(&mut self, event: SimEvent) {
        let crew_id = match &event.crew_id { Some(id) => id.clone(), None => return };

        let cumulative = self.crews.get(&crew_id).map_or(0, |c| c.cumulative_work_min);
        if let Some(crew) = self.crews.get_mut(&crew_id) {
            crew.set_state(CrewState::RestingHome);
        }

        if cumulative > self.norms.max_monthly_work_min {
            self.add_violation(ViolationType::MonthlyOvertime, Severity::Critical, &crew_id,
                None, None,
                &format!("Месячная норма {} мин превышена", cumulative),
                "Приказ Минтранса №339, п.4.3");
        }

        let rest_min = self.prng.range(self.norms.min_home_rest_min as f32, (self.norms.min_home_rest_min + 120) as f32) as u64;

        if (rest_min as u32) < self.norms.min_home_rest_min {
            self.add_violation(ViolationType::ShortHomeRest, Severity::Warning, &crew_id,
                None, None,
                &format!("Домашний отдых {} мин < минимума {} мин", rest_min, self.norms.min_home_rest_min),
                "Приказ Минтранса №339, п.4.4");
        }

        self.emit(self.current_time + rest_min, EventKind::HomeRestEnd, Some(crew_id), None, None, 10);
    }

    fn handle_home_rest_end(&mut self, event: SimEvent) {
        let crew_id = match &event.crew_id { Some(id) => id.clone(), None => return };
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
    }

    // === VIOLATIONS ===

    fn handle_violation_detected(&mut self, _event: SimEvent, _vtype: &ViolationType) {}
}
